# Animated Preview Reel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a third output type to Mosaic: a single animated WebP "preview reel" per video, assembled from N short clips sampled at equal positions.

**Architecture:** Per-file two-phase pipeline. **Extract phase:** loop `N` calls to ffmpeg using fast `-ss` seek to cut `clip_length_secs` clips into a `tempfile::TempDir`, re-encoded uniformly to H.264 + yuv420p so the concat demuxer will accept them. **Stitch phase:** one final ffmpeg call using `-f concat -safe 0 -i concat.txt`, applying `-vf fps=<cap>` and encoding via `-c:v libwebp -loop 0 -quality <q>`. Pure helpers (`build_extract_args`, `build_stitch_args`, `write_concat_list`) are unit-tested; only orchestration touches the filesystem/subprocess. Third Tauri command `generate_preview_reels` mirrors the existing `generate_screenshots` / `generate_contact_sheets` shape.

**Tech Stack:** Rust (Tauri 2), tokio, tempfile, serde, ffmpeg (libx264 + libwebp encoders); vanilla JS/HTML/CSS frontend.

**Spec:** `docs/superpowers/specs/2026-04-15-animated-preview-reel-design.md`.

---

## File Structure

**New files:**
- `src-tauri/src/preview_reel.rs` — `PreviewOptions`, pure arg builders, concat-list writer, `generate()` orchestration.

**Modified files:**
- `src-tauri/src/lib.rs` — gate `preview_reel` behind `#[cfg(any(test, feature="test-api"))]`; register new Tauri command.
- `src-tauri/src/output_path.rs` — add `DEFAULT_PREVIEW_SUFFIX` and `preview_reel_path`.
- `src-tauri/src/commands.rs` — add `generate_preview_reels` command.
- `src-tauri/tests/integration.rs` — extend end-to-end test to cover a reel.
- `src/index.html` — third Generate checkbox + third settings section.
- `src/options.js` — `readPreviewOpts`, extend `readProduce`/`applyProduce`/`applyOpts`.
- `src/main.js` — third `invoke` call in `onGenerate`, action-bar labels, output preview, suffix default, enforce-at-least-one.

---

## Task 1: `PreviewOptions` struct + module skeleton

**Files:**
- Create: `src-tauri/src/preview_reel.rs`
- Modify: `src-tauri/src/lib.rs` (add module gate, right after `screenshots`)

- [ ] **Step 1: Create the module with just the options struct and a placeholder generate fn**

Write `src-tauri/src/preview_reel.rs`:

```rust
use crate::ffmpeg::RunError;
use crate::jobs::ProgressReporter;
use crate::video_info::VideoInfo;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreviewOptions {
    pub count: u32,
    pub clip_length_secs: u32,
    pub height: u32,
    pub fps: u32,
    pub quality: u32,
    #[serde(default)]
    pub suffix: String,
}

pub async fn generate(
    _source: &Path,
    _info: &VideoInfo,
    _out: &Path,
    _opts: &PreviewOptions,
    _ffmpeg: &Path,
    _cancelled: Arc<AtomicBool>,
    _reporter: &ProgressReporter<'_>,
) -> Result<(), RunError> {
    unimplemented!("wired up in a later task")
}
```

- [ ] **Step 2: Register module in `src-tauri/src/lib.rs`**

In `src-tauri/src/lib.rs`, add these lines immediately after the existing `screenshots` block (after line 29):

```rust
#[cfg(any(test, feature = "test-api"))]
pub mod preview_reel;
#[cfg(not(any(test, feature = "test-api")))]
mod preview_reel;
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: success (warnings about unused imports/fields are fine).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/preview_reel.rs src-tauri/src/lib.rs
git commit -m "feat(preview-reel): scaffold module and PreviewOptions"
```

---

## Task 2: `preview_reel_path` in `output_path.rs`

**Files:**
- Modify: `src-tauri/src/output_path.rs`

- [ ] **Step 1: Write the failing tests**

Append inside the `mod tests` block of `src-tauri/src/output_path.rs` (right before the closing `}` of the module):

```rust
#[test]
fn preview_reel_simple_case() {
    let p = preview_reel_path(
        Path::new("/videos/movie.mkv"),
        Path::new("/videos"),
        "",
        &|_| false,
    );
    assert_eq!(p, PathBuf::from("/videos/movie - reel.webp"));
}

#[test]
fn preview_reel_custom_suffix() {
    let p = preview_reel_path(
        Path::new("/videos/movie.mkv"),
        Path::new("/videos"),
        "_preview",
        &|_| false,
    );
    assert_eq!(p, PathBuf::from("/videos/movie_preview.webp"));
}

#[test]
fn preview_reel_appends_suffix_when_file_exists() {
    let taken: HashSet<PathBuf> = ["/out/movie - reel.webp", "/out/movie - reel (1).webp"]
        .into_iter().map(PathBuf::from).collect();
    let p = preview_reel_path(
        Path::new("/videos/movie.mkv"),
        Path::new("/out"),
        "",
        &|p| taken.contains(p),
    );
    assert_eq!(p, PathBuf::from("/out/movie - reel (2).webp"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test --lib output_path::tests::preview_reel`
Expected: compile errors — `preview_reel_path` not found.

- [ ] **Step 3: Add the constant and function**

In `src-tauri/src/output_path.rs`, immediately after `pub const DEFAULT_SHOTS_SUFFIX: ...` (line 4), add:

```rust
pub const DEFAULT_PREVIEW_SUFFIX: &str = " - reel";
```

After the existing `screenshot_path` function (at the end, before `jpeg_qv`), add:

```rust
pub fn preview_reel_path(
    source: &Path,
    out_dir: &Path,
    suffix: &str,
    exists_fn: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    let base = format!("{}{}", stem(source), resolved(suffix, DEFAULT_PREVIEW_SUFFIX));
    let candidate = out_dir.join(format!("{}.webp", base));
    if !exists_fn(&candidate) { return candidate; }
    let mut n = 1;
    loop {
        let c = out_dir.join(format!("{} ({}).webp", base, n));
        if !exists_fn(&c) { return c; }
        n += 1;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib output_path::tests::preview_reel`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/output_path.rs
git commit -m "feat(preview-reel): preview_reel_path with collision handling"
```

---

## Task 3: `build_extract_args` (pure helper)

**Files:**
- Modify: `src-tauri/src/preview_reel.rs`

- [ ] **Step 1: Write the failing tests**

Append to the end of `src-tauri/src/preview_reel.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_info(duration: f64, height: u32) -> VideoInfo {
        VideoInfo {
            filename: String::new(),
            duration_secs: duration,
            size_bytes: None,
            bit_rate: None,
            video: crate::video_info::VideoStream {
                codec: String::new(),
                profile: None,
                width: 1920,
                height,
                fps: 30.0,
                bit_rate: None,
            },
            audio: None,
        }
    }

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
        assert!(args.windows(2).any(|w| w[0] == "-ss" && w[1] == "12.500"));
        assert!(args.windows(2).any(|w| w[0] == "-i" && w[1] == "/v/movie.mkv"));
        assert!(args.windows(2).any(|w| w[0] == "-t" && w[1] == "5.000"));
        assert!(args.iter().any(|a| a == "-an"));
        assert!(args.windows(2).any(|w| w[0] == "-vf" && w[1] == "scale=-2:480"));
        assert_eq!(args.last().unwrap(), "/tmp/out/clip_01.mp4");
    }

    #[test]
    fn extract_args_skips_scale_when_source_smaller_or_equal() {
        let info = sample_info(60.0, 480);
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            &info,
            12.0,
            5,
            480,
            &PathBuf::from("/tmp/out/clip_01.mp4"),
        );
        assert!(!args.iter().any(|a| a == "-vf"));
    }

    #[test]
    fn extract_args_clamps_duration_when_overshoots_end() {
        let info = sample_info(10.0, 1080);
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            &info,
            8.0,
            5,
            480,
            &PathBuf::from("/tmp/out/clip_03.mp4"),
        );
        // 10.0 - 8.0 = 2.0 (less than requested 5)
        assert!(args.windows(2).any(|w| w[0] == "-t" && w[1] == "2.000"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib preview_reel::tests`
Expected: compile errors — `build_extract_args` not found.

- [ ] **Step 3: Implement `build_extract_args`**

In `src-tauri/src/preview_reel.rs`, add above the existing `generate` function:

```rust
pub fn build_extract_args(
    source: &Path,
    info: &VideoInfo,
    timestamp: f64,
    clip_length_secs: u32,
    target_height: u32,
    output: &Path,
) -> Vec<String> {
    let desired = clip_length_secs as f64;
    let remaining = (info.duration_secs - timestamp).max(0.0);
    let duration = desired.min(remaining);

    let mut args: Vec<String> = vec![
        "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
        "-ss".into(), format!("{:.3}", timestamp),
        "-i".into(), source.to_string_lossy().into_owned(),
        "-t".into(), format!("{:.3}", duration),
        "-an".into(),
    ];
    if info.video.height > target_height {
        args.push("-vf".into());
        args.push(format!("scale=-2:{}", target_height));
    }
    args.extend([
        "-c:v".into(), "libx264".into(),
        "-preset".into(), "veryfast".into(),
        "-crf".into(), "23".into(),
        "-pix_fmt".into(), "yuv420p".into(),
    ]);
    args.push(output.to_string_lossy().into_owned());
    args
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib preview_reel::tests`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/preview_reel.rs
git commit -m "feat(preview-reel): build_extract_args pure helper"
```

---

## Task 4: `build_stitch_args` (pure helper)

**Files:**
- Modify: `src-tauri/src/preview_reel.rs`

- [ ] **Step 1: Write the failing tests**

Inside the existing `mod tests { ... }` block in `src-tauri/src/preview_reel.rs`, append:

```rust
#[test]
fn stitch_args_uses_concat_demuxer_and_libwebp() {
    let args = build_stitch_args(
        Path::new("/tmp/concat.txt"),
        24,
        75,
        Path::new("/out/movie - reel.webp"),
    );
    assert_eq!(args[0], "-hide_banner");
    assert!(args.windows(2).any(|w| w[0] == "-f" && w[1] == "concat"));
    assert!(args.windows(2).any(|w| w[0] == "-safe" && w[1] == "0"));
    assert!(args.windows(2).any(|w| w[0] == "-i" && w[1] == "/tmp/concat.txt"));
    assert!(args.windows(2).any(|w| w[0] == "-vf" && w[1] == "fps=24"));
    assert!(args.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libwebp"));
    assert!(args.windows(2).any(|w| w[0] == "-loop" && w[1] == "0"));
    assert!(args.windows(2).any(|w| w[0] == "-quality" && w[1] == "75"));
    assert_eq!(args.last().unwrap(), "/out/movie - reel.webp");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib preview_reel::tests::stitch_args`
Expected: compile error — `build_stitch_args` not found.

- [ ] **Step 3: Implement `build_stitch_args`**

In `src-tauri/src/preview_reel.rs`, above `generate`:

```rust
pub fn build_stitch_args(
    concat_list: &Path,
    fps: u32,
    quality: u32,
    output: &Path,
) -> Vec<String> {
    vec![
        "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
        "-f".into(), "concat".into(),
        "-safe".into(), "0".into(),
        "-i".into(), concat_list.to_string_lossy().into_owned(),
        "-vf".into(), format!("fps={}", fps),
        "-c:v".into(), "libwebp".into(),
        "-loop".into(), "0".into(),
        "-quality".into(), format!("{}", quality),
        output.to_string_lossy().into_owned(),
    ]
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib preview_reel::tests::stitch_args`
Expected: 1 test passes.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/preview_reel.rs
git commit -m "feat(preview-reel): build_stitch_args pure helper"
```

---

## Task 5: `write_concat_list` (pure helper with escaping)

**Files:**
- Modify: `src-tauri/src/preview_reel.rs`

- [ ] **Step 1: Write the failing tests**

Inside the `mod tests` block in `src-tauri/src/preview_reel.rs`, append:

```rust
#[test]
fn concat_list_basic() {
    let list = render_concat_list(&[
        PathBuf::from("/tmp/a/clip_01.mp4"),
        PathBuf::from("/tmp/a/clip_02.mp4"),
    ]);
    assert_eq!(
        list,
        "file '/tmp/a/clip_01.mp4'\nfile '/tmp/a/clip_02.mp4'\n"
    );
}

#[test]
fn concat_list_escapes_single_quote_and_backslash() {
    // ffmpeg concat demuxer: inside single-quoted values, backslash and
    // single quote must each be backslash-escaped.
    let list = render_concat_list(&[
        PathBuf::from(r"/tmp/o'brien\videos/clip.mp4"),
    ]);
    assert_eq!(list, "file '/tmp/o\\'brien\\\\videos/clip.mp4'\n");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib preview_reel::tests::concat_list`
Expected: compile error — `render_concat_list` not found.

- [ ] **Step 3: Implement `render_concat_list`**

In `src-tauri/src/preview_reel.rs`, above `generate`:

```rust
pub fn render_concat_list(paths: &[std::path::PathBuf]) -> String {
    let mut out = String::new();
    for p in paths {
        let s = p.to_string_lossy();
        let escaped = s.replace('\\', "\\\\").replace('\'', "\\'");
        out.push_str("file '");
        out.push_str(&escaped);
        out.push_str("'\n");
    }
    out
}
```

Note the order: backslash replacement must run **before** single-quote replacement, otherwise the `\` that we introduce for quote escaping would be double-escaped.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib preview_reel::tests::concat_list`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/preview_reel.rs
git commit -m "feat(preview-reel): render_concat_list with ffmpeg escaping"
```

---

## Task 6: `preview_reel::generate` orchestration

**Files:**
- Modify: `src-tauri/src/preview_reel.rs`

- [ ] **Step 1: Replace the placeholder `generate` with the real implementation**

In `src-tauri/src/preview_reel.rs`, replace the existing placeholder `generate` function with:

```rust
pub async fn generate(
    source: &Path,
    info: &VideoInfo,
    out: &Path,
    opts: &PreviewOptions,
    ffmpeg: &Path,
    cancelled: Arc<AtomicBool>,
    reporter: &ProgressReporter<'_>,
) -> Result<(), RunError> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let timestamps = crate::layout::sample_timestamps(info.duration_secs, opts.count);
    if timestamps.is_empty() {
        return Err(RunError::NonZero {
            code: -1,
            stderr: "source duration too short for preview reel".into(),
        });
    }
    let total_steps = (timestamps.len() as u32) + 1; // extracts + stitch

    let tmp = tempfile::TempDir::new()?;
    let mut clips: Vec<std::path::PathBuf> = Vec::with_capacity(timestamps.len());

    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        (reporter.emit)(idx, total_steps, &format!("Reel clip {}/{}", idx, timestamps.len()));

        let clip = tmp.path().join(format!("clip_{:03}.mp4", idx));
        let args = build_extract_args(source, info, *ts, opts.clip_length_secs, opts.height, &clip);
        crate::ffmpeg::run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
        clips.push(clip);
    }

    (reporter.emit)(total_steps, total_steps, "Stitching reel");
    let concat_list = tmp.path().join("concat.txt");
    std::fs::write(&concat_list, render_concat_list(&clips))?;

    let args = build_stitch_args(&concat_list, opts.fps, opts.quality, out);
    crate::ffmpeg::run_cancellable(ffmpeg, &args, cancelled.clone()).await?;

    // `tmp` drops here, removing all extracted clips and concat.txt.
    Ok(())
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: success.

- [ ] **Step 3: Verify unit tests still pass**

Run: `cd src-tauri && cargo test --lib preview_reel`
Expected: 6 tests pass (3 extract, 1 stitch, 2 concat).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/preview_reel.rs
git commit -m "feat(preview-reel): generate() orchestration with tempdir and cancel"
```

---

## Task 7: `generate_preview_reels` Tauri command

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add imports and the new command in `commands.rs`**

At the top of `src-tauri/src/commands.rs`, add (alongside the existing `use crate::screenshots::...` line):

```rust
use crate::preview_reel::{self, PreviewOptions};
use crate::output_path::preview_reel_path;
```

At the end of `src-tauri/src/commands.rs`, just before the final `fn resolve_out_dir` helper, add:

```rust
#[tauri::command]
pub async fn generate_preview_reels(
    app: AppHandle,
    state: State<'_, Arc<JobState>>,
    items: Vec<QueueItem>,
    opts: PreviewOptions,
    output: OutputLocation,
) -> Result<(), String> {
    state.begin()?;
    let tools = locate_tools().map_err(|e| e.to_string())?;
    let total = items.len();
    let mut completed = 0u32;
    let mut failed = 0u32;
    let mut cancelled_count = 0u32;

    for (i, item) in items.iter().enumerate() {
        if state.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            cancelled_count = (total - i) as u32;
            break;
        }
        let _ = app.emit("job:file-start", serde_json::json!({
            "fileId": item.id, "index": i + 1, "total": total
        }));

        let source = PathBuf::from(&item.path);
        let out_dir = resolve_out_dir(&source, &output);
        let info = match probe(&tools.ffprobe, &item.path).await {
            Ok(i) => i,
            Err(e) => {
                failed += 1;
                let _ = app.emit("job:file-failed", serde_json::json!({ "fileId": item.id, "error": e }));
                continue;
            }
        };

        let out = preview_reel_path(&source, &out_dir, &opts.suffix, &|p| p.exists());
        let id = item.id.clone();
        let app2 = app.clone();
        let reporter = ProgressReporter {
            emit: &move |step, total_steps, label| {
                let _ = app2.emit("job:step", serde_json::json!({
                    "fileId": id, "step": step, "totalSteps": total_steps, "label": label
                }));
            },
        };

        match preview_reel::generate(
            &source, &info, &out, &opts, &tools.ffmpeg,
            state.cancelled.clone(), &reporter,
        ).await {
            Ok(()) => {
                completed += 1;
                let _ = app.emit("job:file-done", serde_json::json!({
                    "fileId": item.id, "outputPath": out.to_string_lossy()
                }));
            }
            Err(crate::ffmpeg::RunError::Killed) => {
                cancelled_count += 1;
                break;
            }
            Err(e) => {
                failed += 1;
                let _ = app.emit("job:file-failed", serde_json::json!({
                    "fileId": item.id, "error": e.to_string()
                }));
            }
        }
    }

    state.end();
    let _ = app.emit("job:finished", serde_json::json!({
        "completed": completed, "failed": failed, "cancelled": cancelled_count
    }));
    Ok(())
}
```

- [ ] **Step 2: Register the command in `src-tauri/src/lib.rs`**

In the `invoke_handler![...]` block (currently lines 59-67), add `commands::generate_preview_reels,` on a new line after `commands::generate_screenshots,`.

- [ ] **Step 3: Build**

Run: `cd src-tauri && cargo build`
Expected: success.

- [ ] **Step 4: Run all Rust tests**

Run: `cd src-tauri && cargo test --lib`
Expected: existing 34 tests + the 3 new `output_path` tests + the 6 new `preview_reel` tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(preview-reel): generate_preview_reels Tauri command"
```

---

## Task 8: HTML — third checkbox + third settings section

**Files:**
- Modify: `src/index.html`

- [ ] **Step 1: Add the third "Generate" checkbox**

In `src/index.html`, inside `#run-options` → first `.ro-row`, after the existing Contact Sheet checkbox (line 73), insert:

```html
<label class="check"><input type="checkbox" id="prod-preview" /><span>Animated Preview</span></label>
```

The final `.ro-row` should look like:

```html
<div class="ro-row">
  <span class="ro-label">Generate</span>
  <label class="check"><input type="checkbox" id="prod-shots" checked /><span>Screenshots</span></label>
  <label class="check"><input type="checkbox" id="prod-sheet" checked /><span>Contact Sheet</span></label>
  <label class="check"><input type="checkbox" id="prod-preview" /><span>Animated Preview</span></label>
</div>
```

(Default unchecked so existing users don't get surprise WebPs on first run after upgrade.)

- [ ] **Step 2: Add the third settings section**

In `src/index.html`, inside `#settings-view > .settings-body`, after the closing `</section>` of the Contact Sheet section (after line 138), insert:

```html
<section class="settings-section">
  <h3>Animated Preview</h3>
  <div class="grid">
    <label><span class="lbl">Clips</span><input type="number" id="preview-count" min="1" max="20" value="6" /></label>
    <label><span class="lbl">Clip length (s)</span><input type="number" id="preview-clip-length" min="1" max="30" value="5" /></label>
    <label><span class="lbl">Height (px)</span><input type="number" id="preview-height" min="120" max="2160" value="480" /></label>
    <label><span class="lbl">FPS cap</span><input type="number" id="preview-fps" min="5" max="60" value="24" /></label>
    <label><span class="lbl">Quality</span><input type="number" id="preview-quality" min="0" max="100" value="75" /></label>
    <label class="wide"><span class="lbl">Filename suffix</span><input type="text" id="preview-suffix" value=" - reel" spellcheck="false" /></label>
  </div>
</section>
```

- [ ] **Step 3: Sanity-check the page loads**

Run: `pnpm tauri dev` (leave open in another terminal) and verify the new checkbox appears in the main view and the new section appears in Settings. Close the window.

- [ ] **Step 4: Commit**

```bash
git add src/index.html
git commit -m "feat(preview-reel): UI checkbox and settings section"
```

---

## Task 9: `options.js` — read/apply PreviewOptions

**Files:**
- Modify: `src/options.js`

- [ ] **Step 1: Add `readPreviewOpts` and extend the produce helpers**

Replace the entire contents of `src/options.js` with:

```javascript
export function readSheetOpts() {
  return {
    cols: int('sheet-cols'),
    rows: int('sheet-rows'),
    width: int('sheet-width'),
    gap: int('sheet-gap'),
    thumb_font_size: int('sheet-thumb-font'),
    header_font_size: int('sheet-header-font'),
    show_timestamps: checked('sheet-timestamps'),
    show_header: checked('sheet-header'),
    format: select('sheet-format'),
    jpeg_quality: int('sheet-quality'),
    suffix: text('sheet-suffix'),
  };
}
export function readShotsOpts() {
  return {
    count: int('shots-count'),
    format: select('shots-format'),
    jpeg_quality: int('shots-quality'),
    suffix: text('shots-suffix'),
  };
}
export function readPreviewOpts() {
  return {
    count: int('preview-count'),
    clip_length_secs: int('preview-clip-length'),
    height: int('preview-height'),
    fps: int('preview-fps'),
    quality: int('preview-quality'),
    suffix: text('preview-suffix'),
  };
}
export function readOutput() {
  const mode = document.querySelector('input[name="out"]:checked').value;
  const custom = document.getElementById('custom-folder-path').textContent || null;
  return { mode, custom };
}
export function readProduce() {
  return {
    shots: document.getElementById('prod-shots').checked,
    sheet: document.getElementById('prod-sheet').checked,
    preview: document.getElementById('prod-preview').checked,
  };
}
export function applyProduce(produce) {
  if (!produce) return;
  const s = document.getElementById('prod-shots');
  const c = document.getElementById('prod-sheet');
  const p = document.getElementById('prod-preview');
  if (s && typeof produce.shots === 'boolean') s.checked = produce.shots;
  if (c && typeof produce.sheet === 'boolean') c.checked = produce.sheet;
  if (p && typeof produce.preview === 'boolean') p.checked = produce.preview;
}
export function applyOpts(sheet, shots, preview, out) {
  if (sheet) for (const [k, v] of Object.entries(sheet)) setField(`sheet-${mapKey(k)}`, v);
  if (shots) for (const [k, v] of Object.entries(shots)) setField(`shots-${mapKey(k)}`, v);
  if (preview) for (const [k, v] of Object.entries(preview)) setField(`preview-${mapKey(k)}`, v);
  if (out) {
    document.querySelector(`input[name="out"][value="${out.mode}"]`)?.click();
    if (out.custom) document.getElementById('custom-folder-path').textContent = out.custom;
  }
}

function mapKey(k) {
  return {
    thumb_font_size: 'thumb-font',
    header_font_size: 'header-font',
    show_timestamps: 'timestamps',
    show_header: 'header',
    jpeg_quality: 'quality',
    clip_length_secs: 'clip-length',
  }[k] || k;
}
function int(id) { return parseInt(document.getElementById(id).value, 10); }
function checked(id) { return document.getElementById(id).checked; }
function select(id) { return document.getElementById(id).value; }
function text(id) {
  const el = document.getElementById(id);
  return el ? el.value : '';
}
function setField(id, v) {
  const el = document.getElementById(id);
  if (!el) return;
  if (el.type === 'checkbox') el.checked = !!v;
  else el.value = v;
}
```

Key changes vs. the current file:
- New `readPreviewOpts`.
- `readProduce` now includes `preview`.
- `applyProduce` handles `preview`.
- `applyOpts` signature is now `(sheet, shots, preview, out)` — the `preview` argument is inserted **before** `out` (updates to callers follow in the next task).
- `mapKey` maps `clip_length_secs` → `clip-length` so the DOM id `preview-clip-length` resolves.

- [ ] **Step 2: Commit**

```bash
git add src/options.js
git commit -m "feat(preview-reel): options.js read/apply PreviewOptions"
```

---

## Task 10: `main.js` — wire the third invoke, labels, preview, settings

**Files:**
- Modify: `src/main.js`

- [ ] **Step 1: Update the import**

In `src/main.js`, change the import line (currently line 6):

```javascript
import { readSheetOpts, readShotsOpts, readOutput, readProduce, applyOpts, applyProduce } from './options.js';
```

to:

```javascript
import { readSheetOpts, readShotsOpts, readPreviewOpts, readOutput, readProduce, applyOpts, applyProduce } from './options.js';
```

- [ ] **Step 2: Update `loadSettings` to load preview opts**

Replace the `async function loadSettings()` body (currently lines 37-53) with:

```javascript
async function loadSettings() {
  try {
    store = await Store.load('settings.json');
    const saved = {
      sheet: await store.get('sheet'),
      shots: await store.get('shots'),
      preview: await store.get('preview'),
      out: await store.get('out'),
      produce: await store.get('produce'),
    };
    applyOpts(saved.sheet, saved.shots, saved.preview, saved.out);
    applyProduce(saved.produce);
    updateQualityVisibility();
    refreshActionBar();
  } catch (e) {
    console.error('settings load failed:', e);
  }
}
```

- [ ] **Step 3: Update `doSave` to persist preview opts**

Replace `doSave` (currently lines 174-181) with:

```javascript
async function doSave() {
  if (!store) return;
  await store.set('sheet', readSheetOpts());
  await store.set('shots', readShotsOpts());
  await store.set('preview', readPreviewOpts());
  await store.set('out', readOutput());
  await store.set('produce', readProduce());
  await store.save();
}
```

- [ ] **Step 4: Update `enforceProduceAtLeastOne` to consider all three**

Replace `enforceProduceAtLeastOne` (currently lines 218-222) with:

```javascript
function enforceProduceAtLeastOne() {
  const shots = document.getElementById('prod-shots');
  const sheet = document.getElementById('prod-sheet');
  const preview = document.getElementById('prod-preview');
  if (!shots.checked && !sheet.checked && !preview.checked) shots.checked = true;
}
```

- [ ] **Step 5: Add preview suffix default in `wireButtons`**

In `wireButtons` (around line 129), update the `suffixDefaults` object to include the reel suffix:

```javascript
const suffixDefaults = {
  'shots-suffix': '_screenshot_',
  'sheet-suffix': '_contact_sheet',
  'preview-suffix': ' - reel',
};
```

- [ ] **Step 6: Update `onGenerate` to run a third pass for preview**

Replace the entire `onGenerate` function (currently lines 229-286) with:

```javascript
async function onGenerate() {
  const produce = readProduce();
  const types = [];
  if (produce.shots) types.push('shots');
  if (produce.sheet) types.push('sheet');
  if (produce.preview) types.push('preview');
  if (!types.length) { showBanner('Pick at least one output type.'); return; }

  // Sweep any rows stuck in Running from a previous cancel before building the candidate list
  for (const it of queue.values()) {
    if (it.status === 'Running') queue.update(it.id, { status: 'Cancelled' });
  }
  const candidates = queue.values();
  if (!candidates.length) { showBanner('No files to process.'); return; }

  running = true;
  userCancelled = false;
  refreshActionBar();
  document.getElementById('btn-cancel').disabled = false;
  document.getElementById('status').textContent = '';
  const out = readOutput();
  const statusEl = document.getElementById('status');

  const prettyNames = { shots: 'Screenshots', sheet: 'Contact Sheets', preview: 'Animated Previews' };

  try {
    for (let i = 0; i < types.length; i++) {
      if (userCancelled) break;
      const type = types[i];
      const pretty = prettyNames[type];

      for (const it of candidates) {
        queue.update(it.id, { status: 'Pending', progress: null, error: null, outputPath: null });
      }

      statusEl.textContent = types.length > 1
        ? `Pass ${i + 1} of ${types.length} · ${pretty}`
        : `Generating ${pretty.toLowerCase()}`;

      const items = candidates.map(c => ({ id: c.id, path: c.path }));
      if (type === 'shots') {
        await invoke('generate_screenshots', { items, opts: readShotsOpts(), output: out });
      } else if (type === 'sheet') {
        await invoke('generate_contact_sheets', { items, opts: readSheetOpts(), output: out });
      } else {
        await invoke('generate_preview_reels', { items, opts: readPreviewOpts(), output: out });
      }
    }
    if (userCancelled) {
      statusEl.textContent = 'Cancelled';
    } else {
      statusEl.textContent = types.length > 1 ? 'All passes complete' : 'Done';
    }
  } finally {
    for (const it of queue.values()) {
      if (it.status === 'Running') queue.update(it.id, { status: 'Cancelled', progress: null });
    }
    running = false;
    document.getElementById('btn-cancel').disabled = true;
    refreshActionBar();
  }
}
```

- [ ] **Step 7: Update `refreshActionBar` button label + disabled logic**

Replace `refreshActionBar` (currently lines 294-307) with:

```javascript
function refreshActionBar() {
  const runnable = queue.values().filter(i => i.status !== 'Running');
  const produce = readProduce();
  const gen = document.getElementById('btn-generate');
  const label = gen.querySelector('.gen-label');
  const count = (produce.shots ? 1 : 0) + (produce.sheet ? 1 : 0) + (produce.preview ? 1 : 0);
  let base;
  if (count === 0) base = 'Generate';
  else if (count > 1) base = 'Generate';
  else if (produce.shots) base = 'Generate Screenshots';
  else if (produce.sheet) base = 'Generate Contact Sheets';
  else base = 'Generate Animated Previews';
  label.textContent = runnable.length > 0 ? `${base} (${runnable.length})` : base;
  gen.disabled = running || runnable.length === 0 || count === 0 || !toolsOk;
  renderOutputPreview();
}
```

- [ ] **Step 8: Update `renderOutputPreview` to include reel filename**

Replace `renderOutputPreview` (currently lines 309-341) with:

```javascript
function renderOutputPreview() {
  const preview = document.getElementById('output-preview');
  if (!preview) return;
  const first = queue.values()[0];
  const produce = readProduce();
  const anySelected = produce.shots || produce.sheet || produce.preview;
  if (!first || !anySelected) {
    preview.classList.add('hidden');
    preview.textContent = '';
    return;
  }
  const out = readOutput();
  const dir = out.mode === 'custom' && out.custom ? out.custom : dirname(first.path);
  const stem = basename(first.path).replace(/\.[^./\\]+$/, '');
  const parts = [];
  if (produce.shots) {
    const s = readShotsOpts();
    const ext = s.format === 'Jpeg' ? 'jpg' : 'png';
    const suffix = s.suffix || '_screenshot_';
    parts.push(`${stem}${suffix}01.${ext}`);
  }
  if (produce.sheet) {
    const s = readSheetOpts();
    const ext = s.format === 'Jpeg' ? 'jpg' : 'png';
    const suffix = s.suffix || '_contact_sheet';
    parts.push(`${stem}${suffix}.${ext}`);
  }
  if (produce.preview) {
    const p = readPreviewOpts();
    const suffix = p.suffix || ' - reel';
    parts.push(`${stem}${suffix}.webp`);
  }
  const count = queue.size();
  const firstPath = joinPath(dir, parts[0]);
  const also = parts.length > 1 ? ' +' : '';
  const moreFiles = count > 1 ? ` (+${count - 1} more)` : '';
  preview.textContent = `→ ${firstPath}${also}${moreFiles}`;
  preview.classList.remove('hidden');
}
```

- [ ] **Step 9: Smoke-test the app**

Run: `pnpm tauri dev`
- Verify the three "Generate" checkboxes appear.
- Toggling "Animated Preview" alone changes the button label to "Generate Animated Previews".
- Settings view now has three sections; typing in the Preview fields persists across app restart.
- Output preview shows `stem - reel.webp` when only Animated Preview is checked.

Close the app after the smoke test.

- [ ] **Step 10: Commit**

```bash
git add src/main.js
git commit -m "feat(preview-reel): main.js wires third invoke, labels, preview"
```

---

## Task 11: End-to-end integration test

**Files:**
- Modify: `src-tauri/tests/integration.rs`

- [ ] **Step 1: Append the new test**

At the bottom of `src-tauri/tests/integration.rs`, append:

```rust
#[tokio::test]
async fn end_to_end_animated_preview_reel() {
    if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
        eprintln!("skipping: ffmpeg/ffprobe not installed");
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "sample.mp4"].iter().collect();
    assert!(fixture.exists(), "missing test fixture {}", fixture.display());

    let info = mosaic_lib::ffmpeg_test_hook_probe(&tools.ffprobe, &fixture.to_string_lossy()).await.unwrap();
    assert!(info.duration_secs > 1.0);

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sample - reel.webp");

    let reporter = mosaic_lib::jobs::ProgressReporter { emit: &|_, _, _| {} };
    let opts = mosaic_lib::preview_reel::PreviewOptions {
        count: 3,
        clip_length_secs: 1,
        height: 240,
        fps: 12,
        quality: 60,
        suffix: String::new(),
    };

    mosaic_lib::preview_reel::generate(
        &fixture, &info, &out, &opts, &tools.ffmpeg,
        Arc::new(AtomicBool::new(false)), &reporter,
    ).await.unwrap();

    assert!(out.exists(), "reel not written");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.len() > 100, "reel suspiciously small: {} bytes", bytes.len());

    // WebP container: "RIFF"....WEBP (bytes 0-3 = RIFF, bytes 8-11 = WEBP).
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WEBP");

    // Animated WebP must include a VP8X chunk with the animation flag (0x02) set.
    // VP8X layout: "VP8X" (4) + chunk_size=10 (4) + flags byte + ...
    let vp8x_pos = bytes.windows(4).position(|w| w == b"VP8X")
        .expect("missing VP8X chunk — not an animated WebP");
    let flags_byte = bytes[vp8x_pos + 8];
    assert!(flags_byte & 0x02 != 0, "animation flag not set in VP8X flags byte: {:#04x}", flags_byte);
}
```

- [ ] **Step 2: Run the integration test**

Run:
```bash
cd src-tauri && PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --features test-api --test integration end_to_end_animated_preview_reel
```

Expected: 1 test passes.

If you see a `libwebp: Unknown encoder` error, the ffmpeg on `PATH` was built without libwebp. Confirm with `ffmpeg -hide_banner -encoders | grep webp` — should include `libwebp`.

- [ ] **Step 3: Run the full test suite**

Run: `cd src-tauri && PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --features test-api`
Expected: all tests pass (34 existing unit + 3 new `output_path` + 6 new `preview_reel` unit + existing integration + new integration = 2 integration tests).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tests/integration.rs
git commit -m "test(preview-reel): end-to-end animated WebP integration test"
```

---

## Task 12: Manual end-to-end check in the real app

- [ ] **Step 1: Run the app and test with a real video**

Run: `pnpm tauri dev`

Check each:
- Add a ~30s video.
- Enable only "Animated Preview". Click Generate.
- Verify progress shows "Reel clip 1/6", "Reel clip 2/6", ..., "Stitching reel".
- Verify `<video-stem> - reel.webp` is produced next to the source.
- Open the WebP in a browser (`file:///` URL or drag to a Chrome tab) — it should play back as an animation with 6 clips of ~5s each.
- Enable Animated Preview **plus** Contact Sheet and Screenshots. Click Generate.
- Verify three passes run sequentially and all three outputs are produced.
- Cancel mid-generation and confirm the active child process dies promptly and the UI shows "Cancelled".

- [ ] **Step 2: Commit anything leftover (if nothing to commit, skip)**

```bash
git status
# if there are uncommitted changes, add and commit with an appropriate message
```

---

## Post-implementation

- `CLAUDE.md` is authoritative on pipeline separation, cancellation model, drawtext escaping, and output-contract rules — all of which the new module follows. No update to `CLAUDE.md` is required.
- No new Tauri capability is needed (the new command is a custom command, not a filesystem or shell plugin call).
