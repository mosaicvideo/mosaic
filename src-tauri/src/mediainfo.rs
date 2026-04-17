//! Parse `mediainfo --Output=JSON` into the subset of metadata we surface in
//! the contact-sheet header.
//!
//! MediaInfo is a first-party prerequisite (enforced at startup by
//! [`crate::ffmpeg::locate_tools`]), but its output format can change
//! across versions and individual fields are not guaranteed. Every helper
//! here returns `Option` and silently drops anything it can't parse so a
//! single unexpected field never breaks the header — the builder falls
//! back to ffprobe data for any missing enrichment.
//!
//! MediaInfo emits nearly every numeric field as a JSON *string*, so parsers
//! here go through `&str`-to-number conversions rather than relying on serde
//! types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Subset of MediaInfo metadata the contact-sheet header consumes.
///
/// Every field is independently optional so that a partial MediaInfo record
/// (e.g. an audio track without a language tag, or an SDR source with no
/// HDR fields) can still contribute whatever it has without blocking the
/// rest. All strings are unescaped — callers that embed them in drawtext
/// filters must escape themselves.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Enrichment {
    /// General-track `Format`: container name like "Matroska", "MPEG-4".
    pub container_format: Option<String>,
    /// General-track `Title` / `Movie`: friendly source title.
    pub title: Option<String>,
    /// Video-track `BitDepth`: 8, 10, or 12 for common sources.
    pub video_bit_depth: Option<u8>,
    /// Displayable HDR format string built from `HDR_Format` +
    /// `HDR_Format_Compatibility` — e.g. "Dolby Vision / HDR10".
    pub video_hdr_format: Option<String>,
    /// Audio-track commercial name, preferring `Format_Commercial_IfAny`
    /// (e.g. "DTS-HD Master Audio", "Dolby Atmos") before falling back to
    /// the raw `Format`.
    pub audio_commercial_name: Option<String>,
    /// Audio channel layout summary: "stereo", "5.1", "7.1", or `{n} ch`.
    pub audio_channel_layout: Option<String>,
    /// Audio-track `Language` — ISO 639-ish code (e.g. "en", "ja").
    pub audio_language: Option<String>,
}

/// Parse a MediaInfo JSON document. Returns `None` on any structural failure
/// (invalid JSON, missing `media.track`, empty tracks) so the caller can
/// trivially fall back. Returns `Some(Enrichment::default())` for valid JSON
/// that contains none of the fields we care about — that's still a "success,
/// nothing to enrich" signal.
pub fn parse_enrichment(json: &str) -> Option<Enrichment> {
    let root: Value = serde_json::from_str(json).ok()?;
    let tracks = root.get("media")?.get("track")?.as_array()?;
    if tracks.is_empty() { return None; }

    let mut e = Enrichment::default();
    for t in tracks {
        match track_type(t) {
            Some("General") => populate_general(&mut e, t),
            Some("Video") => populate_video(&mut e, t),
            Some("Audio") => populate_audio(&mut e, t),
            _ => {}
        }
    }
    Some(e)
}

fn track_type(t: &Value) -> Option<&str> {
    t.get("@type").and_then(Value::as_str)
}

fn str_field(t: &Value, key: &str) -> Option<String> {
    let s = t.get(key)?.as_str()?.trim();
    if s.is_empty() { None } else { Some(s.to_string()) }
}

fn populate_general(e: &mut Enrichment, t: &Value) {
    e.container_format = str_field(t, "Format");
    // Prefer Title; fall back to Movie (mkv uses both interchangeably).
    e.title = str_field(t, "Title").or_else(|| str_field(t, "Movie"));
}

fn populate_video(e: &mut Enrichment, t: &Value) {
    e.video_bit_depth = str_field(t, "BitDepth").and_then(|s| s.parse().ok());
    e.video_hdr_format = collapse_hdr_format(
        str_field(t, "HDR_Format").as_deref(),
        str_field(t, "HDR_Format_Compatibility").as_deref(),
    );
}

fn populate_audio(e: &mut Enrichment, t: &Value) {
    e.audio_commercial_name = str_field(t, "Format_Commercial_IfAny").or_else(|| str_field(t, "Format"));
    e.audio_language = str_field(t, "Language");

    let channels: Option<u32> = str_field(t, "Channels").and_then(|s| s.parse().ok());
    let layout_raw = str_field(t, "ChannelLayout_Original").or_else(|| str_field(t, "ChannelLayout"));
    e.audio_channel_layout = channel_layout_display(channels, layout_raw.as_deref());
}

/// Combine `HDR_Format` with `HDR_Format_Compatibility` into a short display
/// string. MediaInfo emits strings like `"Dolby Vision / SMPTE ST 2086"` in
/// `HDR_Format`; the compatibility field independently reports `"Blu-ray /
/// HDR10"`. We prefer the headline format, then append a friendlier HDR10
/// tag if present in the compatibility field.
fn collapse_hdr_format(hdr: Option<&str>, compat: Option<&str>) -> Option<String> {
    let hdr = hdr?;
    let head = hdr.split('/').next().unwrap_or(hdr).trim();
    if head.is_empty() { return None; }

    // HDR10 appears in compatibility for DV sources. Surface it when the
    // headline isn't already mentioning HDR10.
    let hdr10 = compat.map(|s| s.contains("HDR10")).unwrap_or(false);
    if hdr10 && !head.contains("HDR10") {
        Some(format!("{} / HDR10", head))
    } else {
        Some(head.to_string())
    }
}

/// Map `(channel_count, ChannelLayout)` to a short display string.
///
/// Prefers the explicit layout string for surround configurations because
/// MediaInfo sometimes reports `Channels=6` for 5.1 with an LFE channel
/// folded in, and other times reports `7` with LFE as a separate count.
/// Falls back to `N ch` when the layout is ambiguous or missing.
fn channel_layout_display(channels: Option<u32>, layout: Option<&str>) -> Option<String> {
    if let Some(layout) = layout {
        let l = layout.to_ascii_uppercase();
        let has_lfe = l.contains("LFE");
        // Count spatial channels by stripping LFE then counting tokens.
        let spatial = l
            .split_whitespace()
            .filter(|tok| *tok != "LFE")
            .count();
        match (spatial, has_lfe) {
            (2, false) => return Some("stereo".into()),
            (3, false) => return Some("3.0".into()),
            (5, true) => return Some("5.1".into()),
            (6, true) => return Some("6.1".into()),
            (7, true) => return Some("7.1".into()),
            _ => {}
        }
    }
    match channels? {
        1 => Some("mono".into()),
        2 => Some("stereo".into()),
        n => Some(format!("{} ch", n)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{}", name)).unwrap()
    }

    #[test]
    fn returns_none_on_malformed_json() {
        assert!(parse_enrichment("not json").is_none());
    }

    #[test]
    fn returns_none_on_missing_media() {
        assert!(parse_enrichment(r#"{"other": {}}"#).is_none());
    }

    #[test]
    fn returns_none_on_empty_tracks() {
        assert!(parse_enrichment(r#"{"media": {"track": []}}"#).is_none());
    }

    #[test]
    fn default_when_no_fields_of_interest() {
        // Valid shape but every known field absent → empty Enrichment, not None.
        let json = r#"{"media": {"track": [{"@type": "General"}]}}"#;
        let e = parse_enrichment(json).unwrap();
        assert_eq!(e, Enrichment::default());
    }

    #[test]
    fn parses_awaken_hdr_dv_fixture() {
        let e = parse_enrichment(&fixture("mediainfo_awaken.json")).unwrap();
        assert_eq!(e.container_format.as_deref(), Some("Matroska"));
        assert_eq!(e.title.as_deref(), Some("Awaken (2018) UHD Disc Sample"));
        assert_eq!(e.video_bit_depth, Some(10));
        // DV source with HDR10 compatibility collapses into a compound label.
        assert_eq!(e.video_hdr_format.as_deref(), Some("Dolby Vision / HDR10"));
        assert_eq!(e.audio_commercial_name.as_deref(), Some("DTS-ES"));
        // DTS-ES exposes 6.1 via ChannelLayout_Original = "C L R Ls Rs Cb LFE".
        assert_eq!(e.audio_channel_layout.as_deref(), Some("6.1"));
        assert_eq!(e.audio_language.as_deref(), Some("en"));
    }

    #[test]
    fn parses_iphone_fixture() {
        let e = parse_enrichment(&fixture("mediainfo_iphone.json")).unwrap();
        assert_eq!(e.container_format.as_deref(), Some("MPEG-4"));
        assert_eq!(e.video_bit_depth, Some(8));
        assert!(e.video_hdr_format.is_none(), "iphone MOV is SDR");
        assert_eq!(e.audio_commercial_name.as_deref(), Some("AAC"));
        assert_eq!(e.audio_channel_layout.as_deref(), Some("stereo"));
    }

    #[test]
    fn collapse_hdr_adds_hdr10_when_compatibility_includes_it() {
        assert_eq!(
            collapse_hdr_format(Some("Dolby Vision / SMPTE ST 2086"), Some("Blu-ray / HDR10")),
            Some("Dolby Vision / HDR10".into())
        );
    }

    #[test]
    fn collapse_hdr_does_not_duplicate_hdr10() {
        // Headline already mentions HDR10 — don't append it twice.
        assert_eq!(
            collapse_hdr_format(Some("HDR10"), Some("HDR10")),
            Some("HDR10".into())
        );
    }

    #[test]
    fn collapse_hdr_returns_none_when_empty() {
        assert_eq!(collapse_hdr_format(None, Some("HDR10")), None);
        assert_eq!(collapse_hdr_format(Some(""), None), None);
    }

    #[test]
    fn channel_layout_recognises_5_1_via_lfe_token() {
        // Canonical 5.1: five spatial channels plus LFE.
        assert_eq!(
            channel_layout_display(Some(6), Some("C L R Ls Rs LFE")),
            Some("5.1".into())
        );
    }

    #[test]
    fn channel_layout_recognises_6_1_via_back_centre_and_lfe() {
        // DTS-ES / 6.1: 5.1 plus a center-back speaker.
        assert_eq!(
            channel_layout_display(Some(7), Some("C L R Ls Rs Cb LFE")),
            Some("6.1".into())
        );
    }

    #[test]
    fn channel_layout_recognises_stereo() {
        assert_eq!(
            channel_layout_display(Some(2), Some("L R")),
            Some("stereo".into())
        );
    }

    #[test]
    fn channel_layout_falls_back_to_count_when_ambiguous() {
        // 4-channel quad: no LFE, 4 spatial channels — doesn't match any
        // known layout, so we surface the raw count.
        assert_eq!(
            channel_layout_display(Some(4), Some("L R Ls Rs")),
            Some("4 ch".into())
        );
    }

    #[test]
    fn channel_layout_falls_back_to_count_without_layout() {
        assert_eq!(channel_layout_display(Some(8), None), Some("8 ch".into()));
        assert_eq!(channel_layout_display(Some(1), None), Some("mono".into()));
        assert_eq!(channel_layout_display(None, None), None);
    }
}
