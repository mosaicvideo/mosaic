# Mosaic — video contact sheet & thumbnail generator

[![Release](https://img.shields.io/github/v/release/mosaicvideo/mosaic?color=00c96a)](https://github.com/mosaicvideo/mosaic/releases/latest)
[![CI](https://github.com/mosaicvideo/mosaic/actions/workflows/ci.yml/badge.svg)](https://github.com/mosaicvideo/mosaic/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey)

Mosaic turns any video into a **contact sheet**, a set of **screenshots**, or an **animated preview** — in batches, on your own machine. Drop in a folder, pick what to produce, walk away. Every pipeline drives your local `ffmpeg`, so nothing is ever uploaded anywhere.

Cross-platform desktop app (macOS, Windows, Linux) plus a `mosaic-cli` binary for scripts, CI and headless servers. Free and open source.

**[Download](https://github.com/mosaicvideo/mosaic/releases/latest)** · **[Website](https://mosaicvideo.github.io/)** · **[Guide & FAQ](https://mosaicvideo.github.io/guide.html)** · **[CLI reference](https://mosaicvideo.github.io/cli.html)**

![Mosaic app window showing a drag-and-drop batch queue with video files processing](https://mosaicvideo.github.io/assets/screenshots/hero.webp)

## What is a contact sheet?

A single image holding a grid of frames sampled evenly across a video, each stamped with its timestamp. One glance tells you what a file contains, how it was shot, and whether the encode holds up — without scrubbing through a player. Archivists, editors, encoders and anyone maintaining a large media library use them as a visual index.

## Four output types

| Output | What you get | Formats |
|---|---|---|
| **Contact sheets** | Grid of timestamped frames at configurable rows × columns, with an optional metadata header | PNG, JPEG |
| **Screenshots** | Individual full-resolution frames at evenly-spaced timestamps, numbered and named after the source | PNG, JPEG |
| **Animated preview reels** | Short clips stitched end to end into one reel — motion preview without opening a player | WebP, WebM, GIF |
| **Animated contact sheets** | A grid where every cell loops a short motion clip | WebP |

## Features

- **Drag-and-drop batch queue** — drop files or whole folders, with per-file progress and a cancel button that genuinely stops work mid-encode
- **HDR auto-tonemap** — HDR10, HLG and Dolby Vision (including Profile 5, via IPT-PQ-C2 → BT.709) produce clean SDR thumbnails with no manual tonemap step
- **MediaInfo enrichment** — headers carry real commercial codec names (*Dolby Digital Plus*, not `eac3`), plus HDR format, bit depth, channel layout and language
- **MediaInfo viewer** — full metadata for any queued file, in-app
- **45 container formats** — anything `ffmpeg` can decode: MP4, MKV, MOV, AVI, WebM, MXF, R3D, TS, M2TS and more
- **Local only** — videos never leave the machine. Zero analytics, zero telemetry, no account. The only network call is an update check against GitHub's public API
- **Auto-update** — from v0.1.2 onward, new releases install in one click after on-device signature verification
- **Configurable** — grid size, quality, fonts, themes, output suffixes and destinations
- **Dark/light theme**, following system preference

## Install

Download the latest build from [Releases](https://github.com/mosaicvideo/mosaic/releases/latest):

| Platform | Files |
|---|---|
| macOS | `.dmg` (universal — Apple Silicon and Intel) |
| Windows | `.exe` installer or `.msi` (x64 and ARM64) |
| Linux | `.AppImage`, `.deb`, `.rpm` (x64) |

macOS builds are Developer-ID signed and notarized. **Windows builds are not code-signed**, so SmartScreen warns on first install — click **More info** → **Run anyway**. Every build is produced by GitHub Actions from a tagged commit, so you can verify the chain.

### Prerequisites

Mosaic requires [**ffmpeg**](https://ffmpeg.org/) (with `ffprobe`) and [**MediaInfo CLI**](https://mediaarea.net/en/MediaInfo) on your `PATH`. The app checks at startup and shows the exact install command if either is missing.

```sh
# macOS
brew install ffmpeg-full mediainfo

# Windows
winget install Gyan.FFmpeg MediaArea.MediaInfo.CLI

# Debian / Ubuntu
apt install ffmpeg mediainfo
```

> On macOS use `ffmpeg-full`, not `ffmpeg`. The default Homebrew bottle omits **libfreetype** (needed for text overlays) and **libzimg** (needed for HDR tonemapping). Mosaic prefers `ffmpeg-full` automatically when both are installed.

## Command line

`mosaic-cli` runs the same four pipelines with no GUI — for batch jobs, headless servers and CI.

```sh
# macOS / Linux
curl -LsSf https://mosaicvideo.github.io/install.sh | sh

# Windows (PowerShell)
irm https://mosaicvideo.github.io/install.ps1 | iex
```

The installer resolves the latest release, verifies its SHA-256 checksum, and installs to a user-scoped directory. Or grab a `mosaic-cli-*` binary from the [latest release](https://github.com/mosaicvideo/mosaic/releases/latest) — every release ships `SHA256SUMS`.

```console
$ mosaic-cli sheet ~/films/arrival.mkv --cols 5 --rows 4
/Users/abi/films/arrival_sheet.png
1 done · 0 failed · 0 cancelled

$ mosaic-cli screenshots ~/films/arrival.mkv -o ./out --count 4
/Users/abi/out/arrival_screens_01.png
/Users/abi/out/arrival_screens_02.png
/Users/abi/out/arrival_screens_03.png
/Users/abi/out/arrival_screens_04.png
1 done · 0 failed · 0 cancelled
```

Subcommands: `sheet`, `screenshots`, `reel`, `animated-sheet`, `probe`, `completions`, `manpage`.

Paths go to **stdout** and the run summary to **stderr**, so output pipes straight into `xargs` without filtering:

```sh
mosaic-cli sheet ~/films/ -o ./sheets | xargs -n1 basename
```

Full flag reference, config file format, shell completions and troubleshooting: **[CLI page](https://mosaicvideo.github.io/cli.html)**.

## Why another contact sheet tool?

Tools for this have existed for years, but nearly all of them are command-line only, single-platform, or unmaintained.

[**vcsi**](https://github.com/amietn/vcsi) is the best-known option and is actively maintained — a Python CLI that also drives ffmpeg. If you live in a terminal and want static contact sheets, it is an excellent fit and Mosaic does not try to replace it.

Mosaic exists because there was no maintained **cross-platform GUI** for this, and because static grids are not always what you want:

- A real desktop app with a batch queue, signed and notarized on macOS, that auto-updates — alongside a CLI, not instead of one
- **Animated** outputs — preview reels and animated contact sheets, not just static grids
- **HDR tonemapping** built in, including Dolby Vision Profile 5, so sheets are not washed-out grey
- **MediaInfo** enrichment for accurate codec naming in headers

## Development

Requires Node.js with `pnpm`, Rust stable, and the same `ffmpeg` / `ffprobe` / `mediainfo` prerequisites as above.

```sh
pnpm install
pnpm tauri dev        # run the desktop app
pnpm tauri build      # production bundle
pnpm build:web        # fast frontend-only build
pnpm dev:cli -- sheet movie.mkv   # run the CLI from source
```

### Tests

```sh
cd src-tauri && cargo test --features test-api
```

The `test-api` feature exposes internal modules so the end-to-end integration test can drive them; without it only the unit tests run, and the integration test is skipped via `required-features`. On macOS the integration test needs `ffmpeg-full` for the `drawtext` filter:

```sh
PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --features test-api
```

Lint gates — both configurations must be clean:

```sh
cargo clippy --all-targets --features test-api,cli -- -D warnings
cargo clippy -- -D warnings
```

### Architecture

Tauri 2 app. A Rust backend orchestrates `ffmpeg` / `ffprobe` subprocesses; a vanilla HTML/CSS/JS frontend talks to it over Tauri's `invoke` / `listen` IPC.

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

`mosaic-cli` is a sibling Cargo package that depends on this crate and reuses the same pipeline modules.

The website lives in a separate repo: [mosaicvideo/mosaicvideo.github.io](https://github.com/mosaicvideo/mosaicvideo.github.io).

## License

[MIT](LICENSE)
