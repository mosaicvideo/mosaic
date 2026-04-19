# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

Use `pnpm tauri dev` for development and `pnpm tauri build` for production bundles. Rust tests live under `src-tauri/` — plain `cargo test` runs unit tests; add `--features test-api` for integration tests. Run `cargo` from `src-tauri/` (the `Cargo.toml` isn't at repo root). For frontend-only changes, `pnpm build:web` is a fast Vite build that catches syntax/import errors without launching Tauri.

Always run `cargo clippy --all-targets --features test-api,cli -- -D warnings` after Rust changes and before reporting work complete. The plain `cargo clippy -- -D warnings` (GUI-only, no features) must also be clean. Clippy must be clean — fix warnings, don't silence them.

Vite (port 5173) serves `src/` and is spawned automatically by `tauri dev` via `beforeDevCommand`. Tauri's file watcher hot-rebuilds the Rust crate on `src-tauri/**` changes; Vite HMR handles `src/**`.

The integration test requires ffmpeg with the `drawtext` filter. On macOS the default Homebrew `ffmpeg` bottle is built **without** libfreetype. Install `brew install ffmpeg-full` and run tests with `PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --features test-api`. The app's `locate_tools()` already checks `ffmpeg-full` first on macOS.

## Architecture

Tauri 2 app. Rust backend orchestrates `ffmpeg`/`ffprobe` subprocesses; vanilla HTML/CSS/JS frontend talks to it via `invoke`/`listen`.

**Pipeline separation — the core of the codebase.** Pure logic (`drawtext.rs`, `layout.rs`, `header.rs`, `output_path.rs`, `video_info.rs`) is fully unit-testable with no subprocess dependency. Orchestration modules (`contact_sheet.rs`, `screenshots.rs`, `preview_reel.rs`, `animated_sheet.rs`) take pure-logic outputs and build ffmpeg arg vectors, delegating subprocess I/O to `ffmpeg.rs`. Commands (`commands.rs`) wrap everything with Tauri handlers and per-file job loops. Keep this layering intact when adding features: put new math/parsing in pure modules with tests, not inline in the orchestration layer. **For animated outputs (reel, animated sheet), use `layout::sample_clip_timestamps(dur, n, clip_len)` — not `sample_timestamps` — so timestamps leave room for `-t clip_len` and don't produce empty mp4s near the video end.**

**Four output types.** Contact sheets (grid JPEG/PNG), screenshots (individual frames), animated preview reels (WebP/WebM/GIF stitched from short clips), and animated contact sheets (WebP grid of animated clips). Each has its own orchestration module and its own `generate_*` Tauri command; `output_path` exposes a `*_path` builder per type with a configurable suffix.

**`test-api` / `cli` feature.** `lib.rs` uses `#[cfg(any(test, feature = "test-api", feature = "cli"))] pub mod` to expose internal modules only during tests or CLI builds — they are `mod` (private) in production GUI builds. The integration test (`tests/integration.rs`) declares `required-features = ["test-api"]` and the CLI integration test (`tests/cli.rs`) declares `required-features = ["cli", "test-api"]` in `Cargo.toml` so a plain `cargo test` silently skips both instead of failing to link. Do NOT change these modules back to unconditional `pub mod`; it widens the public API surface of `mosaic_lib`.

**ffmpeg argv prelude.** All pipelines begin with `ffmpeg::base_args()` (`-hide_banner -loglevel error -y`). Extend from that helper, don't inline the prelude.

**Cancellation model.** `JobState` (in `jobs.rs`) holds a shared `Arc<AtomicBool>`. `ffmpeg::run_cancellable` spawns the child with `kill_on_drop(true)`, drains stderr in a concurrent task to prevent pipe-fill deadlock, and races `child.wait()` against a poll of the cancel flag in `tokio::select!`. New long-running ffmpeg calls must go through `run_cancellable`, not `run_capture`, or they won't respond to the Cancel button.

**drawtext escaping.** `escape_drawtext` in `drawtext.rs` escapes `\ : ' % , [ ] ;` — more than ffmpeg's documented four, because the header pipeline concatenates two filters with a `,` graph separator; the extras defend against filename-based filter injection. When adding new `drawtext=text='...'` call sites, always pass user-derived text through `escape_drawtext`.

**Output contract.** `output_path::contact_sheet_path` / `screenshot_path` own filename generation including the `foo (1)` / `foo (2)` collision suffix. These are pure (take an `exists_fn` callback so tests can mock the filesystem). Don't replicate this logic elsewhere. The infix between stem and extension/index is user-configurable (`SheetOptions.suffix`, `ScreenshotsOptions.suffix`); empty falls back to the `DEFAULT_SHEET_SUFFIX` / `DEFAULT_SHOTS_SUFFIX` constants at the top of `output_path.rs`.

**Progress events.** Rust emits `job:file-start`, `job:step`, `job:file-done`, `job:file-failed`, `job:finished` via `AppHandle::emit`. The frontend's `wireEvents()` in `main.js` is the only place listening; route new progress signals through the same event names or add new ones in parallel.

**Additional Tauri commands.** Besides the generate/probe/check-tools commands: `scan_folder(path, recursive)` returns video paths under a directory (used by "Add Folder"); `reveal_in_finder(path)` shells out to `open -R` / `explorer /select` / `xdg-open` (used when a Done queue row is clicked); `get_video_exts()` returns the canonical 45-extension list (`VIDEO_EXTS` in `input_scan.rs`); `run_mediainfo(path)` for the metadata viewer; `cancel_job()` flips the shared cancel flag.

**Tool prerequisites.** `locate_tools()` in `ffmpeg.rs` resolves ffmpeg, ffprobe, and mediainfo with a Homebrew-aware search (`ffmpeg-full` first on macOS, then `which`, then `/opt/homebrew/bin` / `/usr/local/bin` fallbacks — macOS GUI apps don't inherit shell `PATH`). All three are first-party requirements; any missing binary fails `check_tools` at startup and the frontend shows the tools-missing state.

**MediaInfo integration.** Two roles: (1) probe-time enrichment — `commands::probe` runs `mediainfo --Output=JSON` alongside ffprobe and attaches a parsed subset (HDR format, commercial audio codec, bit depth, channel layout, language) to `VideoInfo.enrichment`; the header builder prefers these over raw ffprobe fields where available. (2) per-file metadata viewer — `run_mediainfo(path)` shells out for the full human-readable output surfaced in `src/mediainfo.js`'s modal. Parser lives in `mediainfo.rs`; only a small field subset is extracted — unknown fields degrade gracefully to ffprobe fallbacks.

**Frontend views.** The main window has two overlaid views sharing `grid-row: 2` — `#main-view` (dropzone + queue + run-options + action bar) and `#settings-view` (all `SheetOptions`/`ScreenshotsOptions` fields in two stacked sections). Toggled by the gear icon in the header; `Esc` closes settings. There are no tabs. The "Generate" checkboxes in `#run-options` (internally `readProduce()`/`applyProduce()`) are the source of truth for what `onGenerate` runs. Don't reintroduce tabs or couple `onGenerate` to which settings section is visible.

**Tools-missing state.** When `check_tools` fails, `main.js` adds `.tools-missing` to `#app` and swaps the queue area for `#tools-error` (install instructions + Retry). CSS disables the dropzone, run-options, action bar, and settings icon via `opacity: 0.4; pointer-events: none`. `toolsOk` in `main.js` also feeds into the Generate button's disabled state. Retry re-invokes `check_tools`; on success the UI un-dims.

**Window title.** Set at runtime in `lib.rs`'s `setup()` hook via `window.set_title("Mosaic {version}")` using `app.package_info().version`. Don't set a static title in `tauri.conf.json` — it'll be overwritten on launch.

## CLI binary

- The CLI is a **sibling Cargo package** at `mosaic-cli/`, not a `[[bin]]` inside the Tauri crate. This avoids `tauri build` trying to bundle the CLI into `.app`/`.deb` (the bundler iterates every `[[bin]]` in Cargo.toml and ignores `required-features`). `mosaic-cli` depends on the Tauri crate as a path dep (`mosaic = { path = "../src-tauri", features = ["cli"] }`), so it consumes `mosaic_lib` internals via the existing `feature = "cli"` visibility switch.
- Build: `cargo build --release` from `mosaic-cli/`.
- Test: `cargo test` from `mosaic-cli/` (on macOS, prepend `PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH"` — same ffmpeg-full requirement as the main integration test). The CLI's `tests/cli.rs` uses `assert_cmd` to exec `mosaic-cli` against `../src-tauri/tests/fixtures/sample.mp4`.
- **Dev workflow:** `pnpm dev:cli -- <subcommand> [args]` is the quickest path for iteration (wraps `cargo run --manifest-path mosaic-cli/Cargo.toml --bin mosaic-cli --`). Args after `--` forward to the binary.
- **Module layout:** `mosaic-cli/src/` contains `main.rs` (entry + dispatch), `cli.rs` (clap structs), `config.rs` (`~/.mosaic-cli.toml` loader + validation), `font.rs` (embedded DejaVuSans), `hints.rs` (tool-missing install hints), `progress.rs` (indicatif wrapper), `signals.rs` (ctrl-c handler), and `run/` (per-subcommand implementations + shared helpers `inputs.rs`, `format.rs`, `suffix.rs`). Font is embedded via `include_bytes!("../../src-tauri/assets/fonts/DejaVuSans.ttf")` reaching into the sibling crate.
- **Hook functions:** `lib.rs` exposes `ffmpeg_test_hook_locate`, `ffmpeg_test_hook_probe`, `video_info_test_hook_parse` under the same cfg as the pipeline modules. The `test_hook_` prefix is historical — they're now CLI-production entry points too. Rename deferred.
- `defaults.rs` is the shared source of truth for GUI HTML defaults and CLI flag defaults. `scripts/sync-defaults.mjs` reads `defaults.rs` and patches the matching `<input>`/`<select>` values in `src/index.html`; it runs automatically via `pnpm version:bump`. When adding a new shipping default: add it to `defaults.rs`, extend the key map in `sync-defaults.mjs`, extend the test in `defaults.rs::tests`.
- `input_scan.rs` owns folder-scan logic (moved out of `commands.rs`). Both the Tauri `scan_folder` command and the CLI's input expander call into it — don't duplicate the scan logic.
- Config file: `~/.mosaic-cli.toml`, auto-created on first run with every key commented out. Override path via `$MOSAIC_CLI_CONFIG`. CLI flags always take precedence over the config file.
- Pipeline modules (`video_info`, `ffmpeg`, `contact_sheet`, etc.) are exposed publicly under **both** `feature = "test-api"` **and** `feature = "cli"` via the two-branch `cfg` pattern in `lib.rs` (`#[cfg(any(test, feature = "test-api", feature = "cli"))] pub mod`). When adding a new pipeline module that the CLI needs, match that shape — do not widen to an unconditional `pub mod`.
- `mosaic-cli` is included in CI release builds as a separate artifact (`mosaic-cli-*`); the release workflow builds it from `mosaic-cli/` alongside the Tauri GUI bundle (they use separate `target/` directories).
- **Install scripts.** `site/install.sh` (POSIX sh, macOS + Linux) and `site/install.ps1` (Windows PowerShell) are static scripts served via the existing `pages.yml` GitHub Pages deployment. They resolve the latest release tag via the GitHub API, download the matching `mosaic-cli-*` artifact and `SHA256SUMS`, verify the checksum, and install to a user-scoped directory. No re-deploy needed per release — the scripts don't bake version numbers. Lint with `shellcheck site/install.sh` locally; CI runs this on every push via `.github/workflows/ci.yml`. The script uses `# shellcheck disable=SC2088` / `SC2016` directives where tildes and `$PATH` are intentional literal advice text rather than paths to expand.
- **Shell completions + man page.** Runtime clap introspection via `mosaic-cli completions <shell>` and `mosaic-cli manpage`. No build-time assets, no release-asset proliferation — the binary generates its own completion script and roff man page on demand. Both subcommands short-circuit before config load and tool probe in `main.rs`, so they work on a fresh install without `~/.mosaic-cli.toml` or ffmpeg present. Deps: `clap_complete`, `clap_mangen` in `mosaic-cli/Cargo.toml`.

## ffmpeg quirks to know

- `-ss <ts> -i <input>` is fast (keyframe) seek — matches the original bash script's behaviour. Rendered timestamp overlay may drift from actual frame time on files with sparse keyframes. Do not move `-ss` after `-i` without discussing.
- `ffmpeg -filter_complex vstack` is used to composite header + grid. PNG-only, no-header contact sheets skip this step and `std::fs::rename` the grid directly (faster, lossless).
- JPEG quality is mapped to `-q:v` via `output_path::jpeg_qv` (libmjpeg's 2–31 scale; 100→2, 50→15). This lives in `output_path.rs`, not in either pipeline module.

## Theming

`src/style.css` defines both dark (default) and light palettes via CSS variables under `:root` and `@media (prefers-color-scheme: light)`. `color-scheme: light dark` is set so native form controls theme correctly. Don't hardcode colours in component selectors — use the tokens (`--bg`, `--panel`, `--accent`, etc.) so both modes stay in sync.

## Icons & bundling

Full icon set lives in `src-tauri/icons/` (`.icns`, `.ico`, plus platform PNGs generated by `pnpm tauri icon`). `tauri.conf.json` `bundle.icon` must list the icon files — an empty array means nothing is embedded (this was the long-standing bug that made the window icon blank). If regenerating icons, re-run `pnpm tauri icon <source.png>`; the source PNG should be 1024×1024 with transparent background.

## Showcase site

`site/` is a standalone static HTML/CSS/JS site deployed to `https://mosaicvideo.github.io/mosaic/` via `.github/workflows/pages.yml`. Two pages (`index.html`, `guide.html`), no build step. `site/assets/download.js` upgrades download buttons from a GitHub Releases API fetch at runtime — falls back to `releases/latest` without JS. Preview locally with `cd site && python3 -m http.server 8000`. The site is intentionally decoupled from the app — site copy updates don't need a version bump.

## Releasing

Run `node scripts/bump-version.mjs <version> --tag` to update all three version files (`package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`), commit, and tag. Push the tag with `git push origin v<version>` to trigger the release workflow. CI builds four platform artifacts: macOS universal (signed + notarized via Developer ID), Windows x64 + ARM64 (unsigned — SmartScreen warning expected), Linux x64 (AppImage + deb + rpm). Each has a `.sig` companion (minisign/ed25519) and a combined `latest.json` uploads to the release for the auto-updater. Releases start as drafts — review, then publish so `releases/latest/download/latest.json` resolves for installed users.

Each CLI artifact is accompanied by a `.sha256` file; a `finalize` job in `release.yml` aggregates them (via `gh release download --pattern 'mosaic-cli-*.sha256'`) into a single `SHA256SUMS` release asset so users — and the install scripts — can verify downloads with `shasum -a 256 -c SHA256SUMS`.

Signing secrets live in repo Actions settings: `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`, `KEYCHAIN_PASSWORD`, `TAURI_SIGNING_PRIVATE_KEY`. The macOS-only keychain import step in `release.yml` runs before `tauri-action`.

## Auto-update

`tauri-plugin-updater` + `tauri-plugin-process`. Frontend calls `check()` on startup (fire-and-forget in `main.js` `checkForUpdate()`), shows a native `ask()` dialog if an update exists, then `downloadAndInstall()` + `relaunch()`. Errors are swallowed silently so dev-mode launches and offline starts aren't disrupted.

Endpoint config lives in `tauri.conf.json` `plugins.updater.endpoints` → `github.com/mosaicvideo/mosaic/releases/latest/download/latest.json`. Public key (ed25519/minisign) is embedded in the same file; private key lives at `~/.tauri/mosaic-updater.key` locally and in the `TAURI_SIGNING_PRIVATE_KEY` CI secret. Losing the private key orphans installed users — back it up.

Granted permissions in `src-tauri/capabilities/default.json`: `updater:default`, `process:default`, `dialog:allow-message`. Adding new plugins requires adding their permissions here too or the frontend `invoke` silently 404s.

## References

- Design spec: `docs/2026-04-14-mosaic-design.md`
- Implementation plan: `docs/2026-04-14-mosaic-plan.md`
- CLI spec: `docs/superpowers/specs/2026-04-18-mosaic-cli-design.md`
- CLI plan: `docs/superpowers/plans/2026-04-18-mosaic-cli.md`
- Distribution plan: `docs/2026-04-14-mosaic-distribution-plan.md`
- Test samples (HDR10, DV, various codecs): https://kodi.wiki/view/Samples
