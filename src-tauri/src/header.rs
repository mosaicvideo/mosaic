use crate::drawtext::{escape_drawtext, format_hms_plain};
use crate::video_info::VideoInfo;

fn format_gib(bytes: u64) -> String {
    format!("{:.2} GiB", bytes as f64 / 1_073_741_824.0)
}

fn format_mbps(bps: u64) -> String {
    format!("{:.1} Mb/s", bps as f64 / 1_000_000.0)
}

fn format_kbps(bps: u64) -> String {
    format!("{} kb/s", bps / 1000)
}

pub fn build_header_lines(info: &VideoInfo, display_filename: &str) -> (String, String) {
    let line1 = escape_drawtext(display_filename);

    let hms = format_hms_plain(info.duration_secs);

    let mut parts: Vec<String> = Vec::new();
    if let Some(sz) = info.size_bytes { parts.push(format!("Size: {}", format_gib(sz))); }
    parts.push(format!("Duration: {}", hms));
    if let Some(br) = info.bit_rate { parts.push(format!("Bitrate: {}", format_mbps(br))); }

    let v = &info.video;
    let v_profile = v.profile.as_deref().map(|p| format!(" ({})", p)).unwrap_or_default();
    let v_br = v.bit_rate.map(|b| format!(" | {}", format_kbps(b))).unwrap_or_default();
    let v_seg = format!(
        "Video: {}{} | {}x{}{} | {:.2} fps",
        v.codec, v_profile, v.width, v.height, v_br, v.fps
    );

    let mut segments = vec![parts.join(", "), v_seg];

    if let Some(a) = &info.audio {
        let a_profile = a.profile.as_deref().map(|p| format!(" ({})", p)).unwrap_or_default();
        let a_rate = a.sample_rate.map(|r| format!(" | {} Hz", r)).unwrap_or_default();
        let a_ch = match a.channels {
            Some(2) => " | stereo".to_string(),
            Some(n) => format!(" | {} ch", n),
            None => String::new(),
        };
        let a_br = a.bit_rate.map(|b| format!(" | {}", format_kbps(b))).unwrap_or_default();
        segments.push(format!("Audio: {}{}{}{}{}", a.codec, a_profile, a_rate, a_ch, a_br));
    }

    let line2 = escape_drawtext(&segments.join("  |  "));
    (line1, line2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video_info::{AudioStream, VideoInfo, VideoStream};

    fn make_info() -> VideoInfo {
        VideoInfo {
            filename: "/tmp/x.mkv".into(),
            duration_secs: 3723.0, // 01:02:03
            size_bytes: Some(1_073_741_824), // 1.00 GiB
            bit_rate: Some(5_000_000),
            video: VideoStream {
                codec: "h264".into(),
                profile: Some("High".into()),
                width: 1920,
                height: 1080,
                fps: 23.976,
                bit_rate: Some(4_500_000),
                is_hdr: false,
                color_transfer: None,
                dv_profile: None,
            },
            audio: Some(AudioStream {
                codec: "aac".into(),
                profile: Some("LC".into()),
                sample_rate: Some(48000),
                channels: Some(2),
                bit_rate: Some(128_000),
            }),
        }
    }

    #[test]
    fn line1_is_escaped_filename() {
        let (l1, _) = build_header_lines(&make_info(), "it's : a test.mkv");
        assert_eq!(l1, r"it\'s \: a test.mkv");
    }

    #[test]
    fn line2_includes_size_duration_bitrate() {
        let (_, l2) = build_header_lines(&make_info(), "x.mkv");
        assert!(l2.contains("Size"));
        assert!(l2.contains("1.00 GiB"));
        assert!(l2.contains(r"01\:02\:03"));
        assert!(l2.contains("5.0 Mb/s"));
    }

    #[test]
    fn line2_includes_video_details() {
        let (_, l2) = build_header_lines(&make_info(), "x.mkv");
        assert!(l2.contains("h264 (High)"));
        assert!(l2.contains("1920x1080"));
        assert!(l2.contains("4500 kb/s"));
        assert!(l2.contains("23.98 fps"));
    }

    #[test]
    fn line2_includes_audio_stereo() {
        let (_, l2) = build_header_lines(&make_info(), "x.mkv");
        assert!(l2.contains("aac (LC)"));
        assert!(l2.contains("48000 Hz"));
        assert!(l2.contains("stereo"));
        assert!(l2.contains("128 kb/s"));
    }

    #[test]
    fn line2_omits_audio_when_missing() {
        let mut info = make_info();
        info.audio = None;
        let (_, l2) = build_header_lines(&info, "x.mkv");
        assert!(!l2.contains("Audio"));
    }

    #[test]
    fn line2_renders_multichannel() {
        let mut info = make_info();
        info.audio.as_mut().unwrap().channels = Some(6);
        let (_, l2) = build_header_lines(&info, "x.mkv");
        assert!(l2.contains("6 ch"));
    }
}
