# Mosaic

Cross-platform desktop app for generating video contact sheets, screenshots, animated preview reels, and animated contact sheets. Drag-and-drop batch queue, live progress, cancel support. Built with Tauri 2 + Rust + vanilla HTML/CSS/JS.

Site & downloads: <https://mosaicvideo.github.io/mosaic/>

## Features

- **Contact sheets** — grid of thumbnails with optional header and timestamp overlays (PNG/JPEG)
- **Screenshots** — individual frames at evenly-spaced timestamps (PNG/JPEG)
- **Animated preview reels** — short clips stitched into a single animation (WebP/WebM/GIF)
- **Animated contact sheets** — grid of animated clips (WebP)
- **MediaInfo viewer** — click the info icon in any queue row for full metadata (codec, bitrate, HDR profile, audio tracks, etc.)
- **HDR auto-tonemap** — HDR10, HLG, and Dolby Vision (incl. Profile 5 via IPT-PQ-C2 → BT.709) produce clean SDR thumbnails automatically
- **Auto-update** — from v0.1.2 onward, new releases install with one click after on-device signature verification
- Drag-and-drop batch queue with per-file progress and cancel
- Configurable grid size, quality, fonts, themes, and output suffixes
- Dark/light theme (follows system preference)
- macOS, Windows, and Linux

## Install

Download the latest release from [GitHub Releases](https://github.com/mosaicvideo/mosaic/releases). Available as `.dmg` (macOS universal), `.exe`/`.msi` (Windows x64 + ARM64), and `.AppImage`/`.deb`/`.rpm` (Linux x64).

**Windows note:** builds aren't code-signed — SmartScreen will warn on first install. Click **More info** → **Run anyway**.

**Requires [ffmpeg](https://ffmpeg.org/) and [MediaInfo CLI](https://mediaarea.net/en/MediaInfo) installed separately** — the app checks for `ffmpeg`, `ffprobe`, and `mediainfo` on your PATH at startup and shows install instructions if any are missing.

- macOS: `brew install ffmpeg-full mediainfo`
- Windows: `winget install ffmpeg MediaArea.MediaInfo.CLI`
- Linux: `apt install ffmpeg mediainfo`

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
94 unit tests run; the integration test is gated via `required-features`.

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
