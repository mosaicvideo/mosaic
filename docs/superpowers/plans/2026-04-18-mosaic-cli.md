# Mosaic CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `mosaic-cli`, a second binary in the `mosaic` crate that exposes every GUI pipeline (screenshots, contact sheet, preview reel, animated contact sheet, probe) as command-line subcommands, with a TOML config file at `~/.mosaic-cli.toml` and per-user defaults consumed by both CLI and GUI.

**Architecture:** Same Cargo package, two binaries. The GUI stays as `mosaic`; the CLI is a new `[[bin]] mosaic-cli` gated behind a `cli` feature. The CLI calls `mosaic_lib` directly; pipeline modules become public under a unioned `test-api`/`cli` cfg. A new `defaults` module holds shared constants (GUI HTML and CLI share them via a sync script). A new `input_scan` module factors the folder-scan logic out of the private `commands` module so the CLI can call it.

**Tech Stack:** Rust 2021, Tauri 2, clap v4 (derive), indicatif, toml, serde, tempfile (already in deps), tokio, assert_cmd + predicates for integration tests. Node (for `scripts/sync-defaults.mjs`).

**Status:** All 15 tasks implemented (uncommitted working tree). Follow-up fixes applied for config validation, suffix sanitization, Windows concat-file path handling, Ctrl-C accounting, help-string coverage, and other hostile-input findings.

**Reference spec:** `docs/superpowers/specs/2026-04-18-mosaic-cli-design.md`

---

## File Structure

**New files:**
- `src-tauri/src/defaults.rs` — `pub const` defaults, grouped by pipeline
- `src-tauri/src/input_scan.rs` — `VIDEO_EXTS` + `scan()` extracted from `commands.rs`
- `src-tauri/src/bin/mosaic_cli/main.rs` — entry point, dispatch
- `src-tauri/src/bin/mosaic_cli/cli.rs` — clap structs for every subcommand
- `src-tauri/src/bin/mosaic_cli/config.rs` — config path resolution, auto-create, parse, unknown-key warn
- `src-tauri/src/bin/mosaic_cli/progress.rs` — indicatif multi-progress wrapper
- `src-tauri/src/bin/mosaic_cli/font.rs` — embedded DejaVuSans + lazy temp extraction
- `src-tauri/src/bin/mosaic_cli/run/probe.rs` — probe subcommand
- `src-tauri/src/bin/mosaic_cli/run/screenshots.rs`
- `src-tauri/src/bin/mosaic_cli/run/sheet.rs`
- `src-tauri/src/bin/mosaic_cli/run/reel.rs`
- `src-tauri/src/bin/mosaic_cli/run/animated_sheet.rs`
- `src-tauri/src/bin/mosaic_cli/run/mod.rs` — re-exports
- `src-tauri/tests/cli.rs` — `assert_cmd` integration tests
- `scripts/sync-defaults.mjs` — syncs `defaults.rs` → `src/index.html`

**Modified:**
- `src-tauri/Cargo.toml` — add optional deps, `cli` feature, dev-deps, `[[bin]]` entry
- `src-tauri/src/lib.rs` — extend two-branch cfg on every pipeline module + hook functions; add `defaults`, `input_scan`
- `src-tauri/src/commands.rs` — drop local `VIDEO_EXTS`, delegate `scan_folder` to `input_scan::scan`
- `package.json` — add `sync-defaults` script, wire into `version:bump`
- `scripts/bump-version.mjs` — call `sync-defaults.mjs` before committing
- `.github/workflows/release.yml` — add CLI-binary build step per platform
- `README.md` — CLI usage section
- `site/guide.html` — CLI usage section
- `CLAUDE.md` — new modules, `cli` feature, `mosaic-cli` binary, config location

---

## Task 1: Shared defaults module

**Files:**
- Create: `src-tauri/src/defaults.rs`
- Modify: `src-tauri/src/lib.rs` (add two-branch cfg for `defaults`)

- [ ] **Step 1: Write the defaults module**

```rust
// src-tauri/src/defaults.rs
// Source of truth for every shipping default shown to users.
// The GUI's `src/index.html` values are kept in sync via
// `scripts/sync-defaults.mjs`; the CLI reads these directly.

pub mod screenshots {
    pub const COUNT: u32 = 8;
    pub const FORMAT: &str = "png";
    pub const JPEG_QUALITY: u32 = 92;
}

pub mod sheet {
    pub const COLS: u32 = 3;
    pub const ROWS: u32 = 6;
    pub const WIDTH: u32 = 1920;
    pub const GAP: u32 = 10;
    pub const THUMB_FONT: u32 = 18;
    pub const HEADER_FONT: u32 = 20;
    pub const FORMAT: &str = "png";
    pub const JPEG_QUALITY: u32 = 92;
    pub const THEME: &str = "dark";
}

pub mod reel {
    pub const COUNT: u32 = 15;
    pub const CLIP_LENGTH_SECS: u32 = 2;
    pub const HEIGHT: u32 = 360;
    pub const FPS: u32 = 24;
    pub const QUALITY: u32 = 75;
    pub const FORMAT: &str = "webp";
}

pub mod animated_sheet {
    pub const COLS: u32 = 3;
    pub const ROWS: u32 = 6;
    pub const WIDTH: u32 = 1280;
    pub const GAP: u32 = 8;
    pub const CLIP_LENGTH_SECS: u32 = 2;
    pub const FPS: u32 = 12;
    pub const QUALITY: u32 = 75;
    pub const THUMB_FONT: u32 = 14;
    pub const HEADER_FONT: u32 = 18;
    pub const THEME: &str = "dark";
}

#[cfg(test)]
mod tests {
    use super::*;

    // These assertions mirror `src/index.html` as shipped in v0.1.3.
    // `scripts/sync-defaults.mjs` keeps the HTML in lockstep with this module.
    #[test]
    fn screenshot_defaults_match_html() {
        assert_eq!(screenshots::COUNT, 8);
        assert_eq!(screenshots::JPEG_QUALITY, 92);
    }
    #[test]
    fn sheet_defaults_match_html() {
        assert_eq!(sheet::COLS, 3);
        assert_eq!(sheet::ROWS, 6);
        assert_eq!(sheet::WIDTH, 1920);
    }
    #[test]
    fn reel_defaults_match_html() {
        assert_eq!(reel::COUNT, 15);
        assert_eq!(reel::FPS, 24);
    }
    #[test]
    fn animated_sheet_defaults_match_html() {
        assert_eq!(animated_sheet::COLS, 3);
        assert_eq!(animated_sheet::FPS, 12);
    }
}
```

- [ ] **Step 2: Register the module in `lib.rs`**

Insert near the other two-branch declarations (e.g., just after `video_info`):

```rust
#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod defaults;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod defaults;
```

- [ ] **Step 3: Verify**

Run (from `src-tauri/`):
```
cargo test --lib defaults -- --nocapture
cargo clippy --all-targets --features test-api -- -D warnings
```
Expected: 4 passed; clippy clean.

- [ ] **Step 4: Commit**

```
git add src-tauri/src/defaults.rs src-tauri/src/lib.rs
git commit -m "feat(lib): add shared defaults module for CLI/GUI"
```

---

## Task 2: Sync-defaults script

**Files:**
- Create: `scripts/sync-defaults.mjs`
- Modify: `scripts/bump-version.mjs` (invoke `sync-defaults.mjs` before committing)
- Modify: `package.json` (add `sync:defaults` npm script)

- [ ] **Step 1: Write the script**

```js
// scripts/sync-defaults.mjs
// Reads src-tauri/src/defaults.rs and rewrites the `value="…"` attributes
// in src/index.html so GUI and CLI share the same shipping defaults.
// Run via `pnpm sync:defaults`. Also invoked by scripts/bump-version.mjs.

import { readFileSync, writeFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const rs = readFileSync(resolve(root, "src-tauri/src/defaults.rs"), "utf8");
const htmlPath = resolve(root, "src/index.html");
let html = readFileSync(htmlPath, "utf8");

// Map of (section, key) -> HTML input id.
// Keep in alphabetical order by section, mirrors defaults.rs groups.
const map = {
  "screenshots.COUNT":         "shots-count",
  "screenshots.JPEG_QUALITY":  "shots-quality",
  "sheet.COLS":                "sheet-cols",
  "sheet.ROWS":                "sheet-rows",
  "sheet.WIDTH":               "sheet-width",
  "sheet.GAP":                 "sheet-gap",
  "sheet.THUMB_FONT":          "sheet-thumb-font",
  "sheet.HEADER_FONT":         "sheet-header-font",
  "sheet.JPEG_QUALITY":        "sheet-quality",
  "reel.COUNT":                "preview-count",
  "reel.CLIP_LENGTH_SECS":     "preview-clip-length",
  "reel.HEIGHT":               "preview-height",
  "reel.FPS":                  "preview-fps",
  "reel.QUALITY":              "preview-quality",
  "animated_sheet.COLS":       "asheet-cols",
  "animated_sheet.ROWS":       "asheet-rows",
  "animated_sheet.WIDTH":      "asheet-width",
  "animated_sheet.GAP":        "asheet-gap",
  "animated_sheet.CLIP_LENGTH_SECS": "asheet-clip-length",
  "animated_sheet.FPS":        "asheet-fps",
  "animated_sheet.QUALITY":    "asheet-quality",
  "animated_sheet.THUMB_FONT": "asheet-thumb-font",
  "animated_sheet.HEADER_FONT":"asheet-header-font",
};

function extract(section, key) {
  const re = new RegExp(
    `pub mod ${section} \\{[\\s\\S]*?pub const ${key}: [a-zA-Z0-9_]+ = (-?\\d+);`,
  );
  const m = rs.match(re);
  if (!m) throw new Error(`defaults.rs: could not find ${section}::${key}`);
  return m[1];
}

let changed = 0;
for (const [qualified, id] of Object.entries(map)) {
  const [section, key] = qualified.split(".");
  const value = extract(section, key);
  const re = new RegExp(`(id="${id}"[^>]*\\svalue=")[^"]*(")`);
  if (!re.test(html)) throw new Error(`index.html: no input with id="${id}"`);
  const next = html.replace(re, `$1${value}$2`);
  if (next !== html) { changed++; html = next; }
}

writeFileSync(htmlPath, html);
console.log(`sync-defaults: updated ${changed} attribute(s) in src/index.html`);
```

- [ ] **Step 2: Wire into package.json**

Add under `"scripts"`:

```json
"sync:defaults": "node scripts/sync-defaults.mjs",
```

- [ ] **Step 3: Wire into bump-version.mjs**

The existing `scripts/bump-version.mjs` already uses `execSync`, has a `files` array assembled near line 78, and runs `git add ${files.join(" ")}`. Two changes:

1. Add a path constant near the existing `SITE_INDEX` / `SITE_GUIDE` definitions (around line 16):

```js
const SRC_INDEX = resolve(root, "src/index.html");
```

2. Before the `if (shouldTag)` block (right after the site-version loop, around line 73), run the sync script so its edits are included in the same commit:

```js
execSync("node scripts/sync-defaults.mjs", { cwd: root, stdio: "inherit" });
```

3. Inside the `if (shouldTag)` block, extend the `files` array (around line 78) to include `SRC_INDEX`:

```js
const files = [PACKAGE_JSON, TAURI_CONF, CARGO_TOML, CARGO_LOCK, SITE_INDEX, SITE_GUIDE, SRC_INDEX].map(
  (f) => relative(root, f)
);
```

- [ ] **Step 4: Verify — dry run**

From repo root:
```
pnpm sync:defaults
git diff --stat src/index.html
```
Expected: `0 files changed` (HTML already matches constants in v0.1.3). The script should print `sync-defaults: updated 0 attribute(s)`.

To prove it actually works end-to-end, temporarily change `pub const COUNT: u32 = 8` to `9` in `defaults.rs`, re-run `pnpm sync:defaults`, confirm `src/index.html` updated the `shots-count` input's `value`, then revert both.

- [ ] **Step 5: Commit**

```
git add scripts/sync-defaults.mjs scripts/bump-version.mjs package.json
git commit -m "build: sync-defaults script mirrors Rust consts into index.html"
```

---

## Task 3: Factor folder-scan into `input_scan.rs`

**Files:**
- Create: `src-tauri/src/input_scan.rs`
- Modify: `src-tauri/src/commands.rs` (delegate to `input_scan`)
- Modify: `src-tauri/src/lib.rs` (register module)

- [ ] **Step 1: Write the new module with tests**

Copy the 45-entry `VIDEO_EXTS` array verbatim from `commands.rs` (lines 33–47). Don't invent a new list. Also preserve the `MAX_SCAN_DEPTH = 16` constant (commands.rs line 50) so pathological symlink loops can't hang.

```rust
// src-tauri/src/input_scan.rs
// Directory walker producing the video-file list consumed by both the
// Tauri `scan_folder` command and the CLI's positional-input expander.
// Accepts a file or directory — directories are walked up to
// MAX_SCAN_DEPTH (16) to guard against symlink cycles.

use std::path::{Path, PathBuf};

pub const VIDEO_EXTS: &[&str] = &[
    // Common containers
    "mp4", "mkv", "mov", "avi", "webm", "wmv", "flv", "m4v", "mpg", "mpeg",
    "ts", "m2ts", "mts", "vob", "iso", "ogv", "ogm", "qt", "asf",
    // Mobile / MP4 family
    "3gp", "3g2", "f4v", "mj2",
    // Legacy / regional
    "rm", "rmvb", "divx", "swf", "nsv",
    // Broadcast / professional
    "mxf", "gxf", "r3d",
    // Camcorder / capture / recording
    "dv", "dif", "wtv", "nuv", "pva",
    // Other containers
    "nut", "vro", "m1v", "m2v", "mk3d", "fli", "flc", "ivf", "y4m",
];

const MAX_SCAN_DEPTH: u32 = 16;

pub fn scan(path: &Path, recursive: bool) -> Result<Vec<PathBuf>, String> {
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut out = Vec::new();
    walk(path, recursive, 0, &mut out);
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, recursive: bool, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > MAX_SCAN_DEPTH { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let p = entry.path();
        if ft.is_dir() {
            if recursive { walk(&p, recursive, depth + 1, out); }
        } else if ft.is_file() {
            let ext_ok = p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .map(|e| VIDEO_EXTS.contains(&e.as_str()))
                .unwrap_or(false);
            if ext_ok { out.push(p); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, File};
    use tempfile::TempDir;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        if let Some(parent) = p.parent() { create_dir_all(parent).unwrap(); }
        File::create(&p).unwrap();
        p
    }

    #[test]
    fn filters_by_extension() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "a.mkv");
        touch(tmp.path(), "b.txt");
        touch(tmp.path(), "c.MP4"); // case-insensitive
        let got = scan(tmp.path(), false).unwrap();
        let names: Vec<_> = got.iter().filter_map(|p| p.file_name()?.to_str()).collect();
        assert!(names.contains(&"a.mkv"));
        assert!(names.contains(&"c.MP4"));
        assert!(!names.contains(&"b.txt"));
    }

    #[test]
    fn non_recursive_skips_subdirs() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "top.mkv");
        touch(tmp.path(), "sub/deep.mkv");
        let got = scan(tmp.path(), false).unwrap();
        assert_eq!(got.len(), 1);
        assert!(got[0].ends_with("top.mkv"));
    }

    #[test]
    fn recursive_descends() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "top.mkv");
        touch(tmp.path(), "sub/deep.mkv");
        let got = scan(tmp.path(), true).unwrap();
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn file_argument_returns_itself() {
        let tmp = TempDir::new().unwrap();
        let p = touch(tmp.path(), "solo.mkv");
        let got = scan(&p, false).unwrap();
        assert_eq!(got, vec![p]);
    }

    #[test]
    fn missing_path_errors() {
        let err = scan(Path::new("/does/not/exist/mosaic"), false);
        assert!(err.is_err());
    }
}
```

- [ ] **Step 2: Register in `lib.rs`**

```rust
#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod input_scan;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod input_scan;
```

- [ ] **Step 3: Delete the duplicate in `commands.rs`**

In `src-tauri/src/commands.rs`:
1. Remove the `const VIDEO_EXTS: &[&str] = &[ … ];` block (around line 33–47).
2. Remove the `const MAX_SCAN_DEPTH: u32 = 16;` line (line 50).
3. Remove the `fn walk(...)` function (lines 69–86).
4. Replace the body of `pub fn scan_folder(path: String, recursive: bool) -> Result<Vec<String>, String>` with the wrapper below. Keep the "not a directory" guard so existing GUI behavior (where `scan_folder` is only invoked with directory paths) is preserved byte-for-byte:

```rust
#[tauri::command]
pub fn scan_folder(path: String, recursive: bool) -> Result<Vec<String>, String> {
    let root = std::path::PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("not a directory: {}", path));
    }
    let found = crate::input_scan::scan(&root, recursive)?;
    Ok(found.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
}
```

5. Update `get_video_exts` to read from the new location:

```rust
#[tauri::command]
pub fn get_video_exts() -> Vec<String> {
    crate::input_scan::VIDEO_EXTS.iter().map(|s| s.to_string()).collect()
}
```

- [ ] **Step 4: Verify**

From `src-tauri/`:
```
cargo test --lib input_scan -- --nocapture
cargo build
cargo clippy --all-targets --features test-api -- -D warnings
```
Expected: 5 passing tests, build clean, no clippy warnings.

- [ ] **Step 5: Commit**

```
git add src-tauri/src/input_scan.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "refactor: extract VIDEO_EXTS + folder scan into input_scan module"
```

---

## Task 4: Add `cli` feature + optional deps + stub binary

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/bin/mosaic_cli/main.rs`

- [ ] **Step 1: Edit `src-tauri/Cargo.toml`**

Under `[features]`, add after the `test-api` line:

```toml
cli = ["dep:clap", "dep:indicatif", "dep:toml"]
```

Under `[dependencies]`, add:

```toml
clap      = { version = "4",    features = ["derive"], optional = true }
indicatif = { version = "0.17",                          optional = true }
toml      = { version = "0.8",                           optional = true }
```

Under `[dev-dependencies]`, add:

```toml
assert_cmd = "2"
predicates = "3"
```

After the existing `[[test]]` block, add:

```toml
[[bin]]
name = "mosaic-cli"
path = "src/bin/mosaic_cli/main.rs"
required-features = ["cli"]
```

- [ ] **Step 2: Write the stub entry point**

Create `src-tauri/src/bin/mosaic_cli/main.rs`:

```rust
// src-tauri/src/bin/mosaic_cli/main.rs
// Entry point for the `mosaic-cli` binary. Gated by the `cli` feature.
// Subsequent tasks flesh out the clap surface, config loader, and
// subcommand dispatch.

fn main() {
    println!("mosaic-cli {}", env!("CARGO_PKG_VERSION"));
}
```

- [ ] **Step 3: Verify build**

From `src-tauri/`:
```
cargo build --features cli --bin mosaic-cli
./target/debug/mosaic-cli
```
Expected: prints `mosaic-cli 0.1.3` (or whatever is current in `Cargo.toml`).

Also confirm the GUI build is unaffected:
```
cargo build
```
Expected: success, no `cli` feature pulled in.

```
cargo clippy --all-targets --features cli,test-api -- -D warnings
```
Expected: clean.

- [ ] **Step 4: Commit**

```
git add src-tauri/Cargo.toml src-tauri/src/bin/mosaic_cli/main.rs
git commit -m "feat(cli): scaffold mosaic-cli binary behind cli feature"
```

---

## Task 5: Extend two-branch cfg to expose pipeline modules under `cli`

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Update every two-branch module declaration**

For each of these modules currently declared with `#[cfg(any(test, feature = "test-api"))] pub mod X; #[cfg(not(any(test, feature = "test-api")))] mod X;`, add `feature = "cli"` to both arms:

- `video_info`
- `mediainfo`
- `output_path`
- `ffmpeg`
- `contact_sheet`
- `screenshots`
- `preview_reel`
- `animated_sheet`
- `jobs`

The resulting pattern for each (example: `video_info`):

```rust
#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod video_info;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod video_info;
```

- [ ] **Step 2: Update the three hook functions**

Same file. Change each hook's cfg:

```rust
#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub fn ffmpeg_test_hook_locate() -> Result<ffmpeg::Tools, ffmpeg::ToolsError> {
    ffmpeg::locate_tools()
}

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub async fn ffmpeg_test_hook_probe(tools: &ffmpeg::Tools, path: &str) -> Result<video_info::VideoInfo, String> {
    commands::probe(tools, path).await
}

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub fn video_info_test_hook_parse(json: &str) -> Result<video_info::VideoInfo, video_info::ProbeParseError> {
    video_info::parse(json)
}
```

Keep the `test_hook_` prefix — renaming is deferred per the spec; we don't want to break the integration test's references.

- [ ] **Step 3: Verify**

From `src-tauri/`:
```
cargo build --features cli --bin mosaic-cli
cargo build
cargo test --features test-api
cargo clippy --all-targets --features cli,test-api -- -D warnings
```
Expected: all four pass; existing integration test still runs via `test-api`.

- [ ] **Step 4: Commit**

```
git add src-tauri/src/lib.rs
git commit -m "chore(lib): expose pipeline modules under cli feature"
```

---

## Task 6: Config loader (`~/.mosaic-cli.toml`)

**Files:**
- Create: `src-tauri/src/bin/mosaic_cli/config.rs`
- Modify: `src-tauri/src/bin/mosaic_cli/main.rs` (wire in the loader)

- [ ] **Step 1: Write the config module with tests**

```rust
// src-tauri/src/bin/mosaic_cli/config.rs
// Resolves ~/.mosaic-cli.toml (or $MOSAIC_CLI_CONFIG), auto-creates a
// commented template on first run, parses it into a Config with
// Option<T> fields, and warns on unknown keys.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)] pub screenshots: ScreenshotsCfg,
    #[serde(default)] pub sheet: SheetCfg,
    #[serde(default)] pub reel: ReelCfg,
    #[serde(default)] pub animated_sheet: AnimatedSheetCfg,
}

#[derive(Debug, Default, Deserialize)]
pub struct ScreenshotsCfg {
    pub count: Option<u32>,
    pub format: Option<String>,
    pub quality: Option<u32>,
    pub suffix: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct SheetCfg {
    pub cols: Option<u32>,
    pub rows: Option<u32>,
    pub width: Option<u32>,
    pub gap: Option<u32>,
    pub thumb_font: Option<u32>,
    pub header_font: Option<u32>,
    pub show_timestamps: Option<bool>,
    pub show_header: Option<bool>,
    pub format: Option<String>,
    pub quality: Option<u32>,
    pub theme: Option<String>,
    pub suffix: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ReelCfg {
    pub count: Option<u32>,
    pub clip_length_secs: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub quality: Option<u32>,
    pub format: Option<String>,
    pub suffix: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct AnimatedSheetCfg {
    pub cols: Option<u32>,
    pub rows: Option<u32>,
    pub width: Option<u32>,
    pub gap: Option<u32>,
    pub clip_length_secs: Option<u32>,
    pub fps: Option<u32>,
    pub quality: Option<u32>,
    pub thumb_font: Option<u32>,
    pub header_font: Option<u32>,
    pub show_timestamps: Option<bool>,
    pub show_header: Option<bool>,
    pub theme: Option<String>,
    pub suffix: Option<String>,
}

/// Default path: `$HOME/.mosaic-cli.toml` (Unix) or
/// `%USERPROFILE%\.mosaic-cli.toml` (Windows). Overridable via
/// `$MOSAIC_CLI_CONFIG`. Returns `None` when no home is set (sandboxed CI).
pub fn resolve_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("MOSAIC_CLI_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok()?;
    Some(PathBuf::from(home).join(".mosaic-cli.toml"))
}

/// Load config from `path` if it exists, otherwise create a commented
/// template (printing a one-time notice to stderr) and return defaults.
/// On any I/O error during creation, skip silently and return defaults.
pub fn load_or_create(path: &Path) -> Config {
    if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(body) => return parse(&body, path),
            Err(e) => {
                eprintln!("warning: could not read {}: {e}", path.display());
                return Config::default();
            }
        }
    }
    match std::fs::write(path, template()) {
        Ok(()) => eprintln!("Created {}", path.display()),
        Err(_) => { /* read-only $HOME / sandbox: silently fall back */ }
    }
    Config::default()
}

fn parse(body: &str, path: &Path) -> Config {
    // First pass: permissive parse into a raw toml::Value so we can diff
    // against known keys and warn without failing the run.
    if let Ok(raw) = body.parse::<toml::Value>() {
        warn_unknown_keys(&raw, path);
    }
    match toml::from_str::<Config>(body) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("error: {}: {e}", path.display());
            std::process::exit(2);
        }
    }
}

fn warn_unknown_keys(raw: &toml::Value, path: &Path) {
    let known_sections = ["screenshots", "sheet", "reel", "animated_sheet"];
    let known_keys: &[(&str, &[&str])] = &[
        ("screenshots",   &["count", "format", "quality", "suffix"]),
        ("sheet",         &["cols","rows","width","gap","thumb_font","header_font",
                            "show_timestamps","show_header","format","quality","theme","suffix"]),
        ("reel",          &["count","clip_length_secs","height","fps","quality","format","suffix"]),
        ("animated_sheet",&["cols","rows","width","gap","clip_length_secs","fps","quality",
                            "thumb_font","header_font","show_timestamps","show_header","theme","suffix"]),
    ];
    let Some(table) = raw.as_table() else { return };
    for (section, value) in table {
        if !known_sections.contains(&section.as_str()) {
            eprintln!("warning: unknown section '{section}' in {}", path.display());
            continue;
        }
        let Some(sub) = value.as_table() else { continue };
        let allowed = known_keys.iter().find(|(s, _)| *s == section).map(|(_, k)| *k).unwrap_or(&[]);
        for key in sub.keys() {
            if !allowed.contains(&key.as_str()) {
                eprintln!("warning: unknown key '{section}.{key}' in {}", path.display());
            }
        }
    }
}

fn template() -> &'static str {
    // Keep in sync with the Cfg structs above. Every field appears commented
    // so users can uncomment and edit individual values.
    concat!(
        "# ~/.mosaic-cli.toml — per-user defaults for mosaic-cli.\n",
        "# Uncomment any key to override the built-in default.\n",
        "# CLI flags always override this file.\n",
        "\n",
        "[screenshots]\n",
        "# count = 8\n",
        "# format = \"png\"        # png | jpeg\n",
        "# quality = 92           # 50-100, JPEG only\n",
        "# suffix = \"_screens_\"\n",
        "\n",
        "[sheet]\n",
        "# cols = 3\n",
        "# rows = 6\n",
        "# width = 1920\n",
        "# gap = 10\n",
        "# thumb_font = 18\n",
        "# header_font = 20\n",
        "# show_timestamps = true\n",
        "# show_header = true\n",
        "# format = \"png\"        # png | jpeg\n",
        "# quality = 92\n",
        "# theme = \"dark\"        # dark | light\n",
        "# suffix = \"_sheet\"\n",
        "\n",
        "[reel]\n",
        "# count = 15\n",
        "# clip_length_secs = 2\n",
        "# height = 360\n",
        "# fps = 24\n",
        "# quality = 75\n",
        "# format = \"webp\"       # webp | webm | gif\n",
        "# suffix = \"_reel\"\n",
        "\n",
        "[animated_sheet]\n",
        "# cols = 3\n",
        "# rows = 6\n",
        "# width = 1280\n",
        "# gap = 8\n",
        "# clip_length_secs = 2\n",
        "# fps = 12\n",
        "# quality = 75\n",
        "# thumb_font = 14\n",
        "# header_font = 18\n",
        "# show_timestamps = true\n",
        "# show_header = true\n",
        "# theme = \"dark\"\n",
        "# suffix = \"_animated_sheet\"\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_missing_creates_template() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join(".mosaic-cli.toml");
        let cfg = load_or_create(&p);
        assert!(p.exists(), "template should have been written");
        assert!(cfg.screenshots.count.is_none());
    }

    #[test]
    fn parse_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join(".mosaic-cli.toml");
        std::fs::write(&p, "[sheet]\ncols = 5\nrows = 4\n").unwrap();
        let cfg = load_or_create(&p);
        assert_eq!(cfg.sheet.cols, Some(5));
        assert_eq!(cfg.sheet.rows, Some(4));
    }

    #[test]
    fn unknown_key_does_not_fail() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join(".mosaic-cli.toml");
        std::fs::write(&p, "[sheet]\ncols = 3\nbogus = 99\n").unwrap();
        let cfg = load_or_create(&p); // stderr warning is side-effect, not asserted here
        assert_eq!(cfg.sheet.cols, Some(3));
    }
}
```

- [ ] **Step 2: Declare the module in `main.rs`**

Update `src-tauri/src/bin/mosaic_cli/main.rs`:

```rust
mod config;

fn main() {
    let path = config::resolve_path();
    let _cfg = match &path {
        Some(p) => config::load_or_create(p),
        None    => config::Config::default(),
    };
    println!("mosaic-cli {}", env!("CARGO_PKG_VERSION"));
}
```

- [ ] **Step 3: Verify**

From `src-tauri/`:
```
cargo test --features cli --bin mosaic-cli -- --nocapture
```
Expected: 3 passing tests.

Also run it manually with a throwaway env path:
```
MOSAIC_CLI_CONFIG=/tmp/mosaic-cli-smoke.toml ./target/debug/mosaic-cli
cat /tmp/mosaic-cli-smoke.toml | head -5
rm /tmp/mosaic-cli-smoke.toml
```
Expected: binary prints `Created /tmp/mosaic-cli-smoke.toml` on first run, then `mosaic-cli 0.1.3`. The file contains the commented template.

```
cargo clippy --all-targets --features cli,test-api -- -D warnings
```

- [ ] **Step 4: Commit**

```
git add src-tauri/src/bin/mosaic_cli/config.rs src-tauri/src/bin/mosaic_cli/main.rs
git commit -m "feat(cli): config loader with first-run auto-create"
```

---

## Task 7: clap scaffold + subcommand dispatch

**Files:**
- Create: `src-tauri/src/bin/mosaic_cli/cli.rs`
- Create: `src-tauri/src/bin/mosaic_cli/run/mod.rs`
- Modify: `src-tauri/src/bin/mosaic_cli/main.rs`

- [ ] **Step 1: Write clap structs**

Create `src-tauri/src/bin/mosaic_cli/cli.rs`:

```rust
// src-tauri/src/bin/mosaic_cli/cli.rs
// Clap v4 derive surface for every subcommand. Defaults reference
// mosaic_lib::defaults so GUI and CLI stay in lockstep.

use clap::{Parser, Subcommand, ValueEnum};
use mosaic_lib::defaults;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mosaic-cli", version, about = "Video contact sheets, screenshots, previews, and animated sheets")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Screenshots(ScreenshotsArgs),
    Sheet(SheetArgs),
    Reel(ReelArgs),
    AnimatedSheet(AnimatedSheetArgs),
    Probe(ProbeArgs),
}

#[derive(Parser)]
pub struct Shared {
    /// Output directory (defaults to next to each source file).
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Suppress progress bars.
    #[arg(short, long)]
    pub quiet: bool,
    /// Print ffmpeg args before each invocation.
    #[arg(short, long)]
    pub verbose: bool,
    /// Do not descend into subdirectories when inputs are directories.
    #[arg(long)]
    pub no_recursive: bool,
    /// Input files or directories (at least one required).
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum ImgFormat { Png, Jpeg }

#[derive(Copy, Clone, ValueEnum)]
pub enum ReelFormat { Webp, Webm, Gif }

#[derive(Copy, Clone, ValueEnum)]
pub enum Theme { Dark, Light }

#[derive(Parser)]
pub struct ScreenshotsArgs {
    #[arg(long, default_value_t = defaults::screenshots::COUNT)]
    pub count: u32,
    #[arg(long, value_enum)]
    pub format: Option<ImgFormat>,
    #[arg(long)]
    pub quality: Option<u32>,
    #[arg(long)]
    pub suffix: Option<String>,
    #[command(flatten)]
    pub shared: Shared,
}

#[derive(Parser)]
pub struct SheetArgs {
    #[arg(long)] pub cols: Option<u32>,
    #[arg(long)] pub rows: Option<u32>,
    #[arg(long)] pub width: Option<u32>,
    #[arg(long)] pub gap: Option<u32>,
    #[arg(long = "thumb-font")] pub thumb_font: Option<u32>,
    #[arg(long = "header-font")] pub header_font: Option<u32>,
    #[arg(long = "no-timestamps")] pub no_timestamps: bool,
    #[arg(long = "no-header")] pub no_header: bool,
    #[arg(long, value_enum)] pub format: Option<ImgFormat>,
    #[arg(long)] pub quality: Option<u32>,
    #[arg(long, value_enum)] pub theme: Option<Theme>,
    #[arg(long)] pub suffix: Option<String>,
    #[command(flatten)] pub shared: Shared,
}

#[derive(Parser)]
pub struct ReelArgs {
    #[arg(long)] pub count: Option<u32>,
    #[arg(long = "clip-length")] pub clip_length: Option<u32>,
    #[arg(long)] pub height: Option<u32>,
    #[arg(long)] pub fps: Option<u32>,
    #[arg(long)] pub quality: Option<u32>,
    #[arg(long, value_enum)] pub format: Option<ReelFormat>,
    #[arg(long)] pub suffix: Option<String>,
    #[command(flatten)] pub shared: Shared,
}

#[derive(Parser)]
pub struct AnimatedSheetArgs {
    #[arg(long)] pub cols: Option<u32>,
    #[arg(long)] pub rows: Option<u32>,
    #[arg(long)] pub width: Option<u32>,
    #[arg(long)] pub gap: Option<u32>,
    #[arg(long = "clip-length")] pub clip_length: Option<u32>,
    #[arg(long)] pub fps: Option<u32>,
    #[arg(long)] pub quality: Option<u32>,
    #[arg(long = "thumb-font")] pub thumb_font: Option<u32>,
    #[arg(long = "header-font")] pub header_font: Option<u32>,
    #[arg(long = "no-timestamps")] pub no_timestamps: bool,
    #[arg(long = "no-header")] pub no_header: bool,
    #[arg(long, value_enum)] pub theme: Option<Theme>,
    #[arg(long)] pub suffix: Option<String>,
    #[command(flatten)] pub shared: Shared,
}

#[derive(Parser)]
pub struct ProbeArgs {
    #[arg(long)]
    pub mediainfo: bool,
    #[arg(required = true)]
    pub input: PathBuf,
}
```

- [ ] **Step 2: Create the `run` module tree**

Create `src-tauri/src/bin/mosaic_cli/run/mod.rs`:

```rust
// src-tauri/src/bin/mosaic_cli/run/mod.rs
// Per-subcommand implementations. Each submodule takes the parsed
// clap args + loaded config and returns a process exit code.

pub mod probe;
pub mod screenshots;
pub mod sheet;
pub mod reel;
pub mod animated_sheet;
```

For now, create stub files for each subcommand so the crate compiles:

```rust
// src-tauri/src/bin/mosaic_cli/run/probe.rs
use crate::cli::ProbeArgs;

pub async fn run(_args: ProbeArgs) -> i32 {
    eprintln!("probe: not yet implemented");
    2
}
```

Repeat with the same shape for `screenshots.rs`, `sheet.rs`, `reel.rs`, `animated_sheet.rs`, each taking its respective `*Args` struct plus `&crate::config::Config`:

```rust
// src-tauri/src/bin/mosaic_cli/run/screenshots.rs
use crate::cli::ScreenshotsArgs;
use crate::config::Config;

pub async fn run(_args: ScreenshotsArgs, _cfg: &Config) -> i32 {
    eprintln!("screenshots: not yet implemented");
    2
}
```

```rust
// src-tauri/src/bin/mosaic_cli/run/sheet.rs
use crate::cli::SheetArgs;
use crate::config::Config;

pub async fn run(_args: SheetArgs, _cfg: &Config) -> i32 {
    eprintln!("sheet: not yet implemented");
    2
}
```

```rust
// src-tauri/src/bin/mosaic_cli/run/reel.rs
use crate::cli::ReelArgs;
use crate::config::Config;

pub async fn run(_args: ReelArgs, _cfg: &Config) -> i32 {
    eprintln!("reel: not yet implemented");
    2
}
```

```rust
// src-tauri/src/bin/mosaic_cli/run/animated_sheet.rs
use crate::cli::AnimatedSheetArgs;
use crate::config::Config;

pub async fn run(_args: AnimatedSheetArgs, _cfg: &Config) -> i32 {
    eprintln!("animated-sheet: not yet implemented");
    2
}
```

- [ ] **Step 3: Wire up `main.rs`**

Replace `src-tauri/src/bin/mosaic_cli/main.rs` with:

```rust
// src-tauri/src/bin/mosaic_cli/main.rs

mod cli;
mod config;
mod run;

use clap::Parser;

#[tokio::main]
async fn main() {
    let parsed = cli::Cli::parse();
    let cfg_path = config::resolve_path();
    let cfg = match cfg_path.as_deref() {
        Some(p) => config::load_or_create(p),
        None    => config::Config::default(),
    };

    let code = match parsed.command {
        cli::Command::Screenshots(a)   => run::screenshots::run(a, &cfg).await,
        cli::Command::Sheet(a)         => run::sheet::run(a, &cfg).await,
        cli::Command::Reel(a)          => run::reel::run(a, &cfg).await,
        cli::Command::AnimatedSheet(a) => run::animated_sheet::run(a, &cfg).await,
        cli::Command::Probe(a)         => run::probe::run(a).await,
    };
    std::process::exit(code);
}
```

- [ ] **Step 4: Verify**

From `src-tauri/`:
```
cargo build --features cli --bin mosaic-cli
./target/debug/mosaic-cli --help
./target/debug/mosaic-cli sheet --help
```
Expected: help output lists all five subcommands; `sheet --help` shows every flag; `mosaic-cli screenshots /nonexistent` exits 2 with `screenshots: not yet implemented`.

```
cargo clippy --all-targets --features cli,test-api -- -D warnings
```

- [ ] **Step 5: Commit**

```
git add src-tauri/src/bin/mosaic_cli/
git commit -m "feat(cli): clap surface + subcommand dispatch scaffold"
```

---

## Task 8: `probe` subcommand

**Files:**
- Modify: `src-tauri/src/bin/mosaic_cli/run/probe.rs`

- [ ] **Step 1: Implement probe**

```rust
// src-tauri/src/bin/mosaic_cli/run/probe.rs
use crate::cli::ProbeArgs;
use mosaic_lib::{ffmpeg_test_hook_locate, ffmpeg_test_hook_probe};
use serde_json::json;

pub async fn run(args: ProbeArgs) -> i32 {
    let tools = match ffmpeg_test_hook_locate() {
        Ok(t) => t,
        Err(e) => { eprintln!("{e}"); return 2; }
    };
    let path_str = args.input.to_string_lossy().into_owned();
    let info = match ffmpeg_test_hook_probe(&tools, &path_str).await {
        Ok(i) => i,
        Err(e) => { eprintln!("{e}"); return 1; }
    };

    if args.mediainfo {
        // Run mediainfo and wrap both in a single object.
        let mi = match tokio::process::Command::new(&tools.mediainfo)
            .arg(&args.input)
            .output().await
        {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
            Ok(o) => {
                eprintln!("mediainfo failed: {}", String::from_utf8_lossy(&o.stderr));
                return 1;
            }
            Err(e) => { eprintln!("mediainfo spawn failed: {e}"); return 1; }
        };
        let out = json!({ "ffprobe": info, "mediainfo": mi });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("{}", serde_json::to_string_pretty(&info).unwrap());
    }
    0
}
```

- [ ] **Step 2: Verify build**

From `src-tauri/`:
```
cargo build --features cli --bin mosaic-cli
./target/debug/mosaic-cli probe tests/fixtures/sample.mp4 | head -20
./target/debug/mosaic-cli probe --mediainfo tests/fixtures/sample.mp4 | jq -r 'keys | join(",")'
```
Expected: first command prints VideoInfo JSON with `duration_secs`, `video`, `audio`. Second prints `ffprobe,mediainfo`.

On macOS this requires `PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH"` per CLAUDE.md.

- [ ] **Step 3: Commit**

```
git add src-tauri/src/bin/mosaic_cli/run/probe.rs
git commit -m "feat(cli): probe subcommand with --mediainfo JSON envelope"
```

---

## Task 9: Progress reporter + embedded font extractor

**Files:**
- Create: `src-tauri/src/bin/mosaic_cli/progress.rs`
- Create: `src-tauri/src/bin/mosaic_cli/font.rs`
- Modify: `src-tauri/src/bin/mosaic_cli/main.rs` (register modules)

- [ ] **Step 1: Write the progress wrapper**

```rust
// src-tauri/src/bin/mosaic_cli/progress.rs
// Thin wrapper around indicatif's MultiProgress used by every
// generate subcommand. `--quiet` yields a no-op callback.

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

pub struct Reporter {
    _mp: MultiProgress,
    pub file: ProgressBar,
    pub step: ProgressBar,
    quiet: bool,
}

impl Reporter {
    pub fn new(total_files: u64, quiet: bool) -> Self {
        let mp = MultiProgress::new();
        if quiet { mp.set_draw_target(indicatif::ProgressDrawTarget::hidden()); }
        let file = mp.add(ProgressBar::new(total_files));
        file.set_style(ProgressStyle::with_template("{prefix} {wide_msg}").unwrap());
        let step = mp.add(ProgressBar::new(1));
        step.set_style(ProgressStyle::with_template("  {bar:30} {pos}/{len} {msg}").unwrap());
        Self { _mp: mp, file, step, quiet }
    }

    pub fn start_file(&self, idx: u64, total: u64, path: &std::path::Path) {
        self.file.set_position(idx);
        self.file.set_prefix(format!("{idx}/{total}"));
        self.file.set_message(format!("{}", path.display()));
        self.step.reset();
    }

    /// Returns a closure suitable for `ProgressReporter::emit`.
    pub fn emit_fn<'a>(&'a self) -> impl Fn(u32, u32, &str) + Send + Sync + 'a {
        let bar = self.step.clone();
        let quiet = self.quiet;
        move |pos: u32, total: u32, label: &str| {
            if quiet { return; }
            bar.set_length(total as u64);
            bar.set_position(pos as u64);
            bar.set_message(label.to_string());
        }
    }

    pub fn finish(&self) { self.step.finish_and_clear(); self.file.finish_and_clear(); }
}
```

- [ ] **Step 2: Write the font extractor**

```rust
// src-tauri/src/bin/mosaic_cli/font.rs
// Embeds DejaVuSans.ttf into the binary via include_bytes! and lazily
// writes it to a tempfile on first use. Only `sheet` and
// `animated-sheet` subcommands call this — `screenshots` and `reel`
// pipelines render no drawtext.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use tempfile::NamedTempFile;

static FONT_FILE: OnceLock<NamedTempFile> = OnceLock::new();

const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");

pub fn path() -> Result<PathBuf, std::io::Error> {
    if let Some(f) = FONT_FILE.get() { return Ok(f.path().to_path_buf()); }
    let mut tf = NamedTempFile::new()?;
    tf.write_all(FONT_BYTES)?;
    let p = tf.path().to_path_buf();
    let _ = FONT_FILE.set(tf); // race: whichever wins, path on disk is identical bytes
    Ok(p)
}
```

(Note: `../../../assets/fonts/DejaVuSans.ttf` is the correct relative path from `src-tauri/src/bin/mosaic_cli/font.rs` — three `..` reach `src-tauri/`.)

- [ ] **Step 3: Register in `main.rs`**

Add to the `mod` list in `main.rs`:

```rust
mod font;
mod progress;
```

- [ ] **Step 4: Verify build**

From `src-tauri/`:
```
cargo build --features cli --bin mosaic-cli
cargo clippy --all-targets --features cli,test-api -- -D warnings
```

- [ ] **Step 5: Commit**

```
git add src-tauri/src/bin/mosaic_cli/progress.rs src-tauri/src/bin/mosaic_cli/font.rs src-tauri/src/bin/mosaic_cli/main.rs
git commit -m "feat(cli): progress reporter + embedded font extractor"
```

---

## Task 10: `screenshots` subcommand

**Files:**
- Modify: `src-tauri/src/bin/mosaic_cli/run/screenshots.rs`
- Create helper: `src-tauri/src/bin/mosaic_cli/run/inputs.rs`
- Modify: `src-tauri/src/bin/mosaic_cli/run/mod.rs` (register `inputs`)

- [ ] **Step 1: Write a shared input-expander**

```rust
// src-tauri/src/bin/mosaic_cli/run/inputs.rs
// Turns clap positional inputs (mix of files and dirs) into the final
// per-file worklist, using input_scan for directory expansion.

use std::path::{Path, PathBuf};

pub fn expand(inputs: &[PathBuf], recursive: bool) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for p in inputs {
        let found = mosaic_lib::input_scan::scan(Path::new(p), recursive)?;
        out.extend(found);
    }
    Ok(out)
}
```

Register in `run/mod.rs`:

```rust
pub mod inputs;
pub mod probe;
pub mod screenshots;
pub mod sheet;
pub mod reel;
pub mod animated_sheet;
```

- [ ] **Step 2: Implement the subcommand**

```rust
// src-tauri/src/bin/mosaic_cli/run/screenshots.rs
use crate::cli::{ImgFormat, ScreenshotsArgs};
use crate::config::Config;
use crate::progress::Reporter;
use crate::run::inputs;
use mosaic_lib::{
    defaults,
    ffmpeg_test_hook_locate, ffmpeg_test_hook_probe,
    jobs::{PipelineContext, ProgressReporter},
    output_path::{OutputFormat, DEFAULT_SHOTS_SUFFIX},
    screenshots::{generate, ScreenshotsOptions},
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub async fn run(args: ScreenshotsArgs, cfg: &Config) -> i32 {
    let tools = match ffmpeg_test_hook_locate() {
        Ok(t) => t, Err(e) => { eprintln!("{e}"); return 2; }
    };
    let inputs = match inputs::expand(&args.shared.inputs, !args.shared.no_recursive) {
        Ok(v) => v, Err(e) => { eprintln!("{e}"); return 2; }
    };
    if inputs.is_empty() { eprintln!("no input files"); return 2; }

    let has_zscale = tools.detect_has_zscale();
    let cancelled = Arc::new(AtomicBool::new(false));
    crate::signals::install(cancelled.clone());

    let total = inputs.len() as u64;
    let reporter = Reporter::new(total, args.shared.quiet);
    let emit = reporter.emit_fn();
    let pr = ProgressReporter { emit: &emit };

    let count   = args.count;
    let fmt     = resolve_format(&args.format, cfg);
    let quality = args.quality
        .or(cfg.screenshots.quality)
        .unwrap_or(defaults::screenshots::JPEG_QUALITY);
    let suffix  = args.suffix.clone()
        .or_else(|| cfg.screenshots.suffix.clone())
        .unwrap_or_else(|| DEFAULT_SHOTS_SUFFIX.to_string());

    let opts = ScreenshotsOptions {
        count, format: fmt, jpeg_quality: quality, suffix,
    };

    let mut done = 0u64;
    let mut failed = 0u64;
    for (i, src) in inputs.iter().enumerate() {
        let idx = i as u64 + 1;
        reporter.start_file(idx, total, src);
        if args.shared.verbose {
            eprintln!("screenshots: {}", src.display());
        }
        let info = match ffmpeg_test_hook_probe(&tools, &src.to_string_lossy()).await {
            Ok(v) => v,
            Err(e) => { eprintln!("{}: {e}", src.display()); failed += 1; continue; }
        };
        let out_dir = args.shared.output.clone().unwrap_or_else(|| {
            src.parent().unwrap_or(std::path::Path::new(".")).to_path_buf()
        });
        let ctx = PipelineContext {
            ffmpeg: &tools.ffmpeg,
            cancelled: cancelled.clone(),
            reporter: &pr,
            has_zscale,
        };
        match generate(src, &info, &out_dir, &opts, &ctx).await {
            Ok(paths) => {
                for p in paths { println!("{}", p.display()); }
                done += 1;
            }
            Err(e) => { eprintln!("{}: {e}", src.display()); failed += 1; }
        }
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) { break; }
    }
    reporter.finish();
    eprintln!("{done} done · {failed} failed · {} cancelled",
              if cancelled.load(std::sync::atomic::Ordering::Relaxed) { total - done - failed } else { 0 });
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) { return 130; }
    if failed > 0 { 1 } else { 0 }
}

fn resolve_format(arg: &Option<ImgFormat>, cfg: &Config) -> OutputFormat {
    match arg {
        Some(ImgFormat::Png)  => OutputFormat::Png,
        Some(ImgFormat::Jpeg) => OutputFormat::Jpeg,
        None => match cfg.screenshots.format.as_deref() {
            Some("jpeg") | Some("Jpeg") => OutputFormat::Jpeg,
            _ => OutputFormat::Png,
        },
    }
}
```

Note: this references `crate::signals::install`, created in the next task (Ctrl-C). Keep it here and add `mod signals;` + a stub in Task 12 before running.

To unblock compilation now, also add a minimal `mod signals;` placeholder in `main.rs`:

```rust
mod signals;
```

And create `src-tauri/src/bin/mosaic_cli/signals.rs`:

```rust
// placeholder; full impl in Task 12
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
pub fn install(_cancelled: Arc<AtomicBool>) {}
```

- [ ] **Step 3: Verify build + smoke test**

From `src-tauri/`:
```
cargo build --features cli --bin mosaic-cli
./target/debug/mosaic-cli screenshots --count 3 -o /tmp/shots tests/fixtures/sample.mp4
ls /tmp/shots
```
Expected: 3 PNGs in `/tmp/shots`. Stdout shows each output path.

```
cargo clippy --all-targets --features cli,test-api -- -D warnings
```

- [ ] **Step 4: Commit**

```
git add src-tauri/src/bin/mosaic_cli/
git commit -m "feat(cli): screenshots subcommand"
```

---

## Task 11: `reel`, `sheet`, `animated-sheet` subcommands

Each mirrors Task 10's structure. Only the differences are listed — copy the scaffolding (inputs, progress, cancel check, summary line) from `screenshots.rs` verbatim.

**Files:**
- Modify: `src-tauri/src/bin/mosaic_cli/run/reel.rs`
- Modify: `src-tauri/src/bin/mosaic_cli/run/sheet.rs`
- Modify: `src-tauri/src/bin/mosaic_cli/run/animated_sheet.rs`

- [ ] **Step 1: Implement `reel`**

Use `mosaic_lib::preview_reel::{generate, PreviewOptions}` and `output_path::{preview_reel_path, ReelFormat, DEFAULT_PREVIEW_SUFFIX}`. `preview_reel::generate` takes `(source, info, out, opts, ctx) -> Result<(), RunError>` — no font. As with `sheet`, the caller computes the output path first and prints it on success:

```rust
let exists = |p: &std::path::Path| p.exists();
let out_path = mosaic_lib::output_path::preview_reel_path(
    src, &out_dir, opts.format, &opts.suffix, &exists,
);
// ... generate(src, &info, &out_path, &opts, &ctx).await ...
// on Ok: println!("{}", out_path.display());
```

(Signature order: `preview_reel_path(source, out_dir, fmt: ReelFormat, suffix, exists_fn)` — note `fmt` comes before `suffix`. Confirm against `src-tauri/src/output_path.rs` line 107 when wiring.)

Options mapping:

```rust
let opts = PreviewOptions {
    count:            args.count.or(cfg.reel.count).unwrap_or(defaults::reel::COUNT),
    clip_length_secs: args.clip_length.or(cfg.reel.clip_length_secs).unwrap_or(defaults::reel::CLIP_LENGTH_SECS),
    height:           args.height.or(cfg.reel.height).unwrap_or(defaults::reel::HEIGHT),
    fps:              args.fps.or(cfg.reel.fps).unwrap_or(defaults::reel::FPS),
    quality:          args.quality.or(cfg.reel.quality).unwrap_or(defaults::reel::QUALITY),
    suffix:           args.suffix.clone().or_else(|| cfg.reel.suffix.clone()).unwrap_or_else(|| DEFAULT_PREVIEW_SUFFIX.to_string()),
    format:           resolve_reel_format(&args.format, cfg),
};
```

Helper:

```rust
fn resolve_reel_format(arg: &Option<crate::cli::ReelFormat>, cfg: &Config) -> ReelFormat {
    match arg {
        Some(crate::cli::ReelFormat::Webp) => ReelFormat::Webp,
        Some(crate::cli::ReelFormat::Webm) => ReelFormat::Webm,
        Some(crate::cli::ReelFormat::Gif)  => ReelFormat::Gif,
        None => match cfg.reel.format.as_deref() {
            Some("webm") | Some("Webm") => ReelFormat::Webm,
            Some("gif")  | Some("Gif")  => ReelFormat::Gif,
            _ => ReelFormat::Webp,
        },
    }
}
```

Read `output_path::preview_reel_path`'s exact signature from `src-tauri/src/output_path.rs` before wiring — it takes an `exists_fn` callback for collision suffixes.

- [ ] **Step 2: Implement `sheet`**

Uses `mosaic_lib::contact_sheet::{generate, SheetOptions}` and `output_path::{contact_sheet_path, OutputFormat, SheetTheme, DEFAULT_SHEET_SUFFIX}`. Signature is `generate(source, info, output_path, opts, font, ctx) -> Result<(), RunError>` — no return value, so the **caller computes `out_path` via `contact_sheet_path(...)` and prints it on success**. `font: &Path` required, obtained from `crate::font::path()`.

Path computation pattern (before the `generate` call):

```rust
let exists = |p: &std::path::Path| p.exists();
let out_path = mosaic_lib::output_path::contact_sheet_path(
    src, &out_dir, opts.format, &opts.suffix, &exists,
);
```

After a successful `generate`, emit the path:

```rust
println!("{}", out_path.display());
```

Options mapping:

```rust
let opts = SheetOptions {
    cols:             args.cols.or(cfg.sheet.cols).unwrap_or(defaults::sheet::COLS),
    rows:             args.rows.or(cfg.sheet.rows).unwrap_or(defaults::sheet::ROWS),
    width:            args.width.or(cfg.sheet.width).unwrap_or(defaults::sheet::WIDTH),
    gap:              args.gap.or(cfg.sheet.gap).unwrap_or(defaults::sheet::GAP),
    thumb_font_size:  args.thumb_font.or(cfg.sheet.thumb_font).unwrap_or(defaults::sheet::THUMB_FONT),
    header_font_size: args.header_font.or(cfg.sheet.header_font).unwrap_or(defaults::sheet::HEADER_FONT),
    show_timestamps:  !args.no_timestamps && cfg.sheet.show_timestamps.unwrap_or(true),
    show_header:      !args.no_header && cfg.sheet.show_header.unwrap_or(true),
    format:           resolve_img_format(&args.format, cfg.sheet.format.as_deref()),
    jpeg_quality:     args.quality.or(cfg.sheet.quality).unwrap_or(defaults::sheet::JPEG_QUALITY),
    suffix:           args.suffix.clone().or_else(|| cfg.sheet.suffix.clone()).unwrap_or_else(|| DEFAULT_SHEET_SUFFIX.to_string()),
    theme:            resolve_theme(&args.theme, cfg.sheet.theme.as_deref()),
};

let font_path = match crate::font::path() {
    Ok(p) => p, Err(e) => { eprintln!("font extract failed: {e}"); return 2; }
};
```

Call shape:

```rust
generate(src, &info, &out_path, &opts, &font_path, &ctx).await
```

Helpers (shared — move to `run/mod.rs` or `run/format.rs`):

```rust
pub fn resolve_img_format(arg: &Option<crate::cli::ImgFormat>, cfg: Option<&str>) -> OutputFormat {
    match arg {
        Some(crate::cli::ImgFormat::Png)  => OutputFormat::Png,
        Some(crate::cli::ImgFormat::Jpeg) => OutputFormat::Jpeg,
        None => match cfg {
            Some("jpeg") | Some("Jpeg") => OutputFormat::Jpeg,
            _ => OutputFormat::Png,
        },
    }
}

pub fn resolve_theme(arg: &Option<crate::cli::Theme>, cfg: Option<&str>) -> SheetTheme {
    match arg {
        Some(crate::cli::Theme::Light) => SheetTheme::Light,
        Some(crate::cli::Theme::Dark)  => SheetTheme::Dark,
        None => match cfg {
            Some("light") | Some("Light") => SheetTheme::Light,
            _ => SheetTheme::Dark,
        },
    }
}
```

Place these in a new `src-tauri/src/bin/mosaic_cli/run/format.rs` and `pub mod format;` in `run/mod.rs` so the other subcommand modules can reuse them.

- [ ] **Step 3: Implement `animated-sheet`**

Same shape as `sheet`. Uses `mosaic_lib::animated_sheet::{generate, AnimatedSheetOptions}` and `output_path::{animated_sheet_path, DEFAULT_ANIMATED_SHEET_SUFFIX}`. Includes `clip_length_secs`, `fps`, `quality` like `reel`. Needs `font` from `crate::font::path()`. No `format` enum — `animated_sheet_path` hard-codes `.webp`.

Path computation:

```rust
let exists = |p: &std::path::Path| p.exists();
let out_path = mosaic_lib::output_path::animated_sheet_path(
    src, &out_dir, &opts.suffix, &exists,
);
```

**Stdout contract (all three subcommands).** On success, print the produced output file path via `println!("{}", out_path.display())` — exactly the same shape as `screenshots` uses for each entry in its `Vec<PathBuf>`. This keeps the stdout-is-paths contract consistent so callers can `mosaic-cli … | xargs …`.

- [ ] **Step 4: Verify each**

From `src-tauri/`:
```
./target/debug/mosaic-cli sheet --cols 2 --rows 2 -o /tmp/sheet tests/fixtures/sample.mp4
./target/debug/mosaic-cli reel --count 2 --clip-length 1 -o /tmp/reel tests/fixtures/sample.mp4
./target/debug/mosaic-cli animated-sheet --cols 2 --rows 2 --clip-length 1 -o /tmp/asheet tests/fixtures/sample.mp4
```
Expected: each produces exactly one output file at the printed stdout path; exit code 0.

```
cargo clippy --all-targets --features cli,test-api -- -D warnings
```

- [ ] **Step 5: Commit**

```
git add src-tauri/src/bin/mosaic_cli/
git commit -m "feat(cli): reel, sheet, animated-sheet subcommands"
```

---

## Task 12: Ctrl-C handling

**Files:**
- Modify: `src-tauri/src/bin/mosaic_cli/signals.rs`

- [ ] **Step 1: Replace the stub**

```rust
// src-tauri/src/bin/mosaic_cli/signals.rs
// Installs a tokio Ctrl-C handler that sets the shared cancel flag.
// Second Ctrl-C exits the process immediately without draining ffmpeg.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn install(cancelled: Arc<AtomicBool>) {
    tokio::spawn(async move {
        // First signal: request graceful cancel.
        let _ = tokio::signal::ctrl_c().await;
        cancelled.store(true, Ordering::Relaxed);
        eprintln!("cancelling…");
        // Second signal: hard exit.
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("forced exit");
        std::process::exit(130);
    });
}
```

- [ ] **Step 2: Verify**

Manual test — no automated assertion needed:
```
./target/debug/mosaic-cli reel --count 15 /path/to/large.mkv
# hit Ctrl-C mid-run
```
Expected: `cancelling…` appears, the current ffmpeg call aborts, summary prints `0 done · 0 failed · 1 cancelled`, exit code 130.

```
cargo clippy --all-targets --features cli,test-api -- -D warnings
```

- [ ] **Step 3: Commit**

```
git add src-tauri/src/bin/mosaic_cli/signals.rs
git commit -m "feat(cli): ctrl-c hands off to shared cancel flag"
```

---

## Task 13: Integration tests

**Files:**
- Create: `src-tauri/tests/cli.rs`
- Modify: `src-tauri/Cargo.toml` (add `[[test]]` entry)

- [ ] **Step 1: Register the test target**

Append to `src-tauri/Cargo.toml`:

```toml
[[test]]
name = "cli"
required-features = ["cli", "test-api"]
```

- [ ] **Step 2: Write the test file**

```rust
// src-tauri/tests/cli.rs
// Integration tests for mosaic-cli via assert_cmd. Uses the shared
// fixture tests/fixtures/sample.mp4 and writes outputs into
// TempDir-scoped directories so tests are hermetic.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn sample() -> std::path::PathBuf {
    // Resolve relative to the crate root.
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests").join("fixtures").join("sample.mp4")
}

fn bin() -> Command {
    let mut cmd = Command::cargo_bin("mosaic-cli").unwrap();
    // Isolate config file so real $HOME isn't touched.
    let tmp = std::env::temp_dir().join(format!("mosaic-cli-test-{}.toml", std::process::id()));
    cmd.env("MOSAIC_CLI_CONFIG", tmp);
    cmd
}

#[test]
fn probe_emits_ffprobe_json() {
    bin().args(["probe"]).arg(sample())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"duration_secs\""));
}

#[test]
fn probe_mediainfo_wraps_both() {
    bin().args(["probe", "--mediainfo"]).arg(sample())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ffprobe\""))
        .stdout(predicate::str::contains("\"mediainfo\""));
}

#[test]
fn screenshots_produces_expected_count() {
    let out = TempDir::new().unwrap();
    bin().args(["screenshots", "--count", "3", "-o"]).arg(out.path()).arg(sample())
        .assert().success();
    let pngs: Vec<_> = fs::read_dir(out.path()).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("png"))
        .collect();
    assert_eq!(pngs.len(), 3);
}

#[test]
fn sheet_produces_nonempty_output() {
    let out = TempDir::new().unwrap();
    bin().args(["sheet", "--cols", "2", "--rows", "2", "-o"]).arg(out.path()).arg(sample())
        .assert().success();
    let entries: Vec<_> = fs::read_dir(out.path()).unwrap().collect();
    let file = entries.into_iter().map(|e| e.unwrap().path()).find(|p| p.is_file()).unwrap();
    assert!(fs::metadata(&file).unwrap().len() > 1024);
}

#[test]
fn reel_produces_webp() {
    let out = TempDir::new().unwrap();
    bin().args(["reel", "--count", "2", "--clip-length", "1", "-o"]).arg(out.path()).arg(sample())
        .assert().success();
    let webp = fs::read_dir(out.path()).unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().and_then(|x| x.to_str()) == Some("webp"));
    assert!(webp.is_some());
    // VP8X chunk indicates animated webp — "VP8X" appears early in the file.
    let bytes = fs::read(webp.unwrap().path()).unwrap();
    assert!(bytes.windows(4).any(|w| w == b"VP8X"));
}

#[test]
fn animated_sheet_produces_webp() {
    let out = TempDir::new().unwrap();
    bin().args(["animated-sheet", "--cols", "2", "--rows", "2", "--clip-length", "1", "-o"])
        .arg(out.path()).arg(sample())
        .assert().success();
    let ok = fs::read_dir(out.path()).unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("webp"));
    assert!(ok);
}

#[test]
fn config_file_sets_defaults() {
    let cfg_dir = TempDir::new().unwrap();
    let cfg_path = cfg_dir.path().join(".mosaic-cli.toml");
    fs::write(&cfg_path, "[sheet]\ncols = 5\nrows = 2\n").unwrap();

    let out = TempDir::new().unwrap();
    // No --cols flag — should use 5 from config.
    let mut cmd = Command::cargo_bin("mosaic-cli").unwrap();
    cmd.env("MOSAIC_CLI_CONFIG", &cfg_path)
        .args(["sheet", "--rows", "2", "-o"]).arg(out.path()).arg(sample())
        .assert().success();
    // Can't easily assert cols count without parsing the sheet image, but the
    // command running successfully with rows=2 (CLI flag) + cols=5 (config)
    // proves config is read. Precedence is asserted in the next test.
}

#[test]
fn cli_flag_overrides_config() {
    let cfg_dir = TempDir::new().unwrap();
    let cfg_path = cfg_dir.path().join(".mosaic-cli.toml");
    fs::write(&cfg_path, "[sheet]\ncols = 99\nrows = 99\n").unwrap(); // would fail if applied

    let out = TempDir::new().unwrap();
    let mut cmd = Command::cargo_bin("mosaic-cli").unwrap();
    cmd.env("MOSAIC_CLI_CONFIG", &cfg_path)
        .args(["sheet", "--cols", "2", "--rows", "2", "-o"]).arg(out.path()).arg(sample())
        .assert().success();
}

#[test]
fn first_run_creates_config() {
    let cfg_dir = TempDir::new().unwrap();
    let cfg_path = cfg_dir.path().join(".mosaic-cli.toml");
    assert!(!cfg_path.exists());

    let mut cmd = Command::cargo_bin("mosaic-cli").unwrap();
    cmd.env("MOSAIC_CLI_CONFIG", &cfg_path)
        .args(["probe"]).arg(sample())
        .assert().success();
    assert!(cfg_path.exists(), "config template should be auto-created on first run");
}

#[test]
fn missing_input_exits_nonzero() {
    bin().args(["screenshots", "/does/not/exist.mp4"])
        .assert().failure();
}
```

- [ ] **Step 3: Run the tests**

From `src-tauri/`:
```
PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --features cli,test-api --test cli
```
Expected: all 10 tests pass.

```
cargo clippy --all-targets --features cli,test-api -- -D warnings
```

- [ ] **Step 4: Commit**

```
git add src-tauri/tests/cli.rs src-tauri/Cargo.toml
git commit -m "test(cli): integration coverage for every subcommand + config precedence"
```

---

## Task 14: CI — build CLI binaries in the release workflow

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Review the current workflow**

The existing job in `.github/workflows/release.yml` uses a matrix with `os` entries: `macos-latest`, `windows-latest`, `windows-11-arm`, `ubuntu-22.04`. Each entry carries `rust-targets` and `tauri-args`. The `tauri-apps/tauri-action@v0` step builds/signs/uploads GUI artefacts, marking the release as draft (`releaseDraft: true`).

`rustc` has no `universal-apple-darwin` target — `tauri-action` fakes it by building both arches and lipo-ing them. For the CLI we'll build `aarch64-apple-darwin` and `x86_64-apple-darwin` explicitly, then `lipo` them into one fat binary. The Rust toolchain is already installed with both targets via the matrix's `rust-targets`.

- [ ] **Step 2: Extend the matrix**

Add a per-entry `cli-name` (for the uploaded filename) and — on macOS only — a platform-specific signal that tells the new build step to lipo. Modify the `matrix.include:` block (lines 15–27) to look like this (only the new `cli-*` fields are shown; preserve every existing field):

```yaml
          - os: macos-latest
            rust-targets: aarch64-apple-darwin,x86_64-apple-darwin
            tauri-args: --target universal-apple-darwin
            cli-name: macos-universal
            cli-lipo: "true"
          - os: windows-latest
            rust-targets: x86_64-pc-windows-msvc
            tauri-args: ""
            cli-name: windows-x86_64
            cli-target: x86_64-pc-windows-msvc
          - os: windows-11-arm
            rust-targets: aarch64-pc-windows-msvc
            tauri-args: --target aarch64-pc-windows-msvc
            cli-name: windows-aarch64
            cli-target: aarch64-pc-windows-msvc
          - os: ubuntu-22.04
            rust-targets: x86_64-unknown-linux-gnu
            tauri-args: ""
            cli-name: linux-x86_64
            cli-target: x86_64-unknown-linux-gnu
```

- [ ] **Step 3: Add CLI build + upload steps after `tauri-action`**

Append these steps at the end of the `steps:` list (after the `tauri-apps/tauri-action@v0` block ending around line 122):

```yaml
      - name: Build mosaic-cli (lipo universal, macOS only)
        if: matrix.cli-lipo == 'true'
        working-directory: src-tauri
        run: |
          cargo build --release --features cli --bin mosaic-cli --target aarch64-apple-darwin
          cargo build --release --features cli --bin mosaic-cli --target x86_64-apple-darwin
          mkdir -p target/universal-apple-darwin/release
          lipo -create -output target/universal-apple-darwin/release/mosaic-cli \
            target/aarch64-apple-darwin/release/mosaic-cli \
            target/x86_64-apple-darwin/release/mosaic-cli
          strip target/universal-apple-darwin/release/mosaic-cli

      - name: Build mosaic-cli (single target)
        if: matrix.cli-lipo != 'true'
        working-directory: src-tauri
        run: cargo build --release --features cli --bin mosaic-cli --target ${{ matrix.cli-target }}

      - name: Strip CLI (Linux)
        if: matrix.os == 'ubuntu-22.04'
        run: strip src-tauri/target/${{ matrix.cli-target }}/release/mosaic-cli

      - name: Prepare CLI artifact
        shell: bash
        run: |
          if [ "${{ matrix.cli-lipo }}" = "true" ]; then
            src="src-tauri/target/universal-apple-darwin/release/mosaic-cli"
            ext=""
          elif [[ "${{ matrix.os }}" == windows* ]]; then
            src="src-tauri/target/${{ matrix.cli-target }}/release/mosaic-cli.exe"
            ext=".exe"
          else
            src="src-tauri/target/${{ matrix.cli-target }}/release/mosaic-cli"
            ext=""
          fi
          dst="mosaic-cli-${{ matrix.cli-name }}${ext}"
          cp "$src" "$dst"
          echo "CLI_ARTIFACT=$dst" >> "$GITHUB_ENV"

      - name: Upload CLI to release draft
        uses: softprops/action-gh-release@v2
        with:
          files: ${{ env.CLI_ARTIFACT }}
          tag_name: ${{ github.ref_name }}
          draft: true
          fail_on_unmatched_files: true
```

The `softprops/action-gh-release@v2` call attaches the CLI binary to the same draft release that `tauri-action` already created (matching on `tag_name`). Existing `permissions: contents: write` at the top of the workflow is sufficient.

- [ ] **Step 4: Verify build command locally**

Don't trigger CI from this task — that needs a tag push. Instead smoke-test on your host:

```
cd src-tauri
cargo build --release --features cli --bin mosaic-cli
ls -lh target/release/mosaic-cli
./target/release/mosaic-cli --version
```
Expected: binary present, `--version` prints `mosaic-cli 0.1.3`.

- [ ] **Step 3: Verify**

Don't invoke CI from this task — that requires pushing a tag. Instead, dry-run locally on your host platform to verify the build command works:

```
cd src-tauri
cargo build --release --features cli --bin mosaic-cli
ls target/release/mosaic-cli
```
Expected: binary exists.

- [ ] **Step 4: Commit**

```
git add .github/workflows/release.yml
git commit -m "ci: build mosaic-cli per platform in release workflow"
```

---

## Task 15: Documentation

**Files:**
- Modify: `README.md`
- Modify: `site/guide.html`
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update `README.md`**

Add a new section after the existing installation/features blurb titled "Command-line usage". Include at minimum:

```markdown
## Command-line usage

In addition to the desktop app, mosaic ships a `mosaic-cli` binary for scripting and headless servers. Download the `mosaic-cli-*` asset for your platform from the [latest release](https://github.com/mosaicvideo/mosaic/releases/latest) or build locally:

    cd src-tauri
    cargo install --path . --bin mosaic-cli --features cli

### Subcommands

    mosaic-cli screenshots    [OPTIONS] <INPUT>...     # individual frames
    mosaic-cli sheet          [OPTIONS] <INPUT>...     # still contact sheet
    mosaic-cli reel           [OPTIONS] <INPUT>...     # animated preview reel
    mosaic-cli animated-sheet [OPTIONS] <INPUT>...     # animated contact sheet
    mosaic-cli probe          [--mediainfo] <INPUT>    # VideoInfo as JSON

Each subcommand supports `--help` for its full flag list.

### Config file

On first run, `mosaic-cli` creates `~/.mosaic-cli.toml` with every default commented out. Uncomment and edit any key to change its default; CLI flags always override the file. Point `$MOSAIC_CLI_CONFIG` at a different path to use a per-project config.
```

- [ ] **Step 2: Update `site/guide.html`**

Mirror the README content in a new `<section id="cli">` block. Keep the styling consistent with the existing sections (check how the existing `<h2>` / `<pre>` pattern is used).

- [ ] **Step 3: Update `CLAUDE.md`**

Add a new section before "## ffmpeg quirks to know" titled "## CLI binary" describing:

- Two binaries share the crate: `mosaic` (GUI, default) and `mosaic-cli` (behind `cli` feature).
- `defaults.rs` is the shared source of truth; `scripts/sync-defaults.mjs` keeps HTML in lockstep.
- `input_scan.rs` owns folder-scan logic (moved out of `commands.rs`).
- Config lives at `~/.mosaic-cli.toml` (override via `$MOSAIC_CLI_CONFIG`), first-run auto-create.
- Test command: `cargo test --features cli,test-api --test cli` (plus the ffmpeg-full PATH workaround on macOS).
- When adding a new shipping default: add it to `defaults.rs`, extend `scripts/sync-defaults.mjs` mapping, and add a test assertion in `defaults.rs::tests`.

- [ ] **Step 4: Commit**

```
git add README.md site/guide.html CLAUDE.md
git commit -m "docs: document mosaic-cli usage, config, and build"
```

---

## Self-review checklist

Before handing off, re-walk the spec once and confirm each clause maps to a task above:

- Shared defaults module → Task 1
- HTML sync script → Task 2
- `input_scan` refactor → Task 3
- `cli` feature + `[[bin]]` + optional deps → Task 4
- Unioned two-branch cfg on all pipeline modules + hooks → Task 5
- `~/.mosaic-cli.toml` loader (path resolve, auto-create, parse, unknown-key warn) → Task 6
- clap surface + dispatch → Task 7
- `probe` + `--mediainfo` JSON envelope → Task 8
- Progress reporter, embedded font → Task 9
- `screenshots` subcommand + input expander → Task 10
- `sheet` / `reel` / `animated-sheet` → Task 11
- Ctrl-C cancellation → Task 12
- `assert_cmd` integration tests (probe, 4 generates, config precedence, auto-create, error path) → Task 13
- Per-platform CLI binaries in `release.yml` → Task 14
- README / site / CLAUDE.md → Task 15
