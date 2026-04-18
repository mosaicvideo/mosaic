# Mosaic CLI — Design

> **Superseded** — The authoritative spec now lives at
> [`docs/superpowers/specs/2026-04-18-mosaic-cli-design.md`](superpowers/specs/2026-04-18-mosaic-cli-design.md).
> This file is kept for historical reference only. Do not edit further —
> update the superpowers spec instead.

**Date:** 2026-04-14 (updated 2026-04-18)
**Status:** Implemented (uncommitted working tree; pending release)

## Goal

Expose every Mosaic pipeline as a command-line tool `mosaic-cli`, reusing the existing Rust crate. Same output fidelity as the GUI. Batch-friendly for scripts and headless servers. Persistent per-user defaults via a single TOML file; CLI flags override it.

## Non-goals (v1)

- Interactive TUI. Flag-driven only.
- Watching folders / daemon mode.
- Parallel processing across files (matches GUI semantics — ffmpeg already saturates cores per file).
- Replacing the GUI's MediaInfo modal. `mosaic-cli probe --mediainfo` emits structured JSON; no interactive viewer.
- Preset flags (`--preset compact` / `--preset large`).
- NDJSON progress mode.
- Homebrew tap, winget, scoop. Release-page downloads only.

## Binary naming

The CLI ships as `mosaic-cli`. The GUI binary stays `mosaic`. No rename.

One `[[bin]]` entry in `src-tauri/Cargo.toml`, gated behind a `cli` feature so `tauri dev` / `tauri build` don't pull CLI-only deps. `serde` (with `derive`) and `tempfile` are already non-optional top-level deps — only `clap`, `indicatif`, and `toml` need to be `optional = true` and gated through the feature:

```toml
[[bin]]
name = "mosaic-cli"
path = "src/bin/mosaic_cli/main.rs"
required-features = ["cli"]

[dependencies]
# existing deps unchanged (serde, serde_json, tempfile, tokio, thiserror, which, tauri*)
clap      = { version = "4",    features = ["derive"], optional = true }
indicatif = { version = "0.17",                          optional = true }
toml      = { version = "0.8",                           optional = true }

[dev-dependencies]
# add:
assert_cmd = "2"
predicates = "3"

[features]
cli = ["dep:clap", "dep:indicatif", "dep:toml"]
```

The existing GUI binary (implicit `[[bin]] name = "mosaic" path = "src/main.rs"`) is untouched.

## Where it lives

Same package, two binaries. The CLI calls into `mosaic_lib` directly. `lib.rs` today uses a two-branch cfg pattern (`pub mod` under the feature, `mod` otherwise). Extend both branches to include `cli`:

```rust
#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod video_info;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod video_info;
// …same shape for output_path, ffmpeg, contact_sheet, screenshots,
// preview_reel, animated_sheet, jobs, defaults, mediainfo, input_scan
```

The negated arm is required so the module is always declared (just privately) when no gate is active. Each feature enables independently: `cargo test` (tests cfg), `cargo build --features cli`, and `cargo test --features test-api,cli` for CLI integration tests.

**Exposing `probe` and scan helpers.** `commands::probe` is currently `pub(crate)`, reachable from tests only via the `ffmpeg_test_hook_probe` wrapper in `lib.rs`. Extend that wrapper's cfg to include `cli` so the CLI calls it without widening `commands::probe`'s visibility. Same pattern for any other `pub(crate)` helper the CLI needs.

**Factoring the folder scan.** `scan_folder` and `VIDEO_EXTS` live inside the private `commands` module (a Tauri command plus a `const`, not a library function). Factor the scanning logic into a new pure module `src-tauri/src/input_scan.rs`:

```rust
pub const VIDEO_EXTS: &[&str] = &[ /* canonical 45 */ ];
pub fn scan(path: &Path, recursive: bool) -> Result<Vec<PathBuf>, String>;
```

`commands::scan_folder` becomes a thin wrapper. CLI calls `input_scan::scan` directly.

A workspace split (`mosaic-core` / `mosaic-cli` / `mosaic-app`) is deferred. Revisit only if publishing `mosaic-core` to crates.io.

## Prerequisites

Same as the GUI: `ffmpeg`, `ffprobe`, `mediainfo` on `PATH`. `mosaic-cli` calls `ffmpeg::locate_tools()` — missing any of the three fails with install instructions on stderr, exit code `2`.

## Shared defaults module

New `src-tauri/src/defaults.rs` holds every shipping default as a `pub const` grouped by pipeline:

```rust
pub mod screenshots {
    pub const COUNT: u32 = 8;
    pub const FORMAT: &str = "png";
    pub const QUALITY: u32 = 92;
}
pub mod sheet { /* cols, rows, width, gap, … */ }
pub mod reel { /* count, clip_length_secs, height, fps, … */ }
pub mod animated_sheet { /* … */ }
```

Both the CLI and the GUI consume these constants:

- **CLI:** clap `default_value_t = mosaic_lib::defaults::screenshots::COUNT` etc.
- **GUI:** a new `scripts/sync-defaults.mjs` reads the Rust file (via a small `cargo run --bin dump-defaults` helper or regex parse) and rewrites the `value="…"` attributes in `src/index.html`. Run as part of `pnpm version:bump` and as a CI drift check.

Post-v1: promote `defaults` to a typed `DefaultsConfig` struct with `Default` impls on `ScreenshotsOptions` / `SheetOptions` / etc., removing the current "no `Default` impl" note in CLAUDE.md.

## User config file

**Location.** `~/.mosaic-cli.toml`. Override path via `$MOSAIC_CLI_CONFIG=/path/to/file`.

**First-run auto-create.** On any `mosaic-cli` invocation, if the resolved path does not exist, write a fully-commented template (every key present but commented out, showing the built-in default) and print `Created ~/.mosaic-cli.toml` once on stderr. If the parent directory is read-only (CI, sandboxed, no `$HOME`), skip creation silently and use built-ins — do not fail.

**Format.** TOML with per-subcommand sections:

```toml
# ~/.mosaic-cli.toml
# Uncomment and edit to override built-in defaults.
# CLI flags override this file.

[screenshots]
# count = 8
# format = "png"
# quality = 92

[sheet]
# cols = 3
# rows = 6
# theme = "dark"

[reel]
# count = 15
# clip_length_secs = 2

[animated_sheet]
# cols = 3
# rows = 6
# fps = 12
```

**Precedence.** Built-in constants < config file < CLI flags. Resolve at clap-parse time: load the TOML into a `Config` struct with `Option<T>` fields, then pass `.unwrap_or(defaults::…)` into the Options struct.

**Unknown keys.** Warn on stderr (`warning: unknown key 'foo' in ~/.mosaic-cli.toml`), do not fail. Using `serde(deny_unknown_fields = false)` + a post-parse diff against known keys.

**No `config init` subcommand** — first-run auto-create covers scaffolding.

## CLI surface

Using `clap` v4 derive macros.

```
mosaic-cli screenshots     [OPTIONS] <INPUT>...     # individual frames
mosaic-cli sheet           [OPTIONS] <INPUT>...     # still contact sheet
mosaic-cli reel            [OPTIONS] <INPUT>...     # animated preview reel
mosaic-cli animated-sheet  [OPTIONS] <INPUT>...     # animated contact sheet
mosaic-cli probe           [--mediainfo] <INPUT>    # print VideoInfo as JSON
```

No combined subcommand — scripts run the CLI multiple times against the same input for multi-output jobs.

Shared flags on every generate subcommand:

| Flag | Default | Notes |
|---|---|---|
| `-o, --output <DIR>` | next-to-source | custom output directory |
| `-q, --quiet` | off | suppress progress bars |
| `-v, --verbose` | off | print ffmpeg args before each call |
| `--no-recursive` | off | disable directory recursion |

Per-subcommand flag tables map 1-to-1 to the Options structs in the library. Defaults resolve from `mosaic_lib::defaults::*`, not restated inline.

**`screenshots` — maps to `ScreenshotsOptions`**

| Flag | Default |
|---|---|
| `--count <N>` | `defaults::screenshots::COUNT` (8) |
| `--format <png\|jpeg>` | `defaults::screenshots::FORMAT` (png) |
| `--quality <N>` | `defaults::screenshots::QUALITY` (92, JPEG only) |
| `--suffix <S>` | `DEFAULT_SHOTS_SUFFIX` (`"_screens_"`) |

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
| `--suffix <S>` | `DEFAULT_SHEET_SUFFIX` (`"_sheet"`) |

**`reel` — maps to `PreviewOptions`**

| Flag | Default |
|---|---|
| `--count <N>` | 15 |
| `--clip-length <SECS>` | 2 |
| `--height <PX>` | 360 |
| `--fps <N>` | 24 |
| `--quality <N>` | 75 |
| `--format <webp\|webm\|gif>` | webp |
| `--suffix <S>` | `DEFAULT_PREVIEW_SUFFIX` (`"_reel"`) |

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
| `--suffix <S>` | `DEFAULT_ANIMATED_SHEET_SUFFIX` (`"_animated_sheet"`) |

**`probe`** — prints the serde-serialized `VideoInfo` as JSON on stdout. Actual shape (from `video_info.rs`):

```json
{
  "filename": "movie.mkv",
  "duration_secs": 120.5,
  "size_bytes": 1234567,
  "bit_rate": 8000000,
  "video": { "codec": "h264", "width": 1920, "height": 1080, "fps": 23.976, ... },
  "audio": { "codec": "aac", ... },
  "enrichment": { ... }
}
```

With `--mediainfo`, the output is wrapped:

```json
{
  "ffprobe": { /* VideoInfo as above */ },
  "mediainfo": "General\nComplete name  : /path/file.mkv\n..."
}
```

Consumers extract either side cleanly. No text separator form.

## Inputs

- Positional `<INPUT>...` takes any number of file paths.
- A directory argument triggers `input_scan::scan(path, recursive)` (the new module) filtered by the canonical 45-extension list (`input_scan::VIDEO_EXTS`). `--no-recursive` opts out.
- Glob patterns are handled by the shell, not the CLI (`mosaic-cli sheet /Videos/**/*.mkv`).

## Progress

`indicatif` multi-progress:

- Top bar: overall file progress (`1/5: /path/movie.mkv`).
- Second bar: current step within the file. Feeds from a `ProgressReporter` (in `jobs.rs`) whose `emit: &dyn Fn(u32, u32, &str)` callback maps to `set_length` + `set_position` + `set_message`.

`--quiet` uses a no-op reporter and prints only the final summary.

## Pipeline plumbing

Each generate subcommand:

1. Calls `ffmpeg::locate_tools()` → `Tools { ffmpeg, ffprobe, mediainfo }`.
2. Calls `tools.detect_has_zscale()` once per run (not per file) and passes into `PipelineContext`.
3. For each input, calls the `probe` helper (exposed via the hook in `lib.rs`) → `VideoInfo` (ffprobe + MediaInfo concurrently).
4. Builds a `PipelineContext` with `ffmpeg`, `cancelled` (shared `Arc<AtomicBool>`), `reporter`, `has_zscale`.
5. Calls the appropriate pipeline `generate`. The `font: &Path` parameter is present only on `contact_sheet::generate` and `animated_sheet::generate` — `screenshots::generate` and `preview_reel::generate` don't render drawtext and take no font. Actual signatures:

   ```rust
   screenshots::generate(source, info, out_dir, opts, ctx)
   contact_sheet::generate(source, info, output_path, opts, font, ctx)
   preview_reel::generate(source, info, out, opts, ctx)
   animated_sheet::generate(source, info, out, opts, font, ctx)
   ```

## Font asset

`DejaVuSans.ttf` is embedded into the CLI binary via `include_bytes!("../../../assets/fonts/DejaVuSans.ttf")` from `src-tauri/src/bin/mosaic_cli/font.rs` (three `..` hops reach `src-tauri/`). Extract to a `tempfile::NamedTempFile` lazily — only for `sheet` and `animated-sheet` subcommands, since `screenshots` and `reel` pipelines don't render drawtext. Tempfile drops at process exit.

Cost: ~750 KB added to the CLI binary. Benefit: zero external asset dependency — `cargo install` and release-page downloads both work without extra setup.

The GUI continues to resolve the bundled font via Tauri's resource system (`app.path().resolve("assets/fonts/DejaVuSans.ttf", BaseDirectory::Resource)` in `commands.rs`) — unchanged.

## Batch error behavior

Processing is **continue-on-error**, matching GUI semantics: a failed file logs to stderr with the same `RunError` string the GUI shows, the run continues through remaining inputs, and the final summary counts successes and failures separately (`3 done · 1 failed · 0 cancelled`). Exit code is `1` if any file failed, `0` otherwise. No `--fail-fast` flag in v1 — scripts needing fail-fast wrap with `for f in …; do mosaic-cli sheet "$f" || break; done`.

## Cancellation

`tokio::signal::ctrl_c` sets `ctx.cancelled = true` — reuses `run_cancellable` / `run_batch_cancellable` unchanged. Second Ctrl-C exits immediately without draining ffmpeg.

## Exit codes

- `0` — all files succeeded
- `1` — at least one file failed
- `2` — bad args / missing required tool / unreadable input / config parse error
- `130` — cancelled via signal

Summary line on exit mirrors GUI status: `3 done · 1 failed · 0 cancelled`.

## Error output

Stderr gets per-file failures. Stdout stays clean for pipeline composition — `probe` is the main stdout producer; generate subcommands emit the output file path(s) on success, one per line, so they pipe into `xargs`.

With `--verbose`, print the full ffmpeg command for each invocation before running it.

## Tests

Reuse `src-tauri/tests/fixtures/sample.mp4`. Add `src-tauri/tests/cli.rs` using `assert_cmd` + `predicates`, gated behind `cli` + `test-api`:

- `mosaic-cli probe fixtures/sample.mp4` → exit 0, JSON contains `duration_secs`.
- `mosaic-cli probe --mediainfo fixtures/sample.mp4` → exit 0, JSON with both `ffprobe` and `mediainfo` keys.
- `mosaic-cli screenshots --count 3 -o $TMPDIR fixtures/sample.mp4` → 3 PNGs exist.
- `mosaic-cli sheet --cols 2 --rows 2 -o $TMPDIR fixtures/sample.mp4` → file > 1 KB.
- `mosaic-cli reel --count 2 --clip-length 1 -o $TMPDIR fixtures/sample.mp4` → webp with `VP8X` animation flag set.
- `mosaic-cli animated-sheet --cols 2 --rows 2 --clip-length 1 -o $TMPDIR fixtures/sample.mp4` → animated webp.
- Config precedence: write a temp TOML with `[sheet] cols = 5`, set `$MOSAIC_CLI_CONFIG` to it, run `mosaic-cli sheet fixtures/sample.mp4` with no `--cols` flag, assert 5 columns. Then rerun with `--cols 2` and assert 2 columns (flag overrides file).
- First-run auto-create: set `$MOSAIC_CLI_CONFIG` to a path under `$TMPDIR` that does not exist, run any subcommand, assert the file was created.
- Error paths: non-existent input → exit 1; missing ffmpeg on PATH → exit 2 with install hint on stderr; malformed config → exit 2.

## Install / distribute

- Local dev: `cargo install --path src-tauri --bin mosaic-cli --features cli`.
- A step in the release workflow (`release.yml`) emits per-platform CLI binaries alongside the GUI installers:
  - `mosaic-cli-macos-universal`
  - `mosaic-cli-windows-x86_64.exe`
  - `mosaic-cli-windows-aarch64.exe`
  - `mosaic-cli-linux-x86_64`
- Strip debug symbols (`strip -x` / platform equivalent). Skip UPX.
- No Homebrew / winget / scoop in v1.

## Task sketch (for the plan stage)

1. Add `cli` feature + optional deps (`clap`, `indicatif`, `toml` — all `optional = true`). Add `assert_cmd`, `predicates` under `[dev-dependencies]`. Home-dir resolution uses `std::env::var("HOME").or(std::env::var("USERPROFILE"))` — no extra crate.
2. Create `src-tauri/src/defaults.rs` with `pub const` values per pipeline; expose under the unioned two-branch cfg.
3. Create `src-tauri/src/input_scan.rs` with `pub const VIDEO_EXTS` and `pub fn scan(path, recursive)`. Rewrite `commands::scan_folder` as a thin wrapper; remove the duplicate `VIDEO_EXTS`.
4. Write `scripts/sync-defaults.mjs` that updates `src/index.html` from `defaults.rs`; wire into `pnpm version:bump`; add CI drift check.
5. Extend the two-branch cfg on every pipeline-related `pub mod` / `mod` pair in `lib.rs` to include `cli`. Applies to `video_info`, `output_path`, `ffmpeg`, `contact_sheet`, `screenshots`, `preview_reel`, `animated_sheet`, `jobs`, `mediainfo`, plus the new `defaults` and `input_scan`.
6. Extend the cfg on existing hook functions in `lib.rs` (`ffmpeg_test_hook_locate`, `ffmpeg_test_hook_probe`, `video_info_test_hook_parse`) to include `feature = "cli"`. Decision: names retained as-is (`ffmpeg_test_hook_locate`, `ffmpeg_test_hook_probe`, `video_info_test_hook_parse`). The `test_hook_` prefix is historical — these are now dual-purpose entry points for both the integration test and the `mosaic-cli` binary. A rename is deferred to a later release; the spec reserves the right to do it then.
7. Write `src-tauri/src/bin/mosaic_cli/main.rs` — clap structs, config loader, ctrl-c handler, main dispatch.
8. Config loader: resolve path (`$MOSAIC_CLI_CONFIG` or `$HOME/.mosaic-cli.toml` / `%USERPROFILE%\.mosaic-cli.toml`), auto-create commented template (skip silently on read-only dirs / missing `$HOME`), parse into `Config` with `Option<T>` fields, warn on unknown keys.
9. `probe` subcommand (plain + `--mediainfo` JSON variant).
10. `screenshots` / `reel` subcommands — build `PipelineContext`, wire progress reporter. No font.
11. `sheet` / `animated-sheet` subcommands — as above, plus extract embedded `DejaVuSans.ttf` to a tempfile and pass its path as `font`.
12. Ctrl-C → `ctx.cancelled` via `tokio::signal::ctrl_c`.
13. `src-tauri/tests/cli.rs` integration tests.
14. Extend `release.yml` matrix to produce CLI binaries per platform.
15. Update README + `site/guide.html` with CLI usage examples.
16. Update CLAUDE.md: `defaults` module, `input_scan` module, `cli` feature, `mosaic-cli` binary, config file location.
