# Mosaic

Cross-platform desktop GUI for generating video contact sheets and evenly-spaced screenshots. Drag-and-drop batch queue, live progress, cancel support. Built with Tauri + Rust + vanilla HTML/CSS/JS.

## Requirements (dev)

- Node.js + `pnpm`
- Rust stable
- `ffmpeg` and `ffprobe` on PATH with the `drawtext` filter enabled:
  - macOS: `brew install ffmpeg-full` (the default `brew install ffmpeg` bottle omits libfreetype, which `drawtext` needs)
  - Linux: `apt install ffmpeg` (or distro equivalent; most packages include libfreetype)
  - Windows: `winget install ffmpeg` (BtbN "full" build recommended)

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
