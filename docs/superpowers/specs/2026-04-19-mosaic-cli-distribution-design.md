# Mosaic CLI — Distribution & Install UX — Design

**Date:** 2026-04-19
**Status:** Draft
**Related:** `docs/2026-04-14-mosaic-distribution-plan.md`, `docs/superpowers/specs/2026-04-18-mosaic-cli-design.md`

## Goal

Make `mosaic-cli` pleasant to install and use without a package manager. Ship one-liner install scripts (shell + PowerShell), SHA256 checksums for manual verification, runtime shell-completion and man-page generators, and a dedicated `cli.html` reference page on the showcase site. Fix stale CLI documentation along the way.

## Background

- CLI binaries ship today as raw release artifacts: `mosaic-cli-macos-universal` (signed + notarized), `mosaic-cli-windows-x86_64.exe`, `mosaic-cli-windows-aarch64.exe`, `mosaic-cli-linux-x86_64`. Built by `release.yml` matrix, uploaded per-runner via `softprops/action-gh-release`.
- Users currently install by downloading the asset, making it executable, and moving it onto PATH manually. No checksums, no one-liner, no post-install hints.
- CLI docs live inside `site/guide.html#cli` (subcommand list, examples, config file summary). `README.md` has no CLI mention. The guide still contains a stale `cd src-tauri && cargo install --path . --bin mosaic-cli --features cli` snippet — wrong since the CLI moved to the sibling `mosaic-cli/` crate.
- The binary exposes `--help` and `--version` via clap v4 derive. No shell completions, no man page.
- Config file: `~/.mosaic-cli.toml`, override path via `$MOSAIC_CLI_CONFIG`. Precedence (highest first): CLI flags → config file → built-in defaults.

## Scope

**In scope:**

1. `site/install.sh` + `site/install.ps1` — static scripts committed to the repo, served via GitHub Pages.
2. Per-release `SHA256SUMS` file covering every CLI artifact, uploaded to the GitHub Release.
3. Two new `mosaic-cli` subcommands: `completions <shell>` and `manpage`, using `clap_complete` and `clap_mangen`.
4. New `site/cli.html` page — canonical CLI reference. Nav updates on all existing pages.
5. Doc fixes: trim `guide.html#cli` to a pointer, remove stale `cargo install` snippet, add CLI section to `README.md`, update `after_help` URL in `cli.rs`.

**Out of scope (explicitly deferred):**

- Any package manager (Homebrew tap, Scoop bucket, Winget, apt/yum repo).
- crates.io publish + the `mosaic-core` carve-out that would enable it.
- CLI auto-update (re-running the install script is the supported upgrade path).
- Linux aarch64 release build.
- Auto-generated flag reference in `cli.html` (hand-written for now; revisit if drift becomes a problem).
- Uninstall helper script — manual deletion is documented on the page.
- Binary rename from `mosaic-cli` to `mosaic` — deferred indefinitely.

## Design

### 1. Release pipeline additions

Two changes to `release.yml`; `tauri-action` and the existing CLI build/sign/notarize steps are untouched.

**Per-matrix-runner step: produce a single-line checksum file alongside each CLI artifact.**

After the existing "Prepare CLI artifact" step (line ~179), add:

```yaml
- name: Checksum CLI artifact
  shell: bash
  run: |
    if [ "${RUNNER_OS}" = "macOS" ] || [ "${RUNNER_OS}" = "Linux" ]; then
      shasum -a 256 "${CLI_ARTIFACT}" > "${CLI_ARTIFACT}.sha256"
    else
      # Windows runners — use certutil, normalise format to match shasum.
      sha=$(certutil -hashfile "${CLI_ARTIFACT}" SHA256 | sed -n '2p' | tr -d ' \r')
      printf '%s  %s\n' "$sha" "${CLI_ARTIFACT}" > "${CLI_ARTIFACT}.sha256"
    fi
```

Upload step already uploads `${CLI_ARTIFACT}` — extend its `files:` list to include `${CLI_ARTIFACT}.sha256`.

**New finalize job: aggregate all per-artifact checksum files into one `SHA256SUMS`.**

```yaml
finalize:
  needs: build
  runs-on: ubuntu-latest
  steps:
    - name: Download per-artifact checksums from the draft release
      env:
        GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      run: |
        mkdir -p checksums
        gh release download "${GITHUB_REF_NAME}" \
          --repo "${GITHUB_REPOSITORY}" \
          --pattern 'mosaic-cli-*.sha256' \
          --dir checksums
        cat checksums/*.sha256 | sort > SHA256SUMS
    - name: Upload SHA256SUMS
      uses: softprops/action-gh-release@v2
      with:
        files: SHA256SUMS
        tag_name: ${{ github.ref_name }}
        draft: true
        fail_on_unmatched_files: true
```

Rationale: matrix runners can't coordinate directly; the draft release is the rendezvous point. `gh release download` is available by default on `ubuntu-latest` runners.

`SHA256SUMS` format: one line per artifact, `{hash}  {filename}`, sorted — same format `shasum -a 256 -c -` expects.

### 2. `site/install.sh` (POSIX sh, macOS + Linux)

Static script, committed once. Resolves the latest version at runtime.

**Parameters (all env vars):**

| Var | Default | Purpose |
|---|---|---|
| `MOSAIC_INSTALL_DIR` | `$HOME/.local/bin` | Where to drop the binary. |
| `MOSAIC_VERSION` | `latest` | Literal tag (`v0.1.5`) or `latest`. |
| `MOSAIC_NO_MODIFY_PATH` | unset | Reserved; no-op today. The script never modifies PATH — this flag exists for forward compat if that ever changes. |

**Flow:**

1. Detect OS via `uname -s`. Darwin and Linux accepted. Anything else → error: "use install.ps1 on Windows".
2. Detect arch: on Linux, `uname -m` must be `x86_64`; on Darwin any arch maps to the `macos-universal` asset. Linux aarch64 → clear error linking to the issue tracker.
3. Resolve tag: if `MOSAIC_VERSION=latest`, curl `https://api.github.com/repos/mosaicvideo/mosaic/releases/latest` and extract `.tag_name` using a `grep -oE '"tag_name"[^"]*"[^"]*"' | sed` chain — no `jq` dependency, since minimal containers may lack it. If literal, use as-is.
4. Compute asset filename:
   - Darwin: `mosaic-cli-macos-universal`
   - Linux x86_64: `mosaic-cli-linux-x86_64`
5. Download to a `mktemp -d` directory (registered with `trap '...' EXIT` so it's cleaned up on any exit path):
   - the binary asset
   - `SHA256SUMS`
   Prefer `curl -fL`; fall back to `wget -q` if curl missing.
6. Verify: `grep " $asset_name\$" SHA256SUMS | (cd tmp && shasum -a 256 -c -)`. Prefer `shasum`; fall back to `sha256sum` on minimal Linux containers.
7. `install -m 755 tmp/$asset_name "$MOSAIC_INSTALL_DIR/mosaic-cli"`; `mkdir -p` the dir first.
8. Run `"$MOSAIC_INSTALL_DIR/mosaic-cli" --version` as a sanity probe. If the invocation fails, surface stderr and exit non-zero.
9. Post-install output:
   - Installed path + resolved version.
   - PATH guidance if `$MOSAIC_INSTALL_DIR` isn't in `$PATH`: detect shell from `$SHELL`, print the specific `export PATH` line and the rc file to add it to.
   - One-line completions hint for the detected shell, plus link to `https://mosaicvideo.github.io/mosaic/cli.html#completions`.
   - Docs URL: `https://mosaicvideo.github.io/mosaic/cli.html`.

**Error UX:** every failure prints the attempted URL and the error output. A user can replay any step by hand.

**Non-goals:** root/sudo install, system-wide install, auto-uninstall, self-update. User-scoped and fully reversible.

### 3. `site/install.ps1` (PowerShell 5.1+, Windows)

Shape mirrors install.sh. Parameters as PS `param(...)`:

| Param | Default | Purpose |
|---|---|---|
| `-InstallDir` | `"$env:LOCALAPPDATA\Programs\mosaic-cli"` | Where to drop the binary. |
| `-Version` | `"latest"` | Literal tag or `latest`. |

**Flow:**

1. Detect arch via `$env:PROCESSOR_ARCHITECTURE`: `AMD64` → x86_64, `ARM64` → aarch64. Anything else → clear error.
2. Resolve tag via `Invoke-RestMethod "https://api.github.com/repos/mosaicvideo/mosaic/releases/latest"`.
3. Compute asset filename (`mosaic-cli-windows-x86_64.exe` / `mosaic-cli-windows-aarch64.exe`).
4. Download binary + `SHA256SUMS` to `$env:TEMP`.
5. Verify with `Get-FileHash -Algorithm SHA256` against the matching line in `SHA256SUMS`. Case-insensitive hex compare.
6. `New-Item -ItemType Directory -Force $InstallDir`; `Move-Item` the binary to `$InstallDir\mosaic-cli.exe`.
7. Run `& "$InstallDir\mosaic-cli.exe" --version` as a sanity probe.
8. Add `$InstallDir` to user PATH via `[Environment]::SetEnvironmentVariable('Path', ..., 'User')` **only if** not already present. Print: "Restart your terminal for PATH changes to take effect." PowerShell completions hint + docs URL.

**SmartScreen behavior:** Windows CLI remains unsigned (per existing distribution plan). The install script doesn't try to suppress SmartScreen — the binary download happens programmatically so Mark-of-the-Web is avoided, but if the user manually runs the binary from Explorer they may see a warning. The `cli.html` page notes this.

### 4. `completions` and `manpage` subcommands

New deps in `mosaic-cli/Cargo.toml`: `clap_complete = "4"`, `clap_mangen = "0.2"`.

New in `mosaic-cli/src/cli.rs`:

```rust
pub enum Command {
    // ... existing ...
    Completions(CompletionsArgs),
    Manpage,
}

#[derive(Parser)]
#[command(about = "Emit a shell-completion script to stdout")]
pub struct CompletionsArgs {
    #[arg(value_enum)]
    pub shell: Shell,
}
```

Where `Shell` is `clap_complete::Shell`.

`mosaic-cli/src/main.rs` dispatches these without hitting the runtime (no config load, no tool probe — they're pure clap introspection):

```rust
Command::Completions(a) => {
    clap_complete::generate(a.shell, &mut Cli::command(), "mosaic-cli", &mut io::stdout());
    0
}
Command::Manpage => {
    clap_mangen::Man::new(Cli::command()).render(&mut io::stdout())?;
    0
}
```

Both subcommands visible in `--help`. Exit code 0 on success; non-zero only on stdout write failure.

Implementation sized at ~30 lines total across the two handlers.

### 5. `site/cli.html`

New page, same layout primitives as `guide.html` (sidebar TOC + article). Sections:

1. **Hero** — one-line framing, single code block (`mosaic-cli sheet movie.mkv`).
2. **Install**:
   - macOS / Linux: `curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | sh`
   - Windows: `irm https://mosaicvideo.github.io/mosaic/install.ps1 | iex`
   - Manual download table — one row per OS/arch with a direct `releases/latest/download/mosaic-cli-*` link. Uses the existing `download.js` to populate the release version label.
   - Checksum verification: 3-line snippet showing the `shasum -a 256 -c` flow against `SHA256SUMS`.
3. **Requirements** — brief: ffmpeg / ffprobe / mediainfo on PATH. Cross-link to `guide.html#requirements` for install instructions (not duplicated here).
4. **Quick start** — 4 runnable recipes: `sheet movie.mkv`, `screenshots --count 12 -o shots/ movie.mkv`, `reel --count 10 --clip-length 2 movie.mkv`, `animated-sheet --cols 4 --rows 3 movie.mkv`.
5. **Subcommand reference** — one subsection per subcommand:
   - `screenshots`, `sheet`, `reel`, `animated-sheet`, `probe`, `completions`, `manpage`
   - Each: one-line description, synopsis, flag table (Name / Type / Default / Description), 1–2 examples.
   - Defaults mirrored from `src-tauri/src/defaults.rs`.
6. **Config file** — shape of `~/.mosaic-cli.toml`, precedence (flags → config → built-in defaults), `$MOSAIC_CLI_CONFIG` override, the `clip_length_secs` vs `--clip-length` naming caveat (already noted in guide — move it here).
7. **Shell completions** (`#completions`) — per-shell instructions. Anchor linked from the install scripts' post-install output.
   - bash: `mosaic-cli completions bash > ~/.local/share/bash-completion/completions/mosaic-cli`
   - zsh: create `~/.zfunc/`, emit into it, add `fpath=(~/.zfunc $fpath); autoload -Uz compinit; compinit` to `.zshrc` if missing.
   - fish: `mosaic-cli completions fish > ~/.config/fish/completions/mosaic-cli.fish`
   - powershell: emit into `$PROFILE` or dot-source on startup.
8. **Man page** — `mosaic-cli manpage > ~/.local/share/man/man1/mosaic-cli.1` + `man mosaic-cli`.
9. **Upgrading** — re-run the install script. Version is resolved at runtime, so the same one-liner always grabs the latest.
10. **Uninstalling** — `rm $MOSAIC_INSTALL_DIR/mosaic-cli` (or Windows equivalent). PATH reversal instruction if the install script added to it on Windows.
11. **Troubleshooting** — common failures with hints:
    - "ffmpeg not found" → link to Requirements.
    - "Gatekeeper blocked `mosaic-cli`" → shouldn't happen (signed + notarized); if it does, `xattr -d com.apple.quarantine`.
    - Arch mismatch error from install.sh.
    - Checksum mismatch (hint: re-download; possible partial download).
    - GitHub API rate limit (unauth 60 req/hr) → suggest setting `MOSAIC_VERSION=v0.1.5` explicitly or authenticating with `GH_TOKEN`.

**No build step added to the site.** The page is hand-written HTML using existing CSS tokens. `download.js` is reused for version-label population in the manual-download table.

### 6. Changes to existing pages

- **`site/guide.html`** — `#cli` section trimmed to: "For full CLI reference, install instructions, and the subcommand flag reference, see [cli.html](cli.html)." Stale `cargo install` snippet removed. Top nav gets a new `cli` link.
- **`site/index.html`** — small tile in the hero-adjacent section: "CLI for scripts and CI pipelines", one-line description, link to `cli.html`. Top nav gets a new `cli` link.
- **`README.md`** — new "Command-line interface" section (~5 lines): one-line description, the two install one-liners, link to `cli.html`. Positioned after "Install" (GUI) and before "Requirements (dev)".
- **`mosaic-cli/src/cli.rs`** — `after_help` string updates the docs URL from `guide.html` to `cli.html`.
- **`CLAUDE.md`** — new sub-bullet under "CLI binary": install scripts in `site/install.sh` / `site/install.ps1` served via Pages; `completions`/`manpage` subcommands are runtime clap introspection, no build-time assets.

## Testing

### Install script smoke tests (manual, per release candidate)

Run the one-liner fresh on each target. Each test verifies the install succeeds, `--version` works, and one pipeline produces a non-empty output file.

| Platform | Method | Priority |
|---|---|---|
| macOS arm64 | dev machine | must-pass |
| macOS x86_64 | Intel secondary or x86_64 VM | must-pass |
| Ubuntu 22.04 x86_64 | Docker: `ubuntu:22.04 + curl` | must-pass |
| Windows 11 x86_64 | VM | must-pass |
| Windows arm64 | best-effort; skip if no hardware available | nice-to-have |

Fixture: `src-tauri/tests/fixtures/sample.mp4` pulled down via `curl` from the repo. Smoke pipeline: `mosaic-cli probe sample.mp4`.

### CI: script linting

Add to `ci.yml`:

```yaml
- name: Shellcheck install.sh
  run: shellcheck site/install.sh
```

`shellcheck` is pre-installed on `ubuntu-latest`. PSScriptAnalyzer for `install.ps1` is optional — skipped for v1 to keep CI cost down; manual Windows smoke covers the same surface.

### Rust integration tests

Extend `mosaic-cli/tests/cli.rs` using the existing `assert_cmd` + `predicates` setup:

- `completions_zsh_emits_compdef` — `mosaic-cli completions zsh` exits 0; stdout starts with `#compdef`.
- `completions_bash_emits_complete_builtin` — exits 0; stdout contains `complete -F`.
- `completions_fish_emits_complete` — exits 0; stdout contains `complete -c mosaic-cli`.
- `completions_powershell_emits_register_argumentcompleter` — exits 0; stdout contains `Register-ArgumentCompleter`.
- `manpage_emits_th_header` — exits 0; stdout starts with `.TH "MOSAIC-CLI"`.

These tests require neither ffmpeg nor network (pure clap introspection) and run under a plain `cargo test` in `mosaic-cli/`.

### CI: SHA256SUMS sanity

The finalize job inherently fails if any matrix artifact is missing (`fail_on_unmatched_files: true`), so no separate check needed.

## Rollout

1. Implement everything on a feature branch. Implementation plan will split this into reviewable chunks during the `writing-plans` phase.
2. Tag `v0.1.5-rc1`. Verify:
   - All existing GUI + CLI artifacts ship unchanged.
   - New per-artifact `.sha256` files upload.
   - `SHA256SUMS` lands in the finalize job.
   - `cli.html` deploys via Pages.
   - install.sh and install.ps1 reachable at the Pages URLs.
   - One-liner install works end-to-end on all must-pass platforms.
   - `mosaic-cli completions zsh` and `mosaic-cli manpage` produce valid output.
3. Tag `v0.1.5`. Publish the release. Update `site/index.html` CLI tile if any copy changes shook out during RC.
4. Monitor issues for install-script edge cases for ~1 week (weird shells, corporate proxies, minimal containers without curl+wget). Document discovered quirks in `cli.html#troubleshooting`.

## Open questions

- **Windows arm64 smoke testing** — if no hardware is available for the first release, document it as untested-but-built. Flag in release notes.
- **GitHub API rate limits** — 60/hr unauth is fine for install traffic. No mitigation today; flag in troubleshooting.
- **Future: auto-generated flag reference** — if `cli.rs` drifts from `cli.html`, revisit by adding a `scripts/gen-cli-docs.mjs` that shells out to `mosaic-cli sub --help` and emits HTML fragments. Not worth the tooling for 7 subcommands today.

## References

- `docs/2026-04-14-mosaic-distribution-plan.md` — parent distribution plan.
- `docs/superpowers/specs/2026-04-18-mosaic-cli-design.md` — CLI design spec (subcommands, config file, flag surface).
- `CLAUDE.md` — CLI binary section.
- `.github/workflows/release.yml` — existing release pipeline.
- clap_complete docs: <https://docs.rs/clap_complete>
- clap_mangen docs: <https://docs.rs/clap_mangen>
