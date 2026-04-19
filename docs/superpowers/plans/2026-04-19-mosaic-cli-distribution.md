# Mosaic CLI Distribution & Install UX — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship one-liner install scripts (`curl | sh`, `irm | iex`), SHA256 checksums, runtime shell-completion + man-page generators, and a dedicated `cli.html` reference page — no package managers.

**Architecture:** No new release tooling. Existing `release.yml` grows two additions (per-artifact `.sha256` files + a finalize job that aggregates them into `SHA256SUMS`). Two new static scripts (`site/install.sh`, `site/install.ps1`) live in the repo, served via the existing `pages.yml` GitHub Pages deployment, and resolve the latest release tag at runtime. Two new clap-introspection subcommands (`completions`, `manpage`) emit shell completions and a roff man page on demand. New `site/cli.html` replaces the CLI section in `site/guide.html`.

**Tech Stack:** Rust (clap v4 derive, `clap_complete`, `clap_mangen`, `assert_cmd`, `predicates`), GitHub Actions (`softprops/action-gh-release@v2`, `gh` CLI), POSIX sh, PowerShell 5.1+, static HTML/CSS.

**Spec:** `docs/superpowers/specs/2026-04-19-mosaic-cli-distribution-design.md`

---

## File Structure

**New files:**

| Path | Responsibility |
|---|---|
| `site/install.sh` | POSIX sh installer for macOS + Linux. Detects OS/arch, resolves latest release, downloads, verifies SHA256, installs to `$MOSAIC_INSTALL_DIR`. |
| `site/install.ps1` | PowerShell installer for Windows. Same shape as install.sh. |
| `site/cli.html` | Canonical CLI reference page — install, subcommand reference, config, completions, troubleshooting. |

**Modified files:**

| Path | Change |
|---|---|
| `mosaic-cli/Cargo.toml` | Add `clap_complete`, `clap_mangen` deps. |
| `mosaic-cli/src/cli.rs` | Add `Completions` + `Manpage` variants to `Command` enum; update `after_help` docs URL. |
| `mosaic-cli/src/main.rs` | Dispatch `Completions` and `Manpage` before config load. |
| `mosaic-cli/tests/cli.rs` | Add 5 tests for the new subcommands. |
| `.github/workflows/release.yml` | Add "Checksum CLI artifact" step; include `.sha256` in uploads; add `finalize` job. |
| `.github/workflows/ci.yml` | Add shellcheck step for `site/install.sh`. |
| `site/guide.html` | Trim `#cli` section to a pointer; add `cli` nav link; remove stale `cargo install` snippet. |
| `site/index.html` | Add `cli` nav link. |
| `README.md` | New "Command-line interface" section. |
| `CLAUDE.md` | Note install-script location and runtime completion/manpage subcommands. |
| `CHANGELOG.md` | `[Unreleased]` entries. |

---

## Task 1: Add `clap_complete` and `clap_mangen` dependencies

**Files:**
- Modify: `mosaic-cli/Cargo.toml`

- [ ] **Step 1: Add the deps**

In `mosaic-cli/Cargo.toml`, under `[dependencies]`, add immediately after the existing `clap` line:

```toml
clap_complete = "4"
clap_mangen = "0.2"
```

The final `[dependencies]` block should read:

```toml
[dependencies]
mosaic = { path = "../src-tauri", features = ["cli"] }
clap = { version = "4", features = ["derive"] }
clap_complete = "4"
clap_mangen = "0.2"
indicatif = "0.17"
toml = "0.8"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["process", "io-util", "sync", "rt-multi-thread", "macros", "signal"] }
tempfile = "3"
```

- [ ] **Step 2: Verify the deps resolve**

Run: `cargo check --manifest-path mosaic-cli/Cargo.toml`
Expected: exits 0. New crates downloaded and compiled.

- [ ] **Step 3: Commit**

```bash
git add mosaic-cli/Cargo.toml mosaic-cli/Cargo.lock
git commit -m "feat(cli): add clap_complete + clap_mangen deps for completions/manpage subcommands"
```

---

## Task 2: Add `Completions` and `Manpage` subcommands (TDD)

**Files:**
- Modify: `mosaic-cli/src/cli.rs`
- Modify: `mosaic-cli/src/main.rs`
- Modify: `mosaic-cli/tests/cli.rs`

- [ ] **Step 1: Write the failing tests**

Append to `mosaic-cli/tests/cli.rs` (at the very end of the file):

```rust
#[test]
fn completions_zsh_emits_compdef() {
    Command::cargo_bin("mosaic-cli").unwrap()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("#compdef"));
}

#[test]
fn completions_bash_emits_complete_builtin() {
    Command::cargo_bin("mosaic-cli").unwrap()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete -F"));
}

#[test]
fn completions_fish_emits_complete() {
    Command::cargo_bin("mosaic-cli").unwrap()
        .args(["completions", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("complete -c mosaic-cli"));
}

#[test]
fn completions_powershell_emits_register_argumentcompleter() {
    Command::cargo_bin("mosaic-cli").unwrap()
        .args(["completions", "powershell"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Register-ArgumentCompleter"));
}

#[test]
fn manpage_emits_th_header() {
    // clap_mangen emits troff-escape lines before .TH, so assert `.TH` appears,
    // not that it's at line 1.
    Command::cargo_bin("mosaic-cli").unwrap()
        .args(["manpage"])
        .assert()
        .success()
        .stdout(predicate::str::contains(".TH"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path mosaic-cli/Cargo.toml completions_ manpage_ -- --nocapture`
Expected: FAIL. The `completions` and `manpage` subcommands don't exist yet; clap rejects them with `error: unrecognized subcommand`.

- [ ] **Step 3: Add the enum variants in `cli.rs`**

Modify `mosaic-cli/src/cli.rs`. At the top, alongside the existing `use clap::{Parser, Subcommand, ValueEnum};`, add:

```rust
use clap_complete::Shell;
```

Extend the `Command` enum (currently lines 20-27). Replace:

```rust
#[derive(Subcommand)]
pub enum Command {
    Screenshots(ScreenshotsArgs),
    Sheet(SheetArgs),
    Reel(ReelArgs),
    AnimatedSheet(AnimatedSheetArgs),
    Probe(ProbeArgs),
}
```

with:

```rust
#[derive(Subcommand)]
pub enum Command {
    Screenshots(ScreenshotsArgs),
    Sheet(SheetArgs),
    Reel(ReelArgs),
    AnimatedSheet(AnimatedSheetArgs),
    Probe(ProbeArgs),
    Completions(CompletionsArgs),
    #[command(about = "Emit a roff man page to stdout")]
    Manpage,
}
```

At the very end of the file (after `ProbeArgs`), add:

```rust
#[derive(Parser)]
#[command(about = "Emit a shell-completion script to stdout")]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    pub shell: Shell,
}
```

- [ ] **Step 4: Dispatch in `main.rs`**

Modify `mosaic-cli/src/main.rs`. The new subcommands must short-circuit before config load and tool probe — they're pure clap introspection.

Replace the body of `main()` (currently lines 13-47) with:

```rust
#[tokio::main]
async fn main() {
    let parsed = cli::Cli::parse();

    // Self-contained subcommands that don't need config or ffmpeg/ffprobe/mediainfo.
    match &parsed.command {
        cli::Command::Completions(a) => {
            use clap::CommandFactory;
            let mut cmd = cli::Cli::command();
            clap_complete::generate(a.shell, &mut cmd, "mosaic-cli", &mut std::io::stdout());
            return;
        }
        cli::Command::Manpage => {
            use clap::CommandFactory;
            let cmd = cli::Cli::command();
            if let Err(e) = clap_mangen::Man::new(cmd).render(&mut std::io::stdout()) {
                eprintln!("manpage render failed: {e}");
                std::process::exit(1);
            }
            return;
        }
        _ => {}
    }

    let cfg = match config::resolve_path() {
        Some((p, is_explicit)) => match config::load_or_create(&p, is_explicit) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        },
        None => config::Config::default(),
    };
    if let Err(e) = cfg.validate() {
        eprintln!("{e}");
        std::process::exit(2);
    }

    let verbose = match &parsed.command {
        cli::Command::Screenshots(a)   => a.shared.verbose,
        cli::Command::Sheet(a)         => a.shared.verbose,
        cli::Command::Reel(a)          => a.shared.verbose,
        cli::Command::AnimatedSheet(a) => a.shared.verbose,
        cli::Command::Probe(_)         => false,
        cli::Command::Completions(_) | cli::Command::Manpage => unreachable!("handled above"),
    };
    mosaic_lib::ffmpeg::set_verbose(verbose);

    let code = match parsed.command {
        cli::Command::Screenshots(a)   => run::screenshots::run(a, &cfg).await,
        cli::Command::Sheet(a)         => run::sheet::run(a, &cfg).await,
        cli::Command::Reel(a)          => run::reel::run(a, &cfg).await,
        cli::Command::AnimatedSheet(a) => run::animated_sheet::run(a, &cfg).await,
        cli::Command::Probe(a)         => run::probe::run(a).await,
        cli::Command::Completions(_) | cli::Command::Manpage => unreachable!("handled above"),
    };
    std::process::exit(code);
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --manifest-path mosaic-cli/Cargo.toml completions_ manpage_`
Expected: PASS for all 5 tests.

- [ ] **Step 6: Run the full test suite to confirm no regression**

Run: `PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --manifest-path mosaic-cli/Cargo.toml`
Expected: all tests pass, including the existing probe/screenshots/sheet/reel/animated_sheet tests.

- [ ] **Step 7: Clippy check**

Run: `cargo clippy --manifest-path mosaic-cli/Cargo.toml --all-targets -- -D warnings`
Expected: exits 0 with no warnings.

Then the Tauri crate (CLAUDE.md requires both features on **and** off):

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features test-api,cli -- -D warnings`
Expected: exits 0.

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
Expected: exits 0.

- [ ] **Step 8: Commit**

```bash
git add mosaic-cli/src/cli.rs mosaic-cli/src/main.rs mosaic-cli/tests/cli.rs
git commit -m "feat(cli): add completions and manpage subcommands

completions <shell> emits a clap_complete script; manpage emits a roff
man page. Both bypass config load and tool probe — pure clap introspection.
Integration tests cover zsh/bash/fish/powershell output shapes and the
.TH presence in the man page."
```

---

## Task 3: Update `after_help` docs URL to point at the new `cli.html`

**Files:**
- Modify: `mosaic-cli/src/cli.rs:9-14`

- [ ] **Step 1: Change the URL**

In `mosaic-cli/src/cli.rs`, locate the `#[command(...)]` attribute on `struct Cli` (around line 9-14). Replace:

```rust
    after_help = "Config file: ~/.mosaic-cli.toml (auto-created on first run; override path with $MOSAIC_CLI_CONFIG).\nDocs: https://mosaicvideo.github.io/mosaic/guide.html"
```

with:

```rust
    after_help = "Config file: ~/.mosaic-cli.toml (auto-created on first run; override path with $MOSAIC_CLI_CONFIG).\nDocs: https://mosaicvideo.github.io/mosaic/cli.html"
```

- [ ] **Step 2: Verify the build still succeeds**

Run: `cargo check --manifest-path mosaic-cli/Cargo.toml`
Expected: exits 0.

- [ ] **Step 3: Sanity-check that `--help` shows the new URL**

Run: `cargo run --manifest-path mosaic-cli/Cargo.toml -- --help | tail -5`
Expected: output ends with a line containing `https://mosaicvideo.github.io/mosaic/cli.html`.

- [ ] **Step 4: Commit**

```bash
git add mosaic-cli/src/cli.rs
git commit -m "docs(cli): point --help footer at cli.html instead of guide.html"
```

---

## Task 4: Add per-artifact `.sha256` files to the release workflow

**Files:**
- Modify: `.github/workflows/release.yml` (around line 179-202)

- [ ] **Step 1: Add the checksum step and extend upload**

Locate the "Prepare CLI artifact" step (currently around line 179). After that step ends (line 194, `echo "CLI_ARTIFACT=$dst" >> "$GITHUB_ENV"`), and before "Upload CLI to release draft", insert:

```yaml
      - name: Checksum CLI artifact
        shell: bash
        run: |
          if [ "$RUNNER_OS" = "Windows" ]; then
            sha=$(certutil -hashfile "$CLI_ARTIFACT" SHA256 | sed -n '2p' | tr -d ' \r' | tr 'A-Z' 'a-z')
            printf '%s  %s\n' "$sha" "$CLI_ARTIFACT" > "${CLI_ARTIFACT}.sha256"
          else
            shasum -a 256 "$CLI_ARTIFACT" > "${CLI_ARTIFACT}.sha256"
          fi
          cat "${CLI_ARTIFACT}.sha256"
```

Then modify the existing "Upload CLI to release draft" step to upload the `.sha256` file too. Replace:

```yaml
      - name: Upload CLI to release draft
        uses: softprops/action-gh-release@v2
        with:
          files: ${{ env.CLI_ARTIFACT }}
          tag_name: ${{ github.ref_name }}
          draft: true
          fail_on_unmatched_files: true
```

with:

```yaml
      - name: Upload CLI to release draft
        uses: softprops/action-gh-release@v2
        with:
          files: |
            ${{ env.CLI_ARTIFACT }}
            ${{ env.CLI_ARTIFACT }}.sha256
          tag_name: ${{ github.ref_name }}
          draft: true
          fail_on_unmatched_files: true
```

- [ ] **Step 2: Lint the workflow locally if you have `actionlint`**

Run: `actionlint .github/workflows/release.yml` (skip if not installed — the real verification is an RC tag).
Expected: exits 0 with no errors.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci(release): generate per-artifact SHA256 for CLI binaries

Each matrix runner writes a single-line checksum file alongside its
CLI artifact and uploads both to the draft release. Next commit adds
a finalize job that aggregates them into SHA256SUMS."
```

---

## Task 5: Add `finalize` job that aggregates `SHA256SUMS`

**Files:**
- Modify: `.github/workflows/release.yml` (append after the `build` job)

- [ ] **Step 1: Append the finalize job**

At the very end of `.github/workflows/release.yml` (after the last step of the `build` job), add the new job at the same indentation level as `build:`:

```yaml
  finalize:
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - name: Download per-artifact checksums from draft release
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          mkdir -p checksums
          gh release download "$GITHUB_REF_NAME" \
            --repo "$GITHUB_REPOSITORY" \
            --pattern 'mosaic-cli-*.sha256' \
            --dir checksums
          cd checksums
          cat *.sha256 | sort > ../SHA256SUMS
          cd ..
          echo "--- SHA256SUMS ---"
          cat SHA256SUMS

      - name: Upload SHA256SUMS to draft release
        uses: softprops/action-gh-release@v2
        with:
          files: SHA256SUMS
          tag_name: ${{ github.ref_name }}
          draft: true
          fail_on_unmatched_files: true
```

- [ ] **Step 2: Verify the file still parses**

Run: `python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/release.yml"))'`
Expected: exits 0 with no output (no YAML syntax error).

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci(release): add finalize job to publish SHA256SUMS

Waits for all matrix builds, downloads each per-artifact .sha256 file
from the draft release, concatenates them into a sorted SHA256SUMS
file, and uploads it. Users can then verify a downloaded CLI binary
with \`shasum -a 256 -c SHA256SUMS\`."
```

---

## Task 6: Create `site/install.sh`

**Files:**
- Create: `site/install.sh`

- [ ] **Step 1: Write the script**

Create `site/install.sh` with exactly this content:

```sh
#!/bin/sh
# Mosaic CLI installer — macOS + Linux
# Usage: curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | sh
# Env:
#   MOSAIC_INSTALL_DIR  target dir (default: $HOME/.local/bin)
#   MOSAIC_VERSION      tag or "latest" (default: latest)

set -eu

: "${MOSAIC_INSTALL_DIR:=$HOME/.local/bin}"
: "${MOSAIC_VERSION:=latest}"

REPO="mosaicvideo/mosaic"
GH_API="https://api.github.com/repos/${REPO}"
DOCS_URL="https://mosaicvideo.github.io/mosaic/cli.html"

err() { printf 'mosaic-cli: %s\n' "$*" >&2; }
info() { printf 'mosaic-cli: %s\n' "$*" >&2; }

# ── 1. Detect OS + arch ──────────────────────────────────────────
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Darwin) asset="mosaic-cli-macos-universal" ;;
  Linux)
    case "$arch" in
      x86_64|amd64) asset="mosaic-cli-linux-x86_64" ;;
      aarch64|arm64)
        err "Linux aarch64 builds aren't published yet. See ${DOCS_URL}#troubleshooting"
        exit 1 ;;
      *)
        err "Unsupported Linux arch: $arch"
        exit 1 ;;
    esac ;;
  *)
    err "Unsupported OS: $os. On Windows, use install.ps1."
    exit 1 ;;
esac

# ── 2. Pick downloader ───────────────────────────────────────────
if command -v curl >/dev/null 2>&1; then
  dl() { curl -fL --proto '=https' --tlsv1.2 -o "$2" "$1"; }
  fetch() { curl -fsSL --proto '=https' --tlsv1.2 "$1"; }
elif command -v wget >/dev/null 2>&1; then
  dl() { wget -qO "$2" "$1"; }
  fetch() { wget -qO- "$1"; }
else
  err "Neither curl nor wget found on PATH."
  exit 1
fi

# ── 3. Resolve version ───────────────────────────────────────────
if [ "$MOSAIC_VERSION" = "latest" ]; then
  info "resolving latest release tag..."
  tag="$(fetch "${GH_API}/releases/latest" 2>/dev/null \
    | sed -nE 's/.*"tag_name"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' \
    | head -n1)"
  if [ -z "${tag:-}" ]; then
    err "could not resolve latest release tag from ${GH_API}/releases/latest"
    err "(try setting MOSAIC_VERSION=vX.Y.Z explicitly)"
    exit 1
  fi
else
  tag="$MOSAIC_VERSION"
fi

# ── 4. Download to temp dir (cleaned on exit) ────────────────────
tmp="$(mktemp -d 2>/dev/null || mktemp -d -t mosaic-cli)"
trap 'rm -rf "$tmp"' EXIT

base_url="https://github.com/${REPO}/releases/download/${tag}"
info "downloading ${asset} (${tag})"

if ! dl "${base_url}/${asset}" "${tmp}/${asset}"; then
  err "download failed: ${base_url}/${asset}"
  exit 1
fi
if ! dl "${base_url}/SHA256SUMS" "${tmp}/SHA256SUMS"; then
  err "download failed: ${base_url}/SHA256SUMS"
  err "(this release may predate SHA256SUMS; try MOSAIC_VERSION=v0.1.5 or later)"
  exit 1
fi

# ── 5. Verify checksum ───────────────────────────────────────────
info "verifying checksum..."
(
  cd "$tmp"
  expected_line="$(grep " ${asset}\$" SHA256SUMS || true)"
  if [ -z "$expected_line" ]; then
    echo "mosaic-cli: asset ${asset} not listed in SHA256SUMS" >&2
    exit 1
  fi
  if command -v shasum >/dev/null 2>&1; then
    printf '%s\n' "$expected_line" | shasum -a 256 -c - >/dev/null
  elif command -v sha256sum >/dev/null 2>&1; then
    printf '%s\n' "$expected_line" | sha256sum -c - >/dev/null
  else
    echo "mosaic-cli: neither shasum nor sha256sum found — cannot verify" >&2
    exit 1
  fi
) || {
  err "checksum verification failed"
  exit 1
}

# ── 6. Install ───────────────────────────────────────────────────
mkdir -p "$MOSAIC_INSTALL_DIR"
install -m 755 "${tmp}/${asset}" "${MOSAIC_INSTALL_DIR}/mosaic-cli"

# ── 7. Sanity probe ──────────────────────────────────────────────
if ! "${MOSAIC_INSTALL_DIR}/mosaic-cli" --version >/dev/null 2>&1; then
  err "installed binary failed to run. Try:"
  err "  ${MOSAIC_INSTALL_DIR}/mosaic-cli --version"
  exit 1
fi
ver="$("${MOSAIC_INSTALL_DIR}/mosaic-cli" --version 2>/dev/null | awk '{print $2}')"

# ── 8. Post-install report ───────────────────────────────────────
printf '\n'
printf 'Installed mosaic-cli %s\n' "${ver:-unknown}"
printf '  -> %s/mosaic-cli\n' "$MOSAIC_INSTALL_DIR"
printf '\n'

# PATH hint if target isn't on PATH.
case ":${PATH}:" in
  *":${MOSAIC_INSTALL_DIR}:"*) ;;
  *)
    shell_name="$(basename "${SHELL:-sh}")"
    case "$shell_name" in
      zsh) rc="~/.zshrc" ;;
      bash) rc="~/.bashrc (macOS: ~/.bash_profile)" ;;
      fish) rc="~/.config/fish/config.fish" ;;
      *) rc="your shell rc file" ;;
    esac
    printf 'Add this to %s:\n' "$rc"
    printf '  export PATH="%s:$PATH"\n\n' "$MOSAIC_INSTALL_DIR"
    ;;
esac

# Shell-specific completions hint.
shell_name="$(basename "${SHELL:-sh}")"
case "$shell_name" in
  zsh)
    printf 'Enable zsh completions:\n'
    printf '  mkdir -p ~/.zfunc && mosaic-cli completions zsh > ~/.zfunc/_mosaic-cli\n'
    printf "  # ensure 'fpath=(~/.zfunc \$fpath)' is in ~/.zshrc before 'compinit'\n"
    printf '\n'
    ;;
  bash)
    printf 'Enable bash completions:\n'
    printf '  mkdir -p ~/.local/share/bash-completion/completions\n'
    printf '  mosaic-cli completions bash > ~/.local/share/bash-completion/completions/mosaic-cli\n\n'
    ;;
  fish)
    printf 'Enable fish completions:\n'
    printf '  mkdir -p ~/.config/fish/completions\n'
    printf '  mosaic-cli completions fish > ~/.config/fish/completions/mosaic-cli.fish\n\n'
    ;;
esac

printf 'Docs: %s\n' "$DOCS_URL"
```

- [ ] **Step 2: Make the script executable locally (for testing)**

Run: `chmod +x site/install.sh`
Expected: no output.

- [ ] **Step 3: Shellcheck locally if available**

Run: `shellcheck site/install.sh` (install with `brew install shellcheck` if missing).
Expected: exits 0 with no warnings. If any surface, fix them inline — do not silence with comments.

- [ ] **Step 4: Dry-run sanity (offline-safe)**

Run: `sh -n site/install.sh`
Expected: exits 0 (syntax parses). This does NOT execute the script.

- [ ] **Step 5: Commit**

```bash
git add site/install.sh
git commit -m "feat(site): add install.sh (macOS + Linux one-liner installer)

Detects OS/arch, resolves the latest release via GitHub API, downloads
the matching mosaic-cli binary and SHA256SUMS, verifies the checksum,
installs to \$MOSAIC_INSTALL_DIR (default ~/.local/bin), and prints
PATH + completions hints based on the detected shell. Usage:

  curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | sh"
```

---

## Task 7: Create `site/install.ps1`

**Files:**
- Create: `site/install.ps1`

- [ ] **Step 1: Write the script**

Create `site/install.ps1` with exactly this content:

```powershell
# Mosaic CLI installer — Windows
# Usage: irm https://mosaicvideo.github.io/mosaic/install.ps1 | iex
# Params:
#   -InstallDir  target dir (default: $env:LOCALAPPDATA\Programs\mosaic-cli)
#   -Version     tag or 'latest' (default: latest)

[CmdletBinding()]
param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Programs\mosaic-cli",
    [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"
$Repo = "mosaicvideo/mosaic"
$GhApi = "https://api.github.com/repos/$Repo"
$DocsUrl = "https://mosaicvideo.github.io/mosaic/cli.html"

function Die($msg) {
    Write-Host "mosaic-cli: $msg" -ForegroundColor Red
    exit 1
}

function Info($msg) {
    Write-Host "mosaic-cli: $msg"
}

# 1. Detect arch
$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
    "AMD64" { $asset = "mosaic-cli-windows-x86_64.exe" }
    "ARM64" { $asset = "mosaic-cli-windows-aarch64.exe" }
    default { Die "Unsupported Windows arch: $arch" }
}

# 2. Resolve version
if ($Version -eq "latest") {
    Info "resolving latest release tag..."
    try {
        $resp = Invoke-RestMethod -UseBasicParsing -Uri "$GhApi/releases/latest"
        $tag = $resp.tag_name
    } catch {
        Die "could not resolve latest release tag: $_"
    }
} else {
    $tag = $Version
}
if (-not $tag) { Die "empty tag from release API" }

# 3. Download to temp dir
$tmp = Join-Path $env:TEMP "mosaic-cli-install-$([guid]::NewGuid())"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
    $baseUrl = "https://github.com/$Repo/releases/download/$tag"
    Info "downloading $asset ($tag)"
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/$asset" -OutFile (Join-Path $tmp $asset)
    } catch {
        Die "download failed: $baseUrl/$asset"
    }
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/SHA256SUMS" -OutFile (Join-Path $tmp "SHA256SUMS")
    } catch {
        Die "download failed: $baseUrl/SHA256SUMS (this release may predate SHA256SUMS; try -Version v0.1.5 or later)"
    }

    # 4. Verify checksum
    Info "verifying checksum..."
    $sumsPath = Join-Path $tmp "SHA256SUMS"
    $line = Get-Content $sumsPath | Where-Object { $_ -match "\s+$([regex]::Escape($asset))$" } | Select-Object -First 1
    if (-not $line) { Die "asset $asset not listed in SHA256SUMS" }
    $expected = ($line -split "\s+")[0].ToLower()
    $actual = (Get-FileHash -Algorithm SHA256 -Path (Join-Path $tmp $asset)).Hash.ToLower()
    if ($expected -ne $actual) {
        Die "checksum mismatch for $asset (expected $expected, got $actual)"
    }

    # 5. Install
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $dest = Join-Path $InstallDir "mosaic-cli.exe"
    Move-Item -Force -Path (Join-Path $tmp $asset) -Destination $dest

    # 6. Sanity probe
    try {
        $ver = & $dest --version 2>$null
        if ($LASTEXITCODE -ne 0) { throw "nonzero exit" }
    } catch {
        Die "installed binary failed to run: $dest"
    }

    # 7. PATH: add InstallDir to user PATH if not already present
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not ($userPath -split ";" | Where-Object { $_ -eq $InstallDir })) {
        $newPath = if ([string]::IsNullOrEmpty($userPath)) { $InstallDir } else { "$userPath;$InstallDir" }
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        Write-Host ""
        Write-Host "Added $InstallDir to user PATH."
        Write-Host "Restart your terminal for the change to take effect."
    }

    Write-Host ""
    Write-Host "Installed $ver"
    Write-Host "  -> $dest"
    Write-Host ""
    Write-Host "Enable PowerShell completions (add to `$PROFILE to persist):"
    Write-Host "  mosaic-cli completions powershell | Out-String | Invoke-Expression"
    Write-Host ""
    Write-Host "Docs: $DocsUrl"
} finally {
    Remove-Item -Recurse -Force -Path $tmp -ErrorAction SilentlyContinue
}
```

- [ ] **Step 2: PowerShell syntax check (optional — skip if pwsh missing)**

Run: `pwsh -NoProfile -Command "[System.Management.Automation.PSParser]::Tokenize((Get-Content site/install.ps1 -Raw), [ref]\$null) | Out-Null; Write-Host 'parse ok'"`
Expected: prints `parse ok`. Skip if `pwsh` isn't installed locally — Windows smoke test at RC time is the canonical verification.

- [ ] **Step 3: Commit**

```bash
git add site/install.ps1
git commit -m "feat(site): add install.ps1 (Windows one-liner installer)

Mirrors install.sh: resolves latest release via GitHub API, downloads
the matching mosaic-cli .exe and SHA256SUMS, verifies the checksum,
installs to -InstallDir (default %LOCALAPPDATA%\\Programs\\mosaic-cli),
adds to user PATH if missing, and prints completions hint. Usage:

  irm https://mosaicvideo.github.io/mosaic/install.ps1 | iex"
```

---

## Task 8: Add shellcheck step to `ci.yml`

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Read current ci.yml structure**

Run: `head -40 .github/workflows/ci.yml`
Expected: shows the existing jobs. Note the first job's step list — we'll add a new step at the end of an existing job or introduce a tiny new job.

- [ ] **Step 2: Add a shellcheck job**

Append at the end of `.github/workflows/ci.yml`, at the same indentation as existing top-level jobs (typically 2 spaces under `jobs:`):

```yaml
  shellcheck:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Shellcheck install.sh
        run: shellcheck site/install.sh
```

`shellcheck` is pre-installed on `ubuntu-latest` GitHub runners, so no install step is needed.

- [ ] **Step 3: Verify YAML still parses**

Run: `python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/ci.yml"))'`
Expected: exits 0.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: shellcheck site/install.sh on every push

Catches POSIX sh regressions in the installer before they land on main."
```

---

## Task 9: Create `site/cli.html`

**Files:**
- Create: `site/cli.html`

- [ ] **Step 1: Write the page**

Create `site/cli.html` with this content. It mirrors `guide.html`'s layout primitives (nav, sidebar TOC, article). All flag defaults come from `src-tauri/src/defaults.rs`.

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>CLI — Mosaic</title>
  <meta name="description" content="Install and use mosaic-cli — the command-line interface for Mosaic contact sheets, screenshots, animated previews, and probes. One-liner install for macOS, Linux, and Windows.">
  <meta name="theme-color" content="#060807">
  <link rel="icon" type="image/png" href="assets/favicon.png">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="assets/style.css">
</head>
<body>

<nav class="nav">
  <div class="nav-inner">
    <a href="./" class="nav-brand">
      <span class="nav-brand-text">mosaic</span>
      <span class="nav-brand-version" id="nav-version">v0.1.4</span>
    </a>
    <div class="nav-links">
      <a href="guide.html">guide</a>
      <a href="cli.html" class="active">cli</a>
      <a href="guide.html#faq">faq</a>
      <a href="https://github.com/mosaicvideo/mosaic/blob/main/CHANGELOG.md">log</a>
      <a href="https://github.com/mosaicvideo/mosaic">git</a>
    </div>
  </div>
</nav>

<main>
  <div class="container">
    <div class="guide-layout">

      <aside class="guide-toc" aria-label="Table of contents">
        <div class="guide-toc-heading">Contents</div>
        <ul>
          <li><a href="#install">Install</a>
            <ul>
              <li><a href="#install-unix">macOS &amp; Linux</a></li>
              <li><a href="#install-windows">Windows</a></li>
              <li><a href="#install-manual">Manual download</a></li>
              <li><a href="#install-verify">Verify checksum</a></li>
            </ul>
          </li>
          <li><a href="#requirements">Requirements</a></li>
          <li><a href="#quick-start">Quick start</a></li>
          <li><a href="#subcommands">Subcommands</a>
            <ul>
              <li><a href="#sub-screenshots">screenshots</a></li>
              <li><a href="#sub-sheet">sheet</a></li>
              <li><a href="#sub-reel">reel</a></li>
              <li><a href="#sub-animated-sheet">animated-sheet</a></li>
              <li><a href="#sub-probe">probe</a></li>
              <li><a href="#sub-completions">completions</a></li>
              <li><a href="#sub-manpage">manpage</a></li>
            </ul>
          </li>
          <li><a href="#config">Config file</a></li>
          <li><a href="#completions">Shell completions</a></li>
          <li><a href="#manpage">Man page</a></li>
          <li><a href="#upgrading">Upgrading</a></li>
          <li><a href="#uninstalling">Uninstalling</a></li>
          <li><a href="#troubleshooting">Troubleshooting</a></li>
        </ul>
      </aside>

      <article class="guide-content">

        <h1>mosaic-cli</h1>
        <p class="lede">Every Mosaic pipeline from the terminal — scripts, CI jobs, headless servers. Same ffmpeg pipelines as the desktop app, no UI.</p>

        <pre><code>mosaic-cli sheet movie.mkv</code></pre>

        <h2 id="install">Install</h2>

        <h3 id="install-unix">macOS &amp; Linux</h3>
        <pre><code>curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | sh</code></pre>
        <p>Installs to <code>~/.local/bin</code> by default. Override with <code>MOSAIC_INSTALL_DIR</code>:</p>
        <pre><code>curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | MOSAIC_INSTALL_DIR=/usr/local/bin sh</code></pre>

        <h3 id="install-windows">Windows (PowerShell)</h3>
        <pre><code>irm https://mosaicvideo.github.io/mosaic/install.ps1 | iex</code></pre>
        <p>Installs to <code>%LOCALAPPDATA%\Programs\mosaic-cli</code> and adds it to user PATH. Restart your terminal afterward.</p>

        <h3 id="install-manual">Manual download</h3>
        <p>If you'd rather avoid the one-liner, grab a binary directly from the latest release:</p>
        <div class="dl-list" role="list">
          <a id="cli-btn-macos" href="https://github.com/mosaicvideo/mosaic/releases/latest" role="listitem" class="dl-row" data-key="macos">
            <span class="dl-label">macOS — universal</span>
            <span class="dl-path">mosaic-cli-macos-universal</span>
            <span class="dl-size">—</span>
          </a>
          <a id="cli-btn-win-x64" href="https://github.com/mosaicvideo/mosaic/releases/latest" role="listitem" class="dl-row" data-key="win-x64">
            <span class="dl-label">Windows — x64</span>
            <span class="dl-path">mosaic-cli-windows-x86_64.exe</span>
            <span class="dl-size">—</span>
          </a>
          <a id="cli-btn-win-arm" href="https://github.com/mosaicvideo/mosaic/releases/latest" role="listitem" class="dl-row" data-key="win-arm">
            <span class="dl-label">Windows — ARM64</span>
            <span class="dl-path">mosaic-cli-windows-aarch64.exe</span>
            <span class="dl-size">—</span>
          </a>
          <a id="cli-btn-linux" href="https://github.com/mosaicvideo/mosaic/releases/latest" role="listitem" class="dl-row" data-key="linux">
            <span class="dl-label">Linux — x86_64</span>
            <span class="dl-path">mosaic-cli-linux-x86_64</span>
            <span class="dl-size">—</span>
          </a>
        </div>
        <p>On macOS/Linux, make executable and move onto PATH:</p>
        <pre><code>chmod +x mosaic-cli-*
mv mosaic-cli-* ~/.local/bin/mosaic-cli</code></pre>

        <h3 id="install-verify">Verify checksum</h3>
        <p>Each release ships a <code>SHA256SUMS</code> file covering every CLI artifact. To verify:</p>
        <pre><code>curl -LO https://github.com/mosaicvideo/mosaic/releases/latest/download/SHA256SUMS
grep " mosaic-cli-macos-universal$" SHA256SUMS | shasum -a 256 -c -</code></pre>
        <p>Expected output: <code>mosaic-cli-macos-universal: OK</code>. The one-liner install scripts do this automatically.</p>

        <h2 id="requirements">Requirements</h2>
        <p><code>mosaic-cli</code> shells out to <code>ffmpeg</code>, <code>ffprobe</code>, and <code>mediainfo</code> — all three must be on your <code>PATH</code>. The macOS build of <code>ffmpeg</code> from Homebrew's default bottle is missing the <code>drawtext</code> filter; install <code>ffmpeg-full</code> instead.</p>
        <p>See <a href="guide.html#requirements">the GUI guide's Requirements section</a> for per-platform install commands.</p>

        <h2 id="quick-start">Quick start</h2>
        <pre><code># One still contact sheet
mosaic-cli sheet movie.mkv

# 12 screenshots into a specific folder
mosaic-cli screenshots --count 12 -o shots/ movie.mkv

# Animated preview reel: 10 clips at 2 seconds each
mosaic-cli reel --count 10 --clip-length 2 movie.mkv

# Animated contact sheet, 4×3 grid
mosaic-cli animated-sheet --cols 4 --rows 3 movie.mkv</code></pre>
        <p>Inputs can be files or directories. Directories are scanned recursively by default; pass <code>--no-recursive</code> to stay shallow. Stdout is paths-only so output pipes cleanly into <code>xargs</code>; progress and summaries go to stderr.</p>

        <h2 id="subcommands">Subcommands</h2>

        <h3 id="sub-screenshots"><code>screenshots</code></h3>
        <p>Capture individual frames from a video at evenly-spaced timestamps.</p>
        <pre><code>mosaic-cli screenshots [OPTIONS] &lt;INPUT&gt;...</code></pre>
        <p>Common flags:</p>
        <ul>
          <li><code>--count N</code> — number of frames (default: <strong>8</strong>)</li>
          <li><code>--format png|jpeg</code> — output format (default: <strong>png</strong>)</li>
          <li><code>--quality N</code> — JPEG quality 50–100 (default: <strong>92</strong>)</li>
          <li><code>--suffix S</code> — filename infix between stem and index (default: <strong><code>_screens_</code></strong>)</li>
          <li><code>-o DIR</code> — output directory (default: next to each source)</li>
        </ul>
        <pre><code>mosaic-cli screenshots --count 20 --format jpeg --quality 90 -o shots/ movie.mkv</code></pre>

        <h3 id="sub-sheet"><code>sheet</code></h3>
        <p>Generate a still contact sheet — a grid of thumbnails with an optional metadata header.</p>
        <pre><code>mosaic-cli sheet [OPTIONS] &lt;INPUT&gt;...</code></pre>
        <p>Common flags:</p>
        <ul>
          <li><code>--cols N</code> — columns (default: <strong>3</strong>)</li>
          <li><code>--rows N</code> — rows (default: <strong>6</strong>)</li>
          <li><code>--width PX</code> — total sheet width (default: <strong>1920</strong>)</li>
          <li><code>--gap PX</code> — thumbnail spacing (default: <strong>10</strong>)</li>
          <li><code>--format png|jpeg</code> — output format (default: <strong>png</strong>)</li>
          <li><code>--quality N</code> — JPEG quality 50–100 (default: <strong>92</strong>)</li>
          <li><code>--theme dark|light</code> — color theme (default: <strong>dark</strong>)</li>
          <li><code>--no-timestamps</code> / <code>--timestamps</code> — toggle per-thumbnail timestamp overlay</li>
          <li><code>--no-header</code> / <code>--header</code> — toggle the metadata header band</li>
          <li><code>--suffix S</code> — filename infix (default: <strong><code>_sheet</code></strong>)</li>
        </ul>
        <pre><code>mosaic-cli sheet --cols 4 --rows 5 --width 2400 --theme light movie.mkv</code></pre>

        <h3 id="sub-reel"><code>reel</code></h3>
        <p>Stitch short clips into a single animated preview reel (WebP/WebM/GIF).</p>
        <pre><code>mosaic-cli reel [OPTIONS] &lt;INPUT&gt;...</code></pre>
        <p>Common flags:</p>
        <ul>
          <li><code>--count N</code> — number of clips (default: <strong>15</strong>)</li>
          <li><code>--clip-length SECS</code> — seconds per clip (default: <strong>2</strong>)</li>
          <li><code>--height PX</code> — output height; width follows aspect ratio (default: <strong>360</strong>)</li>
          <li><code>--fps N</code> — frame rate, capped at source fps (default: <strong>24</strong>)</li>
          <li><code>--format webp|webm|gif</code> — output container (default: <strong>webp</strong>)</li>
          <li><code>--quality N</code> — encoder quality 0–100 (default: <strong>75</strong>; ignored for GIF)</li>
          <li><code>--suffix S</code> — filename infix (default: <strong><code>_reel</code></strong>)</li>
        </ul>
        <pre><code>mosaic-cli reel --count 8 --clip-length 3 --format gif movie.mkv</code></pre>

        <h3 id="sub-animated-sheet"><code>animated-sheet</code></h3>
        <p>Grid of animated clips — a contact sheet where every cell is a short looping WebP. Output is always WebP.</p>
        <pre><code>mosaic-cli animated-sheet [OPTIONS] &lt;INPUT&gt;...</code></pre>
        <p>Common flags:</p>
        <ul>
          <li><code>--cols N</code> — columns (default: <strong>3</strong>)</li>
          <li><code>--rows N</code> — rows (default: <strong>6</strong>)</li>
          <li><code>--width PX</code> — total sheet width (default: <strong>1280</strong>)</li>
          <li><code>--gap PX</code> — thumbnail spacing (default: <strong>8</strong>)</li>
          <li><code>--clip-length SECS</code> — seconds per animated cell (default: <strong>2</strong>)</li>
          <li><code>--fps N</code> — animated frame rate (default: <strong>12</strong>)</li>
          <li><code>--quality N</code> — WebP encoder quality (default: <strong>75</strong>)</li>
          <li><code>--theme dark|light</code> — color theme (default: <strong>dark</strong>)</li>
          <li><code>--suffix S</code> — filename infix (default: <strong><code>_animated_sheet</code></strong>)</li>
        </ul>

        <h3 id="sub-probe"><code>probe</code></h3>
        <p>Print the parsed ffprobe result as JSON. With <code>--mediainfo</code>, wraps both ffprobe and raw MediaInfo output in an envelope.</p>
        <pre><code>mosaic-cli probe [--mediainfo] &lt;INPUT&gt;</code></pre>
        <pre><code>mosaic-cli probe movie.mkv | jq .duration_secs
mosaic-cli probe --mediainfo movie.mkv | jq .ffprobe.video.color_transfer</code></pre>

        <h3 id="sub-completions"><code>completions</code></h3>
        <p>Emit a shell-completion script to stdout.</p>
        <pre><code>mosaic-cli completions &lt;bash|zsh|fish|powershell|elvish&gt;</code></pre>
        <p>See <a href="#completions">Shell completions</a> for setup instructions per shell.</p>

        <h3 id="sub-manpage"><code>manpage</code></h3>
        <p>Emit a roff-formatted man page to stdout.</p>
        <pre><code>mosaic-cli manpage</code></pre>
        <p>See <a href="#manpage">Man page</a> for install instructions.</p>

        <h2 id="config">Config file</h2>
        <p>On first run, <code>mosaic-cli</code> creates <code>~/.mosaic-cli.toml</code> with every option commented out. Uncomment any key to change its default. The full precedence is:</p>
        <ol>
          <li>Command-line flags (highest)</li>
          <li>Config file at <code>$MOSAIC_CLI_CONFIG</code> (if set) or <code>~/.mosaic-cli.toml</code></li>
          <li>Built-in defaults shown in each subcommand above</li>
        </ol>
        <p>Example config:</p>
        <pre><code>[sheet]
cols = 4
rows = 6
theme = "light"
suffix = "_thumbs"

[reel]
count = 12
clip_length_secs = 3
format = "gif"</code></pre>
        <p class="callout">Note: the config key for reel/animated-sheet clip duration is <code>clip_length_secs</code> (TOML convention, unit explicit), while the CLI flag is <code>--clip-length</code>. The difference is intentional.</p>

        <h2 id="completions">Shell completions</h2>

        <p><strong>zsh:</strong></p>
        <pre><code>mkdir -p ~/.zfunc
mosaic-cli completions zsh > ~/.zfunc/_mosaic-cli</code></pre>
        <p>Ensure these lines are in <code>~/.zshrc</code> before <code>compinit</code>:</p>
        <pre><code>fpath=(~/.zfunc $fpath)
autoload -Uz compinit && compinit</code></pre>

        <p><strong>bash:</strong></p>
        <pre><code>mkdir -p ~/.local/share/bash-completion/completions
mosaic-cli completions bash > ~/.local/share/bash-completion/completions/mosaic-cli</code></pre>

        <p><strong>fish:</strong></p>
        <pre><code>mkdir -p ~/.config/fish/completions
mosaic-cli completions fish > ~/.config/fish/completions/mosaic-cli.fish</code></pre>

        <p><strong>PowerShell:</strong></p>
        <pre><code># temporarily (current session only):
mosaic-cli completions powershell | Out-String | Invoke-Expression

# persistently (append to your profile):
mosaic-cli completions powershell | Out-String | Add-Content $PROFILE</code></pre>

        <h2 id="manpage">Man page</h2>
        <pre><code>mkdir -p ~/.local/share/man/man1
mosaic-cli manpage > ~/.local/share/man/man1/mosaic-cli.1
man mosaic-cli</code></pre>
        <p>If your <code>MANPATH</code> doesn't include <code>~/.local/share/man</code>, add it to your shell rc:</p>
        <pre><code>export MANPATH="$HOME/.local/share/man:$MANPATH"</code></pre>

        <h2 id="upgrading">Upgrading</h2>
        <p>Re-run the install script. The version is resolved at runtime, so the same one-liner always fetches the latest release.</p>

        <h2 id="uninstalling">Uninstalling</h2>
        <p><strong>macOS / Linux:</strong></p>
        <pre><code>rm ~/.local/bin/mosaic-cli
rm ~/.mosaic-cli.toml           # optional: config file</code></pre>
        <p><strong>Windows:</strong></p>
        <pre><code>Remove-Item "$env:LOCALAPPDATA\Programs\mosaic-cli" -Recurse</code></pre>
        <p>Remove the install dir from user PATH via <strong>Settings → System → About → Advanced system settings → Environment Variables</strong>.</p>

        <h2 id="troubleshooting">Troubleshooting</h2>

        <p><strong><code>ffmpeg not found on PATH</code></strong> — install <code>ffmpeg</code>, <code>ffprobe</code>, and <code>mediainfo</code>. See <a href="guide.html#requirements">Requirements</a>.</p>

        <p><strong>Gatekeeper blocks the macOS binary</strong> — shouldn't happen (the macOS CLI is signed and notarized with the same Developer ID as the GUI). If it does, clear the quarantine attribute:</p>
        <pre><code>xattr -d com.apple.quarantine ~/.local/bin/mosaic-cli</code></pre>

        <p><strong>SmartScreen warning on Windows</strong> — the Windows CLI is unsigned. If you downloaded manually via Explorer, Windows may mark the file. Run the one-liner installer instead (programmatic download avoids Mark-of-the-Web), or unblock via PowerShell:</p>
        <pre><code>Unblock-File -Path "$env:LOCALAPPDATA\Programs\mosaic-cli\mosaic-cli.exe"</code></pre>

        <p><strong>Arch mismatch error from install.sh</strong> — Linux aarch64 builds aren't published. Build from source with <code>cargo build --release --manifest-path mosaic-cli/Cargo.toml</code> or open an issue for a prebuilt.</p>

        <p><strong>Checksum mismatch</strong> — re-run the installer (network glitches can produce partial downloads). If it persists, file an issue and include the release tag + platform.</p>

        <p><strong>GitHub API rate limit from install.sh</strong> — unauthenticated requests are capped at 60/hour per IP. If you're behind a shared NAT and hit the limit, pin the version:</p>
        <pre><code>curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | MOSAIC_VERSION=v0.1.5 sh</code></pre>

      </article>
    </div>
  </div>
</main>

<footer class="footer">
  <div class="footer-inner">
    <span>mosaic — <a href="https://github.com/mosaicvideo/mosaic">github.com/mosaicvideo/mosaic</a></span>
    <span>MIT license</span>
  </div>
</footer>

<script src="assets/download.js" defer></script>
</body>
</html>
```

- [ ] **Step 2: Start the local site server and visually inspect**

Run: `cd site && python3 -m http.server 8088`
Then open <http://localhost:8088/cli.html> in a browser.

Expected:
- Page renders with nav, TOC sidebar, and full article.
- All in-page anchor links (`#install`, `#sub-sheet`, etc.) jump to the right headings.
- The manual-download table shows the four rows. `download.js` populates the version label + sizes once it hits the GitHub API (offline: rows stay static with placeholder `—`).
- Cross-links to `guide.html#requirements` navigate correctly if `guide.html` is present.

Stop the server with Ctrl+C.

- [ ] **Step 3: Commit**

```bash
git add site/cli.html
git commit -m "docs(site): add dedicated cli.html reference page

Covers install (one-liners, manual download, checksum verification),
requirements, quick-start recipes, per-subcommand flag reference,
config file, shell completions, man page, upgrading, uninstalling,
and troubleshooting. Mirrors guide.html layout primitives; reuses
download.js for release-asset label population."
```

---

## Task 10: Update `site/guide.html` — nav link, trim CLI section, remove stale snippet

**Files:**
- Modify: `site/guide.html`

- [ ] **Step 1: Add the `cli` nav link**

In `site/guide.html`, find the `<div class="nav-links">` block (around line 23-28). Replace:

```html
    <div class="nav-links">
      <a href="guide.html" class="active">guide</a>
      <a href="#faq">faq</a>
      <a href="https://github.com/mosaicvideo/mosaic/blob/main/CHANGELOG.md">log</a>
      <a href="https://github.com/mosaicvideo/mosaic">git</a>
    </div>
```

with:

```html
    <div class="nav-links">
      <a href="guide.html" class="active">guide</a>
      <a href="cli.html">cli</a>
      <a href="#faq">faq</a>
      <a href="https://github.com/mosaicvideo/mosaic/blob/main/CHANGELOG.md">log</a>
      <a href="https://github.com/mosaicvideo/mosaic">git</a>
    </div>
```

- [ ] **Step 2: Locate the `#cli` section**

Run: `grep -n 'id="cli"\|id="faq"' site/guide.html`
Expected: two line numbers — `id="cli"` opens the section, `id="faq"` opens the next one. Everything between them is the CLI section body.

- [ ] **Step 3: Replace the entire CLI section with a pointer paragraph**

In `site/guide.html`, replace everything from the line `<h2 id="cli">Command-line (CLI)</h2>` up to (but NOT including) `<h2 id="faq">FAQ</h2>` with:

```html
        <h2 id="cli">Command-line (CLI)</h2>
        <p>Mosaic ships a <code>mosaic-cli</code> binary for scripting, CI, and headless servers. Install with one line on any platform:</p>
        <pre><code># macOS / Linux
curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | sh

# Windows (PowerShell)
irm https://mosaicvideo.github.io/mosaic/install.ps1 | iex</code></pre>
        <p>Full subcommand reference, config file format, shell completions, and troubleshooting live on the <a href="cli.html">CLI page</a>.</p>

```

This trims the ~50-line section to ~10 lines and removes the stale `cd src-tauri && cargo install --path . --bin mosaic-cli --features cli` snippet (which was wrong — the CLI moved to a sibling crate).

- [ ] **Step 4: Reload the guide in the browser**

Run: `cd site && python3 -m http.server 8088`
Open <http://localhost:8088/guide.html>. Scroll to the CLI section. Verify:

- Nav shows the new `cli` link.
- `#cli` section is the short pointer with the two one-liners.
- The link to `cli.html` works.
- The FAQ that follows is unchanged.

Stop the server with Ctrl+C.

- [ ] **Step 5: Commit**

```bash
git add site/guide.html
git commit -m "docs(site): trim guide.html CLI section into a pointer to cli.html

Adds 'cli' to the top nav, replaces the in-page CLI reference with a
two-sentence pointer + one-liner install snippets, and removes the
stale 'cd src-tauri && cargo install ...' snippet (wrong since CLI
moved to the sibling crate)."
```

---

## Task 11: Update `site/index.html` — add `cli` nav link

**Files:**
- Modify: `site/index.html`

- [ ] **Step 1: Add the nav link**

In `site/index.html`, find the `<div class="nav-links">` block. Replace:

```html
    <div class="nav-links">
      <a href="guide.html">guide</a>
      <a href="guide.html#faq">faq</a>
      <a href="https://github.com/mosaicvideo/mosaic/blob/main/CHANGELOG.md">log</a>
      <a href="https://github.com/mosaicvideo/mosaic">git</a>
    </div>
```

with:

```html
    <div class="nav-links">
      <a href="guide.html">guide</a>
      <a href="cli.html">cli</a>
      <a href="guide.html#faq">faq</a>
      <a href="https://github.com/mosaicvideo/mosaic/blob/main/CHANGELOG.md">log</a>
      <a href="https://github.com/mosaicvideo/mosaic">git</a>
    </div>
```

- [ ] **Step 2: Visual check**

Run: `cd site && python3 -m http.server 8088`
Open <http://localhost:8088/>. Verify the nav shows `guide | cli | faq | log | git`. Click `cli` — lands on `cli.html`. Stop the server.

- [ ] **Step 3: Commit**

```bash
git add site/index.html
git commit -m "docs(site): add cli link to index.html top nav"
```

---

## Task 12: Rewrite the `Command-line usage` section in `README.md`

**Files:**
- Modify: `README.md:87-107` (the existing `## Command-line usage` section)

README already has a CLI section (lines 87-107), but it contains the same stale `cd src-tauri && cargo install --path . --bin mosaic-cli --features cli` snippet we removed from the site. Replace the section entirely.

- [ ] **Step 1: Confirm the current section content**

Run: `sed -n '87,107p' README.md`
Expected: shows the current `## Command-line usage` block, including the stale `cargo install` snippet at lines 91-92 and the subcommands list.

- [ ] **Step 2: Replace the section**

Replace everything from the line `## Command-line usage` up to (but NOT including) the next `## Docs` heading with:

```markdown
## Command-line usage

In addition to the desktop app, mosaic ships a `mosaic-cli` binary for scripts, CI pipelines, and headless servers — same ffmpeg pipelines, no UI. Install in one line:

```sh
# macOS / Linux
curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | sh

# Windows (PowerShell)
irm https://mosaicvideo.github.io/mosaic/install.ps1 | iex
```

Or grab a `mosaic-cli-*` binary directly from the [latest release](https://github.com/mosaicvideo/mosaic/releases/latest) — every release ships a `SHA256SUMS` file for verification.

Full subcommand reference, flag defaults, config file format, shell completions, and troubleshooting live on the [CLI page](https://mosaicvideo.github.io/mosaic/cli.html).

```

This removes the stale `cd src-tauri && cargo install ...` snippet, the duplicated subcommand list (now canonical on `cli.html`), and the config-file paragraph (also moved to `cli.html`). Contributors who want to build from source find instructions in the "Requirements (dev)" / "Run" / "Build" sections that already exist above.

- [ ] **Step 3: Verify the section reads cleanly**

Run: `awk '/^## Command-line usage$/,/^## Docs$/' README.md`
Expected: shows the new section ending with "CLI page](...)." followed by the `## Docs` heading.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs(readme): rewrite CLI section around install one-liners

Drops the stale 'cd src-tauri && cargo install --path . --bin
mosaic-cli --features cli' snippet (wrong since CLI moved to a
sibling crate) and the duplicated subcommand list. Points at
cli.html for the full reference."
```

---

## Task 13: Update `CLAUDE.md` — note install scripts and runtime subcommands

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Locate the CLI binary section**

Run: `grep -n '^## CLI binary' CLAUDE.md`
Expected: one line number pointing at the section heading.

- [ ] **Step 2: Append install-scripts note and runtime-subcommands note**

At the end of the "## CLI binary" section (immediately before the next `## ` heading), append these two bullets, matching the existing bullet style:

```markdown
- **Install scripts.** `site/install.sh` (POSIX sh, macOS + Linux) and `site/install.ps1` (Windows PowerShell) are static scripts served via the existing `pages.yml` GitHub Pages deployment. They resolve the latest release tag via the GitHub API, download the matching `mosaic-cli-*` artifact and `SHA256SUMS`, verify the checksum, and install to a user-scoped directory. No re-deploy needed per release — the scripts don't bake version numbers. Lint with `shellcheck site/install.sh` locally; CI runs this on every push via `.github/workflows/ci.yml`.
- **Shell completions + man page.** Runtime clap introspection via `mosaic-cli completions <shell>` and `mosaic-cli manpage`. No build-time assets, no release-asset proliferation — the binary generates its own completion script and roff man page on demand. Both subcommands short-circuit before config load and tool probe in `main.rs`, so they work on a fresh install without `~/.mosaic-cli.toml` or ffmpeg present. Deps: `clap_complete`, `clap_mangen` in `mosaic-cli/Cargo.toml`.
```

- [ ] **Step 3: Add SHA256SUMS note to the Releasing section**

Run: `grep -n '^## Releasing' CLAUDE.md`
Expected: one line number.

In the "## Releasing" section, find the sentence starting `CI builds four platform artifacts`. Append to that paragraph (or create a new line after it):

> Each CLI artifact is accompanied by a `.sha256` file; a `finalize` job in `release.yml` aggregates them into a single `SHA256SUMS` release asset so users can verify one-line downloads.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude): document install scripts, runtime subcommands, SHA256SUMS"
```

---

## Task 14: Add `[Unreleased]` entries to `CHANGELOG.md`

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Read the current changelog structure**

Run: `head -30 CHANGELOG.md`
Expected: shows the `[Unreleased]` section at the top (or the most recent release). If `[Unreleased]` doesn't exist yet, add it above the latest release section.

- [ ] **Step 2: Add entries under `[Unreleased]`**

Under the `[Unreleased]` heading, add these bullets under an `### Added` subheading (create if absent):

```markdown
### Added

- **One-liner install scripts for `mosaic-cli`** (`site/install.sh`, `site/install.ps1`) served via GitHub Pages. Detect OS/arch, resolve the latest release via the GitHub API, download + SHA256-verify the matching binary, install to a user-scoped directory (`~/.local/bin` or `%LOCALAPPDATA%\Programs\mosaic-cli`), and print PATH + completions hints. Usage: `curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh | sh` on macOS/Linux, `irm https://mosaicvideo.github.io/mosaic/install.ps1 | iex` on Windows.
- **`SHA256SUMS` release artifact** covering every `mosaic-cli-*` binary. Aggregated per-release in CI from per-artifact `.sha256` files uploaded by each matrix runner. Verify manually with `shasum -a 256 -c SHA256SUMS`.
- **`mosaic-cli completions <shell>`** — emit a shell-completion script to stdout. Supports bash, zsh, fish, powershell, and elvish via `clap_complete`.
- **`mosaic-cli manpage`** — emit a roff man page to stdout via `clap_mangen`. Install with `mosaic-cli manpage > ~/.local/share/man/man1/mosaic-cli.1`.
- **Dedicated `cli.html` reference page** on the showcase site covering install, subcommand flag reference (with defaults pulled from `defaults.rs`), config file, shell completions, man page, upgrading, uninstalling, and troubleshooting. Linked from top nav on every site page.
```

Under a `### Fixed` subheading (create if absent):

```markdown
### Fixed

- **Stale `cargo install` snippet in the guide.** The build-from-source instruction in `guide.html#cli` pointed at `cd src-tauri && cargo install --path . --bin mosaic-cli --features cli` — wrong since the CLI moved to the sibling `mosaic-cli/` crate. The CLI section now points to `cli.html`; build-from-source instructions live in `README.md` and are accurate.
```

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): add Unreleased entries for CLI distribution pass"
```

---

## Task 15: Local end-to-end smoke test

**Files:** none modified — this is a verification pass.

- [ ] **Step 1: Build the CLI from source**

Run: `cargo build --release --manifest-path mosaic-cli/Cargo.toml`
Expected: exits 0.

- [ ] **Step 2: Check all new subcommands produce expected output**

Run each and confirm output shape:

```bash
./mosaic-cli/target/release/mosaic-cli --help | tail -5
./mosaic-cli/target/release/mosaic-cli completions zsh | head -3
./mosaic-cli/target/release/mosaic-cli completions bash | grep -c "complete -F"
./mosaic-cli/target/release/mosaic-cli completions fish | head -3
./mosaic-cli/target/release/mosaic-cli completions powershell | head -3
./mosaic-cli/target/release/mosaic-cli manpage | grep -c "\.TH"
```

Expected:
- `--help` footer ends with the `cli.html` URL.
- zsh output starts with `#compdef`.
- bash grep returns a count ≥ 1.
- fish output starts with `complete -c mosaic-cli`.
- powershell output contains `Register-ArgumentCompleter`.
- manpage grep returns a count ≥ 1.

- [ ] **Step 3: Full test suite + clippy**

Run: `PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --manifest-path mosaic-cli/Cargo.toml`
Expected: all tests pass.

Run: `cargo clippy --manifest-path mosaic-cli/Cargo.toml --all-targets -- -D warnings`
Expected: exits 0.

Run (Tauri crate, with features): `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features test-api,cli -- -D warnings`
Expected: exits 0.

Run (Tauri crate, plain GUI-only, per CLAUDE.md): `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
Expected: exits 0.

- [ ] **Step 4: Shellcheck**

Run: `shellcheck site/install.sh` (skip if shellcheck missing locally — CI covers it).
Expected: no warnings.

- [ ] **Step 5: Render and click-check the site locally**

Run: `cd site && python3 -m http.server 8088`
Open each page and click every nav link:

- <http://localhost:8088/> — hero, nav, CLI tile if present.
- <http://localhost:8088/guide.html> — nav has `cli`, `#cli` section is the short pointer, FAQ below unchanged.
- <http://localhost:8088/cli.html> — every TOC anchor works, manual-download table present, all code blocks render.

Stop server with Ctrl+C.

- [ ] **Step 6: Offline install-script dry-run**

Run on macOS or Linux:

```bash
mkdir -p /tmp/mosaic-install-test
MOSAIC_INSTALL_DIR=/tmp/mosaic-install-test sh site/install.sh
/tmp/mosaic-install-test/mosaic-cli --version
/tmp/mosaic-install-test/mosaic-cli completions zsh | head -1
```

Expected:
- Script downloads the latest release, verifies checksum, installs to the override dir.
- `--version` prints `mosaic-cli X.Y.Z`.
- zsh completions output begins with `#compdef`.

Clean up: `rm -rf /tmp/mosaic-install-test`.

**Note:** this test requires a published release with `SHA256SUMS` to exist. If running before the first such release ships, skip this step and rely on the RC-tag verification in the rollout (Section "Rollout" of the spec).

- [ ] **Step 7: No commit for this task** — it's verification, not code change.

---

## Post-plan: push + tag an RC

Once Tasks 1-15 are all on `main` (or merged via PR), cut a release candidate to exercise the CI changes end-to-end:

```bash
git tag v0.1.5-rc1
git push origin v0.1.5-rc1
```

Watch the `release.yml` run complete. Verify in the resulting draft release:

- Every existing artifact is still present (GUI bundles + `latest.json`).
- Each `mosaic-cli-*` artifact has a matching `mosaic-cli-*.sha256` file.
- A single `SHA256SUMS` file sits at the top level.
- `cat SHA256SUMS` (via `gh release view`) shows one line per CLI artifact, sorted.

Then smoke-test the one-liner against the RC tag:

```bash
MOSAIC_VERSION=v0.1.5-rc1 sh <(curl -LsSf https://mosaicvideo.github.io/mosaic/install.sh)
```

…repeating across the matrix listed in the spec's Testing section (macOS arm64, macOS x86_64, Ubuntu 22.04 x86_64, Windows 11 x86_64). If all pass, tag `v0.1.5` final.

---

## Self-review notes (for the implementer)

- If a step's "Run" command fails unexpectedly, STOP. Don't improvise — check against the spec (`docs/superpowers/specs/2026-04-19-mosaic-cli-distribution-design.md`) or re-read the relevant task.
- All clippy runs must pass with `-D warnings`. Fix warnings; don't silence them.
- Do not amend prior commits — each task is its own commit.
- Skip `CHANGELOG.md` entries only if the user has indicated they're tracking changelog differently (default is to keep them).
