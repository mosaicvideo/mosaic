# Extraction Fixes: Accurate Seeking, Parallel Extraction, Audio Stripping

## Problem

On Windows with a 12GB BluRay remux MKV (H.264, TrueHD audio), generating a 3x7 contact sheet took 15+ minutes and produced identical frames for 17 of 21 thumbnails. Root causes:

1. **Seeking inaccuracy**: Input-level `-ss` (keyframe seek) snaps to the same keyframe for multiple timestamps when the container has sparse or problematic cue entries. The frames are literally identical despite correct, distinct timestamps.
2. **Sequential extraction**: All four modules extract thumbnails/clips in a serial loop. Each ffmpeg call waits for the previous one to finish. 21 extractions x ~40s each = ~14 minutes.
3. **Unnecessary audio decoding**: `contact_sheet.rs` and `screenshots.rs` omit `-an`, forcing ffmpeg to demux heavy audio streams (TrueHD in this case) that are immediately discarded.

## Design

### 1. Accurate Seeking via `seek_input_args()`

New function in `ffmpeg.rs` centralizing the seeking + input portion of all extraction commands:

```rust
pub fn seek_input_args(source: &Path, timestamp: f64) -> Vec<String> {
    vec![
        "-ss".into(), format!("{:.3}", timestamp),
        "-copyts".into(),
        "-i".into(), source.to_string_lossy().into_owned(),
        "-ss".into(), format!("{:.3}", timestamp),
        "-an".into(),
    ]
}
```

**How it works:**
- First `-ss <ts>` (input option): fast keyframe seek to nearby position
- `-copyts`: preserves original stream timestamps instead of resetting to 0
- Second `-ss <ts>` (output option): trims output to exactly time `ts` using preserved timestamps
- `-an`: skip audio decoding (no extraction pipeline produces audio)

**Applied to all four modules**, replacing their inline seeking args:

| Module | After `seek_input_args(source, ts)`, append... |
|--------|------------------------------------------------|
| `contact_sheet.rs` | `-frames:v 1 -vf <scale+drawtext> <output>` |
| `screenshots.rs` | `-frames:v 1 [-q:v N] <output>` |
| `preview_reel.rs` | `-t <duration> [-vf scale] <encoder> <output>` |
| `animated_sheet.rs` | `-t <duration> -vf <scale+drawtext+pad> -r <fps> <encoder> <output>` |

The existing inline `-an` in `preview_reel.rs` and `animated_sheet.rs` is removed (now in the shared function). Timestamp formatting standardizes on `{:.3}` across all modules.

### 2. Parallel Extraction via `run_batch_cancellable()`

New function in `ffmpeg.rs`:

```rust
pub async fn run_batch_cancellable<F>(
    exe: &Path,
    batch: Vec<Vec<String>>,
    cancelled: Arc<AtomicBool>,
    mut on_done: F,
) -> Result<(), RunError>
where
    F: FnMut(usize),
```

**Internals:**
- `tokio::task::JoinSet` spawns all tasks
- `tokio::sync::Semaphore` bounds concurrency to `available_parallelism().min(8)`, fallback 4
- Each task: acquire permit, run `run_cancellable`, release permit
- `on_done(i)` fires in the caller's context as each task completes (safe to use `&ProgressReporter`)
- First `RunError` aborts all remaining tasks via `set.abort_all()`
- `JoinError` (task panic) converts to `RunError::Io`

**Per-module change** — extraction loop becomes:

```rust
// Build all args upfront
let mut batch = Vec::with_capacity(timestamps.len());
for (i, ts) in timestamps.iter().enumerate() {
    let mut args = base_args();
    args.extend(seek_input_args(source, *ts));
    // ... module-specific output args ...
    batch.push(args);
}

// Execute in parallel, report progress as each completes
let mut done = 0u32;
run_batch_cancellable(ffmpeg, batch, cancelled.clone(), |_| {
    done += 1;
    (reporter.emit)(done, total_steps, &format!("<Label> {}/{}", done, total));
}).await?;
```

Progress reports a running completion count. Thumbnails complete out of order but the count ticks up correctly.

Sequential post-extraction steps (tiling, stitching, header compositing) are unchanged — they run after all extractions complete.

### 3. Files Changed

| File | Changes |
|------|---------|
| `ffmpeg.rs` | Add `seek_input_args()`, `run_batch_cancellable()`, tests for both |
| `contact_sheet.rs` | Use `seek_input_args`, batch extraction loop |
| `screenshots.rs` | Use `seek_input_args`, batch extraction loop |
| `preview_reel.rs` | Use `seek_input_args`, remove inline `-an`/`-ss`, batch extraction loop |
| `animated_sheet.rs` | Use `seek_input_args`, remove inline `-an`/`-ss`, batch extraction loop |

### 4. Test Updates

- `preview_reel::tests` — arg assertions updated for new arg order
- `animated_sheet::tests` — arg assertions updated for new arg order
- New: `ffmpeg::tests::seek_input_args_shape` — verifies arg structure
- New: `ffmpeg::tests::run_batch_cancellable_*` — success path, early error abort

### 5. What Does Not Change

- Sequential post-extraction steps (tiling, stitching, compositing)
- Cancellation model (`Arc<AtomicBool>`, shared across parallel tasks)
- Output file naming and collision handling (`output_path.rs`)
- Progress event names (`job:step`, `job:file-done`, etc.)
- ffmpeg base args prelude (`base_args()`)
- The `run_cancellable` function itself (still used by batch internally + post-extraction steps)

## Verification

1. `cargo check` — compilation passes
2. `cargo test --lib` — all unit tests pass (including updated arg assertions)
3. Manual test on Windows with a large MKV:
   - Contact sheet: all thumbnails show distinct frames
   - Generation time significantly reduced vs. sequential baseline
   - Cancel button still works mid-extraction (aborts parallel tasks)
4. Manual test on macOS — no regression on existing functionality
