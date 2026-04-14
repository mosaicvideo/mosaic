# Mosaic CLI — Plan

**Date:** 2026-04-14
**Status:** Draft, not yet implemented

## Goal

Expose the Mosaic pipelines (screenshots + contact sheet) as a command-line tool `mosaic`, reusing the existing Rust crate. Same output fidelity as the GUI. Batch-friendly for scripts and headless servers.

## Non-goals (v1)

- Interactive TUI. Keep it flag-driven.
- Watching folders / daemon mode.
- Parallel processing across files (matches GUI semantics — ffmpeg already saturates cores per file).
- Duplicating GUI settings persistence. The CLI is stateless; every run is explicit.

## Binary naming (important)

The user-facing command is `mosaic` (i.e. `mosaic screenshots movie.mkv`). That collides with the current Cargo package, whose default binary is also `mosaic` — today it's the Tauri GUI. We need to rename the GUI's internal binary so the `mosaic` name is free for the CLI. macOS users never see the GUI binary's filename (buried inside `Mosaic.app/Contents/MacOS/` and shown as "Mosaic" via `productName` in `tauri.conf.json`), so renaming it has no UX impact.

Resolution: explicit `[[bin]]` entries — `mosaic-app` for the GUI, `mosaic` for the CLI. Also set `mainBinaryName: "mosaic-app"` in `tauri.conf.json` so `tauri dev`/`tauri build` target the right binary.

## Where it lives

Two options:

**A. Same package, two binaries (low effort, recommended first step).**
In `src-tauri/Cargo.toml`:

```toml
[[bin]]
name = "mosaic-app"
path = "src/main.rs"

[[bin]]
name = "mosaic"
path = "src/bin/mosaic.rs"
required-features = ["cli"]
```

Gate clap/indicatif behind a `cli` feature so `tauri dev` doesn't pull them. The CLI calls into `mosaic_lib` directly — which is why we already have the `test-api` / `pub mod` pattern; CLI can be `#[cfg(feature = "cli")]`-gated the same way, or we promote the core modules to always-public and keep `test-api` only for the test hook functions.

**B. Cargo workspace split (clean, more work).**
Restructure into:

```
mosaic/
├── Cargo.toml                 # workspace root
├── crates/
│   ├── mosaic-core/           # pipelines, ffmpeg, pure logic — no tauri
│   ├── mosaic-cli/            # clap + indicatif, depends on core, binary name: mosaic
│   └── mosaic-app/            # renamed src-tauri, depends on core + tauri
```

Each crate names its own binary: `mosaic-cli` crate produces `mosaic`, `mosaic-app` produces `mosaic-app`. No collision.

Pros: clean dependency separation, core is reusable, CI can test core without spinning up Tauri. Cons: noisy refactor, rewrites import paths everywhere.

**Recommendation:** start with (A). Promote to (B) only if we ever want to publish `mosaic-core` to crates.io.

## CLI surface

Using `clap` v4 derive macros.

```
mosaic screenshots [OPTIONS] <INPUT>...
mosaic sheet       [OPTIONS] <INPUT>...
mosaic both        [OPTIONS] <INPUT>...
mosaic probe       <INPUT>              # print VideoInfo as JSON
```

Common flags on every subcommand:

| Flag | Default | Notes |
|---|---|---|
| `-o, --output <DIR>` | next-to-source | custom output directory |
| `--format <png\|jpeg>` | png | |
| `--quality <N>` | 92 | JPEG only, 50–100 |
| `-q, --quiet` | off | suppress progress bars |
| `-v, --verbose` | off | show ffmpeg args |

`screenshots` extras:

| Flag | Default |
|---|---|
| `--count <N>` | 10 |

`sheet` extras (match `SheetOptions`):

| Flag | Default |
|---|---|
| `--cols <N>` | 3 |
| `--rows <N>` | 7 |
| `--width <N>` | 1920 |
| `--gap <N>` | 10 |
| `--thumb-font <N>` | 18 |
| `--header-font <N>` | 20 |
| `--no-timestamps` | off | inverse of GUI's "Show timestamps" |
| `--no-header` | off | inverse of GUI's "Show info header" |

`both` accepts all flags from both (prefixed `--shots-*` / `--sheet-*`) or, simpler v1, reuses per-subcommand defaults and only exposes shared flags like `--format`/`--output`.

## Inputs

- Positional `<INPUT>...` takes any number of file paths.
- A directory argument triggers the same recursive scan used by the GUI's "Add Folder" (`scan_folder`). `--no-recursive` opts out.
- Glob patterns are handled by the shell, not the CLI (`mosaic screenshots /Videos/**/*.mkv`).

## Progress

`indicatif` multi-progress:

- Top bar: overall file progress (`1/5: /path/movie.mkv`).
- Second bar: current step within the file (reuses `ProgressReporter` emit — maps step/total/label to `set_message` + `set_position`).

`--quiet` uses a no-op reporter and prints only final status.

## Cancellation

`tokio::signal::ctrl_c` sets the `JobState.cancelled` `AtomicBool` — reuses `run_cancellable` unchanged. Second Ctrl-C exits immediately without waiting for ffmpeg to drain.

## Exit codes

- `0` — all files succeeded
- `1` — at least one file failed
- `2` — bad args / missing ffmpeg / unreadable input
- `130` — cancelled via signal

Summary line on exit mirrors GUI status: `3 done · 1 failed · 0 cancelled`.

## Error output

Stderr gets per-file failures with the same `RunError` string the GUI shows in red. Stdout stays clean so the CLI composes with pipelines (though the CLI isn't really stdout-producing — exit code is the signal).

With `--verbose`, print the full ffmpeg command for each invocation before running it. Useful for debugging drawtext filter issues.

## Tests

- Reuse `tests/integration.rs` fixture (`sample.mp4`).
- Add `tests/cli.rs` using `assert_cmd` + `predicates`:
  - `mosaic probe fixtures/sample.mp4` → exit 0, JSON contains `duration_secs`.
  - `mosaic screenshots --count 3 -o $TMPDIR fixtures/sample.mp4` → 3 PNGs exist.
  - `mosaic sheet --cols 2 --rows 2 -o $TMPDIR fixtures/sample.mp4` → file exists > 1KB.
  - Error paths: non-existent file → exit 1, helpful stderr.

## Install / distribute

- `cargo install --path src-tauri --bin mosaic --features cli` for local dev.
- A GitHub Actions step in the release workflow (`strip` + `upx` optional) emits per-platform binaries into the release alongside the GUI installers.
- Homebrew tap and winget/scoop manifests are post-v1.

## Open questions

- Workspace split (B) now or later?
- Should `mosaic both` run both passes on the same file in sequence (like the GUI) or in parallel? Sequential matches current model and is safer.
- Preset flags? (`--preset compact`, `--preset large`) — probably defer.
- Machine-readable progress (`--json` emitting NDJSON per event) for wrapping in other tools — defer to post-v1 unless there's demand.

## Task sketch (for execution later)

1. Add `cli` feature to `Cargo.toml` + gate new deps (`clap`, `indicatif`).
2. Promote pipeline modules to always-`pub` (or keep `test-api`/`cli` gating — decide).
3. Write `src/bin/mosaic.rs` with clap structs mapping 1-to-1 to `SheetOptions` / `ScreenshotsOptions`.
4. Implement `probe` subcommand — wraps `video_info::parse` + `run_capture`.
5. Implement `screenshots` subcommand using existing `screenshots::generate`.
6. Implement `sheet` subcommand using existing `contact_sheet::generate`.
7. Implement `both` — sequential invocation of the two above.
8. Hook Ctrl-C handler to `JobState.cancelled`.
9. `tests/cli.rs` integration tests via `assert_cmd`.
10. Update README with CLI usage examples.
