# Mosaic

Cross-platform desktop app for generating video contact sheets, screenshots, animated preview reels, and animated contact sheets. Drag-and-drop batch queue, live progress, cancel support. Built with Tauri 2 + Rust + vanilla HTML/CSS/JS.

## Features

- **Contact sheets** — grid of thumbnails with optional header and timestamp overlays (PNG/JPEG)
- **Screenshots** — individual frames at evenly-spaced timestamps (PNG/JPEG)
- **Animated preview reels** — short clips stitched into a single animation (WebP/WebM/GIF)
- **Animated contact sheets** — grid of animated clips (WebP)
- **MediaInfo viewer** — click the info icon in any queue row for full metadata (codec, bitrate, HDR profile, audio tracks, etc.)
- Drag-and-drop batch queue with per-file progress and cancel
- Configurable grid size, quality, fonts, themes, and output suffixes
- Dark/light theme (follows system preference)
- macOS, Windows, and Linux

## Install

Download the latest release from [GitHub Releases](https://github.com/mosaicvideo/mosaic/releases). Available as `.dmg` (macOS), `.exe` installer (Windows), and `.AppImage`/`.deb` (Linux).

**Note:** macOS builds are signed and notarized from v0.1.2 onward. Windows builds are still unsigned — click through the SmartScreen warning (More info → Run anyway).

**Requires [ffmpeg](https://ffmpeg.org/) installed separately** — the app checks for `ffmpeg` and `ffprobe` on your PATH at startup and shows install instructions if they're missing.

**Optional: [MediaInfo CLI](https://mediaarea.net/en/MediaInfo)** — enables the per-file metadata viewer. Install via `brew install mediainfo` / `winget install MediaArea.MediaInfo.CLI` / `apt install mediainfo`. The app works fine without it (info icon shows install instructions instead).

## Requirements (dev)

- Node.js + `pnpm`
- Rust stable
- `ffmpeg` and `ffprobe` on PATH with the `drawtext` filter enabled:
  - macOS: `brew install ffmpeg-full` (the default `brew install ffmpeg` bottle omits libfreetype, which `drawtext` needs)
  - Linux: `apt install ffmpeg` (or distro equivalent; most packages include libfreetype)
  - Windows: `choco install ffmpeg-full` (or `winget install Gyan.FFmpeg` for the BtbN full build)

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
cd src-tauri && cargo test --features test-api
```

The `test-api` feature exposes internal modules (and a couple of test hooks) so
the end-to-end integration test can drive them. Without the feature only the
74 unit tests run; the integration test is gated via `required-features`.

On macOS the integration test requires `ffmpeg-full` for the `drawtext` filter:

```
PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --features test-api
```

## Architecture

Rust backend orchestrates `ffmpeg`/`ffprobe` subprocesses; the frontend talks to it via Tauri's `invoke`/`listen` IPC.

```
Pure logic      layout, drawtext, header, output_path, video_info
                  (no I/O, fully unit-tested)
                           ↓
Orchestration   contact_sheet, screenshots, preview_reel, animated_sheet
                  (build ffmpeg arg vectors)
                           ↓
I/O             ffmpeg.rs  (subprocess spawn, cancellation, batch parallelism)
                           ↓
Commands        commands.rs  (Tauri handlers, per-file job loops, progress events)
```

## Docs

- Design spec: `docs/2026-04-14-mosaic-design.md`
- Implementation plan: `docs/2026-04-14-mosaic-plan.md`
- CLI plan (future): `docs/2026-04-14-mosaic-cli-plan.md`
- Distribution plan (future): `docs/2026-04-14-mosaic-distribution-plan.md`
