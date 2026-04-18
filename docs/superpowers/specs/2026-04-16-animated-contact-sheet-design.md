# Animated Contact Sheet — Design

**Status:** Shipped in v0.1.1 (commit `cf24027`, 2026-04-16).
**Scope:** Fourth output type, parallel to contact sheet / screenshots / animated preview reel.

## Goal

Produce a single animated WebP that is spatially a grid of `cols × rows` short looping clips sampled at equal positions across the source — the animated analogue of the still contact sheet.

## Module layout

- `src-tauri/src/animated_sheet.rs` — orchestration + filter-graph construction (`build_extract_args`, `build_stitch_args`, `generate`).
- Reuses `drawtext::{font_for_ffmpeg, timestamp_overlay, header_overlay}`, `layout::{compute_sheet_layout, xstack_layout, thumb_height, header_height, line_height, sample_clip_timestamps}`, `ffmpeg::{base_args, seek_input_args_clip, h264_clip_encoder, run_batch_cancellable, run_cancellable, tonemap_filter}`, `header::build_header_lines`. Several of these were extracted from `contact_sheet.rs` / `preview_reel.rs` as a prerequisite.

## Pipeline

Two-phase, matching the still contact sheet's extract-then-stitch split:

**Phase 1 — per-cell extraction (parallel via `run_batch_cancellable`).**

For each sampled timestamp `t_i`:
1. `ffmpeg -ss t_i -i <source> -t clip_length_secs -vf "<tonemap>,scale=W:H[,drawtext timestamp],pad=(W+gap)x(H+gap):gap/2:gap/2:<bg>" -r <fps> -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p tmp/cell_NNN.mp4`
2. Sampling uses `layout::sample_clip_timestamps(dur, n, clip_len)` rather than `sample_timestamps` — leaves `clip_len` headroom at the end so `-t` doesn't produce empty clips on short files. This is the same sampler the preview reel uses.
3. Each cell is pre-padded by `gap` in the extract step (not in stitch) so `xstack` sees uniformly-sized inputs and the `layout=` expression is a simple step-multiple grid.
4. `yuv420p` / `veryfast` / CRF 23 — cheap re-encode, universal decoder compatibility, filter-graph-friendly. Centralised in `ffmpeg::h264_clip_encoder`.

**Phase 2 — stitch into a single `-filter_complex` graph.**

```
[0:v][1:v]…[N-1:v] xstack=inputs=N:layout=<expr> [xs];
[xs] pad=grid_w × grid_h:gap/2:gap/2:<bg> [grid];

# When show_header:
color=c=<bg>:s=grid_w × header_h:d=clip_length_secs:r=fps,
  drawtext=… (one per header line, emitted by header_overlay) [hdr];
[hdr][grid] vstack [out]
```

- Output is mapped from `[out]` when the header is present, `[grid]` otherwise.
- Encoded with `-c:v libwebp -loop 0 -quality <q>` → animated WebP.
- The header panel is a synthesised `lavfi color` source at matching fps + duration so it plays in lockstep with the grid. Static text drawn by the same `drawtext` chain the still sheet uses.

## Layout bounds

xstack caps at 32 inputs (`AV_FILTER_MAX_INPUTS` in libavfilter). Minimum 2 cells (a single-cell grid is degenerate). The orchestrator fails fast outside `[2, 32]` with a friendly message instead of letting ffmpeg emit a cryptic error.

## Options

```rust
pub struct AnimatedSheetOptions {
    pub cols: u32,
    pub rows: u32,
    pub width: u32,                 // total grid width incl. outer pad
    pub gap: u32,                   // inter-cell AND outer pad
    pub clip_length_secs: u32,      // per cell
    pub fps: u32,                   // output frame rate
    pub quality: u32,               // libwebp 0..=100
    pub thumb_font_size: u32,       // drawtext on cells
    pub header_font_size: u32,
    pub show_timestamps: bool,
    pub show_header: bool,
    pub suffix: String,             // filename infix; empty ⇒ DEFAULT_ANIMATED_SHEET_SUFFIX
    pub theme: SheetTheme,          // Dark | Light
}
```

## Progress events

One `job:step` per cell during Phase 1 (`"Cell i/total"`), then one final step for Phase 2 (`"Stitching sheet"`). Total steps = `layout.total + 1`.

## Cancellation

Both phases use `run_batch_cancellable` / `run_cancellable`, which spawn ffmpeg with `kill_on_drop(true)` and race `child.wait()` against the shared `Arc<AtomicBool>` flag. Matches the existing model.

## Output path

`output_path::animated_sheet_path` — same collision-suffix logic as the still sheet, with `DEFAULT_ANIMATED_SHEET_SUFFIX` (`"_animated_sheet"`) as the fallback infix when `opts.suffix` is empty.

## Scope exclusions

- No per-cell encode reuse — each run re-extracts clips. Caching is out of scope.
- No format output choice — animated WebP only (GIF / APNG not worth the bitrate/size tradeoff for a default grid).
- No per-cell start-offset customisation — sampler decides timestamps.
