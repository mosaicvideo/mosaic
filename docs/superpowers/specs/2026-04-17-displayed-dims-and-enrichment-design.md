# Displayed Dimensions, MediaInfo Enrichment, Multi-Line Header — Design

**Status:** Shipped in v0.1.3 (commit `d6ccefb`, 2026-04-17).
**Scope:** Three coordinated changes that fix aspect-ratio bugs on anamorphic / rotated / 3D-Bluray sources, add MediaInfo-derived metadata to the contact-sheet header, and restructure the header into one section per line.

## 1. Displayed dimensions

### Problem

`VideoStream.width`/`height` historically carried **encoded** pixel dims straight from ffprobe. That produced wrong aspect on three input classes:

- **Anamorphic:** 9:16 phone clip encoded in a 1080×1080 frame with SAR `9:16` — sheets came out as stretched squares.
- **Rotated:** portrait iPhone video encoded 1920×1080 with Display Matrix rotation -90 — sheets came out landscape with upside-down / sideways content depending on format.
- **3D Blu-ray MVC:** ffprobe lists a zero-dim dependent-enhancement stream first; taking it produced sheets with thumb_h collapsed to a square.

PNG/JPEG/WebP containers don't carry SAR metadata (or carry it in ways most viewers ignore), so per-pipeline `scale=W:-2` auto-height wasn't enough — we had to compute the displayed grid explicitly and emit `scale=W:H`.

### Fix

`video_info::parse` now returns **displayed square-pixel** dimensions:

```rust
fn displayed_dims(enc_w, enc_h, sar: Option<(u32, u32)>, rotation: Option<i32>) -> (u32, u32) {
    let w = match sar {
        Some((num, den)) if den > 0 => round_even(enc_w as f64 * num as f64 / den as f64),
        _ => enc_w,
    };
    let h = enc_h;
    let swap = matches!(rotation, Some(r) if r.abs() % 180 == 90);
    if swap { (h, w) } else { (w, h) }
}
```

- `sar` parsed from `stream.sample_aspect_ratio`. `"N/A"`, `"1:1"`, `"0:1"` collapse to `None` so the transform is a no-op on square-pixel sources.
- `rotation` parsed from the `Display Matrix` side-data entry (`rotation: i32` in ffprobe JSON).
- `round_even` floors to an even non-negative integer ≥ 2 so `yuv420p` subsampling is happy.
- 3D MVC fix: prefer the first video stream with **real** dims over the first video stream at all. Zero-dim dependent layer is skipped. The base-layer AVC track has real dims and wins.

Both `rotation` and `sar` are kept on `VideoStream` (`sar: Option<(u32, u32)>`, `rotation: Option<i32>` — the latter `#[serde(skip)]` because only the Rust side needs it).

### Threading through pipelines

- `layout::thumb_height(thumb_w, src_w, src_h)` and `layout::thumb_width(thumb_h, src_w, src_h)` centralise the aspect math. Moved from `animated_sheet.rs` for reuse.
- `contact_sheet.rs` + `animated_sheet.rs`: compute `thumb_h` from displayed dims, emit `scale=W:H` (was `scale=W:-2`).
- `preview_reel.rs`: gate on `info.video.sar.is_some()` in addition to `height > target_height`, so anamorphic sources always get scaled out to square pixels even when the height fits.
- `screenshots.rs`: emit a `scale=W:H` when `sar.is_some()` (rotation alone is handled by ffmpeg's autorotate — the decoded frame is already upright; SAR is the only reason images need explicit resizing).

### Tests

`video_info.rs` adds ten unit tests covering SAR 9:16, SAR 1:1 no-op, SAR N/A, rotation ±90 / 180, combined SAR+rotation, DOVI + rotation coexistence, 3D MVC stream skip, and `round_even` edge cases. `preview_reel.rs` adds anamorphic + small-source regression cases. `layout.rs` tests cover landscape / portrait / anamorphic / zero-dim inputs.

## 2. MediaInfo → first-party prerequisite + enrichment

### Prerequisite promotion

v0.1.1 added MediaInfo as an **optional** per-file metadata viewer (see `2026-04-16-mediainfo-modal-design.md`). v0.1.3 promoted it to a required tool:

- `ffmpeg::Tools` gains `mediainfo: PathBuf`.
- `locate_tools()` resolves all three binaries with the same Homebrew-priority + `which::which` + fallback-paths search. Missing any of them fails `check_tools`.
- `ToolsError` has a `MediaInfo` variant.
- `locate_mediainfo()` standalone function and `check_mediainfo` Tauri command are removed.
- `run_mediainfo` no longer returns install instructions on `Err` — it assumes the binary exists (guaranteed by the startup check).

### Enrichment pipeline

`src-tauri/src/mediainfo.rs` (new module) parses `mediainfo --Output=JSON` into:

```rust
pub struct Enrichment {
    pub container_format: Option<String>,        // "Matroska", "MPEG-4"
    pub title: Option<String>,                   // General.Title / General.Movie
    pub video_bit_depth: Option<u8>,             // 8 / 10 / 12
    pub video_hdr_format: Option<String>,        // "Dolby Vision / HDR10"
    pub audio_commercial_name: Option<String>,   // "DTS-HD MA", "Dolby Atmos"
    pub audio_channel_layout: Option<String>,    // "5.1", "7.1", "stereo"
    pub audio_language: Option<String>,          // "en", "ja"
}
```

Every field is independently optional. The parser silently drops fields it can't parse — MediaInfo's JSON format is version-dependent and unknown fields shouldn't break the header.

### Concurrent probe

`commands::probe` runs ffprobe + MediaInfo in parallel with `tokio::join!`:

```rust
let (ffprobe_res, enrichment) = tokio::join!(
    run_capture(&tools.ffprobe, &ffprobe_args),
    probe_mediainfo(&tools.mediainfo, path),
);
```

Both probes hit the same file, but each is IO-bound on its own read path, so parallelism roughly halves drag-and-drop probe latency. Enrichment failure is best-effort: `probe_mediainfo` returns `None` on any error and the header falls back to ffprobe fields.

`VideoInfo` gains `enrichment: Option<Enrichment>` (skipped in serialisation when `None`).

## 3. Multi-line header

### Shape change

`build_header_lines` returned `(String, String)` in v0.1.2. Now returns `Vec<String>`:

1. **Line 1 — title or filename.** Prefers `enrichment.title` over the file basename when present.
2. **Line 2 — file meta.** `Size: X GiB | Duration: HH:MM:SS | Bitrate: Y Mb/s`.
3. **Line 3 — video.** `Video: codec (profile) | WxH | bit_depth | hdr_format | bitrate | fps`, with `bit_depth` and `hdr_format` only present when enrichment provides them.
4. **Line 4 — audio** (omitted when no audio stream). `Audio: commercial_name | Hz | channel_layout | bitrate [lang]`. Commercial name, channel layout, and the bracketed language suffix all come from enrichment; ffprobe fields are fallbacks.

### Why per-section lines

v0.1.2 concatenated everything onto a single "line 2" with `  |  ` separators. On narrow grids (animated sheet defaults to 1280px; preview reel 853×480) the audio segment routinely clipped off the right edge.

### Drawtext + layout changes

- `drawtext::header_overlay` takes `&[String]` and emits one `drawtext=` node per line at `y = gap + i * line_h`.
- `layout::header_height(font, gap, lines: u32)` replaces the hard-coded 2-line height.
- All four pipelines updated to pass `lines.len() as u32` so the header panel fits exactly, regardless of audio presence or enrichment state.

## 4. Side effect: `has_zscale` moved off `Tools`

`Tools.has_zscale: bool` was eagerly computed at `locate_tools()` time. Replaced with `Tools::detect_has_zscale()` — the probe is now called once per batch in `commands::run_batch` and passed into each `PipelineContext`. Rationale: probing requires spawning `ffmpeg -filters`, which is cheap but not free, and the answer is invariant across a batch anyway. Keeping it off per-file hot paths (`probe_video`, `run_mediainfo`) matters for drag-and-drop responsiveness.

## Scope exclusions

- No UI for manually overriding detected SAR / rotation — if ffprobe is wrong, the thumbnail is wrong.
- No enrichment caching — re-probed on every job. MediaInfo is fast enough that this doesn't matter.
- No per-track MediaInfo (e.g. multiple audio tracks) — the header shows the first audio stream ffprobe found, with enrichment from whichever audio track MediaInfo saw first.
- No schema version pinning on MediaInfo JSON — we treat every field as optional so MediaInfo upgrades don't break us.
