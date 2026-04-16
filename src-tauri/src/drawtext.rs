/// Escape a string so it can appear inside a drawtext `text='...'` argument.
/// Handles backslash, colon, single-quote, and percent. Additionally escapes
/// filter-graph separators (`,` `[` `]` `;`) as defense-in-depth so that a
/// malicious filename cannot break out and inject filters if quoting is ever
/// bypassed in a future refactor.
pub fn escape_drawtext(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            ':' => out.push_str(r"\:"),
            '\'' => out.push_str(r"\'"),
            '%' => out.push_str("%%"),
            ',' => out.push_str(r"\,"),
            '[' => out.push_str(r"\["),
            ']' => out.push_str(r"\]"),
            ';' => out.push_str(r"\;"),
            c => out.push(c),
        }
    }
    out
}

/// Format a duration in seconds as `HH:MM:SS` (plain, unescaped).
pub fn format_hms_plain(seconds: f64) -> String {
    let total = seconds as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Format a duration in seconds as `HH\:MM\:SS` (escaped for drawtext `text='…'`).
pub fn format_hms_escaped(seconds: f64) -> String {
    format_hms_plain(seconds).replace(':', r"\:")
}

/// Normalise a Windows-style path string for use as an ffmpeg `fontfile=`
/// value: strip the `\\?\` extended-length prefix, convert backslashes to
/// forward slashes, and escape the drive-letter colon.
///
/// Pure string manipulation — no Windows APIs — so it compiles and tests
/// on all platforms.
fn normalise_win_font_path(mut s: String) -> String {
    // Strip \\?\ prefix when followed by a drive letter (C:\...).
    // Leave \\?\UNC\ paths alone — we don't expect them, but be safe.
    if s.starts_with(r"\\?\") && s.as_bytes().get(5) == Some(&b':') {
        s = s[4..].to_string();
    }
    s = s.replace('\\', "/");
    if let Some(idx) = s.find(':') {
        s.replace_range(idx..idx + 1, r"\:");
    }
    s
}

/// Render a filesystem path for use as `drawtext` `fontfile=`. On Windows the
/// drive-letter colon would otherwise be interpreted as an ffmpeg option
/// separator, so we normalise slashes and escape the colon.
///
/// Tauri's resource resolver (and some Windows APIs) may return paths with the
/// `\\?\` extended-length prefix. That prefix **requires** backslashes, so it
/// breaks once we normalise to forward slashes for ffmpeg. We strip it first —
/// the underlying drive-letter path works fine without it for paths under
/// MAX_PATH, and all our bundled-font paths are short.
pub fn font_for_ffmpeg(p: &std::path::Path) -> String {
    let s = p.to_string_lossy().into_owned();
    if cfg!(windows) {
        normalise_win_font_path(s)
    } else {
        s
    }
}

/// Per-cell timestamp overlay for contact-sheet thumbnails. `hms_escaped` must
/// already be drawtext-safe (use [`format_hms_escaped`]); `font_ffmpeg` must
/// come from [`font_for_ffmpeg`]. `shadowcolor` is typically the inverse of
/// `fontcolor` to keep the stamp readable against varied video content.
pub fn timestamp_overlay(
    hms_escaped: &str,
    font_ffmpeg: &str,
    font_size: u32,
    fontcolor: &str,
    shadowcolor: &str,
) -> String {
    format!(
        "drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor={}:shadowcolor={}:shadowx=1:shadowy=1:x=5:y=h-th-5",
        hms_escaped, font_ffmpeg, font_size, fontcolor, shadowcolor
    )
}

/// Two-line header drawtext chain (line1 above line2), padded `gap` from the
/// left/top edge with `line_h` between the two lines. Both lines must already
/// be drawtext-escaped ([`build_header_lines`][crate::header::build_header_lines]
/// returns them escaped); `font_ffmpeg` must come from [`font_for_ffmpeg`].
pub fn header_overlay(
    line1_escaped: &str,
    line2_escaped: &str,
    font_ffmpeg: &str,
    font_size: u32,
    fontcolor: &str,
    gap: u32,
    line_h: u32,
) -> String {
    format!(
        "drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor={}:x={}:y={},drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor={}:x={}:y={}",
        line1_escaped, font_ffmpeg, font_size, fontcolor, gap, gap,
        line2_escaped, font_ffmpeg, font_size, fontcolor, gap, gap + line_h
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_backslash_first() {
        assert_eq!(escape_drawtext(r"a\b"), r"a\\b");
    }

    #[test]
    fn escapes_colon() {
        assert_eq!(escape_drawtext("a:b"), r"a\:b");
    }

    #[test]
    fn escapes_single_quote() {
        assert_eq!(escape_drawtext("it's"), r"it\'s");
    }

    #[test]
    fn escapes_percent() {
        assert_eq!(escape_drawtext("50%"), "50%%");
    }

    #[test]
    fn escapes_combined() {
        // order matters: backslash first so earlier escapes aren't re-escaped
        assert_eq!(escape_drawtext(r"C:\a'b%"), r"C\:\\a\'b%%");
    }

    #[test]
    fn escapes_comma() {
        assert_eq!(escape_drawtext("a,b"), r"a\,b");
    }

    #[test]
    fn escapes_open_bracket() {
        assert_eq!(escape_drawtext("a[b"), r"a\[b");
    }

    #[test]
    fn escapes_close_bracket() {
        assert_eq!(escape_drawtext("a]b"), r"a\]b");
    }

    #[test]
    fn escapes_semicolon() {
        assert_eq!(escape_drawtext("a;b"), r"a\;b");
    }

    #[test]
    fn escapes_combined_graph_separators() {
        // a malicious filename attempting to break out of quoting and inject
        // a new filter node should have all separators neutralised.
        assert_eq!(
            escape_drawtext("evil',[x];y:z%"),
            r"evil\'\,\[x\]\;y\:z%%"
        );
    }

    #[test]
    fn formats_hms_zero() {
        assert_eq!(format_hms_escaped(0.0), r"00\:00\:00");
    }

    #[test]
    fn formats_hms_typical() {
        // 1h 2m 3s
        assert_eq!(format_hms_escaped(3723.0), r"01\:02\:03");
    }

    #[test]
    fn formats_hms_truncates_fraction() {
        assert_eq!(format_hms_escaped(59.999), r"00\:00\:59");
    }

    // --- normalise_win_font_path tests (Windows font path logic, tested cross-platform) ---

    #[test]
    fn win_font_plain_drive_path() {
        assert_eq!(
            normalise_win_font_path(r"C:\Fonts\DejaVu.ttf".into()),
            r"C\:/Fonts/DejaVu.ttf"
        );
    }

    #[test]
    fn win_font_strips_extended_length_prefix() {
        // Tauri on Windows produces \\?\ prefixed paths for resources.
        // After normalisation the prefix must be gone and the drive colon
        // escaped — otherwise ffmpeg can't load the font.
        assert_eq!(
            normalise_win_font_path(r"\\?\F:\Program Files (x86)\mosaic\fonts\DejaVu.ttf".into()),
            r"F\:/Program Files (x86)/mosaic/fonts/DejaVu.ttf"
        );
    }

    #[test]
    fn win_font_preserves_unc_prefix() {
        // \\?\UNC\... paths should NOT be stripped (they're a different beast).
        let input = r"\\?\UNC\server\share\font.ttf".to_string();
        let out = normalise_win_font_path(input);
        // The \\?\ is kept because byte 5 is 'U', not ':'.
        assert!(out.starts_with("//?/UNC/"), "got: {}", out);
    }
}
