# Changelog

All notable changes to Mosaic will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[unreleased]: https://github.com/bhutanio/mosaic/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/bhutanio/mosaic/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/bhutanio/mosaic/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/bhutanio/mosaic/releases/tag/v0.1.0
