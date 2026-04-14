# Mosaic Packaging & Distribution — Plan

**Date:** 2026-04-14
**Status:** Draft, not yet implemented

## Goal

Automate multi-platform builds and GitHub releases: tag `v*` → CI produces signed macOS `.dmg`, Windows `.exe`/`.msi`, Linux `.deb`/`.AppImage`, attaches them to a draft release. Users download, double-click, app runs.

## Distribution targets (priority order)

1. **macOS universal (arm64 + x86_64)** — primary dev platform, where most testing happens. `.dmg` containing the `.app`.
2. **Windows x86_64** — `.exe` (NSIS installer). `.msi` optional later.
3. **Linux x86_64** — `.AppImage` (most portable) + `.deb` (Debian/Ubuntu).

CLI binaries (from the CLI plan — the binary is named `mosaic`) ride along in the same release with a target suffix: `mosaic-macos-arm64`, `mosaic-windows-x86_64.exe`, `mosaic-linux-x86_64`, etc.

## Pre-flight

### Icons

Source a 1024×1024 PNG (solid background, bold mark). Then:

```
pnpm tauri icon path/to/mosaic-source.png
```

Regenerates:
- `src-tauri/icons/icon.icns` (macOS)
- `src-tauri/icons/icon.ico` (Windows)
- Multiple PNG sizes for Linux

Update `tauri.conf.json` `bundle.icon` to reference them. Replace the 104-byte placeholder committed in Task 1.

### Version source of truth

Three files carry versions: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`. Keep in sync via:

- **Manual** — bump all three, commit, tag. Simple, error-prone.
- **Script** — `pnpm version <new>` updates `package.json`; a small Node/shell wrapper also rewrites Cargo.toml and tauri.conf.json. Commit + tag.
- **release-please** — GitHub-native conventional-commit-driven release bot. Opens a PR that bumps versions and generates CHANGELOG. Merge → tag auto-pushed. Recommended once the project stabilizes.

v1 choice: manual script (`scripts/bump-version.mjs`). Move to release-please after a few releases land cleanly.

### Identifier + metadata

Already set: `identifier: com.mosaic.app`, `productName: Mosaic`. Before first public release:

- Pick a different identifier if we ever want to ship in the Mac App Store (must match developer ID).
- Add `homepage`, `description`, `license`, `authors` to `Cargo.toml` — used by Linux package managers.
- Add `shortDescription` and `longDescription` to `tauri.conf.json` `bundle` — shown in installers.

## GitHub Actions release workflow

Single workflow file, matrix build.

```yaml
name: release
on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          - { os: macos-latest,  target: aarch64-apple-darwin,        rust-targets: 'aarch64-apple-darwin,x86_64-apple-darwin' }
          - { os: windows-latest, target: x86_64-pc-windows-msvc,     rust-targets: 'x86_64-pc-windows-msvc' }
          - { os: ubuntu-22.04,   target: x86_64-unknown-linux-gnu,   rust-targets: 'x86_64-unknown-linux-gnu' }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: ${{ matrix.rust-targets }} }
      - uses: pnpm/action-setup@v4
      - uses: actions/setup-node@v4
        with: { node-version: 20, cache: 'pnpm' }
      - run: pnpm install --frozen-lockfile
      - name: Linux system deps
        if: matrix.os == 'ubuntu-22.04'
        run: sudo apt-get update && sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          # signing secrets (see below)
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "Mosaic ${{ github.ref_name }}"
          releaseBody: "See CHANGELOG.md"
          releaseDraft: true
          prerelease: false
          args: ${{ matrix.os == 'macos-latest' && '--target universal-apple-darwin' || '' }}
```

`tauri-apps/tauri-action` creates the draft release on first run and uploads artifacts from every matrix job to it. Review the draft, edit notes, click Publish.

Universal macOS build: pass `--target universal-apple-darwin` so the `.dmg` runs on both Apple Silicon and Intel Macs.

## Code signing

### macOS (required for frictionless distribution)

Without signing + notarization, users get *"Mosaic.app can't be opened because Apple cannot check it for malicious software"* and must right-click → Open to bypass. Acceptable for a v0 release; annoying forever.

Requires:
- **Apple Developer Program membership** — $99/year.
- **Developer ID Application certificate** exported as `.p12` (base64-encode for the secret).
- App-specific password for notarytool.

Secrets in GitHub:

| Secret | From |
|---|---|
| `APPLE_CERTIFICATE` | `.p12` base64 |
| `APPLE_CERTIFICATE_PASSWORD` | `.p12` export password |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID` | Apple ID email |
| `APPLE_PASSWORD` | app-specific password |
| `APPLE_TEAM_ID` | 10-char team ID |

`tauri-action` picks these up automatically.

### Windows

Without signing, SmartScreen shows a "Windows protected your PC" popup that users must click through. Certificate options:

- **Standard Authenticode** — ~$200-400/year from DigiCert/Sectigo/SSL.com. Requires hardware token (YubiKey or cloud HSM) since 2023.
- **SignPath.io free tier** — free for open-source, signs via CI without requiring cert possession.
- **Extended Validation (EV)** — instant SmartScreen reputation, ~$400-600/year, also hardware-token.

Secrets:

| Secret | Notes |
|---|---|
| `WINDOWS_CERTIFICATE` | `.pfx` base64 |
| `WINDOWS_CERTIFICATE_PASSWORD` | |

Skip for v0; revisit when Windows becomes a priority platform.

### Linux

No signing. `.AppImage` is a single file; `.deb` users `apt install ./mosaic.deb`.

## ffmpeg distribution strategy

Biggest unresolved question. Current state: users install ffmpeg themselves; the app shows a banner if missing. For a general-audience release that's a showstopper.

**Option 1 — keep it, document it.** README-level instructions per OS. Zero bundle overhead. Requires a technical user.

**Option 2 — sidecar bundle.** Declare platform-specific ffmpeg binaries in `tauri.conf.json`:

```json
"bundle": {
  "externalBin": ["binaries/ffmpeg", "binaries/ffprobe"]
}
```

Tauri looks for `binaries/ffmpeg-<target-triple>` (e.g. `ffmpeg-aarch64-apple-darwin`) at build time and bundles them. `locate_tools()` gains a first-priority check for `app_handle.path().resolve("binaries/ffmpeg", Resource)`.

Per-platform sources:
- macOS: `ffmpeg-full` from Homebrew — but it's GPL'd and keg-only; for distribution, use the **Evermeet.cx** static build or build-from-source. Both include drawtext.
- Windows: **BtbN** GPL full builds (`ffmpeg-master-latest-win64-gpl.zip`).
- Linux: **John Van Sickle** static GPL build (`ffmpeg-release-amd64-static.tar.xz`).

Download script in CI fetches these into `src-tauri/binaries/` before `tauri build`. Bundle size grows by ~60-80 MB per platform.

**Licensing note:** shipping GPL ffmpeg means Mosaic's binary distribution becomes GPL-licensed. LGPL builds exist but lack libx264/libx265 — fine for our use (we don't encode video, just extract frames).

**Recommendation:** v0 ships with Option 1 + clear docs. v1 adds Option 2 with LGPL builds after confirming drawtext is present.

## Auto-updater (post-v1)

Tauri has `tauri-plugin-updater`. Needs:

1. A signing keypair (`tauri signer generate`).
2. Public key embedded in `tauri.conf.json`.
3. A versioned JSON manifest hosted somewhere stable:
   ```json
   {
     "version": "1.2.0",
     "notes": "Fixes drawtext escape for commas",
     "pub_date": "2026-05-01T12:00:00Z",
     "platforms": {
       "darwin-aarch64": { "signature": "...", "url": "https://github.com/you/mosaic/releases/download/v1.2.0/Mosaic_universal.app.tar.gz" }
     }
   }
   ```

GitHub Releases can serve as host via a stable URL like `https://github.com/you/mosaic/releases/latest/download/latest.json`. CI generates and uploads this JSON alongside the artifacts.

Skip until after the first couple of manual releases prove the flow works.

## Release cadence + CHANGELOG

Keep a `CHANGELOG.md` using Keep-a-Changelog format. Every PR that changes behaviour adds an entry. Version bump = cut release notes from the `[Unreleased]` section.

Conventional Commits enable automation (release-please) later if we want to switch from manual.

## Task sketch (for execution later)

1. Generate real icons, replace placeholder.
2. Add version-bump script.
3. Write `.github/workflows/release.yml` with the matrix above.
4. First test release: push a `v0.1.0-rc1` tag, verify unsigned builds land in a draft release on each platform.
5. Install unsigned artifacts on each target OS; verify the app launches and basic workflows run.
6. Document the ffmpeg requirement prominently in README.
7. (Optional, when ready) Add macOS signing + notarization.
8. (Optional) Add sidecar ffmpeg bundling.
9. (Later) Add `tauri-plugin-updater` + release workflow step to publish `latest.json`.
10. (Later) Windows signing, Linux repo submissions (Flathub, AUR).
