# MediaInfo Modal

> **Status:** Superseded in part by `2026-04-17-displayed-dims-and-enrichment-design.md` (v0.1.3).
>
> The **optional-MediaInfo** model described below (`locate_mediainfo()`, `check_mediainfo` Tauri command, `run_mediainfo` returning install instructions on `Err`) no longer reflects the code. In v0.1.3 MediaInfo was promoted to a first-party prerequisite: `locate_tools()` resolves it alongside ffmpeg/ffprobe, `check_mediainfo` is gone, and `run_mediainfo` now assumes the binary exists. The modal itself (queue-row icon, frontend structure, keyboard dismissal, copy button) is still accurate — read this doc for that part only.
>
> For the enrichment pipeline that parses MediaInfo JSON into contact-sheet header fields, see the v0.1.3 design doc.

Add a per-file MediaInfo viewer to the queue. An icon in each queue row opens a modal displaying raw `mediainfo` CLI output with a copy-to-clipboard button.

## Backend

### Tool location

Add `locate_mediainfo()` in `ffmpeg.rs` using `which::which("mediainfo")`. Returns `Option<PathBuf>` — MediaInfo is optional, unlike ffmpeg/ffprobe which are required.

### Tauri commands

**`check_mediainfo() -> bool`**
Calls `locate_mediainfo()` and returns whether the binary was found. Called once at frontend init. No error banner — result only affects the info icon's behavior.

**`run_mediainfo(path: String) -> Result<String, String>`**
Locates the `mediainfo` binary. If not found, returns an `Err` with install instructions. If found, runs `mediainfo <path>` via `std::process::Command`, captures stdout, and returns the text. No cancellation needed — MediaInfo completes in under a second.

### What doesn't change

- `check_tools` / `locate_tools` — ffmpeg/ffprobe checking is unchanged
- `Tools` struct — MediaInfo is independent, not bundled into it
- `VideoInfo` / probe pipeline — unrelated

## Frontend

### Queue row icon

Each queue row gets an info icon button using the filled file-text SVG:

```svg
<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
  <path stroke="none" d="M0 0h24v24H0z" fill="none" />
  <path d="M12 2l.117 .007a1 1 0 0 1 .876 .876l.007 .117v4l.005 .15a2 2 0 0 0 1.838 1.844l.157 .006h4l.117 .007a1 1 0 0 1 .876 .876l.007 .117v9a3 3 0 0 1 -2.824 2.995l-.176 .005h-10a3 3 0 0 1 -2.995 -2.824l-.005 -.176v-14a3 3 0 0 1 2.824 -2.995l.176 -.005zm3 14h-6a1 1 0 0 0 0 2h6a1 1 0 0 0 0 -2m0 -4h-6a1 1 0 0 0 0 2h6a1 1 0 0 0 0 -2m-5 -4h-1a1 1 0 1 0 0 2h1a1 1 0 0 0 0 -2" />
  <path d="M19 7h-4l-.001 -4.001z" />
</svg>
```

**Placement:** Between the status cell and the remove (x) button in `buildRow()`.

**Behavior:**
- Always visible, always clickable — not gated by row status or MediaInfo availability
- On click: calls `invoke('run_mediainfo', { path })`, opens the modal with the result

### Reveal-on-click narrowing

Currently the entire `<li>` row has an `onclick` that calls `reveal_in_finder` when the row status is "Done". With the info icon now in the row, this must be narrowed: move the reveal handler from `el.onclick` to `nameEl.onclick` (the `.name` span). Only clicking the filename opens the output folder. This prevents the info icon click from triggering reveal, and avoids needing `e.stopPropagation()` hacks.

### Modal

A single shared `<div id="mediainfo-modal">` element, added to `#app`.

**Structure:**
```
#mediainfo-modal (hidden by default)
  .modal-backdrop          — semi-transparent overlay, click to dismiss
  .modal-container         — centered panel, max-width 700px, max-height 80vh
    .modal-header
      .modal-title         — filename
      button.modal-copy    — "Copy" (copies raw text to clipboard)
      button.modal-close   — "x" close button
    .modal-body
      pre                  — raw MediaInfo output, monospace, vertical scroll
```

**States:**
- **Loading:** While `run_mediainfo` is in-flight, the `<pre>` shows "Loading..."
- **Success:** `<pre>` contains raw MediaInfo text output
- **Error:** `<pre>` replaced with install instructions:
  - macOS: `brew install mediainfo`
  - Windows: `winget install mediainfo`
  - Linux: `apt install mediainfo`

**Dismissal:** Click backdrop, click x button, or press Escape. The existing `keydown` listener for Escape (which closes Settings) is extended to also close this modal (modal takes priority if both are open, though in practice they won't overlap).

**Copy button:** Uses `navigator.clipboard.writeText()`. Button text briefly changes to "Copied!" on success, then reverts after ~1.5 seconds.

### Styling

Uses existing CSS variable tokens so both dark and light themes work:

- Backdrop: `rgba(0, 0, 0, 0.5)`
- Modal panel: `var(--panel)` background, `var(--border)` border
- Text: `var(--text)` for the `<pre>` content
- Copy/close buttons: styled like existing `.secondary` buttons
- `<pre>`: `font-family: monospace`, `white-space: pre`, `overflow-y: auto`

### CSS grid impact

The modal is positioned `fixed` over the entire viewport — it doesn't affect the existing two-view grid layout (`#main-view` / `#settings-view`).

## Scope exclusions

- No changes to `check_tools` or the tools-missing UI state
- No changes to settings persistence
- No parsed/sectioned MediaInfo output — raw text only
- No caching of MediaInfo results — re-runs on each click
- MediaInfo binary is not bundled — requires user installation
