# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

Use `pnpm tauri dev` for development and `pnpm tauri build` for production bundles. Rust tests live under `src-tauri/` — plain `cargo test` runs unit tests; add `--features test-api` for integration tests. Run `cargo` from `src-tauri/` (the `Cargo.toml` isn't at repo root). For frontend-only changes, `pnpm build:web` is a fast Vite build that catches syntax/import errors without launching Tauri.

Vite (port 5173) serves `src/` and is spawned automatically by `tauri dev` via `beforeDevCommand`. Tauri's file watcher hot-rebuilds the Rust crate on `src-tauri/**` changes; Vite HMR handles `src/**`.

The integration test requires ffmpeg with the `drawtext` filter. On macOS the default Homebrew `ffmpeg` bottle is built **without** libfreetype. Install `brew install ffmpeg-full` and run tests with `PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --features test-api`. The app's `locate_tools()` already checks `ffmpeg-full` first on macOS.

## Architecture

Tauri 2 app. Rust backend orchestrates `ffmpeg`/`ffprobe` subprocesses; vanilla HTML/CSS/JS frontend talks to it via `invoke`/`listen`.

**Pipeline separation — the core of the codebase.** Pure logic (`drawtext.rs`, `layout.rs`, `header.rs`, `output_path.rs`, `video_info.rs`) is fully unit-testable with no subprocess dependency. Orchestration modules (`contact_sheet.rs`, `screenshots.rs`, `preview_reel.rs`) take pure-logic outputs and build ffmpeg arg vectors, delegating subprocess I/O to `ffmpeg.rs`. Commands (`commands.rs`) wrap everything with Tauri handlers and per-file job loops. Keep this layering intact when adding features: put new math/parsing in pure modules with tests, not inline in the orchestration layer.

**Four output types.** Contact sheets (grid JPEG/PNG), screenshots (individual frames), animated preview reels (WebP/WebM/GIF stitched from short clips), and animated contact sheets (WebP grid of animated clips). Each has its own orchestration module and its own `generate_*` Tauri command; `output_path` exposes a `*_path` builder per type with a configurable suffix.

**`test-api` feature.** `lib.rs` uses `#[cfg(any(test, feature = "test-api"))] pub mod` to expose internal modules only during tests — they are `mod` (private) in production builds. The integration test (`tests/integration.rs`) declares `required-features = ["test-api"]` in `Cargo.toml` so a plain `cargo test` silently skips it instead of failing to link. Do NOT change these modules back to unconditional `pub mod`; it widens the public API surface of `mosaic_lib`.

**ffmpeg argv prelude.** All pipelines begin with `ffmpeg::base_args()` (`-hide_banner -loglevel error -y`). Extend from that helper, don't inline the prelude.

**Cancellation model.** `JobState` (in `jobs.rs`) holds a shared `Arc<AtomicBool>`. `ffmpeg::run_cancellable` spawns the child with `kill_on_drop(true)`, drains stderr in a concurrent task to prevent pipe-fill deadlock, and races `child.wait()` against a poll of the cancel flag in `tokio::select!`. New long-running ffmpeg calls must go through `run_cancellable`, not `run_capture`, or they won't respond to the Cancel button.

**drawtext escaping.** `escape_drawtext` in `drawtext.rs` escapes `\ : ' % , [ ] ;` — more than ffmpeg's documented four, because the header pipeline concatenates two filters with a `,` graph separator; the extras defend against filename-based filter injection. When adding new `drawtext=text='...'` call sites, always pass user-derived text through `escape_drawtext`.

**Output contract.** `output_path::contact_sheet_path` / `screenshot_path` own filename generation including the `foo (1)` / `foo (2)` collision suffix. These are pure (take an `exists_fn` callback so tests can mock the filesystem). Don't replicate this logic elsewhere. The infix between stem and extension/index is user-configurable (`SheetOptions.suffix`, `ScreenshotsOptions.suffix`); empty falls back to the `DEFAULT_SHEET_SUFFIX` / `DEFAULT_SHOTS_SUFFIX` constants at the top of `output_path.rs`.

**Progress events.** Rust emits `job:file-start`, `job:step`, `job:file-done`, `job:file-failed`, `job:finished` via `AppHandle::emit`. The frontend's `wireEvents()` in `main.js` is the only place listening; route new progress signals through the same event names or add new ones in parallel.

**Additional Tauri commands.** Besides the generate/probe/check commands: `scan_folder(path, recursive)` returns all video paths under a directory (used by "Add Folder"); `reveal_in_finder(path)` shells out to `open -R` / `explorer /select` / `xdg-open` (used when a Done queue row is clicked).

**Frontend views.** The main window has two overlaid views sharing `grid-row: 2` — `#main-view` (dropzone + queue + run-options + action bar) and `#settings-view` (all `SheetOptions`/`ScreenshotsOptions` fields in two stacked sections). Toggled by the gear icon in the header; `Esc` closes settings. There are no tabs. The "Generate" checkboxes in `#run-options` (internally `readProduce()`/`applyProduce()`) are the source of truth for what `onGenerate` runs. Don't reintroduce tabs or couple `onGenerate` to which settings section is visible.

**Tools-missing state.** When `check_tools` fails, `main.js` adds `.tools-missing` to `#app` and swaps the queue area for `#tools-error` (install instructions + Retry). CSS disables the dropzone, run-options, action bar, and settings icon via `opacity: 0.4; pointer-events: none`. `toolsOk` in `main.js` also feeds into the Generate button's disabled state. Retry re-invokes `check_tools`; on success the UI un-dims.

## ffmpeg quirks to know

- `-ss <ts> -i <input>` is fast (keyframe) seek — matches the original bash script's behaviour. Rendered timestamp overlay may drift from actual frame time on files with sparse keyframes. Do not move `-ss` after `-i` without discussing.
- `ffmpeg -filter_complex vstack` is used to composite header + grid. PNG-only, no-header contact sheets skip this step and `std::fs::rename` the grid directly (faster, lossless).
- JPEG quality is mapped to `-q:v` via `output_path::jpeg_qv` (libmjpeg's 2–31 scale; 100→2, 50→15). This lives in `output_path.rs`, not in either pipeline module.

## Theming

`src/style.css` defines both dark (default) and light palettes via CSS variables under `:root` and `@media (prefers-color-scheme: light)`. `color-scheme: light dark` is set so native form controls theme correctly. Don't hardcode colours in component selectors — use the tokens (`--bg`, `--panel`, `--accent`, etc.) so both modes stay in sync.

## Icons & bundling

Full icon set lives in `src-tauri/icons/` (`.icns`, `.ico`, plus platform PNGs generated by `pnpm tauri icon`). `tauri.conf.json` `bundle.icon` must list the icon files — an empty array means nothing is embedded (this was the long-standing bug that made the window icon blank). If regenerating icons, re-run `pnpm tauri icon <source.png>`; the source PNG should be 1024×1024 with transparent background.

## Releasing

Run `node scripts/bump-version.mjs <version> --tag` to update all three version files (`package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`), commit, and tag. Push the tag with `git push origin v<version>` to trigger the release workflow. CI builds unsigned artifacts for macOS (universal), Windows (x64), and Linux (x64) and creates a draft GitHub release. Review the draft, then publish.

## References

- Design spec: `docs/2026-04-14-mosaic-design.md`
- Implementation plan: `docs/2026-04-14-mosaic-plan.md`
- CLI plan (future work): `docs/2026-04-14-mosaic-cli-plan.md`
- Distribution plan: `docs/2026-04-14-mosaic-distribution-plan.md`
- Test samples (HDR10, DV, various codecs): https://kodi.wiki/view/Samples
