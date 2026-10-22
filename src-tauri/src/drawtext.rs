/// Escape a string so it can appear inside a drawtext `text='...'` argument.
///
/// The text is unescaped three times: by the filtergraph parser, which takes
/// everything inside `'...'` literally; by the option parser, where `\` escapes
/// the next character; and by drawtext itself, which treats `\` and `%` as
/// special. A quote can't be escaped inside quotes, so `'` closes the quote,
/// adds an escaped one and reopens. `,` `[` `]` `;` are escaped too so a
/// filename can't inject a filter if the quoting is ever lost.
pub fn escape_drawtext(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\\\\"),
            ':' => out.push_str(r"\:"),
            '\'' => out.push_str(r"\'\''"),
            '%' => out.push_str(r"\\%"),
            ',' => out.push_str(r"\,"),
            '[' => out.push_str(r"\["),
            ']' => out.push_str(r"\]"),
            ';' => out.push_str(r"\;"),
            // drawtext breaks the line at a newline, drawing over the next header row.
            c if c.is_control() => out.push(' '),
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
/// value: strip the `\\?\` extended-length prefix and convert backslashes to
/// forward slashes.
///
/// Pure string manipulation — no Windows APIs — so it compiles and tests
/// on all platforms.
fn normalise_win_font_path(mut s: String) -> String {
    // Strip \\?\ prefix when followed by a drive letter (C:\...).
    // Leave \\?\UNC\ paths alone — we don't expect them, but be safe.
    if s.starts_with(r"\\?\") && s.as_bytes().get(5) == Some(&b':') {
        s = s[4..].to_string();
    }
    s.replace('\\', "/")
}

/// Escape a value for a quoted filter option that drawtext doesn't expand,
/// such as `fontfile='...'`. Like [`escape_drawtext`] minus drawtext's own
/// level: the option parser still splits on `:` and unescapes `\\` and `'`.
fn escape_option_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            ':' => out.push_str(r"\:"),
            '\'' => out.push_str(r"\'\''"),
            c => out.push(c),
        }
    }
    out
}

fn font_value(s: String, windows: bool) -> String {
    escape_option_value(&if windows { normalise_win_font_path(s) } else { s })
}

/// Render a filesystem path for use as `drawtext` `fontfile=`. The path is
/// escaped like any option value, so a drive-letter colon or a quote in a
/// folder name (`C:\Users\O'Brien`) can't end the option early.
///
/// Tauri's resource resolver (and some Windows APIs) may return paths with the
/// `\\?\` extended-length prefix. That prefix **requires** backslashes, so it
/// breaks once we normalise to forward slashes for ffmpeg. We strip it first —
/// the underlying drive-letter path works fine without it for paths under
/// MAX_PATH, and all our bundled-font paths are short.
pub fn font_for_ffmpeg(p: &std::path::Path) -> String {
    font_value(p.to_string_lossy().into_owned(), cfg!(windows))
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

/// Multi-line header drawtext chain, one `drawtext=` node per line. Lines are
/// stacked top-to-bottom at `x=gap`, starting at `y=gap` and advancing by
/// `line_h` each. All inputs must be drawtext-escaped already
/// ([`build_header_lines`][crate::header::build_header_lines] returns them
/// escaped); `font_ffmpeg` must come from [`font_for_ffmpeg`].
pub fn header_overlay(
    lines_escaped: &[String],
    font_ffmpeg: &str,
    font_size: u32,
    fontcolor: &str,
    gap: u32,
    line_h: u32,
) -> String {
    lines_escaped
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let y = gap + (i as u32) * line_h;
            format!(
                "drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor={}:x={}:y={}",
                line, font_ffmpeg, font_size, fontcolor, gap, y
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_backslash_for_both_option_parser_and_drawtext() {
        assert_eq!(escape_drawtext(r"a\b"), r"a\\\\b");
    }

    #[test]
    fn escapes_colon() {
        assert_eq!(escape_drawtext("a:b"), r"a\:b");
    }

    #[test]
    fn escapes_single_quote_by_closing_and_reopening_the_quote() {
        assert_eq!(escape_drawtext("it's"), r"it\'\''s");
    }

    #[test]
    fn escapes_percent_for_drawtext_expansion() {
        assert_eq!(escape_drawtext("50%"), r"50\\%");
    }

    #[test]
    fn escapes_combined() {
        assert_eq!(escape_drawtext(r"C:\a'b%"), r"C\:\\\\a\'\''b\\%");
    }

    #[test]
    fn replaces_line_breaks_and_other_control_characters_with_spaces() {
        assert_eq!(escape_drawtext("two\nlines\r\tok"), "two lines  ok");
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
            r"evil\'\''\,\[x\]\;y\:z\\%"
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

    // --- font path tests (Windows logic is tested cross-platform) ---

    #[test]
    fn font_value_escapes_a_quote_in_a_windows_user_folder() {
        assert_eq!(
            font_value(r"C:\Users\O'Brien\AppData\Local\Mosaic\DejaVuSans.ttf".into(), true),
            r"C\:/Users/O\'\''Brien/AppData/Local/Mosaic/DejaVuSans.ttf"
        );
    }

    #[test]
    fn font_value_escapes_quotes_colons_and_backslashes_in_unix_paths() {
        assert_eq!(
            font_value(r"/Applications/Bob's: \apps/DejaVuSans.ttf".into(), false),
            r"/Applications/Bob\'\''s\: \\apps/DejaVuSans.ttf"
        );
    }

    #[test]
    fn win_font_plain_drive_path() {
        assert_eq!(
            font_value(r"C:\Fonts\DejaVu.ttf".into(), true),
            r"C\:/Fonts/DejaVu.ttf"
        );
    }

    #[test]
    fn win_font_strips_extended_length_prefix() {
        // Tauri on Windows produces \\?\ prefixed paths for resources.
        // After normalisation the prefix must be gone and the drive colon
        // escaped — otherwise ffmpeg can't load the font.
        assert_eq!(
            font_value(r"\\?\F:\Program Files (x86)\mosaic\fonts\DejaVu.ttf".into(), true),
            r"F\:/Program Files (x86)/mosaic/fonts/DejaVu.ttf"
        );
    }

    #[test]
    fn header_overlay_single_line_has_no_chain() {
        let s = header_overlay(&["one".into()], "/f.ttf", 20, "white", 10, 26);
        assert!(s.starts_with("drawtext=text='one'"));
        assert!(s.contains(":x=10:y=10"));
        assert!(!s.contains(",drawtext="), "no chained node expected for a single line");
    }

    #[test]
    fn header_overlay_stacks_lines_top_to_bottom() {
        // gap=10, line_h=26 → y positions 10, 36, 62, 88.
        let s = header_overlay(
            &["line1".into(), "line2".into(), "line3".into(), "line4".into()],
            "/f.ttf", 20, "white", 10, 26,
        );
        assert_eq!(s.matches("drawtext=").count(), 4);
        assert!(s.contains(":x=10:y=10"));
        assert!(s.contains(":x=10:y=36"));
        assert!(s.contains(":x=10:y=62"));
        assert!(s.contains(":x=10:y=88"));
        // All lines appear in emission order.
        let p1 = s.find("line1").unwrap();
        let p2 = s.find("line2").unwrap();
        let p3 = s.find("line3").unwrap();
        let p4 = s.find("line4").unwrap();
        assert!(p1 < p2 && p2 < p3 && p3 < p4);
    }

    #[test]
    fn header_overlay_empty_input_yields_empty_string() {
        // Defensive — build_header_lines should never return empty, but the
        // helper must not produce a malformed filter graph if it did.
        assert_eq!(header_overlay(&[], "/f.ttf", 20, "white", 10, 26), "");
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
