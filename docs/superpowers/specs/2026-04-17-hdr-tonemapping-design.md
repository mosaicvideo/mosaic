# HDR Tonemapping & Dolby Vision — Design

**Status:** Shipped in v0.1.1 across commits `2eaba2d` (auto-tonemap, 2026-04-16), `75598d3` (explicit zscale colorspace), `631df07` (skip unknown transfer / DV P5), `f97b985` (API simplification), and `2e1fe1f` (DV P5 color correction, 2026-04-17).

**Scope:** Auto-tonemap HDR10, HLG, and Dolby Vision streams to SDR BT.709 across every pipeline (contact sheet, screenshots, preview reel, animated sheet) so thumbnails render in normal colours on any display, without user configuration.

## Problem

Raw HDR frames written into an 8-bit SDR container look wrong — HDR10 thumbnails come out dim and desaturated, HLG looks washed out, Dolby Vision Profile 5 reads as garishly green/purple because ffmpeg interprets its IPT-PQ-C2 pixels as YCbCr. We want one filter that detects the source format from probe data and produces a correct BT.709 frame.

## The single entry point

`ffmpeg::tonemap_filter(has_zscale: bool, color_transfer: Option<&str>, dv_profile: Option<u8>) -> Option<String>`

Every extraction site calls this with the probe's `color_transfer` + `dv_profile` plus the pipeline-wide `has_zscale` flag. `None` means "no extra filter needed"; otherwise the returned string is prepended to the `-vf` chain.

## Decision table

| Input | `has_zscale` | Result |
|---|---|---|
| `dv_profile == Some(5)` | any | `DV_P5_COLOR_MATRIX` (colorchannelmixer) |
| `color_transfer == Some("smpte2084")` (PQ / HDR10) | `true` | full zscale tonemap chain |
| `color_transfer == Some("arib-std-b67")` (HLG) | `true` | full zscale tonemap chain |
| PQ / HLG | `false` | `None` — silently skip |
| unknown / `None` / `bt709` / anything else | any | `None` |

The `color_transfer` constants live in `video_info.rs` (`PQ_TRANSFER`, `HLG_TRANSFER`) so there's one source of truth.

## Filter chains

**HDR10 / HLG (zscale):**
```
zscale=tin={tin}:min=bt2020nc:pin=bt2020:t=linear:npl=100,
format=gbrpf32le,
zscale=p=bt709,
tonemap=hable:desat=0,
zscale=t=bt709:m=bt709:r=tv,format=yuv420p
```

Notes:
- `tin` (transfer-in) is set explicitly (`smpte2084` / `arib-std-b67`) — `fix: specify explicit zscale input colorspace` (`75598d3`) added this because a bare `zscale=t=linear` triggered "no path between colorspaces" errors on some PQ sources where the input side wasn't inferrable.
- `min=bt2020nc:pin=bt2020` pins both matrix and primaries so the RGB conversion is unambiguous.
- `format=gbrpf32le` between the two zscale calls forces 32-bit float RGB, which the `tonemap=hable` operator expects.
- `npl=100` (nominal peak luminance 100 nits) is an aggressive target that compresses HDR dynamic range into SDR — deliberate tradeoff for contact-sheet thumbnails where blown-out highlights are worse than compressed ones.
- Hable tonemap with `desat=0` — keeps colours closer to the highlight hue rather than desaturating as luminance compresses.

**Dolby Vision Profile 5 (colorchannelmixer):**

```rust
const DV_P5_COLOR_MATRIX: &str = "colorchannelmixer=\
    rr=0.2938:rg=0.3557:rb=0.3504:\
    gr=0.3508:gg=0.7312:gb=-0.0821:\
    br=-0.1610:bg=1.0337:bb=0.1275";
```

Derived from libplacebo's IPT coefficients (Ebner & Fairchild 1998 inverse matrix, BT.2020 LMS → RGB Hunt-Pointer-Estevez transform, 2% crosstalk). Why a matrix instead of zscale:

- DV Profile 5 pixels are IPT-PQ-C2, not YCbCr. ffmpeg's zscale path assumes YCbCr input and produces garbage.
- The matrix correction works on any ffmpeg build (no zscale / libzimg required). That's a portability win: Windows / distro ffmpegs that lack libzimg still produce correct colours on DV P5.
- The tradeoff is that PQ gamma isn't inverted, so the result is slightly washed out. Acceptable for thumbnails; not acceptable for production output (not our use case).

DV P5 branch runs **before** the zscale guard so a source with `has_zscale=false` still gets corrected. DV P5 also typically has `color_transfer = None` in ffprobe, which is why the simpler "only PQ/HLG get tonemapped" rule wasn't sufficient — the DV branch needed an independent signal (`dv_profile` from side data).

## `has_zscale` probing

`Tools::detect_has_zscale()` spawns `ffmpeg -filters` once per batch (in `commands::run_batch`) and greps for `zscale` in the output. Not per-file — that would cost a subprocess spawn per queue item. The value is immutable per `Tools` instance; re-check only happens when the user retries the tools-missing state.

## Probe integration

`video_info::VideoStream` carries `color_transfer: Option<String>` and `dv_profile: Option<u8>`:
- `color_transfer` from `stream.color_transfer` in ffprobe JSON.
- `dv_profile` from the `DOVI configuration record` side data's `dv_profile` field.
- `is_hdr` is a derived bool (true if transfer is PQ/HLG OR there's a DOVI record) — used for UI display only, not for tonemapping routing.

## Call sites

All four orchestration modules build `tonemap_filter(…)` once per file and prepend the result to their `-vf` chain:

- `contact_sheet.rs` — extract step.
- `screenshots.rs` — per-frame extract.
- `preview_reel.rs` — clip extract.
- `animated_sheet.rs` — per-cell extract.

## Test coverage

`ffmpeg.rs` has unit tests for every branch: PQ+zscale, HLG+zscale, PQ without zscale (returns `None`), SDR (returns `None`), unknown transfer (returns `None`), DV P5 with and without zscale (always returns the matrix), DV P5 matrix identity. No subprocess — all string-level assertions.

## Scope exclusions

- No Dolby Vision Profile 7 / 8 / 10 — those advertise standard HDR10 base layers and flow through the PQ path naturally. P5 is the only one that needs the matrix correction.
- No dynamic metadata (HDR10+, per-frame DV) — treated as static HDR10/PQ. Thumbnails are stills; per-scene tone curves aren't worth the complexity.
- No user knob for tonemap target / style — `hable:desat=0:npl=100` is opinionated for thumbnails.
- No 10-bit output — everything finishes in `yuv420p` for compatibility.
