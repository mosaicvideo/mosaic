# Mosaic: video contact sheets, screenshots and animated previews

[![Release](https://img.shields.io/github/v/release/mosaicvideo/mosaic?color=00c96a)](https://github.com/mosaicvideo/mosaic/releases/latest)
[![CI](https://github.com/mosaicvideo/mosaic/actions/workflows/ci.yml/badge.svg)](https://github.com/mosaicvideo/mosaic/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey)

Mosaic makes contact sheets, screenshots and animated previews from your videos, a whole folder at a time. It runs `ffmpeg` on your own computer, so your videos are never uploaded anywhere.

It's a desktop app for macOS, Windows and Linux, plus `mosaic-cli` for scripts and servers. Free and open source.

**[Download](https://github.com/mosaicvideo/mosaic/releases/latest)** · **[Website](https://mosaicvideo.github.io/)** · **[Guide & FAQ](https://mosaicvideo.github.io/guide.html)** · **[CLI reference](https://mosaicvideo.github.io/cli.html)** · **[Changelog](CHANGELOG.md)**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://mosaicvideo.github.io/assets/screenshots/app-dark.webp">
  <img src="https://mosaicvideo.github.io/assets/screenshots/app-light.webp" width="800" alt="The Mosaic window making contact sheets and screenshots for a queue of seven videos">
</picture>

## What it makes

Pick any mix of the four outputs, and Mosaic makes each one for every video in the queue.

| Output | What you get | Formats |
|---|---|---|
| **Contact sheet** | A grid of timestamped frames, with the file's details in a header on top | PNG, JPEG |
| **Screenshots** | Full-size frames taken at even intervals through the video | PNG, JPEG |
| **Animated preview** | Short clips joined into one looping preview | WebP, WebM, GIF |
| **Animated contact sheet** | A contact sheet where every frame is a moving clip | WebP |

<p>
  <img src="https://mosaicvideo.github.io/assets/screenshots/output-contact-sheet.webp" height="320" alt="A contact sheet: a grid of timestamped frames under a header of file details">
  <img src="https://mosaicvideo.github.io/assets/screenshots/output-animated-sheet.webp" height="320" alt="An animated contact sheet: a three-by-three grid of looping clips, each stamped with its time">
</p>

<sub>Big Buck Bunny © Blender Foundation, CC BY 3.0</sub>

A contact sheet shows at a glance what a file contains and whether the encode holds up, without scrubbing through it in a player. Archivists, editors, encoders and anyone with a large video library use them as a visual index.

## Features

- **Batch queue.** Drop files or whole folders. Each file shows its progress, and Cancel stops the work right away.
- **HDR that looks right.** HDR10, HLG and Dolby Vision (Profile 5 included) come out with natural colors, not washed-out grey.
- **Accurate headers.** MediaInfo supplies the codec names you'd expect (*Dolby Digital Plus*, not `eac3`), plus HDR format, bit depth, channel layout and language. The header leads with the filename; the title embedded in the file is an option.
- **MediaInfo viewer** for the full metadata of any file in the queue.
- **43 container formats**, anything `ffmpeg` can read: MP4, MKV, MOV, AVI, WebM, MXF, TS, M2TS and more.
- **Settings for everything**: grid size, quality, font sizes, sheet theme, output names and folders.
- **Stays on your computer.** No account, no analytics, no telemetry. The only network request is the update check against GitHub.
- **Auto-update.** New releases install in one click after their signature is checked.
- **Light and dark themes**, following your system.

## Install

Download the latest build from [Releases](https://github.com/mosaicvideo/mosaic/releases/latest):

| Platform | Files |
|---|---|
| macOS | `.dmg` (universal: Apple Silicon and Intel) |
| Windows | `.exe` installer or `.msi` (x64 and Arm64) |
| Linux | `.AppImage`, `.deb`, `.rpm` (x64) |

The macOS build is signed and notarized. **The Windows builds aren't code-signed**, so SmartScreen warns the first time: click **More info**, then **Run anyway**. Every build comes from GitHub Actions running on a tagged commit.

### ffmpeg and MediaInfo

Mosaic needs [**ffmpeg**](https://ffmpeg.org/) (with `ffprobe`) and the [**MediaInfo**](https://mediaarea.net/en/MediaInfo) command-line tool. If either is missing when the app starts, it shows the command for your system, naming only what's missing. On macOS and Windows, the **Install** button runs it for you; on Linux, copy it into a terminal.

To install them yourself:

```sh
# macOS (Homebrew)
brew install ffmpeg-full media-info

# Windows (winget)
winget install --exact --id Gyan.FFmpeg
winget install --exact --id MediaArea.MediaInfo

# Debian, Ubuntu
sudo apt install ffmpeg mediainfo
```

On macOS, use `ffmpeg-full`, not `ffmpeg`. Homebrew's default `ffmpeg` lacks the text overlays that draw timestamps and headers, WebP encoding, and HDR tonemapping. Mosaic picks `ffmpeg-full` when both are installed, and warns if your ffmpeg lacks something an output needs.

## Command line

`mosaic-cli` makes the same four outputs without the app, for batch jobs, servers and CI.

```sh
# macOS and Linux
curl -LsSf https://mosaicvideo.github.io/install.sh | sh

# Windows (PowerShell)
irm https://mosaicvideo.github.io/install.ps1 | iex
```

The installer finds the latest release, checks its SHA-256 checksum, and installs into your user folder. You can also download a `mosaic-cli-*` binary from the [latest release](https://github.com/mosaicvideo/mosaic/releases/latest) and check it against `SHA256SUMS`.

```console
$ mosaic-cli sheet ~/films/arrival.mkv --cols 5 --rows 4
/Users/you/films/arrival_sheet.png
1 done · 0 failed · 0 cancelled

$ mosaic-cli screenshots ~/films/arrival.mkv -o ./out --count 4
./out/arrival_screens_01.png
./out/arrival_screens_02.png
./out/arrival_screens_03.png
./out/arrival_screens_04.png
1 done · 0 failed · 0 cancelled
```

Commands: `sheet`, `screenshots`, `reel`, `animated-sheet`, `probe`, `completions`, `manpage`.

Output paths go to stdout and the summary to stderr, so you can pipe the paths straight into another command:

```sh
mosaic-cli sheet ~/films/ -o ./sheets | xargs -n1 basename
```

Defaults live in `~/.mosaic-cli.toml`, created on first run. Every flag, the config file, shell completions and troubleshooting are on the **[CLI page](https://mosaicvideo.github.io/cli.html)**.

## Why another contact sheet tool?

Most tools for this are command-line only, run on one platform, or are no longer maintained. [**vcsi**](https://github.com/amietn/vcsi) is the best-known one and is still maintained: a Python CLI that also uses ffmpeg. If you work in a terminal and want still contact sheets, it's a great fit.

Mosaic adds what was missing:

- A maintained desktop app for all three platforms, with a batch queue and auto-update, alongside a CLI
- Animated previews and animated contact sheets, not just still grids
- HDR tonemapping built in, Dolby Vision Profile 5 included
- MediaInfo for accurate codec names in the header

## Development

You need Node.js with `pnpm`, stable Rust, and ffmpeg and MediaInfo as above.

```sh
pnpm install
pnpm tauri dev                    # run the app
pnpm tauri build                  # build installers
pnpm build:web                    # quick frontend-only build
pnpm dev:cli -- sheet movie.mkv   # run the CLI from source
```

### Tests

Run `cargo` from `src-tauri/`. The `test-api` feature adds the integration tests, which run real ffmpeg and need `drawtext` and `libwebp`. On macOS that means `ffmpeg-full`:

```sh
cd src-tauri
PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --features test-api
cd ../mosaic-cli
PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test
```

Both clippy configurations must be clean:

```sh
cargo clippy --all-targets --features test-api,cli -- -D warnings
cargo clippy -- -D warnings
```

### Architecture

A Tauri 2 app. The Rust backend runs `ffmpeg`, `ffprobe` and `mediainfo`; the plain HTML/CSS/JS frontend talks to it over Tauri's `invoke` and `listen`.

```
Pure logic      layout, drawtext, header, output_path, video_info, install_hint
                  (no I/O, unit-tested)
                           ↓
Orchestration   contact_sheet, screenshots, preview_reel, animated_sheet
                  (build ffmpeg arguments)
                           ↓
I/O             ffmpeg.rs  (find tools, run processes, cancellation)
                           ↓
Commands        commands.rs  (Tauri handlers, per-file job loops, progress events)
```

`mosaic-cli` is a separate Cargo package that depends on this crate and reuses the same pipeline modules.

The website has its own repo: [mosaicvideo/mosaicvideo.github.io](https://github.com/mosaicvideo/mosaicvideo.github.io).

## License

[MIT](LICENSE)
