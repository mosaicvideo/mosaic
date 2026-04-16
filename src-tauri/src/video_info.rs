use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoStream {
    pub codec: String,
    pub profile: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub bit_rate: Option<u64>,
    pub is_hdr: bool,
    /// Raw `color_transfer` tag from ffprobe (e.g. "smpte2084", "arib-std-b67").
    /// Passed to `tonemap_filter` so zscale gets explicit input transfer params.
    pub color_transfer: Option<String>,
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

    let v = root
        .streams
        .iter()
        .find(|s| s.codec_type == "video")
        .ok_or(ProbeParseError::NoVideo)?;

    let is_hdr = matches!(v.color_transfer.as_deref(), Some("smpte2084") | Some("arib-std-b67"))
        || v.side_data_list.as_ref().is_some_and(|list|
            list.iter().any(|sd| sd.side_data_type.as_deref() == Some("DOVI configuration record"))
        );

    let video = VideoStream {
        codec: v.codec_name.clone().unwrap_or_default(),
        profile: v.profile.clone(),
        width: v.width.unwrap_or(0),
        height: v.height.unwrap_or(0),
        fps: v.r_frame_rate.as_deref().and_then(parse_fraction).unwrap_or(0.0),
        bit_rate: v.bit_rate.as_deref().and_then(|s| s.parse().ok()),
        is_hdr,
        color_transfer: v.color_transfer.clone(),
    };

    let audio = root.streams.iter().find(|s| s.codec_type == "audio").map(|a| AudioStream {
        codec: a.codec_name.clone().unwrap_or_default(),
        profile: a.profile.clone(),
        sample_rate: a.sample_rate.as_deref().and_then(|s| s.parse().ok()),
        channels: a.channels,
        bit_rate: a.bit_rate.as_deref().and_then(|s| s.parse().ok()),
    });

    Ok(VideoInfo { filename, duration_secs, size_bytes, bit_rate, video, audio })
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
}
