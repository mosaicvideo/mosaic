# Mosaic CLI — Plan

**Date:** 2026-04-14 (updated 2026-04-18)
**Status:** Draft, not yet implemented

## Goal

Expose every Mosaic pipeline as a command-line tool `mosaic-cli`, reusing the existing Rust crate. Same output fidelity as the GUI. Batch-friendly for scripts and headless servers.

## Non-goals (v1)

- Interactive TUI. Keep it flag-driven.
- Watching folders / daemon mode.
- Parallel processing across files (matches GUI semantics — ffmpeg already saturates cores per file).
- Duplicating GUI settings persistence. The CLI is stateless; every run is explicit.
- Replacing the GUI's MediaInfo modal. `mosaic-cli probe --mediainfo` can dump raw text, but there's no interactive viewer.

## Binary naming

The CLI ships as `mosaic-cli`. The GUI binary stays `mosaic` (current Cargo package name, implicit `mainBinaryName`). No rename of the GUI is needed — the two names don't collide.

One `[[bin]]` entry in `src-tauri/Cargo.toml`, gated behind a `cli` feature so `tauri dev` / `tauri build` don't pull clap/indicatif:

```toml
[[bin]]
name = "mosaic-cli"
path = "src/bin/mosaic_cli.rs"
required-features = ["cli"]

[features]
cli = ["dep:clap", "dep:indicatif"]
```

The existing single-binary GUI (implicit `[[bin]] name = "mosaic" path = "src/main.rs"`) continues to work untouched.

## Where it lives

Same package, two binaries. The CLI calls into `mosaic_lib` directly — the `test-api` feature already exposes the internal modules the CLI needs (`video_info`, `output_path`, `ffmpeg`, `contact_sheet`, `screenshots`, `preview_reel`, `animated_sheet`, `jobs`). Reuse that gate as `cli`, or union them: `#[cfg(any(test, feature = "test-api", feature = "cli"))] pub mod …`.

A Cargo workspace split (`mosaic-core` / `mosaic-cli` / `mosaic-app`) is not worth it at current scale. Revisit only if we want to publish `mosaic-core` to crates.io.

## Prerequisites

Same as the GUI: `ffmpeg`, `ffprobe`, `mediainfo` on `PATH`. `mosaic-cli` uses `ffmpeg::locate_tools()` — missing any of the three is a hard error with install instructions on stderr, exit code 2.

## CLI surface

Using `clap` v4 derive macros.

```
mosaic-cli screenshots     [OPTIONS] <INPUT>...     # individual frames
mosaic-cli sheet           [OPTIONS] <INPUT>...     # still contact sheet
mosaic-cli reel            [OPTIONS] <INPUT>...     # animated preview reel
mosaic-cli animated-sheet  [OPTIONS] <INPUT>...     # animated contact sheet
mosaic-cli probe           [--mediainfo] <INPUT>    # print VideoInfo as JSON
```

The four generate subcommands mirror the GUI's four "Generate" checkboxes. Unlike the original plan's `both`, there's no combined subcommand — at four outputs the combinatorics aren't worth a sugar flag. Scripts that want multiple outputs run the CLI multiple times against the same input; each run reuses ffmpeg's cache-warm files.

Shared flags on every generate subcommand:

| Flag | Default | Notes |
|---|---|---|
| `-o, --output <DIR>` | next-to-source | custom output directory |
| `-q, --quiet` | off | suppress progress bars |
| `-v, --verbose` | off | print ffmpeg args before each call |
| `--no-recursive` | off | disable directory recursion |

Per-subcommand flag tables map 1-to-1 to the Options structs in the library. Defaults below match the **GUI defaults** as shipped in `src/index.html` (`value="…"` attributes) — the Rust Options structs have no `Default` impl, so the CLI must restate them. Keep this table and the HTML in sync, or extract defaults into a shared constants module as a post-v1 refactor. Verified against `src/index.html` as of v0.1.3.

**`screenshots` — maps to `ScreenshotsOptions`**

| Flag | Default |
|---|---|
| `--count <N>` | 8 |
| `--format <png\|jpeg>` | png |
| `--quality <N>` | 92 (JPEG only, 50–100) |
| `--suffix <S>` | `"_screens_"` (= `DEFAULT_SHOTS_SUFFIX`) |

**`sheet` — maps to `SheetOptions`**

| Flag | Default |
|---|---|
| `--cols <N>` | 3 |
| `--rows <N>` | 6 |
| `--width <N>` | 1920 |
| `--gap <N>` | 10 |
| `--thumb-font <N>` | 18 |
| `--header-font <N>` | 20 |
| `--no-timestamps` | off |
| `--no-header` | off |
| `--format <png\|jpeg>` | png |
| `--quality <N>` | 92 |
| `--theme <dark\|light>` | dark |
| `--suffix <S>` | `"_sheet"` (= `DEFAULT_SHEET_SUFFIX`) |

**`reel` — maps to `PreviewOptions`**

| Flag | Default |
|---|---|
| `--count <N>` | 15 |
| `--clip-length <SECS>` | 2 |
| `--height <PX>` | 360 |
| `--fps <N>` | 24 |
| `--quality <N>` | 75 |
| `--format <webp\|webm\|gif>` | webp |
| `--suffix <S>` | `"_reel"` (= `DEFAULT_PREVIEW_SUFFIX`) |

**`animated-sheet` — maps to `AnimatedSheetOptions`**

| Flag | Default |
|---|---|
| `--cols <N>` | 3 |
| `--rows <N>` | 6 |
| `--width <N>` | 1280 |
| `--gap <N>` | 8 |
| `--clip-length <SECS>` | 2 |
| `--fps <N>` | 12 |
| `--quality <N>` | 75 |
| `--thumb-font <N>` | 14 |
| `--header-font <N>` | 18 |
| `--no-timestamps` | off |
| `--no-header` | off |
| `--theme <dark\|light>` | dark |
| `--suffix <S>` | `"_animated_sheet"` (= `DEFAULT_ANIMATED_SHEET_SUFFIX`) |

**`probe`** — prints `VideoInfo` as JSON on stdout. With `--mediainfo`, also runs `mediainfo <path>` and prints the raw text after a `---` separator. Both are useful for debugging / scripting.

## Inputs

- Positional `<INPUT>...` takes any number of file paths.
- A directory argument triggers the GUI's recursive scan (`commands::scan_folder`) filtered by the canonical 45-extension list (`VIDEO_EXTS` in `commands.rs`). `--no-recursive` opts out.
- Glob patterns are handled by the shell, not the CLI (`mosaic-cli sheet /Videos/**/*.mkv`).

## Progress

`indicatif` multi-progress:

- Top bar: overall file progress (`1/5: /path/movie.mkv`).
- Second bar: current step within the file. Feeds from a custom `ProgressReporter` (in `jobs.rs`) whose `emit: &dyn Fn(u32, u32, &str)` callback maps to `set_length` + `set_position` + `set_message`.

`--quiet` uses a no-op reporter and prints only the final summary.

## Pipeline plumbing

Each generate subcommand:

1. Calls `ffmpeg::locate_tools()` → `Tools { ffmpeg, ffprobe, mediainfo }`.
2. Calls `tools.detect_has_zscale()` once per run (not per file) and passes into `PipelineContext`.
3. For each input, calls `commands::probe(&tools, path)` to build a `VideoInfo` (ffprobe + MediaInfo concurrently).
4. Builds a `PipelineContext` with `ffmpeg`, `cancelled` (shared `Arc<AtomicBool>`), `reporter`, `has_zscale`.
5. Calls the appropriate `<pipeline>::generate(source, info, out, opts, font, ctx)`.
6. Uses the bundled `DejaVuSans.ttf` from `src-tauri/assets/fonts/` (same asset the GUI ships) — path resolved via `CARGO_MANIFEST_DIR` at build time or an env override for dev.

## Cancellation

`tokio::signal::ctrl_c` sets `ctx.cancelled = true` — reuses `run_cancellable` / `run_batch_cancellable` unchanged. Second Ctrl-C exits immediately without draining ffmpeg.

## Exit codes

- `0` — all files succeeded
- `1` — at least one file failed
- `2` — bad args / missing required tool (ffmpeg / ffprobe / mediainfo) / unreadable input
- `130` — cancelled via signal

Summary line on exit mirrors GUI status: `3 done · 1 failed · 0 cancelled`.

## Error output

Stderr gets per-file failures with the same `RunError` string the GUI shows. Stdout stays clean for pipeline composition (`probe` is the main stdout producer; generate subcommands emit output file paths on success so they pipe into `xargs`).

With `--verbose`, print the full ffmpeg command for each invocation before running it. Useful for debugging drawtext filter issues.

## Tests

- Reuse `src-tauri/tests/fixtures/sample.mp4`.
- Add `src-tauri/tests/cli.rs` using `assert_cmd` + `predicates`, gated behind `cli` + `test-api`:
  - `mosaic-cli probe fixtures/sample.mp4` → exit 0, JSON contains `duration_secs`.
  - `mosaic-cli probe --mediainfo fixtures/sample.mp4` → exit 0, JSON + `---` + mediainfo text.
  - `mosaic-cli screenshots --count 3 -o $TMPDIR fixtures/sample.mp4` → 3 PNGs exist.
  - `mosaic-cli sheet --cols 2 --rows 2 -o $TMPDIR fixtures/sample.mp4` → file > 1 KB.
  - `mosaic-cli reel --count 2 --clip-length 1 -o $TMPDIR fixtures/sample.mp4` → webp with `VP8X` animation flag set.
  - `mosaic-cli animated-sheet --cols 2 --rows 2 --clip-length 1 -o $TMPDIR fixtures/sample.mp4` → animated webp.
  - Error paths: non-existent file → exit 1; missing ffmpeg on PATH → exit 2 with install hint on stderr.

## Install / distribute

- Local dev: `cargo install --path src-tauri --bin mosaic-cli --features cli,test-api` (test-api is only needed if the modules aren't gated as `cli`-public by then).
- A GitHub Actions step in the release workflow emits per-platform CLI binaries into the release alongside the GUI installers: `mosaic-cli-macos-universal`, `mosaic-cli-windows-x86_64.exe`, `mosaic-cli-windows-aarch64.exe`, `mosaic-cli-linux-x86_64`.
- Strip debug symbols (`strip -x` / `cargo strip`). Skip UPX — the antivirus false-positive rate on Windows isn't worth the 30% size win.
- Homebrew tap and winget/scoop manifests are post-v1.

## Open questions

- Should `probe --mediainfo` emit structured JSON (`{ "ffprobe": {…}, "mediainfo": "<raw text>" }`) or a text separator? Text is simpler; JSON composes better with `jq`.
- NDJSON progress mode (`--json`) for wrapping in other tools — defer unless there's demand.
- Preset flags (`--preset compact`, `--preset large`) — probably defer.
- Should `sheet` + `animated-sheet` share a single flag namespace since most flags overlap? Probably no — makes `--help` clearer per-subcommand.

## Task sketch (for execution later)

1. Add `cli` feature to `src-tauri/Cargo.toml` + gate new deps (`clap`, `indicatif`, `assert_cmd`, `predicates`).
2. Promote pipeline modules to `#[cfg(any(test, feature = "test-api", feature = "cli"))] pub mod …`.
3. Write `src/bin/mosaic_cli.rs` with clap structs mapping 1-to-1 to the four Options structs.
4. Implement `probe` subcommand — wraps `commands::probe` + optionally `run_capture(&tools.mediainfo, &[&path])`.
5. Implement `screenshots` subcommand using `screenshots::generate`.
6. Implement `sheet` subcommand using `contact_sheet::generate`.
7. Implement `reel` subcommand using `preview_reel::generate`.
8. Implement `animated-sheet` subcommand using `animated_sheet::generate`.
9. Hook Ctrl-C handler to `ctx.cancelled`.
10. `src-tauri/tests/cli.rs` integration tests via `assert_cmd`.
11. Extend `release.yml` matrix to produce CLI binaries per platform.
12. Update README + `site/guide.html` with CLI usage examples.
