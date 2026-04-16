# MediaInfo Modal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a per-file MediaInfo viewer to the queue — an icon in each queue row opens a modal displaying raw `mediainfo` CLI output with a copy-to-clipboard button.

**Architecture:** New `locate_mediainfo()` function and two Tauri commands (`check_mediainfo`, `run_mediainfo`) in the Rust backend. Frontend gets a modal component (`mediainfo.js`), an info icon in each queue row, and the reveal-on-click handler is narrowed from the whole row to just the filename.

**Tech Stack:** Rust (Tauri 2 commands, `which` crate), vanilla JS/CSS (no framework)

**Spec:** `docs/superpowers/specs/2026-04-16-mediainfo-modal-design.md`

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src-tauri/src/ffmpeg.rs` | Add `locate_mediainfo()` |
| Modify | `src-tauri/src/commands.rs` | Add `check_mediainfo` and `run_mediainfo` Tauri commands |
| Modify | `src-tauri/src/lib.rs` | Register new commands in `generate_handler![]` |
| Create | `src/mediainfo.js` | Modal DOM creation, open/close logic, copy-to-clipboard |
| Modify | `src/queue.js` | Add info icon button to `buildRow()`, narrow reveal handler to `.name` |
| Modify | `src/main.js` | Import modal, wire Escape key for modal, call `check_mediainfo` at init |
| Modify | `src/style.css` | Modal styles, info icon button styles, updated queue grid columns |

---

### Task 1: Backend — `locate_mediainfo()`

**Files:**
- Modify: `src-tauri/src/ffmpeg.rs:76-107` (after existing `locate_tools()`)

- [ ] **Step 1: Write the test**

Add at the bottom of the `#[cfg(test)] mod tests` block in `src-tauri/src/ffmpeg.rs`:

```rust
#[test]
fn locate_mediainfo_returns_some_when_installed() {
    // Smoke test: if mediainfo is on this machine, we find it.
    // If not installed, the test still passes (returns None).
    let result = locate_mediainfo();
    if which::which("mediainfo").is_ok() {
        assert!(result.is_some());
        assert!(result.unwrap().exists());
    } else {
        assert!(result.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test locate_mediainfo -- --nocapture`
Expected: FAIL — `locate_mediainfo` is not defined yet.

- [ ] **Step 3: Implement `locate_mediainfo()`**

Add this function right after the `locate_tools()` function (after line 107) in `src-tauri/src/ffmpeg.rs`:

```rust
/// Locate the `mediainfo` CLI binary. Returns `None` if not installed.
/// MediaInfo is optional — the app works without it, but the info modal
/// shows install instructions instead of output.
pub fn locate_mediainfo() -> Option<PathBuf> {
    which::which("mediainfo").ok()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test locate_mediainfo -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cd src-tauri && git add src/ffmpeg.rs
git commit -m "feat: add locate_mediainfo() for optional mediainfo binary lookup"
```

---

### Task 2: Backend — Tauri commands `check_mediainfo` and `run_mediainfo`

**Files:**
- Modify: `src-tauri/src/commands.rs:1-18` (imports and top of file)
- Modify: `src-tauri/src/lib.rs:69-80` (command registration)

- [ ] **Step 1: Add `check_mediainfo` command**

Add these two commands in `src-tauri/src/commands.rs`, after the existing `check_tools` command (after line 24):

```rust
#[tauri::command]
pub fn check_mediainfo() -> bool {
    crate::ffmpeg::locate_mediainfo().is_some()
}

#[tauri::command]
pub async fn run_mediainfo(path: String) -> Result<String, String> {
    let bin = crate::ffmpeg::locate_mediainfo().ok_or_else(|| {
        "MediaInfo not found.\n\nInstall it:\n  macOS:   brew install mediainfo\n  Windows: winget install MediaArea.MediaInfo.CLI\n  Linux:   apt install mediainfo".to_string()
    })?;
    let output = std::process::Command::new(&bin)
        .arg(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run mediainfo: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("mediainfo exited with error: {}", stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
```

Note: Uses `std::process::Command` (not tokio) since MediaInfo completes near-instantly and doesn't need cancellation. On Windows, add the `CREATE_NO_WINDOW` flag. Update the import block at the top of the `run_mediainfo` function:

Replace the `run_mediainfo` implementation with this platform-aware version:

```rust
#[tauri::command]
pub async fn run_mediainfo(path: String) -> Result<String, String> {
    let bin = crate::ffmpeg::locate_mediainfo().ok_or_else(|| {
        "MediaInfo not found.\n\nInstall it:\n  macOS:   brew install mediainfo\n  Windows: winget install MediaArea.MediaInfo.CLI\n  Linux:   apt install mediainfo".to_string()
    })?;
    let mut cmd = std::process::Command::new(&bin);
    cmd.arg(&path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output().map_err(|e| format!("Failed to run mediainfo: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("mediainfo exited with error: {}", stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
```

- [ ] **Step 2: Register commands in `lib.rs`**

In `src-tauri/src/lib.rs`, add the two new commands to the `generate_handler![]` macro. The block currently reads:

```rust
.invoke_handler(tauri::generate_handler![
    commands::probe_video,
    commands::check_tools,
    commands::get_video_exts,
    commands::reveal_in_finder,
    commands::scan_folder,
    commands::generate_contact_sheets,
    commands::generate_screenshots,
    commands::generate_preview_reels,
    commands::generate_animated_sheets,
    commands::cancel_job,
])
```

Add the two new entries after `commands::check_tools,`:

```rust
.invoke_handler(tauri::generate_handler![
    commands::probe_video,
    commands::check_tools,
    commands::check_mediainfo,
    commands::run_mediainfo,
    commands::get_video_exts,
    commands::reveal_in_finder,
    commands::scan_folder,
    commands::generate_contact_sheets,
    commands::generate_screenshots,
    commands::generate_preview_reels,
    commands::generate_animated_sheets,
    commands::cancel_job,
])
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles with no errors.

- [ ] **Step 4: Commit**

```bash
cd src-tauri && git add src/commands.rs src/lib.rs
git commit -m "feat: add check_mediainfo and run_mediainfo Tauri commands"
```

---

### Task 3: Frontend — Modal component (`mediainfo.js`)

**Files:**
- Create: `src/mediainfo.js`

- [ ] **Step 1: Create `src/mediainfo.js`**

```javascript
import { invoke } from '@tauri-apps/api/core';

let modal, backdrop, title, pre, copyBtn, copyTimer;

export function createMediaInfoModal() {
  modal = document.createElement('div');
  modal.id = 'mediainfo-modal';
  modal.className = 'hidden';

  backdrop = document.createElement('div');
  backdrop.className = 'modal-backdrop';
  backdrop.onclick = closeMediaInfo;

  const container = document.createElement('div');
  container.className = 'modal-container';

  const header = document.createElement('div');
  header.className = 'modal-header';

  title = document.createElement('span');
  title.className = 'modal-title';

  copyBtn = document.createElement('button');
  copyBtn.className = 'secondary small';
  copyBtn.textContent = 'Copy';
  copyBtn.onclick = onCopy;

  const closeBtn = document.createElement('button');
  closeBtn.className = 'modal-close';
  closeBtn.textContent = '\u00d7';
  closeBtn.title = 'Close';
  closeBtn.onclick = closeMediaInfo;

  header.append(title, copyBtn, closeBtn);

  const body = document.createElement('div');
  body.className = 'modal-body';

  pre = document.createElement('pre');
  body.append(pre);

  container.append(header, body);
  modal.append(backdrop, container);
  document.getElementById('app').append(modal);
}

export async function openMediaInfo(path) {
  if (!modal) return;
  const name = path.replace(/^.*[/\\]/, '');
  title.textContent = name;
  pre.textContent = 'Loading\u2026';
  copyBtn.disabled = true;
  copyBtn.textContent = 'Copy';
  modal.classList.remove('hidden');

  try {
    const text = await invoke('run_mediainfo', { path });
    pre.textContent = text;
    copyBtn.disabled = false;
  } catch (err) {
    pre.textContent = typeof err === 'string' ? err : err?.message || 'Unknown error';
    copyBtn.disabled = true;
  }
}

export function closeMediaInfo() {
  if (modal) modal.classList.add('hidden');
}

export function isMediaInfoOpen() {
  return modal && !modal.classList.contains('hidden');
}

function onCopy() {
  const text = pre?.textContent;
  if (!text) return;
  navigator.clipboard.writeText(text).then(() => {
    copyBtn.textContent = 'Copied!';
    clearTimeout(copyTimer);
    copyTimer = setTimeout(() => { copyBtn.textContent = 'Copy'; }, 1500);
  });
}
```

- [ ] **Step 2: Verify the frontend builds**

Run: `pnpm build:web`
Expected: builds with no errors (module is created but not yet imported — dead code is fine for Vite).

- [ ] **Step 3: Commit**

```bash
git add src/mediainfo.js
git commit -m "feat: add MediaInfo modal component"
```

---

### Task 4: Frontend — Queue row info icon and reveal narrowing

**Files:**
- Modify: `src/queue.js:43-96` (`buildRow` function and row click handler)

- [ ] **Step 1: Add `onInfo` callback parameter to `createQueue`**

In `src/queue.js`, change the `createQueue` function signature on line 16 from:

```javascript
export function createQueue(root, { onReveal, onChange } = {}) {
```

to:

```javascript
export function createQueue(root, { onReveal, onInfo, onChange } = {}) {
```

- [ ] **Step 2: Add info icon button to `buildRow`**

In `src/queue.js` inside the `buildRow` function, add the info button creation after `removeBtn` is created (after line 86) and before the `el.append(...)` line:

```javascript
const infoBtn = document.createElement('button');
infoBtn.className = 'row-info';
infoBtn.title = 'MediaInfo';
infoBtn.innerHTML = '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path stroke="none" d="M0 0h24v24H0z" fill="none"/><path d="M12 2l.117 .007a1 1 0 0 1 .876 .876l.007 .117v4l.005 .15a2 2 0 0 0 1.838 1.844l.157 .006h4l.117 .007a1 1 0 0 1 .876 .876l.007 .117v9a3 3 0 0 1 -2.824 2.995l-.176 .005h-10a3 3 0 0 1 -2.995 -2.824l-.005 -.176v-14a3 3 0 0 1 2.824 -2.995l.176 -.005zm3 14h-6a1 1 0 0 0 0 2h6a1 1 0 0 0 0 -2m0 -4h-6a1 1 0 0 0 0 2h6a1 1 0 0 0 0 -2m-5 -4h-1a1 1 0 1 0 0 2h1a1 1 0 0 0 0 -2"/><path d="M19 7h-4l-.001 -4.001z"/></svg>';
infoBtn.onclick = (e) => {
  e.stopPropagation();
  onInfo?.(it.path);
};
```

- [ ] **Step 3: Update `el.append` and narrow reveal handler**

Change the `el.append` line (currently line 88) from:

```javascript
el.append(idxEl, nameCell, progEl, statusCell, removeBtn);
```

to:

```javascript
el.append(idxEl, nameCell, progEl, statusCell, infoBtn, removeBtn);
```

Then replace the `el.onclick` block (lines 90-93) which currently reads:

```javascript
el.onclick = () => {
  const cur = items.get(it.id);
  if (cur?.status === 'Done' && cur.outputPath) onReveal?.(cur.outputPath);
};
```

with a click handler on `nameEl` instead:

```javascript
nameEl.classList.add('revealable-name');
nameEl.onclick = () => {
  const cur = items.get(it.id);
  if (cur?.status === 'Done' && cur.outputPath) onReveal?.(cur.outputPath);
};
```

- [ ] **Step 4: Update the return value of `buildRow`**

Change the return statement (currently line 95) from:

```javascript
return { el, idxEl, nameEl, metaEl, progEl, statusCell, statusLabel, removeBtn, errorEl: null };
```

to:

```javascript
return { el, idxEl, nameEl, metaEl, progEl, statusCell, statusLabel, infoBtn, removeBtn, errorEl: null };
```

- [ ] **Step 5: Update the `revealable` class in `update()`**

In the `update` function, the `revealable` class is currently toggled on `n.el` (the `<li>`). Since the reveal handler is now on `nameEl`, update line 128 from:

```javascript
n.el.classList.toggle('revealable', it.status === 'Done' && !!it.outputPath);
```

to:

```javascript
n.nameEl.classList.toggle('revealable-name', it.status === 'Done' && !!it.outputPath);
```

And update line 131 from:

```javascript
n.el.classList.add('revealable');
```

to:

```javascript
n.nameEl.classList.add('revealable-name');
```

- [ ] **Step 6: Verify the frontend builds**

Run: `pnpm build:web`
Expected: builds with no errors.

- [ ] **Step 7: Commit**

```bash
git add src/queue.js
git commit -m "feat: add MediaInfo icon to queue rows, narrow reveal to filename"
```

---

### Task 5: Frontend — Wire modal into `main.js`

**Files:**
- Modify: `src/main.js:1-8` (imports), `src/main.js:28-31` (queue creation), `src/main.js:37-48` (init), `src/main.js:131-133` (Escape key handler)

- [ ] **Step 1: Add imports**

Add to the import block at the top of `src/main.js` (after line 7):

```javascript
import { createMediaInfoModal, openMediaInfo, closeMediaInfo, isMediaInfoOpen } from './mediainfo.js';
```

- [ ] **Step 2: Pass `onInfo` to `createQueue`**

Change the `createQueue` call (lines 28-31) from:

```javascript
const queue = createQueue(document.getElementById('queue'), {
  onReveal: (path) => invoke('reveal_in_finder', { path }).catch(console.error),
  onChange: () => { refreshActionBar(); saveSettings(); },
});
```

to:

```javascript
const queue = createQueue(document.getElementById('queue'), {
  onReveal: (path) => invoke('reveal_in_finder', { path }).catch(console.error),
  onInfo: (path) => openMediaInfo(path),
  onChange: () => { refreshActionBar(); saveSettings(); },
});
```

- [ ] **Step 3: Create modal in `init()`**

In the `init()` function, add `createMediaInfoModal();` right before the `loadSettings()` call (before line 45). The relevant section becomes:

```javascript
function init() {
  window.addEventListener('error', (e) => showBanner(`JS error: ${e.message}`));
  window.addEventListener('unhandledrejection', (e) => showBanner(`Promise rejection: ${e.reason?.message || e.reason}`));
  wireButtons();
  wireDropzone(document.getElementById('dropzone'), addPaths);
  wireEvents();
  updateQualityVisibility();
  refreshActionBar();
  createMediaInfoModal();
  loadSettings();
  checkTools();
  getVideoExts();
}
```

- [ ] **Step 4: Wire Escape key for modal**

In the `wireButtons()` function, update the Escape key handler (lines 131-133) from:

```javascript
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && isSettingsOpen()) closeSettings();
});
```

to:

```javascript
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    if (isMediaInfoOpen()) closeMediaInfo();
    else if (isSettingsOpen()) closeSettings();
  }
});
```

- [ ] **Step 5: Verify the frontend builds**

Run: `pnpm build:web`
Expected: builds with no errors.

- [ ] **Step 6: Commit**

```bash
git add src/main.js
git commit -m "feat: wire MediaInfo modal into main app init and keyboard handling"
```

---

### Task 6: Frontend — CSS styles

**Files:**
- Modify: `src/style.css` (add modal styles, info button styles, update queue grid)

- [ ] **Step 1: Update queue grid columns**

In `src/style.css`, change the queue `<li>` grid-template-columns (line 378) from:

```css
grid-template-columns: 30px 1fr 140px 110px 28px;
```

to:

```css
grid-template-columns: 30px 1fr 140px 110px 28px 28px;
```

This adds a 28px column for the info icon between the status cell and the remove button.

- [ ] **Step 2: Update reveal styles**

Replace the existing `.revealable` rules (lines 386-391):

```css
#queue li.revealable {
  cursor: pointer;
}
#queue li.revealable:hover {
  background: var(--elev);
}
```

with:

```css
.revealable-name {
  cursor: pointer;
  border-radius: 2px;
  transition: color var(--t-fast);
}
.revealable-name:hover {
  color: var(--accent);
}
```

- [ ] **Step 3: Add info button styles**

Add after the `.row-remove:hover:not(:disabled)` rule (after line 477):

```css
.row-info {
  padding: 2px;
  color: var(--subtle);
  border-color: transparent;
  background: transparent;
  display: inline-grid;
  place-items: center;
}
.row-info svg {
  width: 15px;
  height: 15px;
}
.row-info:hover:not(:disabled) {
  color: var(--text);
  background: transparent;
}
```

- [ ] **Step 4: Add modal styles**

Add at the end of the file, before the final `.hidden` rule:

```css
/* ─── MediaInfo modal ─── */
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  z-index: 100;
}
.modal-container {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  z-index: 101;
  width: 90vw;
  max-width: 700px;
  max-height: 80vh;
  display: grid;
  grid-template-rows: auto 1fr;
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  overflow: hidden;
}
.modal-header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
}
.modal-title {
  flex: 1;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}
.modal-close {
  padding: 2px 8px;
  font-size: 18px;
  line-height: 1;
  color: var(--subtle);
  border-color: transparent;
  background: transparent;
}
.modal-close:hover {
  color: var(--text);
}
.modal-body {
  overflow: auto;
  padding: 16px;
}
.modal-body pre {
  margin: 0;
  font-family: var(--f-mono);
  font-size: 12px;
  line-height: 1.6;
  color: var(--text);
  white-space: pre;
  tab-size: 4;
}
```

- [ ] **Step 5: Verify the frontend builds**

Run: `pnpm build:web`
Expected: builds with no errors.

- [ ] **Step 6: Commit**

```bash
git add src/style.css
git commit -m "feat: add modal and info icon styles, update queue grid for MediaInfo"
```

---

### Task 7: Manual testing

- [ ] **Step 1: Start the dev server**

Run: `pnpm tauri dev`

- [ ] **Step 2: Test with MediaInfo installed**

1. Drop a video file into the queue
2. Click the file-text icon in the queue row — modal should open showing "Loading..." then the MediaInfo output
3. Click "Copy" — button should change to "Copied!" for 1.5s, paste should contain the raw text
4. Close via: (a) Escape key, (b) x button, (c) clicking backdrop — all three should work
5. Click the filename when a row is "Done" — should reveal the output file in Finder
6. Verify the info icon click does NOT trigger reveal-in-finder

- [ ] **Step 3: Test without MediaInfo installed**

1. Temporarily rename/move the `mediainfo` binary
2. Click the info icon — modal should open with install instructions
3. Copy button should be disabled in error state

- [ ] **Step 4: Test both themes**

1. Switch system appearance to Light mode
2. Open the modal — verify colors use theme tokens correctly
3. Switch back to Dark mode

- [ ] **Step 5: Commit any fixes**

If manual testing surfaced issues, fix and commit them individually.

---

### Task 8: Final verification

- [ ] **Step 1: Run Rust checks**

```bash
cd src-tauri && cargo clippy -- -D warnings && cargo test
```

Expected: no warnings, all tests pass.

- [ ] **Step 2: Run frontend build**

```bash
pnpm build:web
```

Expected: no errors.

- [ ] **Step 3: Verify clean git state**

```bash
git status
git log --oneline -10
```

Confirm all changes are committed and the log looks clean.
