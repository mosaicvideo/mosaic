# Mosaic — Design Spec

**Date:** 2026-04-14
**Status:** Draft (approved for implementation planning)

## 1. Purpose

Mosaic is a cross-platform desktop GUI application for generating video contact sheets and evenly-spaced screenshots from video files. It is a successor to `bender/scripts/contact_sheet.sh` — same output fidelity, but with a drag-and-drop UI, batch processing, and no reliance on Linux-specific font paths or shell utilities (`jq`, `bc`).

## 2. Goals

- One-window GUI to drop videos and produce contact sheets or screenshot sets.
- Batch processing with a live queue, progress, and cancel.
- Two output modes:
  - **Contact sheet** — tiled grid of thumbnails with an info header (matches current script output).
  - **Screenshots** — N evenly-spaced full-size frames per video (no header, no tiling).
- Full control over every relevant parameter, with sensible defaults.
- Runs on macOS, Windows, Linux.

## 3. Non-Goals (v1)

- Bundled ffmpeg binaries. v1 uses system `ffmpeg`/`ffprobe` on `PATH`. Bundling is deferred.
- Video format conversion, trimming, or editing.
- Cloud sync, account system, telemetry.
- Parallel processing across multiple videos (ffmpeg already saturates cores per file).
- Code-signing / notarization infrastructure (deferred until bundling).

## 4. Tech Stack

- **Shell:** Tauri 2.x
- **Backend:** Rust (edition 2021)
  - `tokio` — async runtime for subprocess I/O
  - `serde` / `serde_json` — ffprobe output parsing
  - `tauri` — IPC commands and events
  - `tauri-plugin-dialog` — native file pickers
  - `tauri-plugin-store` — settings persistence
- **Frontend:** Vanilla HTML / CSS / JS. No framework. One `index.html`, hand-written modules.
- **External deps (runtime):** `ffmpeg` and `ffprobe` on `PATH`.
- **Bundled assets:** `DejaVuSans.ttf` — shipped inside the app so `drawtext` works everywhere.

## 5. Architecture

```
┌──────────────────────────────────────────────────────────┐
│                      Tauri Window                        │
│  ┌─────────────────────────────────────────────────────┐ │
│  │               Frontend (HTML/CSS/JS)                │ │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │ │
│  │  │ Dropzone │  │  Queue   │  │   Options form   │   │ │
│  │  └──────────┘  └──────────┘  └──────────────────┘   │ │
│  │          Progress bar + Generate + Cancel           │ │
│  └──────────────▲─────────────────────────┬────────────┘ │
│                 │   events (progress,     │              │
│                 │   file_done, error)     │ invoke()     │
│                 │                         ▼              │
│  ┌─────────────────────────────────────────────────────┐ │
│  │                  Rust Backend                       │ │
│  │  commands: probe, generate_contact_sheet,           │ │
│  │            generate_screenshots, cancel             │ │
│  │  ffmpeg.rs → spawns ffmpeg / ffprobe                │ │
│  └──────────────┬──────────────────────────────────────┘ │
└─────────────────┼────────────────────────────────────────┘
                  ▼
         ffmpeg / ffprobe (system)
```

### Project Layout

```
mosaic/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/default.json
│   ├── assets/fonts/DejaVuSans.ttf
│   └── src/
│       ├── main.rs              # entry, plugin init, command registration
│       ├── lib.rs               # module wiring, tauri::Builder setup
│       ├── commands.rs          # Tauri command handlers
│       ├── ffmpeg.rs            # locate ffmpeg/ffprobe, spawn helpers
│       ├── video_info.rs        # VideoInfo struct + ffprobe JSON parsing
│       ├── contact_sheet.rs     # contact-sheet pipeline + SheetOptions
│       ├── screenshots.rs       # screenshots pipeline + ScreenshotsOptions
│       ├── drawtext.rs          # drawtext escaping + HMS formatter
│       ├── layout.rs            # grid/thumb dims + sample timestamps
│       ├── header.rs            # info header line builder
│       ├── output_path.rs       # filename generation + OutputFormat
│       └── jobs.rs              # JobState (running flag + cancel AtomicBool)
├── src/
│   ├── index.html
│   ├── main.js                  # app bootstrap, IPC wiring, events
│   ├── style.css
│   ├── dropzone.js
│   ├── queue.js
│   └── options.js
├── package.json
├── docs/
│   └── 2026-04-14-mosaic-design.md
└── README.md
```

## 6. UI Specification

Single resizable window, minimum 900×640. Three vertical zones:

### 6.1 Input zone (top, ~30% of height)

- **Dropzone.** Dashed-border area labeled "Drop videos here". Accepts OS drops of files; each dropped path is filtered by extension before being added to the queue.
- **Add Files…** button — native multi-select file picker.
- **Add Folder…** button — native folder picker. v1 adds the folder path as-is (non-recursive). Recursive folder scanning is deferred to post-v1.
- **Queue list.** One row per video:
  - Filename (truncated middle if long).
  - Duration, resolution once probed.
  - Status badge: `Pending` / `Probing` / `Running` / `Done` / `Failed` / `Cancelled`.
  - Error details (expandable) on Failed.
  - Per-row "×" button to remove (disabled while that row is Running).
- **Clear queue** button (right-aligned).

Recognised video extensions: `mp4 mkv mov avi webm wmv flv m4v mpg mpeg ts m2ts`.

### 6.2 Options zone (middle)

Two tabs: **Contact Sheet** (default) and **Screenshots**.

#### Contact Sheet tab

| Field | Type | Default | Notes |
|---|---|---|---|
| Columns | int, 1–10 | 3 | |
| Rows | int, 1–20 | 7 | |
| Total width (px) | int, 320–4096 | 1920 | final image width |
| Gap (px) | int, 0–50 | 10 | |
| Thumb font size | int, 8–72 | 18 | timestamp overlay size |
| Header font size | int, 8–72 | 20 | info header size |
| Show timestamps | checkbox | on | overlay HH:MM:SS on each thumb |
| Show info header | checkbox | on | render the 2-line header |
| Output format | select | PNG | PNG / JPEG |
| JPEG quality | int, 50–100 | 92 | only when format = JPEG |

#### Screenshots tab

| Field | Type | Default | Notes |
|---|---|---|---|
| Count | int, 1–100 | 10 | N evenly-spaced frames |
| Width (px) | int, 320–4096 | 1920 | 0 = keep source width |
| Output format | select | PNG | PNG / JPEG |
| JPEG quality | int, 50–100 | 92 | only when format = JPEG |

#### Output location (shared, below tabs)

- Radio: **Next to source** (default) / **Custom folder**.
- Folder picker button appears when "Custom folder" is selected.

All option values persist between app launches via `tauri-plugin-store` (last-used values pre-filled).

### 6.3 Action bar (bottom)

- **Generate** button. Label follows active tab: "Generate Contact Sheets" / "Generate Screenshots". Disabled when queue is empty or already running.
- **Cancel** button — enabled only while running.
- **Overall progress bar** — weighted by file count (not per-step).
- **Current status line** — e.g. `Processing file 2/5: movie.mkv (thumb 14/21)`.

## 7. File Naming

Given source `path/to/movie.mkv` and format `png`:

- Contact sheet → `path/to/movie_contact_sheet.png` (or `_custom_folder/movie_contact_sheet.png`).
- Screenshots → `path/to/movie_screenshot_01.png`, `_02.png`, … zero-padded to the width required by `count`.

If the destination file exists, append ` (1)`, ` (2)`, etc.

## 8. Processing Pipeline

### 8.1 Probe

```
ffprobe -v error \
  -show_entries format=filename,duration,size,bit_rate \
  -show_entries stream=codec_name,codec_type,width,height,r_frame_rate,sample_rate,channels,bit_rate,profile \
  -of json <input>
```

Parsed into:

```rust
struct VideoInfo {
    filename: String,
    duration_secs: f64,
    size_bytes: Option<u64>,      // ffprobe may omit
    bit_rate: Option<u64>,
    video: VideoStream,           // required; error if missing
    audio: Option<AudioStream>,   // codec, profile, sample_rate, channels, bit_rate
}
```

Failure modes:
- Non-zero ffprobe exit → error surfaced verbatim from `ffprobe` stderr.
- Missing or zero duration → `ProbeParseError::MissingDuration`.
- Missing video stream → `ProbeParseError::NoVideo`.

### 8.2 Contact Sheet

Let `cols`, `rows`, `width`, `gap`, `total = cols*rows`, `thumb_w = ((width - gap*(cols+1)) / cols)` (forced even).

For `i` in `1..=total`:
- `ts = i * duration / (total + 1)`
- `ffmpeg -hide_banner -loglevel error -y -ss <ts> -i <input> -vframes 1 -vf "scale=<thumb_w>:-2[,drawtext=...]" <tmp>/thumb_NN.png`

`drawtext` (when timestamps on):

```
drawtext=text='HH\:MM\:SS':fontfile=<bundled font>:fontsize=<thumb_font>:fontcolor=white:shadowcolor=black:shadowx=1:shadowy=1:x=5:y=h-th-5
```

Tile:

```
ffmpeg -framerate 1 -start_number 1 -i <tmp>/thumb_%0Nd.png \
  -vf "tile=<cols>x<rows>:margin=<gap>:padding=<gap>:color=0x000000" \
  -frames:v 1 <tmp>/grid.png
```

Header (when enabled):
- Two text lines: filename on line 1, details on line 2.
- Details: `Size: G GiB, Duration: HH:MM:SS, Bitrate: M Mb/s  |  Video: codec (profile) | WxH | BR kb/s | fps  |  Audio: codec (profile) | Hz | channels | kb/s`.
- Missing fields are skipped (no trailing separators).
- Header height = `2 * line_h + 2 * gap`, where `line_h = round(header_font * 1.3)`.

```
ffmpeg -f lavfi -i color=c=0x000000:s=<grid_w>x<header_h>:d=1 \
  -vf "drawtext=...:y=<y1>,drawtext=...:y=<y2>" \
  -frames:v 1 <tmp>/header.png
```

Stack (when header enabled):

```
ffmpeg -i <tmp>/header.png -i <tmp>/grid.png \
  -filter_complex vstack -frames:v 1 <output>
```

When header disabled, the grid is the final output (renamed).

JPEG output is achieved by setting the final file extension and adding `-q:v <quality>` (FFmpeg maps to libmjpeg automatically).

### 8.3 Screenshots

For `i` in `1..=count`:
- `ts = i * duration / (count + 1)`
- `ffmpeg -hide_banner -loglevel error -y -ss <ts> -i <input> -vframes 1 -vf "scale=<width>:-2" <output_dir>/<stem>_screenshot_NN.<ext>`

`width=0` → omit the `scale` filter.

### 8.4 Drawtext escaping

Any user-derived text that lands in a `drawtext=text='...'` filter must escape: `\`, `:`, `'`, `%`. Single rust function `escape_drawtext(&str) -> String` covers both the timestamp overlay and header lines.

### 8.5 Temp directories

One `tempfile::TempDir` per file job, dropped when the job ends (success, failure, or cancel). No shared temp state across files.

### 8.6 Progress events

Rust emits Tauri events:

| Event | Payload |
|---|---|
| `job:file-start` | `{ fileId, index, total }` |
| `job:step` | `{ fileId, step, totalSteps, label }` (label: `"Extracting thumb 12/21"`) |
| `job:file-done` | `{ fileId, outputPath }` |
| `job:file-failed` | `{ fileId, error }` |
| `job:finished` | `{ completed, failed, cancelled }` |

Frontend listens and updates queue rows + overall progress.

### 8.7 Cancellation

- Global `AtomicBool` `cancelled` owned by `JobState`.
- Before each ffmpeg spawn, check flag; exit early with `RunError::Killed` if set.
- Each spawned ffmpeg process uses `Child::kill_on_drop(true)`. The runner races `child.wait()` against a short-interval poll of the flag in a `tokio::select!`; if the flag flips first, `child.kill().await` is issued and the polling loop returns `RunError::Killed`.
- Any files not yet started stay as `Pending`; the in-progress file reports `Cancelled` and the queue loop breaks.

## 9. Error Handling

| Condition | User-visible behaviour |
|---|---|
| ffmpeg/ffprobe missing at startup | Red banner with per-OS install instructions; Generate disabled. |
| ffprobe fails for a file | Row marked Failed with expandable stderr. Queue continues. |
| ffmpeg exits non-zero | Same as above. Temp dir cleaned up. |
| Output path not writable | Row marked Failed with the IO error. Queue continues. |
| All files failed | `job:finished` with `{completed: 0, failed: N}`; status line summarises the outcome. |
| User cancels mid-run | Current file → Cancelled; remaining → Pending. |

Errors from each ffmpeg step bubble up through `RunError` and are surfaced per-row as `job:file-failed`. Unexpected panics are not explicitly caught in v1 — if one occurs the Tauri command returns an error string, which the frontend surfaces via the banner.

## 10. Settings Persistence

`tauri-plugin-store` writes to the OS app-data directory:

- macOS: `~/Library/Application Support/com.mosaic.app/settings.json`
- Windows: `%APPDATA%\com.mosaic.app\settings.json`
- Linux: `~/.config/com.mosaic.app/settings.json`

Persisted: all fields on both Options tabs, output location choice, last-used custom folder, last active tab, window size.

## 11. Testing

### Rust

- `drawtext::escape_drawtext` — unit tests for `'`, `:`, `\`, `%`.
- `video_info::parse` — fixtures: typical mp4, mkv with multiple audio tracks, file with no audio, corrupt JSON.
- `contact_sheet::layout` — grid / thumb dims for several cols/rows/width/gap combinations.
- `contact_sheet::interval` — interval math for known duration + total.
- `screenshots::timestamps` — N timestamps are strictly increasing and inside `(0, duration)`.
- Integration: a small `tests/fixtures/sample.mp4` (< 1 MB); generate a 2×2 sheet and 3 screenshots; assert files exist with expected dimensions.

### Frontend

Manual for v1.

## 12. Cross-Platform Notes

- **Font path:** use `app_handle.path().resource_dir()` to resolve the bundled `DejaVuSans.ttf`. Pass an absolute path to `fontfile=` — escape `:` on Windows (`C\:/...`).
- **ffmpeg locator:** `which`-style PATH search on all platforms. On macOS, also probe `/opt/homebrew/bin` and `/usr/local/bin` in case the app was launched without a login shell PATH.
- **Path separators:** use `Path` / `PathBuf` everywhere in Rust. Serialize to strings only when handing to ffmpeg.
- **Drag-drop:** Tauri's `onDragDropEvent` on the main window.

## 13. Out of Scope / Deferred

- Embedded ffmpeg binaries + per-platform installers.
- Code signing / notarization.
- Advanced presets UI ("Small / Default / Large" buttons).
- GPU-accelerated decoding flags.
- Custom fonts beyond the bundled DejaVu Sans.
- Localisation.

## 14. Open Questions

None blocking v1. Items deferred to post-v1 decisions:
- Whether to add a "Preset" selector once users accumulate repeat settings.
- Whether to add an "Open output" action on completed queue rows.

## 15. Appendix — Default Header Layout

Given `width = 1920`, `cols = 3`, `gap = 10`, `header_font_size = 20`:
- `thumb_w = (1920 - 10 * 4) / 3 = 626` (even).
- `grid_w = 4 * 10 + 3 * 626 = 1918`.
- `line_h = round(20 * 1.3) = 26`.
- `header_h = 2 * 26 + 2 * 10 = 72`.
- Final image = `1918 × (72 + 7 * 626 + 8 * 10) = 1918 × 4534` for a 3×7 sheet. Matches the existing script's dimensions.
