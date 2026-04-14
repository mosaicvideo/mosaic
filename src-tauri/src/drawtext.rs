/// Escape a string so it can appear inside a drawtext `text='...'` argument.
/// Handles backslash, colon, single-quote, and percent.
pub fn escape_drawtext(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            ':' => out.push_str(r"\:"),
            '\'' => out.push_str(r"\'"),
            '%' => out.push_str("%%"),
            c => out.push(c),
        }
    }
    out
}

/// Format a duration in seconds as `HH\:MM\:SS` (already escaped for drawtext).
pub fn format_hms_escaped(seconds: f64) -> String {
    let total = seconds as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!(r"{:02}\:{:02}\:{:02}", h, m, s)
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
