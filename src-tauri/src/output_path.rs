use std::path::{Path, PathBuf};

pub const DEFAULT_SHEET_SUFFIX: &str = "_sheet";
pub const DEFAULT_SHOTS_SUFFIX: &str = "_screens_";
pub const DEFAULT_PREVIEW_SUFFIX: &str = "_reel";
pub const DEFAULT_ANIMATED_SHEET_SUFFIX: &str = "_animated_sheet";

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum OutputFormat { Png, Jpeg }

impl OutputFormat {
    pub fn ext(self) -> &'static str {
        match self { Self::Png => "png", Self::Jpeg => "jpg" }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SheetTheme {
    #[default]
    Dark,
    Light,
}

impl SheetTheme {
    /// ffmpeg-compatible `0xRRGGBB` for pad/tile/color-source backgrounds.
    pub fn bg(self) -> &'static str {
        match self { Self::Dark => "0x000000", Self::Light => "0xFFFFFF" }
    }
    pub fn fontcolor(self) -> &'static str {
        match self { Self::Dark => "white", Self::Light => "black" }
    }
    /// drawtext `shadowcolor=` — inverse of text so timestamp halos against
    /// varied video content stay legible in either theme.
    pub fn shadowcolor(self) -> &'static str {
        match self { Self::Dark => "black", Self::Light => "white" }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReelFormat {
    #[default]
    Webp,
    Webm,
    Gif,
}

impl ReelFormat {
    pub fn ext(self) -> &'static str {
        match self { Self::Webp => "webp", Self::Webm => "webm", Self::Gif => "gif" }
    }
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn default_sheet_suffix() -> String { DEFAULT_SHEET_SUFFIX.to_string() }
pub fn default_shots_suffix() -> String { DEFAULT_SHOTS_SUFFIX.to_string() }
pub fn default_preview_suffix() -> String { DEFAULT_PREVIEW_SUFFIX.to_string() }
pub fn default_animated_sheet_suffix() -> String { DEFAULT_ANIMATED_SHEET_SUFFIX.to_string() }

fn collision_free_path(
    out_dir: &Path,
    base: &str,
    ext: &str,
    exists_fn: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    let candidate = out_dir.join(format!("{}.{}", base, ext));
    if !exists_fn(&candidate) { return candidate; }
    let mut n = 1;
    loop {
        let c = out_dir.join(format!("{} ({}).{}", base, n, ext));
        if !exists_fn(&c) { return c; }
        n += 1;
    }
}

pub fn contact_sheet_path(
    source: &Path,
    out_dir: &Path,
    fmt: OutputFormat,
    suffix: &str,
    exists_fn: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    let base = format!("{}{}", stem(source), suffix);
    collision_free_path(out_dir, &base, fmt.ext(), exists_fn)
}

pub fn screenshot_path(
    source: &Path,
    out_dir: &Path,
    fmt: OutputFormat,
    suffix: &str,
    index: u32,
    count: u32,
    exists_fn: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    let width = crate::layout::pad_width_for_count(count);
    let num = format!("{:0width$}", index, width = width);
    let base = format!("{}{}{}", stem(source), suffix, num);
    collision_free_path(out_dir, &base, fmt.ext(), exists_fn)
}

pub fn preview_reel_path(
    source: &Path,
    out_dir: &Path,
    fmt: ReelFormat,
    suffix: &str,
    exists_fn: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    let base = format!("{}{}", stem(source), suffix);
    collision_free_path(out_dir, &base, fmt.ext(), exists_fn)
}

pub fn animated_sheet_path(
    source: &Path,
    out_dir: &Path,
    suffix: &str,
    exists_fn: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    let base = format!("{}{}", stem(source), suffix);
    collision_free_path(out_dir, &base, "webp", exists_fn)
}

/// Map a user-facing JPEG quality (50..=100, higher = better) to libmjpeg's
/// `-q:v` scale (2 best .. 31 worst). 100 → 2, 50 → 15.
pub fn jpeg_qv(q: u32) -> u32 {
    let q = q.clamp(50, 100) as i64;
    (2 + ((100 - q) * 13 / 50)).max(2) as u32
}

/// Map user-facing quality (0..=100) → libvpx-vp9 CRF (0 best .. 63 worst),
/// clamped to a usable band `[4, 50]`. q=100 → 4, q=50 → 32, q=0 → 50.
pub fn vp9_crf(q: u32) -> u32 {
    let q = q.min(100) as i64;
    let raw = 63 - (q * 63 / 100);
    raw.clamp(4, 50) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn sheet_simple_case() {
        let p = contact_sheet_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/videos"),
            OutputFormat::Png,
            DEFAULT_SHEET_SUFFIX,
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/videos/movie_sheet.png"));
    }

    #[test]
    fn sheet_custom_suffix() {
        let p = contact_sheet_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/videos"),
            OutputFormat::Png,
            "-grid",
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/videos/movie-grid.png"));
    }

    #[test]
    fn sheet_empty_suffix_uses_bare_stem() {
        let p = contact_sheet_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/videos"),
            OutputFormat::Jpeg,
            "",
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/videos/movie.jpg"));
    }

    #[test]
    fn sheet_appends_suffix_when_file_exists() {
        let taken: HashSet<PathBuf> = ["/out/movie_sheet.png", "/out/movie_sheet (1).png"]
            .into_iter().map(PathBuf::from).collect();
        let p = contact_sheet_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/out"),
            OutputFormat::Png,
            DEFAULT_SHEET_SUFFIX,
            &|p| taken.contains(p),
        );
        assert_eq!(p, PathBuf::from("/out/movie_sheet (2).png"));
    }

    #[test]
    fn sheet_jpeg_extension() {
        let p = contact_sheet_path(
            Path::new("/a/x.mp4"),
            Path::new("/a"),
            OutputFormat::Jpeg,
            DEFAULT_SHEET_SUFFIX,
            &|_| false,
        );
        assert_eq!(p.extension().unwrap(), "jpg");
    }

    #[test]
    fn screenshot_zero_padded_to_count_width() {
        let p = screenshot_path(
            Path::new("/v/clip.mp4"),
            Path::new("/v"),
            OutputFormat::Png,
            DEFAULT_SHOTS_SUFFIX,
            7,
            100,
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/v/clip_screens_007.png"));
    }

    #[test]
    fn screenshot_min_width_two() {
        let p = screenshot_path(
            Path::new("/v/clip.mp4"),
            Path::new("/v"),
            OutputFormat::Png,
            DEFAULT_SHOTS_SUFFIX,
            3,
            5,
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/v/clip_screens_03.png"));
    }

    #[test]
    fn screenshot_custom_suffix() {
        let p = screenshot_path(
            Path::new("/v/clip.mp4"),
            Path::new("/v"),
            OutputFormat::Png,
            "-shot-",
            3,
            5,
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/v/clip-shot-03.png"));
    }

    #[test]
    fn screenshot_empty_suffix_puts_index_after_stem() {
        let p = screenshot_path(
            Path::new("/v/clip.mp4"),
            Path::new("/v"),
            OutputFormat::Png,
            "",
            1,
            5,
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/v/clip01.png"));
    }

    #[test]
    fn screenshot_appends_suffix_on_collision() {
        let taken: HashSet<PathBuf> = ["/out/clip_screens_01.png"].iter().map(PathBuf::from).collect();
        let p = screenshot_path(
            Path::new("/v/clip.mp4"),
            Path::new("/out"),
            OutputFormat::Png,
            DEFAULT_SHOTS_SUFFIX,
            1,
            5,
            &|p| taken.contains(p),
        );
        assert_eq!(p, PathBuf::from("/out/clip_screens_01 (1).png"));
    }

    #[test]
    fn preview_reel_simple_case() {
        let p = preview_reel_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/videos"),
            ReelFormat::Webp,
            DEFAULT_PREVIEW_SUFFIX,
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/videos/movie_reel.webp"));
    }

    #[test]
    fn preview_reel_custom_suffix() {
        let p = preview_reel_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/videos"),
            ReelFormat::Webp,
            "_preview",
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/videos/movie_preview.webp"));
    }

    #[test]
    fn preview_reel_appends_suffix_when_file_exists() {
        let taken: HashSet<PathBuf> = ["/out/movie_reel.webp", "/out/movie_reel (1).webp"]
            .into_iter().map(PathBuf::from).collect();
        let p = preview_reel_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/out"),
            ReelFormat::Webp,
            DEFAULT_PREVIEW_SUFFIX,
            &|p| taken.contains(p),
        );
        assert_eq!(p, PathBuf::from("/out/movie_reel (2).webp"));
    }

    #[test]
    fn reel_with_empty_suffix_never_overwrites_its_source() {
        let p = preview_reel_path(
            Path::new("/v/movie.webm"),
            Path::new("/v"),
            ReelFormat::Webm,
            "",
            &|p| p == Path::new("/v/movie.webm"),
        );
        assert_eq!(p, PathBuf::from("/v/movie (1).webm"));
    }

    #[test]
    fn preview_reel_webm_extension() {
        let p = preview_reel_path(
            Path::new("/v/movie.mkv"),
            Path::new("/v"),
            ReelFormat::Webm,
            DEFAULT_PREVIEW_SUFFIX,
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/v/movie_reel.webm"));
    }

    #[test]
    fn preview_reel_gif_extension() {
        let p = preview_reel_path(
            Path::new("/v/movie.mkv"),
            Path::new("/v"),
            ReelFormat::Gif,
            DEFAULT_PREVIEW_SUFFIX,
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/v/movie_reel.gif"));
    }

    #[test]
    fn reel_format_default_is_webp() {
        assert_eq!(ReelFormat::default(), ReelFormat::Webp);
    }

    #[test]
    fn vp9_crf_mapping_endpoints_and_midpoint() {
        assert_eq!(vp9_crf(100), 4);
        assert_eq!(vp9_crf(0), 50);
        assert_eq!(vp9_crf(50), 32);
    }

    #[test]
    fn animated_sheet_simple_case() {
        let p = animated_sheet_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/videos"),
            DEFAULT_ANIMATED_SHEET_SUFFIX,
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/videos/movie_animated_sheet.webp"));
    }

    #[test]
    fn animated_sheet_custom_suffix() {
        let p = animated_sheet_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/videos"),
            "-asheet",
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/videos/movie-asheet.webp"));
    }

    #[test]
    fn animated_sheet_appends_suffix_when_file_exists() {
        let taken: HashSet<PathBuf> = [
            "/out/movie_animated_sheet.webp",
            "/out/movie_animated_sheet (1).webp",
        ].into_iter().map(PathBuf::from).collect();
        let p = animated_sheet_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/out"),
            DEFAULT_ANIMATED_SHEET_SUFFIX,
            &|p| taken.contains(p),
        );
        assert_eq!(p, PathBuf::from("/out/movie_animated_sheet (2).webp"));
    }
}
