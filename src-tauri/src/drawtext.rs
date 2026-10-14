use std::path::{Path, PathBuf};

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

/// Strip the Windows extended-length drive prefix before converting slashes.
/// Keep the existing UNC handling; no Windows APIs are needed for this step.
fn normalise_win_path(mut s: String) -> String {
    if s.starts_with(r"\\?\") && s.as_bytes().get(5) == Some(&b':') {
        s = s[4..].to_string();
    }
    s.replace('\\', "/")
}

/// Escape an unquoted option value through both FFmpeg parsing layers:
/// option separators first, then filtergraph separators. This is not shell
/// quoting: the resulting graph is passed directly as one argv element.
fn escape_filter_value(s: &str) -> String {
    fn escape(s: &str, special: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for ch in s.chars() {
            if special.contains(ch) || ch.is_ascii_whitespace() {
                out.push('\\');
            }
            out.push(ch);
        }
        out
    }
    escape(&escape(s, "\\':"), "\\'[],;")
}

/// Render a filesystem path as an unquoted filter option value. Windows
/// drive paths retain the existing slash/prefix normalization; on Unix a
/// backslash is a literal filename character and must not be normalized.
pub fn path_for_ffmpeg(p: &Path) -> String {
    let s = p.to_string_lossy().into_owned();
    let s = if cfg!(windows) { normalise_win_path(s) } else { s };
    escape_filter_value(&s)
}

/// Prepare the explicit font path with the same escaping as header textfiles.
pub fn font_for_ffmpeg(p: &Path) -> String {
    path_for_ffmpeg(p)
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
        "drawtext=text='{}':fontfile={}:fontsize={}:fontcolor={}:shadowcolor={}:shadowx=1:shadowy=1:x=5:y=h-th-5",
        hms_escaped, font_ffmpeg, font_size, fontcolor, shadowcolor
    )
}

/// Multi-line header chain backed by literal UTF-8 textfiles, one node per
/// line. The caller owns the files until FFmpeg finishes. Disabling expansion
/// keeps percent sequences and backslashes literal, including metadata text.
/// Lines start at `x=gap, y=gap` and advance by `line_h`.
pub fn header_overlay(
    textfiles: &[PathBuf],
    font_ffmpeg: &str,
    font_size: u32,
    fontcolor: &str,
    gap: u32,
    line_h: u32,
) -> String {
    textfiles
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let y = gap + (i as u32) * line_h;
            format!(
                "drawtext=textfile={}:expansion=none:fontfile={}:fontsize={}:fontcolor={}:x={}:y={}",
                path_for_ffmpeg(path), font_ffmpeg, font_size, fontcolor, gap, y
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_value_escapes_both_layers() {
        assert_eq!(escape_filter_value(r"a\b:c'd,[e];%☆"), r"a\\\\b\\:c\\\'d\,\[e\]\;%☆");
        assert_eq!(escape_filter_value(" a "), "\\\\\\ a\\\\\\ ");
    }

    #[test]
    fn formats_hms() {
        assert_eq!(format_hms_escaped(0.0), r"00\:00\:00");
        assert_eq!(format_hms_escaped(3723.0), r"01\:02\:03");
        assert_eq!(format_hms_escaped(59.999), r"00\:00\:59");
    }

    #[test]
    fn windows_paths_normalize_before_escaping() {
        assert_eq!(normalise_win_path(r"C:\Fonts\DejaVu.ttf".into()), "C:/Fonts/DejaVu.ttf");
        let path = normalise_win_path(r"\\?\F:\Miki's ☆\font.ttf".into());
        assert_eq!(path, "F:/Miki's ☆/font.ttf");
        assert_eq!(escape_filter_value(&path), r"F\\:/Miki\\\'s\\\ ☆/font.ttf");
        assert!(normalise_win_path(r"\\?\UNC\server\share\font.ttf".into()).starts_with("//?/UNC/"));
    }

    #[test]
    fn header_overlay_single_line_has_no_chain() {
        let s = header_overlay(&["one.txt".into()], "/f.ttf", 20, "white", 10, 26);
        assert!(s.starts_with("drawtext=textfile=one.txt:expansion=none"));
        assert!(s.contains(":x=10:y=10"));
        assert!(!s.contains(",drawtext="));
    }

    #[test]
    fn header_overlay_stacks_lines_top_to_bottom() {
        let s = header_overlay(
            &["line1".into(), "line2".into(), "line3".into(), "line4".into()],
            "/f.ttf", 20, "white", 10, 26,
        );
        assert_eq!(s.matches("drawtext=").count(), 4);
        for y in [10, 36, 62, 88] {
            assert!(s.contains(&format!(":x=10:y={y}")));
        }
        let positions: Vec<_> = (1..=4).map(|i| s.find(&format!("line{i}")).unwrap()).collect();
        assert!(positions.windows(2).all(|p| p[0] < p[1]));
    }

    #[test]
    fn header_overlay_empty_input_yields_empty_string() {
        assert_eq!(header_overlay(&[], "/f.ttf", 20, "white", 10, 26), "");
    }
}
