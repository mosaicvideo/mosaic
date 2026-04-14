use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat { Png, Jpeg }

impl OutputFormat {
    pub fn ext(self) -> &'static str {
        match self { Self::Png => "png", Self::Jpeg => "jpg" }
    }
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn contact_sheet_path(
    source: &Path,
    out_dir: &Path,
    fmt: OutputFormat,
    exists_fn: &dyn Fn(&Path) -> bool,
) -> PathBuf {
    let base = format!("{}_contact_sheet", stem(source));
    let ext = fmt.ext();
    let candidate = out_dir.join(format!("{}.{}", base, ext));
    if !exists_fn(&candidate) { return candidate; }
    let mut n = 1;
    loop {
        let c = out_dir.join(format!("{} ({}).{}", base, n, ext));
        if !exists_fn(&c) { return c; }
        n += 1;
    }
}

pub fn screenshot_path(
    source: &Path,
    out_dir: &Path,
    fmt: OutputFormat,
    index: u32,
    count: u32,
) -> PathBuf {
    let width = count.to_string().len().max(2);
    let num = format!("{:0width$}", index, width = width);
    out_dir.join(format!("{}_screenshot_{}.{}", stem(source), num, fmt.ext()))
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
            &|_| false,
        );
        assert_eq!(p, PathBuf::from("/videos/movie_contact_sheet.png"));
    }

    #[test]
    fn sheet_appends_suffix_when_file_exists() {
        let taken: HashSet<PathBuf> = ["/out/movie_contact_sheet.png", "/out/movie_contact_sheet (1).png"]
            .into_iter().map(PathBuf::from).collect();
        let p = contact_sheet_path(
            Path::new("/videos/movie.mkv"),
            Path::new("/out"),
            OutputFormat::Png,
            &|p| taken.contains(p),
        );
        assert_eq!(p, PathBuf::from("/out/movie_contact_sheet (2).png"));
    }

    #[test]
    fn sheet_jpeg_extension() {
        let p = contact_sheet_path(
            Path::new("/a/x.mp4"),
            Path::new("/a"),
            OutputFormat::Jpeg,
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
            7,
            100,
        );
        assert_eq!(p, PathBuf::from("/v/clip_screenshot_007.png"));
    }

    #[test]
    fn screenshot_min_width_two() {
        let p = screenshot_path(
            Path::new("/v/clip.mp4"),
            Path::new("/v"),
            OutputFormat::Png,
            3,
            5,
        );
        assert_eq!(p, PathBuf::from("/v/clip_screenshot_03.png"));
    }
}
