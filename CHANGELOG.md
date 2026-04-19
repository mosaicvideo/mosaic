# Changelog

All notable changes to Mosaic will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **One-liner install scripts for `mosaic-cli`** (`site/install.sh`, `site/install.ps1`) served via GitHub Pages. Detect OS/arch, resolve the latest release via the GitHub API, download + SHA256-verify the matching binary, install to a user-scoped directory (`~/.local/bin` or `%LOCALAPPDATA%\Programs\mosaic-cli`), and print PATH + completions hints. Usage: `curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | sh` on macOS/Linux, `irm https://mosaicvideo.github.io/mosaic/install.ps1 | iex` on Windows.
- **`SHA256SUMS` release artifact** covering every `mosaic-cli-*` binary. Aggregated per-release in CI from per-artifact `.sha256` files uploaded by each matrix runner. Verify manually with `shasum -a 256 -c SHA256SUMS`.
- **`mosaic-cli completions <shell>`** — emit a shell-completion script to stdout. Supports bash, zsh, fish, powershell, and elvish via `clap_complete`.
- **`mosaic-cli manpage`** — emit a roff man page to stdout via `clap_mangen`. Install with `mosaic-cli manpage > ~/.local/share/man/man1/mosaic-cli.1`.
- **Dedicated `cli.html` reference page** on the showcase site covering install, subcommand flag reference (with defaults pulled from `defaults.rs`), config file, shell completions, man page, upgrading, uninstalling, and troubleshooting. Linked from top nav on every site page.

### Fixed

- **Stale `cargo install` snippet in the guide and README.** The build-from-source instruction at `guide.html#cli` and `README.md#command-line-usage` pointed at `cd src-tauri && cargo install --path . --bin mosaic-cli --features cli` — wrong since the CLI moved to the sibling `mosaic-cli/` crate. Both places now point to `cli.html`; guide's CLI section is a short pointer, README has the install one-liners.

## [0.1.4] - 2026-04-18

### Added

- **Command-line interface (`mosaic-cli`)** — every GUI pipeline available as a subcommand (`screenshots`, `sheet`, `reel`, `animated-sheet`, `probe`). Batch-friendly for scripts and headless servers — stdout is paths-only so it pipes cleanly into `xargs`. Ctrl-C cancellation, continue-on-error batch mode. Per-user config at `~/.mosaic-cli.toml` auto-created on first run (override path via `$MOSAIC_CLI_CONFIG`); CLI flags always override config. Signed + notarized macOS universal binary plus Windows x86_64/aarch64 and Linux x86_64 binaries ship alongside the GUI installers.
- **Multi-output history per queue row** — after a shots + sheet + reel run in a single Generate, the row now remembers all three outputs. Clicking reveals the shared parent folder; hover lists every filename. Previously only the last pass's output was kept.

### Fixed

- **Filename-suffix default mismatch** — clearing the screenshots suffix field used to restore `_screenshot_` on the UI while the backend wrote `_screens_`. UI defaults now match the backend constants.
- **Silent probe failures in the queue** — rows whose probe fails now show `⚠ probe failed` (with the error as a tooltip) instead of leaving the metadata cell permanently blank.
- **Unreadable auto-update dialog** — the native update prompt no longer dumps the full release-notes markdown; it shows just the version and install question. Full notes remain on the release page.

### Changed

- **Faster multi-pass Generate** — the backend now reuses the probe the frontend already performs on queue-add instead of re-probing per pass. A shots + sheet + reel run on 10 files drops from 40 redundant ffprobe + mediainfo invocations to zero.

## [0.1.3] - 2026-04-17

### Added

- **Richer contact-sheet headers via MediaInfo** — headers now show container title, video bit depth, HDR format (Dolby Vision / HDR10 / HLG), commercial audio codec name (e.g. DTS-HD MA, Dolby Atmos), channel layout (5.1, 7.1), and language tag. Header is one section per line so nothing clips on narrow grids.
- **MediaInfo CLI promoted to a required prerequisite** — same tools-missing state as ffmpeg if absent. Enables the enrichment above and powers the existing per-file metadata viewer.

### Fixed

- **Anamorphic sources (phone portrait, non-square SAR)** render with the correct aspect across every output type. Previously a 9:16 phone clip encoded in a 1080×1080 frame came out squashed-square in contact sheets, reels, and screenshots. Mosaic now uses displayed square-pixel dimensions (applying SAR and Display Matrix rotation) for both layout and scaling.
- **Rotated sources (±90° / ±270° Display Matrix)** — portrait orientation is preserved end-to-end instead of silently using encoded landscape dims.
- **3D Blu-ray / MVC inputs** — contact sheets no longer come out as zero-dim squares; the usable base-layer stream is now preferred over the dependent-enhancement view that ffprobe lists first.

### Changed

- Probe pipeline now runs ffprobe and MediaInfo concurrently per file, roughly halving drag-and-drop probe latency.

## [0.1.2] - 2026-04-17

### Added

- **Auto-update** — app checks GitHub for new releases on startup and offers one-click install with signed `.app.tar.gz` bundles (ed25519 signatures verified against embedded public key)
- **macOS code signing + notarization** — Developer ID Application signed, stapled, Gatekeeper-trusted on first open (no right-click → Open dance)
- **Windows ARM64 builds** — native `aarch64-pc-windows-msvc` artifacts alongside x64
- **Version in title bar** — window title now shows `Mosaic 0.1.2` etc., set at startup from `CARGO_PKG_VERSION`

### Fixed

- **MediaInfo lookup on macOS** — `locate_mediainfo` now falls back to `/opt/homebrew/bin` and `/usr/local/bin` when `PATH` doesn't include Homebrew (as happens for apps launched from Finder)

### Notes

v0.1.1 users: **this release requires a manual download** since v0.1.1 predates the updater plugin. From v0.1.2 onward, updates install automatically.

## [0.1.1] - 2026-04-16

### Added

- Auto-tonemap HDR10, HLG, and Dolby Vision thumbnails to SDR across all output types
- MediaInfo modal — click the file icon in any queue row to view raw `mediainfo` output with copy-to-clipboard
- Expanded video format support from 12 to 44 recognized container types

### Fixed

- Clip extraction (preview reels, animated sheets) now uses simple seeking to avoid decode failures on transport stream containers (.ts, .m2ts)
- Explicit zscale input colorspace prevents "no path between colorspaces" errors during HDR tonemapping
- DV Profile 5 streams with unknown transfer function are left untouched instead of producing garbage output

### Changed

- Upgraded thiserror 1→2 and which 6→8
- Reveal-on-click in queue narrowed from whole row to filename only

## [0.1.0] - 2026-04-16

### Added

- Contact sheet generation with configurable grid, timestamps, header, and themes (PNG/JPEG)
- Individual screenshot extraction at evenly-spaced timestamps (PNG/JPEG)
- Animated preview reels stitched from short video clips (WebP/WebM/GIF)
- Animated contact sheets with grid of animated clips (WebP)
- Drag-and-drop batch queue with per-file progress and cancel support
- Dark/light theme following system preference
- Configurable output quality, fonts, suffixes, and output location
- Frame-accurate seeking with parallel extraction
- Folder scanning with recursive depth support
- ffmpeg/ffprobe tool detection with user-friendly error state
- macOS, Windows, and Linux support (requires ffmpeg installed separately)

[unreleased]: https://github.com/mosaicvideo/mosaic/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/mosaicvideo/mosaic/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/mosaicvideo/mosaic/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/mosaicvideo/mosaic/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/mosaicvideo/mosaic/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/mosaicvideo/mosaic/releases/tag/v0.1.0
