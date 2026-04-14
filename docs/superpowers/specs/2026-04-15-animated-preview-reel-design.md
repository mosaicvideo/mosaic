# Animated Preview Reel — Design

**Status:** Approved (design phase)
**Date:** 2026-04-15
**Scope:** Add a third output type to Mosaic alongside contact sheet and screenshots. Produces one animated WebP per video by sampling N short clips at equal positions and concatenating them.

## Goal

For a selected video, produce a single animated WebP "preview reel": `count` clips, each `clip_length_secs` long, sampled at equal positions (same sampler as screenshots), concatenated in order with hard cuts, scaled to a target height, fps-capped, encoded via libwebp.

## User-facing behaviour

- Third Generate checkbox in the main view: **Animated Preview**.
- Third section in the settings view with the fields in `PreviewOptions` below.
- Runs per file in the queue, in parallel with the existing sheet/screenshots pipelines per the Generate checkboxes.
- Output named `<stem><suffix>.webp`, defaulting to `<stem> - reel.webp`, with the same `(1)`, `(2)` collision suffix the other outputs use.

## Non-goals

- Audio in the WebP (WebP has no audio track; `-an` is passed at extract to skip decode).
- Crossfades or separators between clips — hard cuts only.
- Configurable compression_level. Only `quality` is exposed, matching how the other outputs expose a single quality knob.

## Data model

New struct in `src-tauri/src/preview_reel.rs`:

```rust
pub struct PreviewOptions {
    pub count: u32,            // number of clips, default 6
    pub clip_length_secs: u32, // per-clip duration, default 5
    pub height: u32,           // output height px (width auto, preserves aspect), default 480
    pub fps: u32,              // fps cap, default 24
    pub quality: u32,          // libwebp -quality 0..=100, default 75
    #[serde(default)]
    pub suffix: String,        // empty → DEFAULT_PREVIEW_SUFFIX
}
```

Sampling uses the existing `layout::sample_timestamps(duration, count)` — identical to `screenshots.rs`, so "equal positions" means the same positions.

## Module layout

**New module — `src-tauri/src/preview_reel.rs`**
Parallels `screenshots.rs`. Pure helpers (`build_extract_args`, `build_stitch_args`, `write_concat_list`) are unit-tested; only `generate()` touches the filesystem and subprocesses.

**Extended — `output_path.rs`**
Adds `DEFAULT_PREVIEW_SUFFIX: &str = " - reel"` and `preview_reel_path(source, out_dir, suffix) -> PathBuf`. Reuses the existing `exists_fn` callback for collision handling.

**Extended — `commands.rs`**
New Tauri command `generate_preview_reels(app, state, items, opts: PreviewOptions, output: OutputLocation)` mirroring the shape of `generate_screenshots` / `generate_contact_sheets` — each output type is its own command (the existing convention). Per-file job loop: probe → compute `preview_reel_path` → call `preview_reel::generate(...)` → emit `job:file-done` / `job:file-failed`. `Killed` breaks the loop (same cancellation pattern). Registered in `lib.rs` `invoke_handler!` alongside the existing commands.

**Extended — `lib.rs`**
`preview_reel` is gated `#[cfg(any(test, feature = "test-api"))] pub mod preview_reel;` (else `mod`), matching the existing pattern for orchestration modules.

## ffmpeg pipeline

### Extract phase — one invocation per clip

```
ffmpeg -hide_banner -loglevel error -y \
  -ss <timestamp> -i <source> \
  -t <clamped_duration> \
  -an \
  [-vf scale=-2:<height>]        # included only when info.height > opts.height
  -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p \
  <tempdir>/clip_<NN>.mp4
```

- `-ss` before `-i` is fast keyframe seek (matches existing screenshots behaviour).
- `clamped_duration = min(opts.clip_length_secs as f64, info.duration_secs - timestamp)`. Short videos produce shorter tail clips rather than erroring.
- Re-encoding to H.264 + yuv420p guarantees uniform codec params across temp clips so the concat demuxer in the stitch phase works without surprises.
- Scaling only when the source is taller than the target avoids upscaling small sources. Width is `-2` (auto, even).

### Stitch phase — one final invocation

```
ffmpeg -hide_banner -loglevel error -y \
  -f concat -safe 0 -i <tempdir>/concat.txt \
  -vf fps=<fps> \
  -c:v libwebp -loop 0 -quality <q> \
  <output>.webp
```

`concat.txt` is a plain manifest:

```
file 'clip_01.mp4'
file 'clip_02.mp4'
...
```

`-safe 0` is required because the paths are absolute. Paths are escaped for the concat demuxer (single quotes and backslashes) by a pure helper `write_concat_list`.

### Temp directory

`tempfile::TempDir::new()` scoped inside `preview_reel::generate`. Auto-cleanup on drop, including on error or cancellation. The stitch phase runs before the `TempDir` goes out of scope.

## Output path

`output_path::preview_reel_path(source, out_dir, &opts.suffix)` composes `<out_dir>/<stem><effective_suffix>.webp`, where `effective_suffix` is `opts.suffix` if non-empty else `DEFAULT_PREVIEW_SUFFIX`. Collision handling reuses the same `(1)`, `(2)` logic already in `output_path.rs`.

## Frontend integration

**`index.html`**
- New checkbox `#prod-preview` labelled "Animated Preview" inside `#run-options`.
- New `<section>` in `#settings-view` with fields: `preview-count`, `preview-clip-length`, `preview-height`, `preview-fps`, `preview-quality`, `preview-suffix`.

**`src/options.js`**
- `readPreviewOpts()` returns a `PreviewOptions`-shaped object.
- `readProduce()` extended with `preview: checked('prod-preview')`.
- `applyProduce()` and `applyOpts()` extended symmetrically.

**`src/main.js`**
- `onGenerate` gains a third `invoke('generate_preview_reels', { items, opts: readPreviewOpts(), output: out })` call, gated on `produce.preview`. Runs after the existing sheet/screenshots invocations in the same serial sequence (matches how the existing two are ordered today).
- No new event kinds: `job:step` messages `"Reel clip i/N"` and `"Stitching reel"` are rendered verbatim by the existing `wireEvents()`.

**`src/style.css`**
- No new tokens. Reuses the existing `settings-section` rules.

## Progress & events

Reuses existing event names (`job:file-start`, `job:step`, `job:file-done`, `job:file-failed`, `job:finished`). Emits `job:step` entries:

- `"Reel clip i/N"` during each extract iteration (i = 1..=count).
- `"Stitching reel"` once, just before the stitch call.

## Error handling & cancellation

- Both extract loop iterations and the stitch call go through `ffmpeg::run_cancellable`, so the existing Cancel button interrupts mid-extract or mid-stitch.
- First extract failure returns `Err` immediately; `TempDir` drop cleans up.
- Stitch failure emits `job:file-failed` for this file and the job loop proceeds to the next file — same pattern as the existing pipelines.
- Tool availability: the existing `check_tools` gate covers `ffmpeg`/`ffprobe`. libwebp is present in all ffmpeg builds we already require (including `ffmpeg-full` on macOS). If it's somehow missing, the stitch error message from ffmpeg surfaces via `job:file-failed`.

## Testing

**Unit tests in `preview_reel.rs`:**
- `build_extract_args`: clamping when `ts + clip_length > duration`; scale filter included iff `info.height > opts.height`; `-an` always present; -ss before -i.
- `build_stitch_args`: fps/quality/output path wiring; `-loop 0`; `-c:v libwebp`; `-safe 0`.
- `write_concat_list`: escapes single quotes and backslashes per ffmpeg concat demuxer rules.

**Unit tests in `output_path.rs`:**
- `preview_reel_path`: default suffix when empty, custom suffix used when provided, collision suffix `(1)`/`(2)` via mocked `exists_fn`.

**Integration test** in `tests/integration.rs` (required-features `test-api`):
- Extends the existing fixture flow. Requests a reel with count=3, clip_length=1, height=240, fps=12, quality=60.
- Asserts the output file exists and begins with `RIFF....WEBP` magic bytes.
- Asserts a `VP8X` extended chunk is present with the animation flag byte set (bit 1 of the VP8X flags byte = 0x02).

## Filename security

Source paths flow through ffmpeg as argv elements (never shell-interpolated) exactly as they already do in `screenshots.rs` — no new attack surface there. The one new place user-derived strings touch is `concat.txt`. `write_concat_list` escapes single quotes and backslashes to block concat-demuxer injection. A unit test asserts this.

## Risks

1. **libwebp missing from user's ffmpeg build.** Extremely unlikely in current ffmpeg distributions we already require. If it happens, the stitch invocation fails with a clear ffmpeg error routed through `job:file-failed`. No silent fallback.
2. **Very short sources** (`duration < count * clip_length`). Extract clamping handles this; reel simply has shorter tail clips. Unit test covers the clamp.
3. **Very large source files.** Scaling during extract keeps temp files small; H.264 veryfast re-encode is cheap compared to a 4K→WebP single-pass alternative.

## Open questions

None at this stage.
