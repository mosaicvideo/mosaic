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

/// Render a filesystem path for use as `drawtext` `fontfile=`. On Windows the
/// drive-letter colon would otherwise be interpreted as an ffmpeg option
/// separator, so we normalise slashes and escape the colon.
pub fn font_for_ffmpeg(p: &std::path::Path) -> String {
    let mut s = p.to_string_lossy().into_owned();
    if cfg!(windows) {
        s = s.replace('\\', "/");
        if let Some(idx) = s.find(':') {
            s.replace_range(idx..idx + 1, r"\:");
        }
    }
    s
}

/// Per-cell timestamp overlay for contact-sheet thumbnails. `hms_escaped` must
/// already be drawtext-safe (use [`format_hms_escaped`]); `font_ffmpeg` must
/// come from [`font_for_ffmpeg`]. Positions the stamp at the bottom-left of
/// the cell with a subtle black shadow for legibility on mixed backgrounds.
pub fn timestamp_overlay(hms_escaped: &str, font_ffmpeg: &str, font_size: u32) -> String {
    format!(
        "drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:shadowcolor=black:shadowx=1:shadowy=1:x=5:y=h-th-5",
        hms_escaped, font_ffmpeg, font_size
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
    gap: u32,
    line_h: u32,
) -> String {
    format!(
        "drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:x={}:y={},drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:x={}:y={}",
        line1_escaped, font_ffmpeg, font_size, gap, gap,
        line2_escaped, font_ffmpeg, font_size, gap, gap + line_h
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
}
