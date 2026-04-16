# Extraction Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix frame-accurate seeking, parallelize extraction loops, and add `-an` to all extraction pipelines.

**Architecture:** Add two new functions to `ffmpeg.rs` — `seek_input_args` (centralizes seeking+input args) and `run_batch_cancellable` (parallel execution with bounded concurrency). All four extraction modules switch from inline seeking + sequential loops to using these shared functions.

**Tech Stack:** Rust, tokio (JoinSet, Semaphore), existing ffmpeg.rs infrastructure

---

### Task 1: Add `seek_input_args` to ffmpeg.rs

**Files:**
- Modify: `src-tauri/src/ffmpeg.rs:1-7` (add function after `base_args`)
- Modify: `src-tauri/src/ffmpeg.rs:68-90` (add test in `mod tests`)

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block in `src-tauri/src/ffmpeg.rs`, after the `returns_ok_when_tools_present` test:

```rust
#[test]
fn seek_input_args_produces_dual_ss_with_copyts() {
    let args = seek_input_args(std::path::Path::new("/v/movie.mkv"), 42.5);
    assert_eq!(args, vec![
        "-ss", "42.500",
        "-copyts",
        "-i", "/v/movie.mkv",
        "-ss", "42.500",
        "-an",
    ]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib ffmpeg::tests::seek_input_args_produces_dual_ss_with_copyts`
Expected: FAIL — `seek_input_args` does not exist yet.

- [ ] **Step 3: Implement `seek_input_args`**

Add after the `base_args()` function (after line 7) in `src-tauri/src/ffmpeg.rs`:

```rust
/// Seeking + input args shared by all extraction pipelines.
/// Uses dual `-ss` with `-copyts` for frame-accurate seeking:
/// input-level `-ss` does fast keyframe seek, `-copyts` preserves original
/// stream timestamps, output-level `-ss` trims to the exact frame.
/// `-an` strips audio since no extraction pipeline produces audio output.
pub fn seek_input_args(source: &std::path::Path, timestamp: f64) -> Vec<String> {
    vec![
        "-ss".into(), format!("{:.3}", timestamp),
        "-copyts".into(),
        "-i".into(), source.to_string_lossy().into_owned(),
        "-ss".into(), format!("{:.3}", timestamp),
        "-an".into(),
    ]
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test --lib ffmpeg::tests::seek_input_args_produces_dual_ss_with_copyts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ffmpeg.rs
git commit -m "feat: add seek_input_args for frame-accurate dual-ss seeking"
```

---

### Task 2: Add `run_batch_cancellable` to ffmpeg.rs

**Files:**
- Modify: `src-tauri/src/ffmpeg.rs:139-190` (add function after `run_cancellable`)

- [ ] **Step 1: Implement `run_batch_cancellable`**

Add after the `run_cancellable` function (after line 190) in `src-tauri/src/ffmpeg.rs`:

```rust
/// Run multiple ffmpeg commands concurrently with bounded parallelism.
/// `on_done` fires in the caller's context with the original task index
/// each time a command completes. First error aborts all remaining tasks.
pub async fn run_batch_cancellable<F>(
    exe: &std::path::Path,
    batch: Vec<Vec<String>>,
    cancelled: Arc<AtomicBool>,
    mut on_done: F,
) -> Result<(), RunError>
where
    F: FnMut(usize),
{
    let concurrency = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);
    let sem = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut set = tokio::task::JoinSet::new();
    let exe = exe.to_path_buf();

    for (i, args) in batch.into_iter().enumerate() {
        let sem = sem.clone();
        let exe = exe.clone();
        let cancelled = cancelled.clone();
        set.spawn(async move {
            let _permit = sem.acquire().await.map_err(|_| RunError::Killed)?;
            run_cancellable(&exe, &args, cancelled).await?;
            Ok::<usize, RunError>(i)
        });
    }

    while let Some(result) = set.join_next().await {
        match result {
            Ok(Ok(i)) => on_done(i),
            Ok(Err(e)) => {
                set.abort_all();
                return Err(e);
            }
            Err(join_err) => {
                set.abort_all();
                return Err(RunError::Io(std::io::Error::other(join_err)));
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/ffmpeg.rs
git commit -m "feat: add run_batch_cancellable for parallel ffmpeg extraction"
```

---

### Task 3: Update contact_sheet.rs

**Files:**
- Modify: `src-tauri/src/contact_sheet.rs:1-2` (imports)
- Modify: `src-tauri/src/contact_sheet.rs:53-79` (extraction loop)

- [ ] **Step 1: Update imports**

In `src-tauri/src/contact_sheet.rs`, change line 2:

```rust
// Before
use crate::ffmpeg::{run_cancellable, RunError};

// After
use crate::ffmpeg::{run_batch_cancellable, run_cancellable, RunError};
```

- [ ] **Step 2: Replace extraction loop with batch**

Replace lines 53-79 (the `// 1. Extract thumbnails` block) with:

```rust
    // 1. Extract thumbnails (parallel)
    let mut batch = Vec::with_capacity(timestamps.len());
    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        let thumb = tmp.path().join(format!("thumb_{:0width$}.png", idx, width = width_digits));
        let mut vf = format!("scale={}:-2", layout.thumb_w);
        if opts.show_timestamps {
            vf.push(',');
            vf.push_str(&timestamp_overlay(
                &format_hms_escaped(*ts),
                &font_path,
                opts.thumb_font_size,
                opts.theme.fontcolor(),
                opts.theme.shadowcolor(),
            ));
        }
        let mut args = crate::ffmpeg::base_args();
        args.extend(crate::ffmpeg::seek_input_args(source, *ts));
        args.extend([
            "-frames:v".into(), "1".into(),
            "-vf".into(), vf,
            thumb.to_string_lossy().into_owned(),
        ]);
        batch.push(args);
    }

    let mut done = 0u32;
    run_batch_cancellable(ffmpeg, batch, cancelled.clone(), |_| {
        done += 1;
        (reporter.emit)(done, total_steps, &format!("Thumb {}/{}", done, layout.total));
    }).await?;
```

- [ ] **Step 3: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors.

- [ ] **Step 4: Run all tests**

Run: `cd src-tauri && cargo test --lib`
Expected: all tests pass (contact_sheet has no extraction-specific unit tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/contact_sheet.rs
git commit -m "refactor: contact sheet uses seek_input_args + parallel extraction"
```

---

### Task 4: Update screenshots.rs

**Files:**
- Modify: `src-tauri/src/screenshots.rs` (full file — imports + generate function)

- [ ] **Step 1: Rewrite screenshots.rs**

Replace the full contents of `src-tauri/src/screenshots.rs`:

```rust
use crate::ffmpeg::{run_batch_cancellable, RunError};
use crate::jobs::ProgressReporter;
use crate::layout::sample_timestamps;
use crate::output_path::{jpeg_qv, screenshot_path, OutputFormat};
use crate::video_info::VideoInfo;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenshotsOptions {
    pub count: u32,
    pub format: OutputFormat,
    pub jpeg_quality: u32,
    #[serde(default)]
    pub suffix: String,
}

pub async fn generate(
    source: &Path,
    info: &VideoInfo,
    out_dir: &Path,
    opts: &ScreenshotsOptions,
    ffmpeg: &Path,
    cancelled: Arc<AtomicBool>,
    reporter: &ProgressReporter<'_>,
) -> Result<Vec<std::path::PathBuf>, RunError> {
    std::fs::create_dir_all(out_dir)?;
    let timestamps = sample_timestamps(info.duration_secs, opts.count);
    let total = opts.count;

    let mut batch = Vec::with_capacity(timestamps.len());
    let mut outputs = Vec::with_capacity(timestamps.len());
    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        let out = screenshot_path(source, out_dir, opts.format, &opts.suffix, idx, opts.count);
        let mut args = crate::ffmpeg::base_args();
        args.extend(crate::ffmpeg::seek_input_args(source, *ts));
        args.extend(["-frames:v".into(), "1".into()]);
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", jpeg_qv(opts.jpeg_quality))]);
        }
        args.push(out.to_string_lossy().into_owned());
        batch.push(args);
        outputs.push(out);
    }

    let mut done = 0u32;
    run_batch_cancellable(ffmpeg, batch, cancelled.clone(), |_| {
        done += 1;
        (reporter.emit)(done, total, &format!("Shot {}/{}", done, total));
    }).await?;

    Ok(outputs)
}
```

- [ ] **Step 2: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/screenshots.rs
git commit -m "refactor: screenshots uses seek_input_args + parallel extraction"
```

---

### Task 5: Update preview_reel.rs

**Files:**
- Modify: `src-tauri/src/preview_reel.rs:1` (imports)
- Modify: `src-tauri/src/preview_reel.rs:22-48` (`build_extract_args`)
- Modify: `src-tauri/src/preview_reel.rs:125-168` (`generate`)
- Modify: `src-tauri/src/preview_reel.rs:192-239` (tests)

- [ ] **Step 1: Update imports**

In `src-tauri/src/preview_reel.rs`, change line 1:

```rust
// Before
use crate::ffmpeg::RunError;

// After
use crate::ffmpeg::{run_batch_cancellable, RunError};
```

- [ ] **Step 2: Update `build_extract_args` to use `seek_input_args`**

Replace the seeking args in `build_extract_args` (lines 34-40):

```rust
// Before
    let mut args = crate::ffmpeg::base_args();
    args.extend([
        "-ss".into(), format!("{:.3}", timestamp),
        "-i".into(), source.to_string_lossy().into_owned(),
        "-t".into(), format!("{:.3}", duration),
        "-an".into(),
    ]);

// After
    let mut args = crate::ffmpeg::base_args();
    args.extend(crate::ffmpeg::seek_input_args(source, timestamp));
    args.extend([
        "-t".into(), format!("{:.3}", duration),
    ]);
```

- [ ] **Step 3: Replace extraction loop in `generate` with batch**

Replace lines 148-158 (the `for` loop through `clips.push(clip)`) with:

```rust
    let mut clips: Vec<std::path::PathBuf> = Vec::with_capacity(timestamps.len());
    let mut batch = Vec::with_capacity(timestamps.len());

    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        let clip = tmp.path().join(format!("clip_{:03}.mp4", idx));
        let args = build_extract_args(source, info, *ts, opts.clip_length_secs, opts.height, &clip);
        batch.push(args);
        clips.push(clip);
    }

    let mut done = 0u32;
    run_batch_cancellable(ffmpeg, batch, cancelled.clone(), |_| {
        done += 1;
        (reporter.emit)(done, total_steps, &format!("Reel clip {}/{}", done, timestamps.len()));
    }).await?;
```

Also update line 156 (the stitch call) to use `crate::ffmpeg::run_cancellable` explicitly since `run_cancellable` is no longer imported at the top:

```rust
// Before
    crate::ffmpeg::run_cancellable(ffmpeg, &args, cancelled.clone()).await?;

// (this line is unchanged — it already uses the full path)
```

- [ ] **Step 4: Update `extract_args_basic` test**

Replace the test at lines 193-209 with:

```rust
    #[test]
    fn extract_args_basic() {
        let info = sample_info(60.0, 1080);
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            &info,
            12.5,
            5,
            480,
            &PathBuf::from("/tmp/out/clip_01.mp4"),
        );
        assert_eq!(args[0], "-hide_banner");
        assert!(args.iter().any(|a| a == "-copyts"));
        assert!(args.iter().any(|a| a == "-an"));
        // Dual -ss: input seek + output trim, both at the same timestamp
        let ss_positions: Vec<usize> = args.iter().enumerate()
            .filter(|(_, a)| a.as_str() == "-ss")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(ss_positions.len(), 2, "expected two -ss args");
        assert_eq!(args[ss_positions[0] + 1], "12.500");
        assert_eq!(args[ss_positions[1] + 1], "12.500");
        assert!(args.windows(2).any(|w| w[0] == "-i" && w[1] == "/v/movie.mkv"));
        assert!(args.windows(2).any(|w| w[0] == "-t" && w[1] == "5.000"));
        assert!(args.windows(2).any(|w| w[0] == "-vf" && w[1] == "scale=-2:480"));
        assert_eq!(args.last().unwrap(), "/tmp/out/clip_01.mp4");
    }
```

- [ ] **Step 5: Run all tests**

Run: `cd src-tauri && cargo test --lib`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/preview_reel.rs
git commit -m "refactor: preview reel uses seek_input_args + parallel extraction"
```

---

### Task 6: Update animated_sheet.rs

**Files:**
- Modify: `src-tauri/src/animated_sheet.rs:2` (imports)
- Modify: `src-tauri/src/animated_sheet.rs:56-98` (`build_extract_args`)
- Modify: `src-tauri/src/animated_sheet.rs:161-238` (`generate`)
- Modify: `src-tauri/src/animated_sheet.rs:283-339` (tests)

- [ ] **Step 1: Update imports**

In `src-tauri/src/animated_sheet.rs`, change line 2:

```rust
// Before
use crate::ffmpeg::{run_cancellable, RunError};

// After
use crate::ffmpeg::{run_batch_cancellable, run_cancellable, RunError};
```

- [ ] **Step 2: Update `build_extract_args` to use `seek_input_args`**

Replace the seeking args in `build_extract_args` (lines 86-94):

```rust
// Before
    let mut args = crate::ffmpeg::base_args();
    args.extend([
        "-ss".into(), format!("{:.3}", timestamp),
        "-i".into(), source.to_string_lossy().into_owned(),
        "-t".into(), format!("{}", clip_length_secs),
        "-an".into(),
        "-vf".into(), vf,
        "-r".into(), format!("{}", fps),
    ]);

// After
    let mut args = crate::ffmpeg::base_args();
    args.extend(crate::ffmpeg::seek_input_args(source, timestamp));
    args.extend([
        "-t".into(), format!("{}", clip_length_secs),
        "-vf".into(), vf,
        "-r".into(), format!("{}", fps),
    ]);
```

- [ ] **Step 3: Replace extraction loop in `generate` with batch**

Replace lines 199-212 (the extraction loop) with:

```rust
    let mut clips: Vec<PathBuf> = Vec::with_capacity(timestamps.len());
    let mut batch = Vec::with_capacity(timestamps.len());
    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        let cell = tmp.path().join(format!("cell_{:03}.mp4", idx));
        let args = build_extract_args(
            source, *ts, layout.thumb_w, thumb_h, opts.gap, opts.fps,
            opts.clip_length_secs, opts.show_timestamps, opts.thumb_font_size,
            opts.theme, font, &cell,
        );
        batch.push(args);
        clips.push(cell);
    }

    let mut done = 0u32;
    run_batch_cancellable(ffmpeg, batch, cancelled.clone(), |_| {
        done += 1;
        (reporter.emit)(done, total_steps, &format!("Cell {}/{}", done, layout.total));
    }).await?;
```

- [ ] **Step 4: Update `extract_args_shape` test**

Replace the test at lines 284-307 with:

```rust
    #[test]
    fn extract_args_shape() {
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            12.5,
            320, 180, 10, 12, 2,
            true, 18, SheetTheme::Dark,
            Path::new("/f/font.ttf"),
            Path::new("/tmp/cell.mp4"),
        );
        assert_eq!(args[0], "-hide_banner");
        assert!(args.iter().any(|a| a == "-copyts"));
        assert!(args.iter().any(|a| a == "-an"));
        // Dual -ss: input seek + output trim
        let ss_positions: Vec<usize> = args.iter().enumerate()
            .filter(|(_, a)| a.as_str() == "-ss")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(ss_positions.len(), 2, "expected two -ss args");
        assert_eq!(args[ss_positions[0] + 1], "12.500");
        assert_eq!(args[ss_positions[1] + 1], "12.500");
        assert!(args.windows(2).any(|w| w[0] == "-i" && w[1] == "/v/movie.mkv"));
        assert!(args.windows(2).any(|w| w[0] == "-t" && w[1] == "2"));
        assert!(args.windows(2).any(|w| w[0] == "-r" && w[1] == "12"));
        assert!(args.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(args.windows(2).any(|w| w[0] == "-pix_fmt" && w[1] == "yuv420p"));
        let vf_pos = args.iter().position(|a| a == "-vf").unwrap();
        let vf = &args[vf_pos + 1];
        assert!(vf.contains("scale=320:180"));
        assert!(vf.contains("drawtext="));
        assert!(vf.contains("pad=330:190:5:5:0x000000"));
        assert_eq!(args.last().unwrap(), "/tmp/cell.mp4");
    }
```

- [ ] **Step 5: Update `extract_args_light_theme_uses_white_bg_and_black_text` test**

Add `-copyts` assertion to the test at lines 310-323. Replace:

```rust
    #[test]
    fn extract_args_light_theme_uses_white_bg_and_black_text() {
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            5.0,
            320, 180, 10, 12, 2,
            true, 18, SheetTheme::Light,
            Path::new("/f/font.ttf"),
            Path::new("/tmp/cell.mp4"),
        );
        let vf = args.iter().position(|a| a == "-vf").map(|i| &args[i + 1]).unwrap();
        assert!(vf.contains("pad=330:190:5:5:0xFFFFFF"));
        assert!(vf.contains("fontcolor=black"));
        assert!(vf.contains("shadowcolor=white"));
    }
```

(This test body is unchanged — it only checks `-vf` content which is unaffected.)

- [ ] **Step 6: Run all tests**

Run: `cd src-tauri && cargo test --lib`
Expected: all 73+ tests pass (some tests got new assertions, count may stay same).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/animated_sheet.rs
git commit -m "refactor: animated sheet uses seek_input_args + parallel extraction"
```

---

### Task 7: Full verification

**Files:** none (read-only verification)

- [ ] **Step 1: Run full unit test suite**

Run: `cd src-tauri && cargo test --lib`
Expected: all tests pass.

- [ ] **Step 2: Run integration tests (if ffmpeg available)**

Run: `cd src-tauri && cargo test --features test-api 2>&1 | tail -20`
Expected: integration tests pass (or skip gracefully if ffmpeg-full not available).

- [ ] **Step 3: Build check**

Run: `cd src-tauri && cargo build`
Expected: compiles with no warnings relevant to our changes.

- [ ] **Step 4: Verify no remaining inline seeking args**

Grep for the old pattern — there should be no more `-vframes` or inline `-ss` + `-i` patterns in the four extraction modules:

Run: `grep -n 'vframes\|"-ss".*"-i"' src-tauri/src/{contact_sheet,screenshots,preview_reel,animated_sheet}.rs`
Expected: no matches (all seeking goes through `seek_input_args`).
