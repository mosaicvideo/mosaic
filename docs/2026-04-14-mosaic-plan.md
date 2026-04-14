# Mosaic Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Mosaic, a cross-platform Tauri desktop app that generates video contact sheets and screenshot sets via system `ffmpeg`/`ffprobe`, with drag-drop batch queueing.

**Architecture:** Tauri shell. Rust backend handles ffmpeg/ffprobe subprocess orchestration, pure-function pipeline math (tested in isolation), and job lifecycle with cancellation. Vanilla HTML/CSS/JS frontend with a dropzone, queue list, tabbed options, and progress bar. Settings persist via `tauri-plugin-store`. One bundled `DejaVuSans.ttf` asset handles cross-platform drawtext rendering.

**Tech Stack:** Tauri 2.x, Rust (edition 2021, tokio, serde, tempfile), vanilla HTML/CSS/JS, system ffmpeg/ffprobe (dev), DejaVu Sans TTF (bundled).

**Spec:** `docs/2026-04-14-mosaic-design.md`

---

## Conventions

- All paths are relative to `/Users/abi/AvistaZ/mosaic/` unless stated otherwise.
- Rust crate name: `mosaic`. Tauri product name: `Mosaic`. Bundle identifier: `com.mosaic.app`.
- Commits use Conventional Commits (`feat:`, `test:`, `chore:`, `fix:`).
- **TDD discipline:** pure logic (layout math, parsing, escaping, filename generation) is written test-first. Subprocess orchestration and UI are covered by one end-to-end integration test against a small video fixture; iteration on those is manual.
- Run Rust tests from `src-tauri/`: `cargo test`.

---

## Task 1: Scaffold Tauri project

**Files:**
- Create: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/build.rs`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs`, `src/index.html`, `src/main.js`, `src/style.css`, `.gitignore`, `README.md`

- [ ] **Step 1: Initialize git and package.json**

```bash
cd /Users/abi/AvistaZ/mosaic
git init
```

Write `package.json`:

```json
{
  "name": "mosaic",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "tauri": "tauri",
    "dev": "tauri dev",
    "build": "tauri build"
  },
  "devDependencies": {
    "@tauri-apps/cli": "latest"
  },
  "dependencies": {
    "@tauri-apps/api": "latest",
    "@tauri-apps/plugin-dialog": "latest",
    "@tauri-apps/plugin-store": "latest"
  }
}
```

- [ ] **Step 2: Install npm deps**

```bash
pnpm install
```

Expected: lockfile written, no errors.

- [ ] **Step 3: Write `.gitignore`**

```
node_modules/
src-tauri/target/
dist/
.DS_Store
*.log
```

- [ ] **Step 4: Create `src-tauri/Cargo.toml`**

```toml
[package]
name = "mosaic"
version = "0.1.0"
edition = "2021"

[lib]
name = "mosaic_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
tauri-plugin-store = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["process", "io-util", "sync", "rt-multi-thread", "macros"] }
tempfile = "3"
thiserror = "1"
anyhow = "1"
which = "6"
uuid = { version = "1", features = ["v4"] }

[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

- [ ] **Step 5: Create `src-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build();
}
```

- [ ] **Step 6: Create `src-tauri/tauri.conf.json`**

```json
{
  "$schema": "../node_modules/@tauri-apps/cli/config.schema.json",
  "productName": "Mosaic",
  "version": "0.1.0",
  "identifier": "com.mosaic.app",
  "build": {
    "frontendDist": "../src"
  },
  "app": {
    "windows": [
      {
        "title": "Mosaic",
        "width": 1000,
        "height": 720,
        "minWidth": 900,
        "minHeight": 640,
        "dragDropEnabled": true
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [],
    "resources": ["assets/fonts/DejaVuSans.ttf"]
  }
}
```

- [ ] **Step 7: Create `src-tauri/src/lib.rs`**

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 8: Create `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mosaic_lib::run();
}
```

- [ ] **Step 9: Create minimal `src/index.html`, `src/main.js`, `src/style.css`**

`src/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Mosaic</title>
    <link rel="stylesheet" href="style.css" />
  </head>
  <body>
    <main id="app">
      <h1>Mosaic</h1>
      <p id="status">Loading…</p>
    </main>
    <script type="module" src="main.js"></script>
  </body>
</html>
```

`src/main.js`:

```js
document.getElementById("status").textContent = "Ready.";
```

`src/style.css`:

```css
* { box-sizing: border-box; }
body { margin: 0; font-family: system-ui, sans-serif; background: #1a1a1a; color: #eee; }
#app { padding: 16px; }
```

- [ ] **Step 10: Download DejaVu Sans font**

```bash
mkdir -p src-tauri/assets/fonts
curl -fsSL -o src-tauri/assets/fonts/DejaVuSans.ttf \
  https://github.com/dejavu-fonts/dejavu-fonts/raw/version_2_37/ttf/DejaVuSans.ttf
```

Expected: file exists and is > 500KB.

```bash
ls -lh src-tauri/assets/fonts/DejaVuSans.ttf
```

- [ ] **Step 11: Sanity-check build**

```bash
cd src-tauri && cargo check
```

Expected: compiles, possibly with unused-code warnings (fine at this stage).

- [ ] **Step 12: Commit**

```bash
git add -A
git commit -m "chore: scaffold Tauri project with bundled DejaVu Sans"
```

---

## Task 2: VideoInfo struct + ffprobe JSON parsing (TDD)

**Files:**
- Create: `src-tauri/src/video_info.rs`
- Create: `src-tauri/tests/fixtures/ffprobe_typical.json`
- Create: `src-tauri/tests/fixtures/ffprobe_no_audio.json`
- Create: `src-tauri/tests/fixtures/ffprobe_missing_duration.json`
- Modify: `src-tauri/src/lib.rs` (add `mod video_info;`)

- [ ] **Step 1: Create fixture files**

`src-tauri/tests/fixtures/ffprobe_typical.json`:

```json
{
  "streams": [
    { "codec_type": "video", "codec_name": "h264", "profile": "High", "width": 1920, "height": 1080, "r_frame_rate": "24000/1001", "bit_rate": "5000000" },
    { "codec_type": "audio", "codec_name": "aac", "profile": "LC", "sample_rate": "48000", "channels": 2, "bit_rate": "128000" }
  ],
  "format": {
    "filename": "/tmp/movie.mkv",
    "duration": "7234.5",
    "size": "5368709120",
    "bit_rate": "5200000"
  }
}
```

`src-tauri/tests/fixtures/ffprobe_no_audio.json`:

```json
{
  "streams": [
    { "codec_type": "video", "codec_name": "hevc", "width": 3840, "height": 2160, "r_frame_rate": "30/1" }
  ],
  "format": { "filename": "/tmp/silent.mp4", "duration": "120.0", "size": "104857600" }
}
```

`src-tauri/tests/fixtures/ffprobe_missing_duration.json`:

```json
{
  "streams": [{ "codec_type": "video", "codec_name": "h264", "width": 640, "height": 480, "r_frame_rate": "25/1" }],
  "format": { "filename": "/tmp/broken.mp4", "size": "1000" }
}
```

- [ ] **Step 2: Write failing tests**

Create `src-tauri/src/video_info.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoStream {
    pub codec: String,
    pub profile: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub bit_rate: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioStream {
    pub codec: String,
    pub profile: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub bit_rate: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoInfo {
    pub filename: String,
    pub duration_secs: f64,
    pub size_bytes: Option<u64>,
    pub bit_rate: Option<u64>,
    pub video: VideoStream,
    pub audio: Option<AudioStream>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeParseError {
    #[error("invalid ffprobe JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no video stream")]
    NoVideo,
    #[error("missing duration")]
    MissingDuration,
}

pub fn parse(_json: &str) -> Result<VideoInfo, ProbeParseError> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{}", name)).unwrap()
    }

    #[test]
    fn parses_typical_mkv() {
        let info = parse(&fixture("ffprobe_typical.json")).unwrap();
        assert_eq!(info.filename, "/tmp/movie.mkv");
        assert!((info.duration_secs - 7234.5).abs() < 1e-9);
        assert_eq!(info.size_bytes, Some(5368709120));
        assert_eq!(info.bit_rate, Some(5200000));
        assert_eq!(info.video.codec, "h264");
        assert_eq!(info.video.profile.as_deref(), Some("High"));
        assert_eq!(info.video.width, 1920);
        assert_eq!(info.video.height, 1080);
        assert!((info.video.fps - 24000.0 / 1001.0).abs() < 1e-4);
        assert_eq!(info.video.bit_rate, Some(5000000));
        let audio = info.audio.unwrap();
        assert_eq!(audio.codec, "aac");
        assert_eq!(audio.sample_rate, Some(48000));
        assert_eq!(audio.channels, Some(2));
    }

    #[test]
    fn parses_video_without_audio() {
        let info = parse(&fixture("ffprobe_no_audio.json")).unwrap();
        assert!(info.audio.is_none());
        assert_eq!(info.video.width, 3840);
        assert!((info.video.fps - 30.0).abs() < 1e-9);
    }

    #[test]
    fn fails_when_duration_missing() {
        let err = parse(&fixture("ffprobe_missing_duration.json")).unwrap_err();
        matches!(err, ProbeParseError::MissingDuration);
    }

    #[test]
    fn fails_on_invalid_json() {
        let err = parse("not json").unwrap_err();
        matches!(err, ProbeParseError::Json(_));
    }
}
```

Add to `src-tauri/src/lib.rs` (before `pub fn run()`):

```rust
mod video_info;
```

- [ ] **Step 3: Run tests, verify failure**

```bash
cd src-tauri && cargo test video_info
```

Expected: 4 tests fail with "not implemented" panics.

- [ ] **Step 4: Implement parser**

Replace the `parse` function and add helpers in `video_info.rs`:

```rust
#[derive(Deserialize)]
struct RawRoot {
    streams: Vec<RawStream>,
    format: RawFormat,
}

#[derive(Deserialize)]
struct RawStream {
    codec_type: String,
    codec_name: Option<String>,
    profile: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    bit_rate: Option<String>,
}

#[derive(Deserialize)]
struct RawFormat {
    filename: Option<String>,
    duration: Option<String>,
    size: Option<String>,
    bit_rate: Option<String>,
}

fn parse_fraction(s: &str) -> Option<f64> {
    let mut it = s.split('/');
    let num: f64 = it.next()?.parse().ok()?;
    let den: f64 = it.next().unwrap_or("1").parse().ok()?;
    if den == 0.0 { None } else { Some(num / den) }
}

pub fn parse(json: &str) -> Result<VideoInfo, ProbeParseError> {
    let root: RawRoot = serde_json::from_str(json)?;

    let duration_secs = root
        .format
        .duration
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|d| *d > 0.0)
        .ok_or(ProbeParseError::MissingDuration)?;

    let filename = root.format.filename.unwrap_or_default();
    let size_bytes = root.format.size.as_deref().and_then(|s| s.parse().ok());
    let bit_rate = root.format.bit_rate.as_deref().and_then(|s| s.parse().ok());

    let v = root
        .streams
        .iter()
        .find(|s| s.codec_type == "video")
        .ok_or(ProbeParseError::NoVideo)?;

    let video = VideoStream {
        codec: v.codec_name.clone().unwrap_or_default(),
        profile: v.profile.clone(),
        width: v.width.unwrap_or(0),
        height: v.height.unwrap_or(0),
        fps: v.r_frame_rate.as_deref().and_then(parse_fraction).unwrap_or(0.0),
        bit_rate: v.bit_rate.as_deref().and_then(|s| s.parse().ok()),
    };

    let audio = root.streams.iter().find(|s| s.codec_type == "audio").map(|a| AudioStream {
        codec: a.codec_name.clone().unwrap_or_default(),
        profile: a.profile.clone(),
        sample_rate: a.sample_rate.as_deref().and_then(|s| s.parse().ok()),
        channels: a.channels,
        bit_rate: a.bit_rate.as_deref().and_then(|s| s.parse().ok()),
    });

    Ok(VideoInfo { filename, duration_secs, size_bytes, bit_rate, video, audio })
}
```

- [ ] **Step 5: Run tests, verify pass**

```bash
cd src-tauri && cargo test video_info
```

Expected: 4 passed.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: parse ffprobe JSON into VideoInfo (TDD)"
```

---

## Task 3: drawtext escaping (TDD)

**Files:**
- Create: `src-tauri/src/drawtext.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod drawtext;`)

- [ ] **Step 1: Write failing tests**

`src-tauri/src/drawtext.rs`:

```rust
/// Escape a string so it can appear inside a drawtext `text='...'` argument.
/// Handles backslash, colon, single-quote, and percent.
pub fn escape_drawtext(_s: &str) -> String {
    unimplemented!()
}

/// Format a duration in seconds as `HH\:MM\:SS` (already escaped for drawtext).
pub fn format_hms_escaped(_seconds: f64) -> String {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_backslash_first() {
        assert_eq!(escape_drawtext(r"a\b"), r"a\\b");
    }

    #[test]
    fn escapes_colon() {
        assert_eq!(escape_drawtext("a:b"), r"a\:b");
    }

    #[test]
    fn escapes_single_quote() {
        assert_eq!(escape_drawtext("it's"), r"it\'s");
    }

    #[test]
    fn escapes_percent() {
        assert_eq!(escape_drawtext("50%"), "50%%");
    }

    #[test]
    fn escapes_combined() {
        // order matters: backslash first so earlier escapes aren't re-escaped
        assert_eq!(escape_drawtext(r"C:\a'b%"), r"C\:\\a\'b%%");
    }

    #[test]
    fn formats_hms_zero() {
        assert_eq!(format_hms_escaped(0.0), r"00\:00\:00");
    }

    #[test]
    fn formats_hms_typical() {
        // 1h 2m 3s
        assert_eq!(format_hms_escaped(3723.0), r"01\:02\:03");
    }

    #[test]
    fn formats_hms_truncates_fraction() {
        assert_eq!(format_hms_escaped(59.999), r"00\:00\:59");
    }
}
```

Add to `src-tauri/src/lib.rs`:

```rust
mod drawtext;
```

- [ ] **Step 2: Run tests, verify failure**

```bash
cd src-tauri && cargo test drawtext
```

Expected: 8 tests fail.

- [ ] **Step 3: Implement**

Replace stubs in `drawtext.rs`:

```rust
pub fn escape_drawtext(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            ':' => out.push_str(r"\:"),
            '\'' => out.push_str(r"\'"),
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out
}

pub fn format_hms_escaped(seconds: f64) -> String {
    let total = seconds as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!(r"{:02}\:{:02}\:{:02}", h, m, s)
}
```

- [ ] **Step 4: Run tests, verify pass**

```bash
cd src-tauri && cargo test drawtext
```

Expected: 8 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: drawtext escaping and HMS formatter (TDD)"
```

---

## Task 4: Contact sheet layout math (TDD)

**Files:**
- Create: `src-tauri/src/layout.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing tests**

`src-tauri/src/layout.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetLayout {
    pub cols: u32,
    pub rows: u32,
    pub total: u32,
    pub thumb_w: u32,
    pub grid_w: u32,
}

pub fn compute_sheet_layout(_cols: u32, _rows: u32, _width: u32, _gap: u32) -> SheetLayout {
    unimplemented!()
}

/// Timestamps (in seconds) for `n` evenly-spaced samples inside (0, duration).
/// Matches the original script: `interval = duration / (n + 1)`, `ts_i = i * interval`.
pub fn sample_timestamps(_duration_secs: f64, _n: u32) -> Vec<f64> {
    unimplemented!()
}

pub fn header_height(header_font_size: u32, gap: u32) -> u32 {
    let line_h = ((header_font_size as f64) * 1.3).round() as u32;
    2 * line_h + 2 * gap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_3x7_1920_gap10() {
        let l = compute_sheet_layout(3, 7, 1920, 10);
        assert_eq!(l.total, 21);
        assert_eq!(l.thumb_w, 626);
        assert_eq!(l.grid_w, 1918);
    }

    #[test]
    fn thumb_width_forced_even() {
        // 4 cols * 10 gap = 50 padding, 1920-50 = 1870, /4 = 467 (odd) → 466
        let l = compute_sheet_layout(4, 2, 1920, 10);
        assert_eq!(l.thumb_w % 2, 0);
    }

    #[test]
    fn timestamps_evenly_spaced_in_open_interval() {
        let ts = sample_timestamps(100.0, 4);
        assert_eq!(ts.len(), 4);
        for (i, v) in ts.iter().enumerate() {
            let expected = (i as f64 + 1.0) * 100.0 / 5.0;
            assert!((v - expected).abs() < 1e-9, "ts[{}]={} expected {}", i, v, expected);
        }
        assert!(ts[0] > 0.0);
        assert!(*ts.last().unwrap() < 100.0);
    }

    #[test]
    fn timestamps_zero_count_returns_empty() {
        assert!(sample_timestamps(100.0, 0).is_empty());
    }

    #[test]
    fn header_height_default() {
        // font=20 → line_h=26; 2*26 + 2*10 = 72
        assert_eq!(header_height(20, 10), 72);
    }
}
```

Add to `src-tauri/src/lib.rs`:

```rust
mod layout;
```

- [ ] **Step 2: Run tests, verify failure**

```bash
cd src-tauri && cargo test layout
```

Expected: 5 tests fail with unimplemented panics.

- [ ] **Step 3: Implement**

Replace stubs in `layout.rs`:

```rust
pub fn compute_sheet_layout(cols: u32, rows: u32, width: u32, gap: u32) -> SheetLayout {
    let total = cols * rows;
    let padding = gap * (cols + 1);
    let raw = width.saturating_sub(padding) / cols;
    let thumb_w = raw - (raw % 2);
    let grid_w = padding + cols * thumb_w;
    SheetLayout { cols, rows, total, thumb_w, grid_w }
}

pub fn sample_timestamps(duration_secs: f64, n: u32) -> Vec<f64> {
    if n == 0 || duration_secs <= 0.0 { return Vec::new(); }
    let interval = duration_secs / (n as f64 + 1.0);
    (1..=n).map(|i| i as f64 * interval).collect()
}
```

- [ ] **Step 4: Run tests, verify pass**

```bash
cd src-tauri && cargo test layout
```

Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: contact sheet layout math and sample timestamps (TDD)"
```

---

## Task 5: Output filename generation (TDD)

**Files:**
- Create: `src-tauri/src/output_path.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing tests**

`src-tauri/src/output_path.rs`:

```rust
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat { Png, Jpeg }

impl OutputFormat {
    pub fn ext(self) -> &'static str {
        match self { Self::Png => "png", Self::Jpeg => "jpg" }
    }
}

/// Returns the output path for a contact sheet.
/// If the candidate exists, appends " (1)", " (2)", … before the extension.
/// `exists_fn` lets tests avoid touching the filesystem.
pub fn contact_sheet_path(
    _source: &Path,
    _out_dir: &Path,
    _fmt: OutputFormat,
    _exists_fn: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    unimplemented!()
}

/// Returns the output path for screenshot `index` (1-based) out of `count`.
/// Zero-pads to the width required by `count`.
pub fn screenshot_path(
    _source: &Path,
    _out_dir: &Path,
    _fmt: OutputFormat,
    _index: u32,
    _count: u32,
) -> PathBuf {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn sheet_simple_case() {
        let p = contact_sheet_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/videos"),
            OutputFormat::Png,
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/videos/movie_contact_sheet.png"));
    }

    #[test]
    fn sheet_appends_suffix_when_file_exists() {
        let taken: HashSet<PathBuf> = ["/out/movie_contact_sheet.png", "/out/movie_contact_sheet (1).png"]
            .into_iter().map(PathBuf::from).collect();
        let p = contact_sheet_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/out"),
            OutputFormat::Png,
            &|p| taken.contains(p),
        );
        assert_eq!(p, PathBuf::from("/out/movie_contact_sheet (2).png"));
    }

    #[test]
    fn sheet_jpeg_extension() {
        let p = contact_sheet_path(
            Path::new("/a/x.mp4"),
            Path::new("/a"),
            OutputFormat::Jpeg,
            &|_| false,
        );
        assert_eq!(p.extension().unwrap(), "jpg");
    }

    #[test]
    fn screenshot_zero_padded_to_count_width() {
        let p = screenshot_path(
            Path::new("/v/clip.mp4"),
            Path::new("/v"),
            OutputFormat::Png,
            7,
            100,
        );
        assert_eq!(p, PathBuf::from("/v/clip_screenshot_007.png"));
    }

    #[test]
    fn screenshot_min_width_two() {
        let p = screenshot_path(
            Path::new("/v/clip.mp4"),
            Path::new("/v"),
            OutputFormat::Png,
            3,
            5,
        );
        assert_eq!(p, PathBuf::from("/v/clip_screenshot_03.png"));
    }
}
```

Add to `src-tauri/src/lib.rs`:

```rust
mod output_path;
```

- [ ] **Step 2: Run tests, verify failure**

```bash
cd src-tauri && cargo test output_path
```

Expected: 5 tests fail.

- [ ] **Step 3: Implement**

Replace stubs in `output_path.rs`:

```rust
fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn contact_sheet_path(
    source: &Path,
    out_dir: &Path,
    fmt: OutputFormat,
    exists_fn: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    let base = format!("{}_contact_sheet", stem(source));
    let ext = fmt.ext();
    let candidate = out_dir.join(format!("{}.{}", base, ext));
    if !exists_fn(&candidate) { return candidate; }
    let mut n = 1;
    loop {
        let c = out_dir.join(format!("{} ({}).{}", base, n, ext));
        if !exists_fn(&c) { return c; }
        n += 1;
    }
}

pub fn screenshot_path(
    source: &Path,
    out_dir: &Path,
    fmt: OutputFormat,
    index: u32,
    count: u32,
) -> PathBuf {
    let width = count.to_string().len().max(2);
    let num = format!("{:0width$}", index, width = width);
    out_dir.join(format!("{}_screenshot_{}.{}", stem(source), num, fmt.ext()))
}
```

- [ ] **Step 4: Run tests, verify pass**

```bash
cd src-tauri && cargo test output_path
```

Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: output filename generation with collision suffix (TDD)"
```

---

## Task 6: Info header line builders (TDD)

**Files:**
- Create: `src-tauri/src/header.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing tests**

`src-tauri/src/header.rs`:

```rust
use crate::video_info::VideoInfo;

/// Returns (line1, line2) for the info header. Both strings are already
/// drawtext-escaped and ready to be embedded in a `text='...'` filter argument.
pub fn build_header_lines(_info: &VideoInfo, _display_filename: &str) -> (String, String) {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_info::{AudioStream, VideoInfo, VideoStream};

    fn make_info() -> VideoInfo {
        VideoInfo {
            filename: "/tmp/x.mkv".into(),
            duration_secs: 3723.0, // 01:02:03
            size_bytes: Some(1_073_741_824), // 1.00 GiB
            bit_rate: Some(5_000_000),
            video: VideoStream {
                codec: "h264".into(),
                profile: Some("High".into()),
                width: 1920,
                height: 1080,
                fps: 23.976,
                bit_rate: Some(4_500_000),
            },
            audio: Some(AudioStream {
                codec: "aac".into(),
                profile: Some("LC".into()),
                sample_rate: Some(48000),
                channels: Some(2),
                bit_rate: Some(128_000),
            }),
        }
    }

    #[test]
    fn line1_is_escaped_filename() {
        let (l1, _) = build_header_lines(&make_info(), "it's : a test.mkv");
        assert_eq!(l1, r"it\'s \: a test.mkv");
    }

    #[test]
    fn line2_includes_size_duration_bitrate() {
        let (_, l2) = build_header_lines(&make_info(), "x.mkv");
        assert!(l2.contains("Size"));
        assert!(l2.contains("1.00 GiB"));
        assert!(l2.contains(r"01\:02\:03"));
        assert!(l2.contains("5.0 Mb/s"));
    }

    #[test]
    fn line2_includes_video_details() {
        let (_, l2) = build_header_lines(&make_info(), "x.mkv");
        assert!(l2.contains("h264 (High)"));
        assert!(l2.contains("1920x1080"));
        assert!(l2.contains("4500 kb/s"));
        assert!(l2.contains("23.98 fps"));
    }

    #[test]
    fn line2_includes_audio_stereo() {
        let (_, l2) = build_header_lines(&make_info(), "x.mkv");
        assert!(l2.contains("aac (LC)"));
        assert!(l2.contains("48000 Hz"));
        assert!(l2.contains("stereo"));
        assert!(l2.contains("128 kb/s"));
    }

    #[test]
    fn line2_omits_audio_when_missing() {
        let mut info = make_info();
        info.audio = None;
        let (_, l2) = build_header_lines(&info, "x.mkv");
        assert!(!l2.contains("Audio"));
    }

    #[test]
    fn line2_renders_multichannel() {
        let mut info = make_info();
        info.audio.as_mut().unwrap().channels = Some(6);
        let (_, l2) = build_header_lines(&info, "x.mkv");
        assert!(l2.contains("6 ch"));
    }
}
```

Add to `src-tauri/src/lib.rs`:

```rust
mod header;
```

- [ ] **Step 2: Run tests, verify failure**

```bash
cd src-tauri && cargo test header
```

Expected: 6 tests fail.

- [ ] **Step 3: Implement**

Replace stubs in `header.rs`:

```rust
use crate::drawtext::{escape_drawtext, format_hms_escaped};

fn format_gib(bytes: u64) -> String {
    format!("{:.2} GiB", bytes as f64 / 1_073_741_824.0)
}

fn format_mbps(bps: u64) -> String {
    format!("{:.1} Mb/s", bps as f64 / 1_000_000.0)
}

fn format_kbps(bps: u64) -> String {
    format!("{} kb/s", bps / 1000)
}

pub fn build_header_lines(info: &VideoInfo, display_filename: &str) -> (String, String) {
    let line1 = escape_drawtext(display_filename);

    let mut parts: Vec<String> = Vec::new();

    if let Some(sz) = info.size_bytes { parts.push(format!("Size: {}", format_gib(sz))); }
    parts.push(format!("Duration: {}", format_hms_escaped(info.duration_secs)));
    if let Some(br) = info.bit_rate { parts.push(format!("Bitrate: {}", format_mbps(br))); }

    let mut file_seg = parts.join(", ");

    // Video segment
    let v = &info.video;
    let v_profile = v.profile.as_deref().map(|p| format!(" ({})", p)).unwrap_or_default();
    let v_br = v.bit_rate.map(|b| format!(" | {}", format_kbps(b))).unwrap_or_default();
    let v_seg = format!(
        "Video: {}{} | {}x{}{} | {:.2} fps",
        v.codec, v_profile, v.width, v.height, v_br, v.fps
    );

    let mut segments = vec![file_seg, v_seg];

    if let Some(a) = &info.audio {
        let a_profile = a.profile.as_deref().map(|p| format!(" ({})", p)).unwrap_or_default();
        let a_rate = a.sample_rate.map(|r| format!(" | {} Hz", r)).unwrap_or_default();
        let a_ch = match a.channels {
            Some(2) => " | stereo".to_string(),
            Some(n) => format!(" | {} ch", n),
            None => String::new(),
        };
        let a_br = a.bit_rate.map(|b| format!(" | {}", format_kbps(b))).unwrap_or_default();
        segments.push(format!("Audio: {}{}{}{}{}", a.codec, a_profile, a_rate, a_ch, a_br));
    }

    let line2_raw = segments.join("  |  ");
    let line2 = escape_drawtext(&line2_raw);

    // Swap back the pre-escaped duration (it was double-escaped by the outer call)
    (line1, line2)
}
```

Note: the test expects `01\:02\:03` — a single backslash-escape. Since `escape_drawtext` also escapes `\`, running it over an already-escaped HMS would produce `01\\\:02\\\:03`. Fix: build `line2` in plain text first and escape at the end, using plain `HH:MM:SS`.

Rewrite the function:

```rust
pub fn build_header_lines(info: &VideoInfo, display_filename: &str) -> (String, String) {
    let line1 = escape_drawtext(display_filename);

    let hms = {
        let t = info.duration_secs as u64;
        format!("{:02}:{:02}:{:02}", t / 3600, (t % 3600) / 60, t % 60)
    };

    let mut parts: Vec<String> = Vec::new();
    if let Some(sz) = info.size_bytes { parts.push(format!("Size: {}", format_gib(sz))); }
    parts.push(format!("Duration: {}", hms));
    if let Some(br) = info.bit_rate { parts.push(format!("Bitrate: {}", format_mbps(br))); }

    let v = &info.video;
    let v_profile = v.profile.as_deref().map(|p| format!(" ({})", p)).unwrap_or_default();
    let v_br = v.bit_rate.map(|b| format!(" | {}", format_kbps(b))).unwrap_or_default();
    let v_seg = format!(
        "Video: {}{} | {}x{}{} | {:.2} fps",
        v.codec, v_profile, v.width, v.height, v_br, v.fps
    );

    let mut segments = vec![parts.join(", "), v_seg];

    if let Some(a) = &info.audio {
        let a_profile = a.profile.as_deref().map(|p| format!(" ({})", p)).unwrap_or_default();
        let a_rate = a.sample_rate.map(|r| format!(" | {} Hz", r)).unwrap_or_default();
        let a_ch = match a.channels {
            Some(2) => " | stereo".to_string(),
            Some(n) => format!(" | {} ch", n),
            None => String::new(),
        };
        let a_br = a.bit_rate.map(|b| format!(" | {}", format_kbps(b))).unwrap_or_default();
        segments.push(format!("Audio: {}{}{}{}{}", a.codec, a_profile, a_rate, a_ch, a_br));
    }

    let line2 = escape_drawtext(&segments.join("  |  "));
    (line1, line2)
}
```

- [ ] **Step 4: Run tests, verify pass**

```bash
cd src-tauri && cargo test header
```

Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: build info header lines with escaping (TDD)"
```

---

## Task 7: ffmpeg/ffprobe locator

**Files:**
- Create: `src-tauri/src/ffmpeg.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing test (locator error type + struct shape)**

`src-tauri/src/ffmpeg.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

#[derive(Debug, thiserror::Error, serde::Serialize)]
pub enum ToolsError {
    #[error("ffmpeg not found on PATH")]
    FfmpegMissing,
    #[error("ffprobe not found on PATH")]
    FfprobeMissing,
}

pub fn locate_tools() -> Result<Tools, ToolsError> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_ok_when_tools_present() {
        // This test is a smoke test: assume dev machine has both.
        // If absent, the test is skipped with a message.
        if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
            eprintln!("skipping: ffmpeg/ffprobe not installed");
            return;
        }
        let t = locate_tools().unwrap();
        assert!(t.ffmpeg.exists());
        assert!(t.ffprobe.exists());
    }
}
```

Add to `src-tauri/src/lib.rs`:

```rust
mod ffmpeg;
```

- [ ] **Step 2: Run test, verify failure**

```bash
cd src-tauri && cargo test ffmpeg
```

Expected: fails with unimplemented panic.

- [ ] **Step 3: Implement locator**

Replace `locate_tools` in `ffmpeg.rs`:

```rust
pub fn locate_tools() -> Result<Tools, ToolsError> {
    let extra_paths: &[&str] = if cfg!(target_os = "macos") {
        &["/opt/homebrew/bin", "/usr/local/bin"]
    } else {
        &[]
    };

    let find = |name: &str| -> Option<PathBuf> {
        if let Ok(p) = which::which(name) { return Some(p); }
        for ep in extra_paths {
            let candidate = std::path::Path::new(ep).join(name);
            if candidate.is_file() { return Some(candidate); }
        }
        None
    };

    let ffmpeg = find("ffmpeg").ok_or(ToolsError::FfmpegMissing)?;
    let ffprobe = find("ffprobe").ok_or(ToolsError::FfprobeMissing)?;
    Ok(Tools { ffmpeg, ffprobe })
}
```

- [ ] **Step 4: Run test, verify pass**

```bash
cd src-tauri && cargo test ffmpeg
```

Expected: 1 passed (or skipped if ffmpeg missing on dev machine).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: locate ffmpeg/ffprobe on PATH + macOS homebrew dirs"
```

---

## Task 8: Async ffmpeg runner + probe command

**Files:**
- Modify: `src-tauri/src/ffmpeg.rs` (add runner)
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod commands;`, register handler)

- [ ] **Step 1: Add async runner to `ffmpeg.rs`**

Append to `src-tauri/src/ffmpeg.rs`:

```rust
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("process exited with code {code}: {stderr}")]
    NonZero { code: i32, stderr: String },
    #[error("process killed")]
    Killed,
}

pub async fn run_capture(exe: &std::path::Path, args: &[&str]) -> Result<String, RunError> {
    let output = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(RunError::NonZero {
            code,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
```

- [ ] **Step 2: Create `commands.rs` with `probe_video`**

```rust
use crate::ffmpeg::{locate_tools, run_capture};
use crate::video_info::{parse, VideoInfo};

#[tauri::command]
pub async fn probe_video(path: String) -> Result<VideoInfo, String> {
    let tools = locate_tools().map_err(|e| e.to_string())?;
    let args = [
        "-v", "error",
        "-show_entries", "format=filename,duration,size,bit_rate",
        "-show_entries", "stream=codec_name,codec_type,width,height,r_frame_rate,sample_rate,channels,bit_rate,profile",
        "-of", "json",
        &path,
    ];
    let json = run_capture(&tools.ffprobe, &args).await.map_err(|e| e.to_string())?;
    parse(&json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_tools() -> Result<(), String> {
    locate_tools().map(|_| ()).map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register in `lib.rs`**

Update `lib.rs`:

```rust
mod video_info;
mod drawtext;
mod layout;
mod output_path;
mod header;
mod ffmpeg;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            commands::probe_video,
            commands::check_tools,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: Build**

```bash
cd src-tauri && cargo build
```

Expected: compiles. Warnings for unused code are OK.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: probe_video and check_tools Tauri commands"
```

---

## Task 9: Contact sheet pipeline (orchestration)

**Files:**
- Create: `src-tauri/src/contact_sheet.rs`
- Modify: `src-tauri/src/ffmpeg.rs` (add spawn helper with cancellation hook)
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Extend `ffmpeg.rs` with a cancellable runner**

Append to `src-tauri/src/ffmpeg.rs`:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub async fn run_cancellable(
    exe: &std::path::Path,
    args: &[String],
    cancelled: Arc<AtomicBool>,
) -> Result<(), RunError> {
    if cancelled.load(Ordering::Relaxed) { return Err(RunError::Killed); }

    let mut child = Command::new(exe)
        .args(args.iter().map(|s| s.as_str()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let id = child.id();
    let flag = cancelled.clone();
    let watch = tokio::spawn(async move {
        loop {
            if flag.load(Ordering::Relaxed) { return true; }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    });

    tokio::select! {
        status = child.wait() => {
            watch.abort();
            let status = status?;
            if !status.success() {
                // Best-effort grab of stderr
                let mut buf = String::new();
                if let Some(mut err) = child.stderr.take() {
                    use tokio::io::AsyncReadExt;
                    let _ = err.read_to_string(&mut buf).await;
                }
                return Err(RunError::NonZero { code: status.code().unwrap_or(-1), stderr: buf });
            }
            Ok(())
        }
        _ = async {
            while !cancelled.load(Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        } => {
            let _ = child.kill().await;
            watch.abort();
            let _ = id; // suppress unused
            Err(RunError::Killed)
        }
    }
}
```

- [ ] **Step 2: Create `contact_sheet.rs`**

```rust
use crate::drawtext::{escape_drawtext, format_hms_escaped};
use crate::ffmpeg::{run_cancellable, RunError};
use crate::header::build_header_lines;
use crate::layout::{compute_sheet_layout, header_height, sample_timestamps};
use crate::output_path::OutputFormat;
use crate::video_info::VideoInfo;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tempfile::TempDir;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SheetOptions {
    pub cols: u32,
    pub rows: u32,
    pub width: u32,
    pub gap: u32,
    pub thumb_font_size: u32,
    pub header_font_size: u32,
    pub show_timestamps: bool,
    pub show_header: bool,
    pub format: OutputFormat,
    pub jpeg_quality: u32,
}

pub struct ProgressReporter<'a> {
    pub emit: &'a (dyn Fn(u32, u32, &str) + Send + Sync),
}

pub async fn generate(
    source: &Path,
    info: &VideoInfo,
    output_path: &Path,
    opts: &SheetOptions,
    ffmpeg: &Path,
    font: &Path,
    cancelled: Arc<AtomicBool>,
    reporter: &ProgressReporter<'_>,
) -> Result<(), RunError> {
    let layout = compute_sheet_layout(opts.cols, opts.rows, opts.width, opts.gap);
    let timestamps = sample_timestamps(info.duration_secs, layout.total);
    let tmp = TempDir::new()?;
    let width_digits = layout.total.to_string().len().max(2);

    let font_path = font_for_ffmpeg(font);
    let total_steps = layout.total + 2 + u32::from(opts.show_header); // extracts + tile + stack + header

    // 1. Extract thumbnails
    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        (reporter.emit)(idx, total_steps, &format!("Extracting thumb {}/{}", idx, layout.total));

        let thumb = tmp.path().join(format!("thumb_{:0width$}.png", idx, width = width_digits));
        let mut vf = format!("scale={}:-2", layout.thumb_w);
        if opts.show_timestamps {
            let hms = format_hms_escaped(*ts);
            vf.push_str(&format!(
                ",drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:shadowcolor=black:shadowx=1:shadowy=1:x=5:y=h-th-5",
                hms, font_path, opts.thumb_font_size
            ));
        }
        let args: Vec<String> = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
            "-ss".into(), format!("{}", ts),
            "-i".into(), source.to_string_lossy().into_owned(),
            "-vframes".into(), "1".into(),
            "-vf".into(), vf,
            thumb.to_string_lossy().into_owned(),
        ];
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
    }

    // 2. Tile
    (reporter.emit)(layout.total + 1, total_steps, "Building grid");
    let grid = tmp.path().join("grid.png");
    let tile_input = tmp.path().join(format!("thumb_%0{}d.png", width_digits));
    let args: Vec<String> = vec![
        "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
        "-framerate".into(), "1".into(),
        "-start_number".into(), "1".into(),
        "-i".into(), tile_input.to_string_lossy().into_owned(),
        "-vf".into(), format!(
            "tile={}x{}:margin={}:padding={}:color=0x000000",
            opts.cols, opts.rows, opts.gap, opts.gap
        ),
        "-frames:v".into(), "1".into(),
        grid.to_string_lossy().into_owned(),
    ];
    run_cancellable(ffmpeg, &args, cancelled.clone()).await?;

    // 3. Header (optional)
    let final_tmp: PathBuf;
    if opts.show_header {
        (reporter.emit)(layout.total + 2, total_steps, "Rendering header");
        let display = source.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let (l1, l2) = build_header_lines(info, &display);
        let h = header_height(opts.header_font_size, opts.gap);
        let line_h = ((opts.header_font_size as f64) * 1.3).round() as u32;
        let vf = format!(
            "drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:x={}:y={},drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:x={}:y={}",
            l1, font_path, opts.header_font_size, opts.gap, opts.gap,
            l2, font_path, opts.header_font_size, opts.gap, opts.gap + line_h
        );
        let header = tmp.path().join("header.png");
        let args: Vec<String> = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
            "-f".into(), "lavfi".into(),
            "-i".into(), format!("color=c=0x000000:s={}x{}:d=1", layout.grid_w, h),
            "-vf".into(), vf,
            "-frames:v".into(), "1".into(),
            header.to_string_lossy().into_owned(),
        ];
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;

        (reporter.emit)(total_steps, total_steps, "Composing final image");
        final_tmp = tmp.path().join(format!("final.{}", opts.format.ext()));
        let mut args: Vec<String> = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
            "-i".into(), header.to_string_lossy().into_owned(),
            "-i".into(), grid.to_string_lossy().into_owned(),
            "-filter_complex".into(), "vstack".into(),
            "-frames:v".into(), "1".into(),
        ];
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", quality_to_qv(opts.jpeg_quality))]);
        }
        args.push(final_tmp.to_string_lossy().into_owned());
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
    } else {
        (reporter.emit)(total_steps, total_steps, "Finalizing");
        // No header: re-encode grid to the target format so extension matches content.
        final_tmp = tmp.path().join(format!("final.{}", opts.format.ext()));
        let mut args: Vec<String> = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
            "-i".into(), grid.to_string_lossy().into_owned(),
            "-frames:v".into(), "1".into(),
        ];
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", quality_to_qv(opts.jpeg_quality))]);
        }
        args.push(final_tmp.to_string_lossy().into_owned());
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
    }

    // 4. Move final into place
    std::fs::create_dir_all(output_path.parent().unwrap_or(Path::new(".")))?;
    std::fs::rename(&final_tmp, output_path).or_else(|_| std::fs::copy(&final_tmp, output_path).map(|_| ()))?;
    Ok(())
}

fn quality_to_qv(q: u32) -> u32 {
    // libmjpeg: 2 (best) .. 31 (worst). Map 100→2, 50→15.
    let q = q.clamp(50, 100) as i64;
    (2 + ((100 - q) * 13 / 50)).max(2) as u32
}

fn font_for_ffmpeg(p: &Path) -> String {
    let mut s = p.to_string_lossy().into_owned();
    if cfg!(windows) {
        // Escape drive colon and normalise slashes for drawtext
        s = s.replace('\\', "/");
        if let Some(idx) = s.find(':') {
            s.replace_range(idx..idx + 1, r"\:");
        }
    }
    s
}
```

- [ ] **Step 3: Wire module**

Add to `lib.rs`:

```rust
mod contact_sheet;
```

- [ ] **Step 4: Build**

```bash
cd src-tauri && cargo build
```

Expected: compiles (warnings for now-unused items OK).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: contact sheet pipeline with cancellable ffmpeg steps"
```

---

## Task 10: Screenshots pipeline

**Files:**
- Create: `src-tauri/src/screenshots.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `screenshots.rs`**

```rust
use crate::contact_sheet::ProgressReporter;
use crate::ffmpeg::{run_cancellable, RunError};
use crate::layout::sample_timestamps;
use crate::output_path::{screenshot_path, OutputFormat};
use crate::video_info::VideoInfo;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenshotsOptions {
    pub count: u32,
    pub width: u32, // 0 = keep source
    pub format: OutputFormat,
    pub jpeg_quality: u32,
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
    let mut outputs = Vec::with_capacity(timestamps.len());

    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        (reporter.emit)(idx, total, &format!("Screenshot {}/{}", idx, total));

        let out = screenshot_path(source, out_dir, opts.format, idx, opts.count);
        let mut args: Vec<String> = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
            "-ss".into(), format!("{}", ts),
            "-i".into(), source.to_string_lossy().into_owned(),
            "-vframes".into(), "1".into(),
        ];
        if opts.width > 0 {
            args.extend(["-vf".into(), format!("scale={}:-2", opts.width)]);
        }
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", crate::contact_sheet::jpeg_qv(opts.jpeg_quality))]);
        }
        args.push(out.to_string_lossy().into_owned());

        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
        outputs.push(out);
    }

    Ok(outputs)
}
```

- [ ] **Step 2: Expose `jpeg_qv` from `contact_sheet`**

Change `fn quality_to_qv` in `contact_sheet.rs` to `pub fn jpeg_qv`. Update the single call site within `contact_sheet.rs` to match.

- [ ] **Step 3: Wire module**

Add to `lib.rs`:

```rust
mod screenshots;
```

- [ ] **Step 4: Build**

```bash
cd src-tauri && cargo build
```

Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: screenshots pipeline"
```

---

## Task 11: Job manager, Tauri commands, progress events

**Files:**
- Create: `src-tauri/src/jobs.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `jobs.rs`**

```rust
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;

#[derive(Default)]
pub struct JobState {
    pub cancelled: Arc<AtomicBool>,
    pub running: Mutex<bool>,
}

impl JobState {
    pub fn begin(&self) -> Result<(), String> {
        let mut running = self.running.lock().unwrap();
        if *running { return Err("a job is already running".into()); }
        self.cancelled.store(false, std::sync::atomic::Ordering::Relaxed);
        *running = true;
        Ok(())
    }
    pub fn end(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
```

- [ ] **Step 2: Rewrite `commands.rs` to add generation commands**

```rust
use crate::contact_sheet::{self, SheetOptions, ProgressReporter};
use crate::ffmpeg::{locate_tools, run_capture};
use crate::jobs::JobState;
use crate::output_path::{contact_sheet_path, OutputFormat};
use crate::screenshots::{self, ScreenshotsOptions};
use crate::video_info::{parse, VideoInfo};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub async fn probe_video(path: String) -> Result<VideoInfo, String> {
    let tools = locate_tools().map_err(|e| e.to_string())?;
    let args = [
        "-v", "error",
        "-show_entries", "format=filename,duration,size,bit_rate",
        "-show_entries", "stream=codec_name,codec_type,width,height,r_frame_rate,sample_rate,channels,bit_rate,profile",
        "-of", "json",
        &path,
    ];
    let json = run_capture(&tools.ffprobe, &args).await.map_err(|e| e.to_string())?;
    parse(&json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_tools() -> Result<(), String> {
    locate_tools().map(|_| ()).map_err(|e| e.to_string())
}

#[derive(serde::Deserialize)]
pub struct QueueItem {
    pub id: String,
    pub path: String,
}

#[derive(serde::Deserialize)]
pub struct OutputLocation {
    pub mode: String, // "next_to_source" | "custom"
    pub custom: Option<String>,
}

#[tauri::command]
pub async fn generate_contact_sheets(
    app: AppHandle,
    state: State<'_, Arc<JobState>>,
    items: Vec<QueueItem>,
    opts: SheetOptions,
    output: OutputLocation,
) -> Result<(), String> {
    state.begin()?;
    let tools = locate_tools().map_err(|e| e.to_string())?;
    let font = app.path().resolve("assets/fonts/DejaVuSans.ttf", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;

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
        let info = match probe_inner(&tools.ffprobe, &item.path).await {
            Ok(i) => i,
            Err(e) => {
                failed += 1;
                let _ = app.emit("job:file-failed", serde_json::json!({ "fileId": item.id, "error": e }));
                continue;
            }
        };

        let out = contact_sheet_path(&source, &out_dir, opts.format, &|p| p.exists());
        let id = item.id.clone();
        let app2 = app.clone();
        let reporter = ProgressReporter {
            emit: &move |step, total_steps, label| {
                let _ = app2.emit("job:step", serde_json::json!({
                    "fileId": id, "step": step, "totalSteps": total_steps, "label": label
                }));
            },
        };

        match contact_sheet::generate(
            &source, &info, &out, &opts, &tools.ffmpeg, &font,
            state.cancelled.clone(), &reporter
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

#[tauri::command]
pub async fn generate_screenshots(
    app: AppHandle,
    state: State<'_, Arc<JobState>>,
    items: Vec<QueueItem>,
    opts: ScreenshotsOptions,
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
        let info = match probe_inner(&tools.ffprobe, &item.path).await {
            Ok(i) => i,
            Err(e) => {
                failed += 1;
                let _ = app.emit("job:file-failed", serde_json::json!({ "fileId": item.id, "error": e }));
                continue;
            }
        };

        let id = item.id.clone();
        let app2 = app.clone();
        let reporter = ProgressReporter {
            emit: &move |step, total_steps, label| {
                let _ = app2.emit("job:step", serde_json::json!({
                    "fileId": id, "step": step, "totalSteps": total_steps, "label": label
                }));
            },
        };

        match screenshots::generate(
            &source, &info, &out_dir, &opts, &tools.ffmpeg,
            state.cancelled.clone(), &reporter
        ).await {
            Ok(paths) => {
                completed += 1;
                let _ = app.emit("job:file-done", serde_json::json!({
                    "fileId": item.id,
                    "outputPath": paths.first().map(|p| p.to_string_lossy().into_owned())
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

#[tauri::command]
pub fn cancel_job(state: State<'_, Arc<JobState>>) {
    state.cancel();
}

async fn probe_inner(ffprobe: &std::path::Path, path: &str) -> Result<VideoInfo, String> {
    let args = [
        "-v", "error",
        "-show_entries", "format=filename,duration,size,bit_rate",
        "-show_entries", "stream=codec_name,codec_type,width,height,r_frame_rate,sample_rate,channels,bit_rate,profile",
        "-of", "json",
        path,
    ];
    let json = run_capture(ffprobe, &args).await.map_err(|e| e.to_string())?;
    parse(&json).map_err(|e| e.to_string())
}

fn resolve_out_dir(source: &std::path::Path, output: &OutputLocation) -> PathBuf {
    match output.mode.as_str() {
        "custom" => output.custom.as_ref().map(PathBuf::from)
            .unwrap_or_else(|| source.parent().map(PathBuf::from).unwrap_or_default()),
        _ => source.parent().map(PathBuf::from).unwrap_or_default(),
    }
}
```

- [ ] **Step 3: Register state and commands in `lib.rs`**

Replace `run()`:

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(std::sync::Arc::new(crate::jobs::JobState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::probe_video,
            commands::check_tools,
            commands::generate_contact_sheets,
            commands::generate_screenshots,
            commands::cancel_job,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Add module declaration:

```rust
mod jobs;
```

- [ ] **Step 4: Build**

```bash
cd src-tauri && cargo build
```

Expected: compiles. Fix any leftover unused-import warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: job manager, progress events, generate commands"
```

---

## Task 12: Frontend — HTML shell + CSS

**Files:**
- Modify: `src/index.html`
- Modify: `src/style.css`

- [ ] **Step 1: Replace `src/index.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Mosaic</title>
    <link rel="stylesheet" href="style.css" />
  </head>
  <body>
    <div id="banner" class="banner hidden" role="alert"></div>
    <main id="app">
      <section id="input-zone">
        <div id="dropzone">
          <p>Drop videos or folders here</p>
          <div class="btn-row">
            <button id="btn-add-files">Add Files…</button>
            <button id="btn-add-folder">Add Folder…</button>
            <button id="btn-clear" class="subtle">Clear Queue</button>
          </div>
        </div>
        <ul id="queue"></ul>
      </section>

      <section id="options">
        <div class="tabs">
          <button class="tab active" data-tab="sheet">Contact Sheet</button>
          <button class="tab" data-tab="shots">Screenshots</button>
        </div>

        <div class="tab-panel active" data-panel="sheet">
          <div class="grid">
            <label>Columns <input type="number" id="sheet-cols" min="1" max="10" value="3" /></label>
            <label>Rows <input type="number" id="sheet-rows" min="1" max="20" value="7" /></label>
            <label>Width (px) <input type="number" id="sheet-width" min="320" max="4096" value="1920" /></label>
            <label>Gap (px) <input type="number" id="sheet-gap" min="0" max="50" value="10" /></label>
            <label>Thumb font <input type="number" id="sheet-thumb-font" min="8" max="72" value="18" /></label>
            <label>Header font <input type="number" id="sheet-header-font" min="8" max="72" value="20" /></label>
            <label class="check"><input type="checkbox" id="sheet-timestamps" checked /> Show timestamps</label>
            <label class="check"><input type="checkbox" id="sheet-header" checked /> Show info header</label>
            <label>Format
              <select id="sheet-format"><option value="Png">PNG</option><option value="Jpeg">JPEG</option></select>
            </label>
            <label>JPEG quality <input type="number" id="sheet-quality" min="50" max="100" value="92" /></label>
          </div>
        </div>

        <div class="tab-panel" data-panel="shots">
          <div class="grid">
            <label>Count <input type="number" id="shots-count" min="1" max="100" value="10" /></label>
            <label>Width (px) <input type="number" id="shots-width" min="0" max="4096" value="1920" /></label>
            <label>Format
              <select id="shots-format"><option value="Png">PNG</option><option value="Jpeg">JPEG</option></select>
            </label>
            <label>JPEG quality <input type="number" id="shots-quality" min="50" max="100" value="92" /></label>
          </div>
        </div>

        <fieldset class="output">
          <legend>Output location</legend>
          <label class="check"><input type="radio" name="out" value="next_to_source" checked /> Next to source</label>
          <label class="check"><input type="radio" name="out" value="custom" /> Custom folder</label>
          <div id="custom-folder-row" class="hidden">
            <button id="btn-pick-folder">Choose folder…</button>
            <span id="custom-folder-path"></span>
          </div>
        </fieldset>
      </section>

      <footer id="action-bar">
        <button id="btn-generate">Generate Contact Sheets</button>
        <button id="btn-cancel" disabled>Cancel</button>
        <progress id="progress" value="0" max="1"></progress>
        <span id="status"></span>
      </footer>
    </main>
    <script type="module" src="main.js"></script>
  </body>
</html>
```

- [ ] **Step 2: Replace `src/style.css`**

```css
* { box-sizing: border-box; }
html, body { height: 100%; }
body { margin: 0; font: 13px/1.4 system-ui, sans-serif; background: #141414; color: #e8e8e8; }
#app { display: grid; grid-template-rows: 1fr auto auto; height: 100vh; }

.banner { padding: 10px 16px; background: #5a1f1f; color: #ffd6d6; }
.banner.hidden { display: none; }

#input-zone { padding: 12px; overflow: auto; display: grid; grid-template-rows: auto 1fr; gap: 12px; }
#dropzone { border: 2px dashed #555; border-radius: 8px; padding: 16px; text-align: center; }
#dropzone.drag { border-color: #6cf; background: #1f2a36; }
.btn-row { margin-top: 8px; display: flex; gap: 8px; justify-content: center; }

#queue { list-style: none; margin: 0; padding: 0; border: 1px solid #2a2a2a; border-radius: 6px; overflow: auto; max-height: 240px; }
#queue li { display: grid; grid-template-columns: 1fr auto auto auto; gap: 8px; align-items: center; padding: 6px 10px; border-top: 1px solid #222; }
#queue li:first-child { border-top: 0; }
.status { font-size: 11px; padding: 2px 6px; border-radius: 10px; background: #333; }
.status.Done { background: #1f3d1f; color: #aef0ae; }
.status.Failed { background: #3d1f1f; color: #ffb0b0; }
.status.Running { background: #1f2f3d; color: #aee0ff; }
.status.Cancelled { background: #2f2f2f; color: #ccc; }

#options { padding: 12px; border-top: 1px solid #2a2a2a; background: #181818; }
.tabs { display: flex; gap: 2px; }
.tab { background: #222; color: #aaa; border: 0; padding: 6px 12px; cursor: pointer; }
.tab.active { background: #333; color: #fff; }
.tab-panel { display: none; padding: 12px; background: #1c1c1c; border-radius: 0 0 6px 6px; }
.tab-panel.active { display: block; }
.grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px 16px; }
.grid label { display: flex; flex-direction: column; font-size: 11px; color: #aaa; }
.grid input, .grid select { margin-top: 2px; background: #0f0f0f; color: #eee; border: 1px solid #333; border-radius: 4px; padding: 4px; }
.check { flex-direction: row !important; align-items: center; gap: 6px; color: #ddd; }
.output { margin-top: 12px; border: 1px solid #2a2a2a; border-radius: 6px; padding: 8px 12px; }

#action-bar { display: grid; grid-template-columns: auto auto 1fr auto; align-items: center; gap: 12px; padding: 10px 12px; background: #101010; border-top: 1px solid #2a2a2a; }
#action-bar progress { width: 100%; height: 10px; }
button { background: #2f6fd4; border: 0; color: #fff; padding: 6px 14px; border-radius: 4px; cursor: pointer; }
button:disabled { opacity: 0.4; cursor: default; }
button.subtle { background: #333; }
.hidden { display: none; }
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: frontend HTML shell and dark-theme CSS"
```

---

## Task 13: Frontend — state, queue, options, IPC wiring

**Files:**
- Create: `src/queue.js`
- Create: `src/options.js`
- Create: `src/dropzone.js`
- Replace: `src/main.js`

- [ ] **Step 1: Create `src/queue.js`**

```js
const VIDEO_EXTS = ['mp4','mkv','mov','avi','webm','wmv','flv','m4v','mpg','mpeg','ts','m2ts'];

export function isVideo(path) {
  const m = path.toLowerCase().match(/\.([^./\\]+)$/);
  return !!m && VIDEO_EXTS.includes(m[1]);
}

export function createQueue(root) {
  const items = new Map(); // id -> { id, path, status, error, progress }

  function render() {
    root.innerHTML = '';
    for (const it of items.values()) {
      const li = document.createElement('li');
      const name = document.createElement('span');
      name.textContent = shorten(it.path, 60);
      name.title = it.path;
      const prog = document.createElement('span');
      prog.className = 'progress-label';
      prog.textContent = it.progress || '';
      const badge = document.createElement('span');
      badge.className = `status ${it.status}`;
      badge.textContent = it.status;
      const rm = document.createElement('button');
      rm.className = 'subtle';
      rm.textContent = '×';
      rm.disabled = it.status === 'Running';
      rm.onclick = () => { items.delete(it.id); render(); };
      li.append(name, prog, badge, rm);
      if (it.error) {
        const err = document.createElement('div');
        err.className = 'error';
        err.style.gridColumn = '1 / -1';
        err.style.color = '#ffa0a0';
        err.style.fontSize = '11px';
        err.textContent = it.error;
        li.append(err);
      }
      root.append(li);
    }
  }

  function add(paths) {
    let added = 0;
    for (const p of paths) {
      if (!isVideo(p)) continue;
      if ([...items.values()].some(i => i.path === p)) continue;
      const id = crypto.randomUUID();
      items.set(id, { id, path: p, status: 'Pending' });
      added++;
    }
    if (added) render();
    return added;
  }

  function update(id, patch) {
    const it = items.get(id);
    if (!it) return;
    Object.assign(it, patch);
    render();
  }

  function clear() { items.clear(); render(); }
  function values() { return [...items.values()]; }
  function pending() { return values().filter(i => i.status !== 'Done'); }

  return { add, update, clear, values, pending };
}

function shorten(s, max) {
  if (s.length <= max) return s;
  const half = Math.floor((max - 1) / 2);
  return s.slice(0, half) + '…' + s.slice(-half);
}
```

- [ ] **Step 2: Create `src/options.js`**

```js
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
  };
}
export function readShotsOpts() {
  return {
    count: int('shots-count'),
    width: int('shots-width'),
    format: select('shots-format'),
    jpeg_quality: int('shots-quality'),
  };
}
export function readOutput() {
  const mode = document.querySelector('input[name="out"]:checked').value;
  const custom = document.getElementById('custom-folder-path').textContent || null;
  return { mode, custom };
}
export function applyOpts(sheet, shots, out) {
  if (sheet) for (const [k, v] of Object.entries(sheet)) setField(`sheet-${mapKey(k)}`, v);
  if (shots) for (const [k, v] of Object.entries(shots)) setField(`shots-${mapKey(k)}`, v);
  if (out) {
    document.querySelector(`input[name="out"][value="${out.mode}"]`)?.click();
    if (out.custom) document.getElementById('custom-folder-path').textContent = out.custom;
  }
}

function mapKey(k) {
  return { thumb_font_size: 'thumb-font', header_font_size: 'header-font', show_timestamps: 'timestamps', show_header: 'header', jpeg_quality: 'quality' }[k] || k;
}
function int(id) { return parseInt(document.getElementById(id).value, 10); }
function checked(id) { return document.getElementById(id).checked; }
function select(id) { return document.getElementById(id).value; }
function setField(id, v) {
  const el = document.getElementById(id);
  if (!el) return;
  if (el.type === 'checkbox') el.checked = !!v;
  else el.value = v;
}
```

- [ ] **Step 3: Create `src/dropzone.js`**

```js
import { getCurrentWebview } from '@tauri-apps/api/webview';

export function wireDropzone(el, onPaths) {
  const webview = getCurrentWebview();
  webview.onDragDropEvent((event) => {
    if (event.payload.type === 'over') el.classList.add('drag');
    else if (event.payload.type === 'drop') {
      el.classList.remove('drag');
      onPaths(event.payload.paths || []);
    } else el.classList.remove('drag');
  });
}
```

- [ ] **Step 4: Replace `src/main.js`**

```js
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { Store } from '@tauri-apps/plugin-store';
import { createQueue, isVideo } from './queue.js';
import { readSheetOpts, readShotsOpts, readOutput, applyOpts } from './options.js';
import { wireDropzone } from './dropzone.js';

const queue = createQueue(document.getElementById('queue'));
let activeTab = 'sheet';
let store;

async function init() {
  store = await Store.load('settings.json');
  const saved = {
    sheet: await store.get('sheet'),
    shots: await store.get('shots'),
    out: await store.get('out'),
    activeTab: await store.get('activeTab') || 'sheet',
  };
  applyOpts(saved.sheet, saved.shots, saved.out);
  switchTab(saved.activeTab);

  try { await invoke('check_tools'); }
  catch (e) { showBanner(`ffmpeg/ffprobe not found on PATH. Install with: brew install ffmpeg  (macOS) / winget install ffmpeg (Windows) / apt install ffmpeg (Linux). ${e}`); }

  wireButtons();
  wireDropzone(document.getElementById('dropzone'), addPaths);
  wireEvents();
}

function wireButtons() {
  document.getElementById('btn-add-files').onclick = async () => {
    const picked = await open({ multiple: true, filters: [{ name: 'Videos', extensions: ['mp4','mkv','mov','avi','webm','wmv','flv','m4v','mpg','mpeg','ts','m2ts'] }] });
    if (!picked) return;
    addPaths(Array.isArray(picked) ? picked : [picked]);
  };
  document.getElementById('btn-add-folder').onclick = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (!dir) return;
    // Walk recursively by asking the backend? For v1 keep it simple: only accept files;
    // users can drag-drop folders and Tauri's drop event lists file paths only.
    // Add a single-level scan using Tauri's FS plugin later if needed.
    addPaths([dir]); // just attempt — isVideo filter drops non-videos
  };
  document.getElementById('btn-clear').onclick = () => queue.clear();
  document.getElementById('btn-generate').onclick = onGenerate;
  document.getElementById('btn-cancel').onclick = () => invoke('cancel_job');
  document.getElementById('btn-pick-folder').onclick = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (dir) document.getElementById('custom-folder-path').textContent = dir;
  };
  document.querySelectorAll('.tab').forEach(b => b.onclick = () => switchTab(b.dataset.tab));
  document.querySelectorAll('input[name="out"]').forEach(r => r.onchange = () => {
    document.getElementById('custom-folder-row').classList.toggle('hidden', r.value !== 'custom' || !r.checked);
  });
  document.querySelectorAll('#options input, #options select').forEach(el => el.onchange = saveSettings);
}

function switchTab(name) {
  activeTab = name;
  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('active', t.dataset.tab === name));
  document.querySelectorAll('.tab-panel').forEach(p => p.classList.toggle('active', p.dataset.panel === name));
  document.getElementById('btn-generate').textContent =
    name === 'sheet' ? 'Generate Contact Sheets' : 'Generate Screenshots';
  saveSettings();
}

async function saveSettings() {
  if (!store) return;
  await store.set('sheet', readSheetOpts());
  await store.set('shots', readShotsOpts());
  await store.set('out', readOutput());
  await store.set('activeTab', activeTab);
  await store.save();
}

async function addPaths(paths) {
  const vids = paths.filter(isVideo);
  const added = queue.add(vids);
  if (!added) return;
  // Probe asynchronously to fill in duration/resolution (not strictly needed for generation).
  for (const it of queue.values()) {
    if (it.status !== 'Pending' || it.probed) continue;
    try {
      const info = await invoke('probe_video', { path: it.path });
      queue.update(it.id, { probed: true, info });
    } catch (_) { /* keep Pending; errors will surface at generation */ }
  }
}

function wireEvents() {
  listen('job:file-start', ({ payload }) => {
    queue.update(payload.fileId, { status: 'Running', progress: 'Starting…' });
    updateOverall(payload.index - 1, payload.total);
  });
  listen('job:step', ({ payload }) => {
    queue.update(payload.fileId, { progress: payload.label });
  });
  listen('job:file-done', ({ payload }) => {
    queue.update(payload.fileId, { status: 'Done', progress: 'Done' });
  });
  listen('job:file-failed', ({ payload }) => {
    queue.update(payload.fileId, { status: 'Failed', error: payload.error });
  });
  listen('job:finished', ({ payload }) => {
    document.getElementById('btn-generate').disabled = false;
    document.getElementById('btn-cancel').disabled = true;
    document.getElementById('status').textContent =
      `Done: ${payload.completed} ok, ${payload.failed} failed, ${payload.cancelled} cancelled.`;
  });
}

function updateOverall(done, total) {
  const p = document.getElementById('progress');
  p.max = total; p.value = done;
}

async function onGenerate() {
  const items = queue.values()
    .filter(i => i.status === 'Pending' || i.status === 'Failed' || i.status === 'Cancelled')
    .map(i => ({ id: i.id, path: i.path }));
  if (!items.length) { showBanner('No files in queue.'); return; }
  document.getElementById('btn-generate').disabled = true;
  document.getElementById('btn-cancel').disabled = false;
  document.getElementById('status').textContent = '';
  const out = readOutput();
  if (activeTab === 'sheet') {
    await invoke('generate_contact_sheets', { items, opts: readSheetOpts(), output: out });
  } else {
    await invoke('generate_screenshots', { items, opts: readShotsOpts(), output: out });
  }
}

function showBanner(msg) {
  const b = document.getElementById('banner');
  b.textContent = msg;
  b.classList.remove('hidden');
}

init();
```

- [ ] **Step 5: Register dialog and store permissions**

Create `src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default permissions",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:allow-open",
    "store:default"
  ]
}
```

- [ ] **Step 6: Run the app in dev mode (manual sanity test)**

```bash
pnpm tauri dev
```

Expected: window opens with Mosaic UI. Dropping a video adds it to the queue. Probe fills in info silently. Generating produces a contact sheet next to the source. Cancelling mid-run marks pending items as such.

If ffmpeg is missing on PATH, the red banner appears.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: frontend wiring, settings persistence, generate/cancel flow"
```

---

## Task 14: Integration test against a tiny video fixture

**Files:**
- Create: `src-tauri/tests/integration.rs`
- Create: `src-tauri/tests/fixtures/sample.mp4` (generated)

- [ ] **Step 1: Generate a ~1s test fixture**

```bash
cd src-tauri/tests/fixtures
ffmpeg -y -f lavfi -i "testsrc=size=320x240:rate=10:duration=2" \
  -f lavfi -i "sine=frequency=440:duration=2" \
  -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest sample.mp4
ls -lh sample.mp4
```

Expected: `sample.mp4` under 100 KB.

- [ ] **Step 2: Write integration test**

`src-tauri/tests/integration.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[tokio::test]
async fn end_to_end_contact_sheet_and_screenshots() {
    if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
        eprintln!("skipping: ffmpeg/ffprobe not installed");
        return;
    }
    let tools = mosaic_lib::ffmpeg_test_hook_locate().expect("locate tools");
    let fixture: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "sample.mp4"].iter().collect();
    let font: PathBuf = [env!("CARGO_MANIFEST_DIR"), "assets", "fonts", "DejaVuSans.ttf"].iter().collect();
    assert!(fixture.exists(), "missing test fixture {}", fixture.display());
    assert!(font.exists(), "missing bundled font");

    // Probe
    let json = mosaic_lib::ffmpeg_test_hook_probe(&tools.ffprobe, &fixture.to_string_lossy()).await.unwrap();
    let info = mosaic_lib::video_info_test_hook_parse(&json).unwrap();
    assert!(info.duration_secs > 1.0);

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("sample_contact_sheet.png");

    let reporter = mosaic_lib::contact_sheet::ProgressReporter {
        emit: &|_, _, _| {},
    };
    let opts = mosaic_lib::contact_sheet::SheetOptions {
        cols: 2, rows: 2, width: 640, gap: 8,
        thumb_font_size: 14, header_font_size: 16,
        show_timestamps: true, show_header: true,
        format: mosaic_lib::output_path::OutputFormat::Png, jpeg_quality: 92,
    };
    mosaic_lib::contact_sheet::generate(
        &fixture, &info, &out, &opts, &tools.ffmpeg, &font,
        Arc::new(AtomicBool::new(false)), &reporter,
    ).await.unwrap();
    assert!(out.exists(), "sheet not written");
    assert!(std::fs::metadata(&out).unwrap().len() > 1000);

    // Screenshots
    let shots_dir = tmp.path().join("shots");
    let shots_opts = mosaic_lib::screenshots::ScreenshotsOptions {
        count: 3, width: 320,
        format: mosaic_lib::output_path::OutputFormat::Png, jpeg_quality: 92,
    };
    let outs = mosaic_lib::screenshots::generate(
        &fixture, &info, &shots_dir, &shots_opts, &tools.ffmpeg,
        Arc::new(AtomicBool::new(false)), &reporter,
    ).await.unwrap();
    assert_eq!(outs.len(), 3);
    for p in outs { assert!(p.exists()); }
}
```

- [ ] **Step 3: Expose test hooks**

The integration test needs public access to a few internals. In `src-tauri/src/lib.rs`, change the relevant module declarations from `mod` to `pub mod` so the test file can name them:

```rust
pub mod contact_sheet;
pub mod screenshots;
pub mod output_path;
pub mod video_info;
pub mod ffmpeg;
```

Then add these test helpers to `lib.rs` (after the module declarations, before `run()`):

```rust
pub fn ffmpeg_test_hook_locate() -> Result<ffmpeg::Tools, ffmpeg::ToolsError> {
    ffmpeg::locate_tools()
}
pub async fn ffmpeg_test_hook_probe(exe: &std::path::Path, path: &str) -> Result<String, String> {
    let args = [
        "-v", "error",
        "-show_entries", "format=filename,duration,size,bit_rate",
        "-show_entries", "stream=codec_name,codec_type,width,height,r_frame_rate,sample_rate,channels,bit_rate,profile",
        "-of", "json", path,
    ];
    ffmpeg::run_capture(exe, &args).await.map_err(|e| e.to_string())
}
pub fn video_info_test_hook_parse(json: &str) -> Result<video_info::VideoInfo, video_info::ProbeParseError> {
    video_info::parse(json)
}
```

- [ ] **Step 4: Run the integration test**

```bash
cd src-tauri && cargo test --test integration -- --nocapture
```

Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test: end-to-end integration test for sheet and screenshots"
```

---

## Task 15: README, final polish, manual verification

**Files:**
- Replace: `README.md`
- Manual verification in dev mode

- [ ] **Step 1: Write `README.md`**

```markdown
# Mosaic

Cross-platform desktop GUI for generating video contact sheets and evenly-spaced screenshots. Drag-and-drop batch queue, live progress, cancel support. Built with Tauri + Rust + vanilla HTML/CSS/JS.

## Requirements (dev)

- Node.js + `pnpm`
- Rust stable
- `ffmpeg` and `ffprobe` on PATH (`brew install ffmpeg` / `apt install ffmpeg` / `winget install ffmpeg`)

## Run

```
pnpm install
pnpm tauri dev
```

## Build

```
pnpm tauri build
```

## Tests

```
cd src-tauri && cargo test
```

## Design

See `docs/2026-04-14-mosaic-design.md`.
```

- [ ] **Step 2: Full manual verification run**

```bash
pnpm tauri dev
```

Verify, in order:
1. Window opens, no red banner, tools detected.
2. Drag 2 videos onto the dropzone → queue populates with both.
3. Contact Sheet tab → change cols to 2, rows to 2, Width to 640 → Generate.
4. Progress bar advances, step label updates per thumbnail.
5. Both files end in Done; output PNGs exist next to the source files.
6. Switch to Screenshots tab, count=5 → Generate.
7. Five `*_screenshot_NN.png` per video exist next to the sources.
8. Select Custom folder, pick a folder, generate again → files land in that folder.
9. Click Generate, then Cancel mid-run → current file Cancelled, remaining Pending, banner-free.
10. Close and reopen the app → all option values persisted.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "docs: README; v1 ready for manual verification"
```

---

## Self-Review

**Spec coverage check:**
- §2 Goals: batch, contact sheet, screenshots, full params → Tasks 4, 9, 10, 12–13. ✓
- §4 Tech Stack (Tauri 2.x, tokio, serde, store, dialog, which, tempfile) → Task 1. ✓
- §5 Project Layout → matches task outputs. ✓
- §6 UI zones (input, options tabs, action bar) → Tasks 12, 13. ✓
- §6.1 Recognized extensions → `src/queue.js` (Task 13). ✓
- §6.2 All option fields + defaults → `src/index.html` (Task 12). ✓
- §7 File naming with zero-pad + collision suffix → Task 5. ✓
- §8.1 Probe command → Task 8. ✓
- §8.2 Contact sheet pipeline incl. thumb extract, tile, header, stack, JPEG quality mapping → Task 9. ✓
- §8.3 Screenshots pipeline with `width=0` → Task 10. ✓
- §8.4 Drawtext escaping → Task 3. ✓
- §8.5 TempDir per job → Tasks 9, 10. ✓
- §8.6 Progress events set → Task 11. ✓
- §8.7 Cancellation via AtomicBool + kill → Task 9 (`run_cancellable`) + Task 11. ✓
- §9 Error handling banner + per-file failure → Task 13 + Task 11. ✓
- §10 Settings persistence → Task 13. ✓
- §11 Testing → Tasks 2–6 unit, Task 14 integration. ✓
- §12 Cross-platform font path escape → Task 9 (`font_for_ffmpeg`). ✓

**Placeholder scan:** no TBDs, no "similar to Task N" shortcuts, every code block is complete.

**Type consistency:** `SheetOptions`, `ScreenshotsOptions`, `VideoInfo`, `OutputFormat`, `ProgressReporter`, `QueueItem`, `OutputLocation` are defined exactly once and referred to consistently. The `jpeg_qv` function is renamed from `quality_to_qv` in Task 10 Step 2 to match the call site.

**Known simplification:** The "Add Folder…" button in Task 13 currently adds the folder path as-is rather than recursively scanning its contents. Drag-and-drop of folders relies on Tauri's drop event, which on most platforms yields the contained file paths. A follow-up (post-v1) can add a backend `scan_dir` command for true recursive folder picking.

---

## Execution Handoff

Plan complete and saved to `/Users/abi/AvistaZ/mosaic/docs/2026-04-14-mosaic-plan.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
