use serde::{Deserialize, Serialize};

/// Transfer functions that indicate HDR content requiring tonemapping.
pub const PQ_TRANSFER: &str = "smpte2084";
pub const HLG_TRANSFER: &str = "arib-std-b67";

/// ffprobe's human-readable tag for Dolby Vision configuration side data.
const SIDE_DATA_DOVI: &str = "DOVI configuration record";
/// ffprobe's human-readable tag for rotation metadata (Display Matrix).
const SIDE_DATA_DISPLAY_MATRIX: &str = "Display Matrix";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoStream {
    pub codec: String,
    pub profile: Option<String>,
    /// Displayed (square-pixel) width: encoded width with SAR applied and
    /// rotation-swapped when ±90°/±270°. Use this for anything that reasons
    /// about the picture the user actually sees.
    pub width: u32,
    /// Displayed (square-pixel) height: rotation-swapped when ±90°/±270°.
    pub height: u32,
    pub fps: f64,
    pub bit_rate: Option<u64>,
    pub is_hdr: bool,
    /// Raw `color_transfer` tag from ffprobe (e.g. "smpte2084", "arib-std-b67").
    /// Passed to `tonemap_filter` so zscale gets explicit input transfer params.
    pub color_transfer: Option<String>,
    /// Dolby Vision profile number from ffprobe side_data (e.g. 5, 7, 8).
    /// Profile 5 requires IPT-PQ-C2 → BT.709 color correction.
    pub dv_profile: Option<u8>,
    /// Raw Display Matrix rotation in degrees (e.g. -90, 90, 180). Only used
    /// during parsing to compute displayed dims; kept on the struct for test
    /// assertions and potential future call sites. `#[serde(skip)]` because
    /// the frontend and JS consumers read displayed `width`/`height` already.
    #[serde(skip)]
    pub rotation: Option<i32>,
    /// Sample aspect ratio as (num, den) when pixels are non-square. None when
    /// ffprobe reports `N/A`, `0:1`, or `1:1` (all equivalent to square pixels).
    pub sar: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioStream {
    pub codec: String,
    pub profile: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub bit_rate: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoInfo {
    pub filename: String,
    pub duration_secs: f64,
    pub size_bytes: Option<u64>,
    pub bit_rate: Option<u64>,
    pub video: VideoStream,
    pub audio: Option<AudioStream>,
    /// Optional MediaInfo-derived enrichment. Populated by
    /// [`crate::commands::probe`] when the `mediainfo` binary is available;
    /// absent when it isn't installed or when its output fails to parse.
    /// Header rendering must treat this as best-effort enrichment — the
    /// ffprobe fields above remain the primary source of truth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrichment: Option<crate::mediainfo::Enrichment>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeParseError {
    #[error("invalid ffprobe JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no video stream")]
    NoVideo,
    #[error("missing duration")]
    MissingDuration,
}

#[derive(Deserialize)]
struct RawSideData {
    side_data_type: Option<String>,
    dv_profile: Option<u8>,
    rotation: Option<i32>,
}

#[derive(Deserialize)]
struct RawRoot {
    streams: Vec<RawStream>,
    format: RawFormat,
}

#[derive(Deserialize)]
struct RawStream {
    codec_type: String,
    codec_name: Option<String>,
    profile: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    bit_rate: Option<String>,
    color_transfer: Option<String>,
    sample_aspect_ratio: Option<String>,
    side_data_list: Option<Vec<RawSideData>>,
}

#[derive(Deserialize)]
struct RawFormat {
    filename: Option<String>,
    duration: Option<String>,
    size: Option<String>,
    bit_rate: Option<String>,
}

fn parse_fraction(s: &str) -> Option<f64> {
    let mut it = s.split('/');
    let num: f64 = it.next()?.parse().ok()?;
    let den: f64 = it.next().unwrap_or("1").parse().ok()?;
    if den == 0.0 { None } else { Some(num / den) }
}

/// Parse ffprobe's `sample_aspect_ratio` field (e.g. `"9:16"`, `"1:1"`, `"N/A"`)
/// into `(num, den)`. Returns `None` for missing, malformed, zero-denominator,
/// and square-pixel cases (`1:1`, `0:1`) so that callers can skip the SAR
/// transformation entirely when it would be a no-op.
fn parse_sar(raw: Option<&str>) -> Option<(u32, u32)> {
    let s = raw?;
    let mut it = s.split(':');
    let num: u32 = it.next()?.parse().ok()?;
    let den: u32 = it.next()?.parse().ok()?;
    if num == 0 || den == 0 || num == den { return None; }
    Some((num, den))
}

/// Floor `x` to the nearest even non-negative integer, clamped to a minimum of
/// 2. Used to keep scaled dimensions `yuv420p`-friendly while guaranteeing we
/// never collapse to a zero-pixel edge on pathological inputs.
fn round_even(x: f64) -> u32 {
    let n = x.max(0.0).floor() as u32;
    let n = n - (n % 2);
    n.max(2)
}

/// Apply SAR (anamorphic pixels) and Display Matrix rotation to encoded
/// dimensions, yielding the square-pixel dimensions of the image the user
/// actually sees. SAR multiplies the width; rotation ±90°/±270° swaps the
/// axes afterwards.
fn displayed_dims(encoded_w: u32, encoded_h: u32, sar: Option<(u32, u32)>, rotation: Option<i32>) -> (u32, u32) {
    let w = match sar {
        Some((num, den)) if den > 0 => round_even(encoded_w as f64 * num as f64 / den as f64),
        _ => encoded_w,
    };
    let h = encoded_h;
    let swap = matches!(rotation, Some(r) if r.abs() % 180 == 90);
    if swap { (h, w) } else { (w, h) }
}

pub fn parse(json: &str) -> Result<VideoInfo, ProbeParseError> {
    let root: RawRoot = serde_json::from_str(json)?;

    let duration_secs = root
        .format
        .duration
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|d| *d > 0.0)
        .ok_or(ProbeParseError::MissingDuration)?;

    let filename = root.format.filename.unwrap_or_default();
    let size_bytes = root.format.size.as_deref().and_then(|s| s.parse().ok());
    let bit_rate = root.format.bit_rate.as_deref().and_then(|s| s.parse().ok());

    // 3D Blu-ray / MVC streams expose a zero-dim dependent-enhancement view as
    // the first `video` stream; the usable base layer is second. Prefer the
    // first video stream with real dimensions so downstream scaling uses the
    // playable stream. Falls back to any video stream if none have dims, so
    // parse errors still surface the original `NoVideo` for truly non-video
    // inputs.
    let v = root
        .streams
        .iter()
        .find(|s| s.codec_type == "video" && s.width.unwrap_or(0) > 0 && s.height.unwrap_or(0) > 0)
        .or_else(|| root.streams.iter().find(|s| s.codec_type == "video"))
        .ok_or(ProbeParseError::NoVideo)?;

    let dovi_record = v.side_data_list.as_ref().and_then(|list|
        list.iter().find(|sd| sd.side_data_type.as_deref() == Some(SIDE_DATA_DOVI))
    );
    let dv_profile = dovi_record.and_then(|sd| sd.dv_profile);
    let is_hdr = matches!(v.color_transfer.as_deref(), Some(PQ_TRANSFER) | Some(HLG_TRANSFER))
        || dovi_record.is_some();

    let rotation = v.side_data_list.as_ref().and_then(|list|
        list.iter().find(|sd| sd.side_data_type.as_deref() == Some(SIDE_DATA_DISPLAY_MATRIX))
            .and_then(|sd| sd.rotation)
    );
    let sar = parse_sar(v.sample_aspect_ratio.as_deref());
    let (disp_w, disp_h) = displayed_dims(
        v.width.unwrap_or(0),
        v.height.unwrap_or(0),
        sar,
        rotation,
    );

    let video = VideoStream {
        codec: v.codec_name.clone().unwrap_or_default(),
        profile: v.profile.clone(),
        width: disp_w,
        height: disp_h,
        fps: v.r_frame_rate.as_deref().and_then(parse_fraction).unwrap_or(0.0),
        bit_rate: v.bit_rate.as_deref().and_then(|s| s.parse().ok()),
        is_hdr,
        color_transfer: v.color_transfer.clone(),
        dv_profile,
        rotation,
        sar,
    };

    let audio = root.streams.iter().find(|s| s.codec_type == "audio").map(|a| AudioStream {
        codec: a.codec_name.clone().unwrap_or_default(),
        profile: a.profile.clone(),
        sample_rate: a.sample_rate.as_deref().and_then(|s| s.parse().ok()),
        channels: a.channels,
        bit_rate: a.bit_rate.as_deref().and_then(|s| s.parse().ok()),
    });

    Ok(VideoInfo { filename, duration_secs, size_bytes, bit_rate, video, audio, enrichment: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{}", name)).unwrap()
    }

    #[test]
    fn parses_typical_mkv() {
        let info = parse(&fixture("ffprobe_typical.json")).unwrap();
        assert_eq!(info.filename, "/tmp/movie.mkv");
        assert!((info.duration_secs - 7234.5).abs() < 1e-9);
        assert_eq!(info.size_bytes, Some(5368709120));
        assert_eq!(info.bit_rate, Some(5200000));
        assert_eq!(info.video.codec, "h264");
        assert_eq!(info.video.profile.as_deref(), Some("High"));
        assert_eq!(info.video.width, 1920);
        assert_eq!(info.video.height, 1080);
        assert!((info.video.fps - 24000.0 / 1001.0).abs() < 1e-4);
        assert_eq!(info.video.bit_rate, Some(5000000));
        let audio = info.audio.unwrap();
        assert_eq!(audio.codec, "aac");
        assert_eq!(audio.sample_rate, Some(48000));
        assert_eq!(audio.channels, Some(2));
    }

    #[test]
    fn parses_video_without_audio() {
        let info = parse(&fixture("ffprobe_no_audio.json")).unwrap();
        assert!(info.audio.is_none());
        assert_eq!(info.video.width, 3840);
        assert!((info.video.fps - 30.0).abs() < 1e-9);
    }

    #[test]
    fn fails_when_duration_missing() {
        let err = parse(&fixture("ffprobe_missing_duration.json")).unwrap_err();
        matches!(err, ProbeParseError::MissingDuration);
    }

    #[test]
    fn fails_on_invalid_json() {
        let err = parse("not json").unwrap_err();
        matches!(err, ProbeParseError::Json(_));
    }

    #[test]
    fn detects_dolby_vision_from_side_data() {
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "hevc", "width": 1920, "height": 1080,
                  "r_frame_rate": "24/1",
                  "side_data_list": [{ "side_data_type": "DOVI configuration record" }] }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert!(info.video.is_hdr);
        assert_eq!(info.video.dv_profile, None); // no dv_profile field in JSON
    }

    #[test]
    fn detects_dv_profile_5() {
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "hevc", "width": 1920, "height": 1080,
                  "r_frame_rate": "24/1",
                  "side_data_list": [{ "side_data_type": "DOVI configuration record", "dv_profile": 5 }] }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert!(info.video.is_hdr);
        assert_eq!(info.video.dv_profile, Some(5));
    }

    #[test]
    fn detects_hlg_from_color_transfer() {
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "hevc", "width": 3840, "height": 2160,
                  "r_frame_rate": "50/1", "color_transfer": "arib-std-b67" }
            ],
            "format": { "duration": "60.0" }
        }"#;
        let info = parse(json).unwrap();
        assert!(info.video.is_hdr);
    }

    #[test]
    fn detects_hdr10_from_color_transfer() {
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "hevc", "width": 3840, "height": 2160,
                  "r_frame_rate": "24/1", "color_transfer": "smpte2084" }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert!(info.video.is_hdr);
    }

    #[test]
    fn sdr_is_not_hdr() {
        let info = parse(&fixture("ffprobe_typical.json")).unwrap();
        assert!(!info.video.is_hdr);
    }

    // --- displayed-dim transformation tests ---
    //
    // `width`/`height` on VideoStream must represent *displayed square-pixel*
    // dimensions so that every downstream site (thumbnail sizing, queue-row
    // meta, header text) renders the aspect the user actually sees — not the
    // encoded shape.

    #[test]
    fn sar_9_16_multiplies_width() {
        // Real sample: 1080×1080 square-encoded but displayed as 9:16 portrait.
        // 1080 × 9/16 = 607.5 → round_even = 606.
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "h264", "width": 1080, "height": 1080,
                  "r_frame_rate": "30/1", "sample_aspect_ratio": "9:16" }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert_eq!(info.video.width, 606);
        assert_eq!(info.video.height, 1080);
        assert_eq!(info.video.sar, Some((9, 16)));
        assert_eq!(info.video.rotation, None);
    }

    #[test]
    fn rotation_minus_90_swaps_width_height() {
        // Phone portrait: encoded 1920×1080, Display Matrix -90 → displayed 1080×1920.
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080,
                  "r_frame_rate": "30/1",
                  "side_data_list": [{ "side_data_type": "Display Matrix", "rotation": -90 }] }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert_eq!(info.video.width, 1080);
        assert_eq!(info.video.height, 1920);
        assert_eq!(info.video.rotation, Some(-90));
    }

    #[test]
    fn rotation_plus_90_also_swaps() {
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080,
                  "r_frame_rate": "30/1",
                  "side_data_list": [{ "side_data_type": "Display Matrix", "rotation": 90 }] }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert_eq!((info.video.width, info.video.height), (1080, 1920));
    }

    #[test]
    fn rotation_180_does_not_swap() {
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080,
                  "r_frame_rate": "30/1",
                  "side_data_list": [{ "side_data_type": "Display Matrix", "rotation": 180 }] }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert_eq!((info.video.width, info.video.height), (1920, 1080));
        assert_eq!(info.video.rotation, Some(180));
    }

    #[test]
    fn sar_1_1_is_noop() {
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080,
                  "r_frame_rate": "30/1", "sample_aspect_ratio": "1:1" }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert_eq!((info.video.width, info.video.height), (1920, 1080));
        assert_eq!(info.video.sar, None, "1:1 SAR collapses to None to skip the no-op transform");
    }

    #[test]
    fn sar_na_is_ignored() {
        // ffprobe emits "N/A" for streams without explicit SAR; must not crash.
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080,
                  "r_frame_rate": "30/1", "sample_aspect_ratio": "N/A" }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert_eq!((info.video.width, info.video.height), (1920, 1080));
        assert_eq!(info.video.sar, None);
    }

    #[test]
    fn rotation_and_sar_both_applied() {
        // Anamorphic portrait edge case: 1080 square encoded + SAR 9:16 + rot 90.
        // SAR first: 606×1080. Then rotation swap: 1080×606.
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "h264", "width": 1080, "height": 1080,
                  "r_frame_rate": "30/1", "sample_aspect_ratio": "9:16",
                  "side_data_list": [{ "side_data_type": "Display Matrix", "rotation": 90 }] }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert_eq!((info.video.width, info.video.height), (1080, 606));
    }

    #[test]
    fn rotation_and_dv_coexist_independently() {
        // Display Matrix and DOVI both sit in side_data_list; parsing must
        // find each by its side_data_type without confusing them.
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "hevc", "width": 3840, "height": 2160,
                  "r_frame_rate": "24/1",
                  "side_data_list": [
                    { "side_data_type": "DOVI configuration record", "dv_profile": 8 },
                    { "side_data_type": "Display Matrix", "rotation": -90 }
                  ] }
            ],
            "format": { "duration": "100.0" }
        }"#;
        let info = parse(json).unwrap();
        assert_eq!(info.video.dv_profile, Some(8));
        assert_eq!(info.video.rotation, Some(-90));
        assert_eq!((info.video.width, info.video.height), (2160, 3840));
    }

    #[test]
    fn skips_zero_dim_video_stream_prefers_real_one() {
        // 3D Blu-ray MVC layout: stream 0 is the dependent enhancement view
        // with zero dims (ffprobe can't parse the non-standard codec header);
        // stream 1 is the real AVC base layer at 1920×1080.
        let json = r#"{
            "streams": [
                { "codec_type": "video", "codec_name": "h264", "width": 0, "height": 0,
                  "r_frame_rate": "90000/1" },
                { "codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080,
                  "profile": "High", "r_frame_rate": "24000/1001" }
            ],
            "format": { "duration": "60.0" }
        }"#;
        let info = parse(json).unwrap();
        assert_eq!((info.video.width, info.video.height), (1920, 1080));
        assert_eq!(info.video.profile.as_deref(), Some("High"));
    }

    #[test]
    fn round_even_floors_to_even_and_minimum_two() {
        assert_eq!(round_even(607.5), 606);
        assert_eq!(round_even(607.9), 606);
        assert_eq!(round_even(608.0), 608);
        assert_eq!(round_even(1.0), 2); // clamp: no zero-width edges
        assert_eq!(round_even(0.0), 2);
    }
}
