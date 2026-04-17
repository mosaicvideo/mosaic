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

/// Build the lines that compose a contact-sheet header. Each element is a
/// fully drawtext-escaped string ready to be concatenated into a filter graph.
///
/// Layout, when audio is present and default (no enrichment):
/// 1. filename
/// 2. Size | Duration | Bitrate
/// 3. Video: codec (profile) | WxH | bitrate | fps
/// 4. Audio: codec (profile) | Hz | channels | bitrate
///
/// The audio line is omitted entirely when `info.audio` is `None`. One section
/// per line keeps narrow grids (e.g. the animated sheet's 1280px default) from
/// clipping the text at the right edge.
pub fn build_header_lines(info: &VideoInfo, display_filename: &str) -> Vec<String> {
    let enrich = info.enrichment.as_ref();
    let mut lines: Vec<String> = Vec::with_capacity(4);

    // Line 1: Prefer the MediaInfo title when present. A good container-level
    // title (e.g. "Awaken (2018) UHD Disc Sample") is much more informative
    // than a mangled filename, so it's worth surfacing when available.
    let title = enrich.and_then(|e| e.title.as_deref());
    lines.push(escape_drawtext(title.unwrap_or(display_filename)));

    let mut file_parts: Vec<String> = Vec::new();
    if let Some(sz) = info.size_bytes { file_parts.push(format!("Size: {}", format_gib(sz))); }
    file_parts.push(format!("Duration: {}", format_hms_plain(info.duration_secs)));
    if let Some(br) = info.bit_rate { file_parts.push(format!("Bitrate: {}", format_mbps(br))); }
    lines.push(escape_drawtext(&file_parts.join(" | ")));

    let v = &info.video;
    let v_profile = v.profile.as_deref().map(|p| format!(" ({})", p)).unwrap_or_default();
    let mut v_parts: Vec<String> = Vec::new();
    v_parts.push(format!("Video: {}{}", v.codec, v_profile));
    v_parts.push(format!("{}x{}", v.width, v.height));
    if let Some(bd) = enrich.and_then(|e| e.video_bit_depth) {
        v_parts.push(format!("{}-bit", bd));
    }
    if let Some(hdr) = enrich.and_then(|e| e.video_hdr_format.as_deref()) {
        v_parts.push(hdr.to_string());
    }
    if let Some(b) = v.bit_rate { v_parts.push(format_kbps(b)); }
    v_parts.push(format!("{:.2} fps", v.fps));
    lines.push(escape_drawtext(&v_parts.join(" | ")));

    if let Some(a) = &info.audio {
        // Codec label: prefer MediaInfo's commercial name ("DTS-HD MA",
        // "Dolby Atmos") — it matches what users see in the file listing of
        // a player, and captures features the codec name alone doesn't.
        let codec_label = enrich
            .and_then(|e| e.audio_commercial_name.clone())
            .unwrap_or_else(|| {
                let a_profile = a.profile.as_deref().map(|p| format!(" ({})", p)).unwrap_or_default();
                format!("{}{}", a.codec, a_profile)
            });
        let mut a_parts: Vec<String> = Vec::new();
        a_parts.push(format!("Audio: {}", codec_label));
        if let Some(r) = a.sample_rate { a_parts.push(format!("{} Hz", r)); }

        // Channel layout: MediaInfo gets to recognise "5.1" where ffprobe
        // only knows "6 channels." Fall back to ffprobe channels when the
        // layout wasn't derivable.
        let layout = enrich.and_then(|e| e.audio_channel_layout.clone());
        if let Some(l) = layout {
            a_parts.push(l);
        } else {
            match a.channels {
                Some(2) => a_parts.push("stereo".into()),
                Some(n) => a_parts.push(format!("{} ch", n)),
                None => {}
            }
        }

        if let Some(b) = a.bit_rate { a_parts.push(format_kbps(b)); }

        // Language tag as a trailing bracketed suffix: "[en]", "[ja]".
        let mut audio_line = a_parts.join(" | ");
        if let Some(lang) = enrich.and_then(|e| e.audio_language.as_deref()) {
            audio_line.push_str(&format!(" [{}]", lang));
        }
        lines.push(escape_drawtext(&audio_line));
    }

    lines
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
                rotation: None,
                sar: None,
            },
            audio: Some(AudioStream {
                codec: "aac".into(),
                profile: Some("LC".into()),
                sample_rate: Some(48000),
                channels: Some(2),
                bit_rate: Some(128_000),
            }),
            enrichment: None,
        }
    }

    #[test]
    fn filename_line_is_escaped() {
        let lines = build_header_lines(&make_info(), "it's : a test.mkv");
        assert_eq!(lines[0], r"it\'s \: a test.mkv");
    }

    #[test]
    fn file_line_includes_size_duration_bitrate() {
        let lines = build_header_lines(&make_info(), "x.mkv");
        let l = &lines[1];
        assert!(l.contains("Size"));
        assert!(l.contains("1.00 GiB"));
        assert!(l.contains(r"01\:02\:03"));
        assert!(l.contains("5.0 Mb/s"));
    }

    #[test]
    fn video_line_includes_codec_dims_bitrate_fps() {
        let lines = build_header_lines(&make_info(), "x.mkv");
        let l = &lines[2];
        assert!(l.contains("h264 (High)"));
        assert!(l.contains("1920x1080"));
        assert!(l.contains("4500 kb/s"));
        assert!(l.contains("23.98 fps"));
    }

    #[test]
    fn audio_line_includes_codec_rate_layout_bitrate() {
        let lines = build_header_lines(&make_info(), "x.mkv");
        let l = &lines[3];
        assert!(l.contains("aac (LC)"));
        assert!(l.contains("48000 Hz"));
        assert!(l.contains("stereo"));
        assert!(l.contains("128 kb/s"));
    }

    #[test]
    fn audio_line_omitted_when_no_audio() {
        let mut info = make_info();
        info.audio = None;
        let lines = build_header_lines(&info, "x.mkv");
        assert_eq!(lines.len(), 3);
        assert!(!lines.iter().any(|l| l.contains("Audio")));
    }

    #[test]
    fn audio_line_renders_multichannel_count() {
        let mut info = make_info();
        info.audio.as_mut().unwrap().channels = Some(6);
        let lines = build_header_lines(&info, "x.mkv");
        assert!(lines[3].contains("6 ch"));
    }

    #[test]
    fn four_lines_emitted_when_audio_present() {
        let lines = build_header_lines(&make_info(), "x.mkv");
        assert_eq!(lines.len(), 4, "filename + file info + video + audio");
    }

    fn enrichment_full() -> crate::mediainfo::Enrichment {
        crate::mediainfo::Enrichment {
            container_format: Some("Matroska".into()),
            title: Some("My Movie Title".into()),
            video_bit_depth: Some(10),
            video_hdr_format: Some("Dolby Vision / HDR10".into()),
            audio_commercial_name: Some("DTS-HD MA".into()),
            audio_channel_layout: Some("5.1".into()),
            audio_language: Some("en".into()),
        }
    }

    #[test]
    fn filename_line_uses_title_when_enriched() {
        let mut info = make_info();
        info.enrichment = Some(enrichment_full());
        let lines = build_header_lines(&info, "ignored-filename.mkv");
        assert_eq!(lines[0], "My Movie Title");
    }

    #[test]
    fn filename_line_falls_back_when_enrichment_has_no_title() {
        let mut info = make_info();
        info.enrichment = Some(crate::mediainfo::Enrichment::default());
        let lines = build_header_lines(&info, "fallback.mkv");
        assert_eq!(lines[0], "fallback.mkv");
    }

    #[test]
    fn video_line_injects_bit_depth_and_hdr() {
        let mut info = make_info();
        info.enrichment = Some(enrichment_full());
        let lines = build_header_lines(&info, "x.mkv");
        let l = &lines[2];
        assert!(l.contains("10-bit"));
        assert!(l.contains("Dolby Vision / HDR10"));
        // Ordering: dims, bit depth, hdr, bitrate, fps — keep bit-depth next
        // to dims since that's where MediaInfo users expect to see it.
        let pos_dims = l.find("1920x1080").unwrap();
        let pos_bit = l.find("10-bit").unwrap();
        let pos_hdr = l.find("Dolby Vision").unwrap();
        assert!(pos_dims < pos_bit && pos_bit < pos_hdr);
    }

    #[test]
    fn audio_line_prefers_commercial_name_and_layout() {
        let mut info = make_info();
        info.enrichment = Some(enrichment_full());
        let lines = build_header_lines(&info, "x.mkv");
        let l = &lines[3];
        assert!(l.contains("DTS-HD MA"));
        assert!(l.contains("5.1"));
        // Commercial name replaces the ffprobe codec — no "aac (LC)" leak.
        assert!(!l.contains("aac"));
    }

    #[test]
    fn audio_line_appends_language_suffix() {
        let mut info = make_info();
        info.enrichment = Some(enrichment_full());
        let lines = build_header_lines(&info, "x.mkv");
        assert!(lines[3].contains(r"\[en\]"), "language tag should be bracketed, got: {}", lines[3]);
    }

    #[test]
    fn audio_line_preserves_ffprobe_channels_when_enrichment_has_no_layout() {
        let mut info = make_info();
        info.enrichment = Some(crate::mediainfo::Enrichment::default());
        let lines = build_header_lines(&info, "x.mkv");
        assert!(lines[3].contains("stereo"));
    }

    #[test]
    fn no_double_pipe_separator_between_sections() {
        // Regression: the legacy layout concatenated sections with "  |  ";
        // the new layout puts each section on its own line so no line should
        // contain the doubled pipe separator.
        let lines = build_header_lines(&make_info(), "x.mkv");
        for l in &lines {
            assert!(!l.contains(r"  \|  "), "doubled pipe leaked into line: {}", l);
        }
    }
}
