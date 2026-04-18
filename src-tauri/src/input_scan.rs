// src-tauri/src/input_scan.rs
// Directory walker producing the video-file list consumed by both the
// Tauri `scan_folder` command and the CLI's positional-input expander.
// Accepts a file or directory — directories are walked up to
// MAX_SCAN_DEPTH (16) to guard against symlink cycles.

use std::path::{Path, PathBuf};

pub const VIDEO_EXTS: &[&str] = &[
    // Common containers
    "mp4", "mkv", "mov", "avi", "webm", "wmv", "flv", "m4v", "mpg", "mpeg",
    "ts", "m2ts", "mts", "vob", "iso", "ogv", "ogm", "qt", "asf",
    // Mobile / MP4 family
    "3gp", "3g2", "f4v", "mj2",
    // Legacy / regional
    "rm", "rmvb", "divx", "swf", "nsv",
    // Broadcast / professional
    "mxf", "gxf", "r3d",
    // Camcorder / capture / recording
    "dv", "dif", "wtv", "nuv", "pva",
    // Other containers
    "nut", "vro", "m1v", "m2v", "mk3d", "fli", "flc", "ivf", "y4m",
];

const MAX_SCAN_DEPTH: u32 = 16;

pub fn scan(path: &Path, recursive: bool) -> Result<Vec<PathBuf>, String> {
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut out = Vec::new();
    walk(path, recursive, 0, &mut out);
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, recursive: bool, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > MAX_SCAN_DEPTH { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let p = entry.path();
        if ft.is_dir() {
            if recursive { walk(&p, recursive, depth + 1, out); }
        } else if ft.is_file() {
            let ext_ok = p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .map(|e| VIDEO_EXTS.contains(&e.as_str()))
                .unwrap_or(false);
            if ext_ok { out.push(p); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, File};
    use tempfile::TempDir;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        if let Some(parent) = p.parent() { create_dir_all(parent).unwrap(); }
        File::create(&p).unwrap();
        p
    }

    #[test]
    fn filters_by_extension() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "a.mkv");
        touch(tmp.path(), "b.txt");
        touch(tmp.path(), "c.MP4"); // case-insensitive
        let got = scan(tmp.path(), false).unwrap();
        let names: Vec<_> = got.iter().filter_map(|p| p.file_name()?.to_str()).collect();
        assert!(names.contains(&"a.mkv"));
        assert!(names.contains(&"c.MP4"));
        assert!(!names.contains(&"b.txt"));
    }

    #[test]
    fn non_recursive_skips_subdirs() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "top.mkv");
        touch(tmp.path(), "sub/deep.mkv");
        let got = scan(tmp.path(), false).unwrap();
        assert_eq!(got.len(), 1);
        assert!(got[0].ends_with("top.mkv"));
    }

    #[test]
    fn recursive_descends() {
        let tmp = TempDir::new().unwrap();
        touch(tmp.path(), "top.mkv");
        touch(tmp.path(), "sub/deep.mkv");
        let got = scan(tmp.path(), true).unwrap();
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn file_argument_returns_itself() {
        let tmp = TempDir::new().unwrap();
        let p = touch(tmp.path(), "solo.mkv");
        let got = scan(&p, false).unwrap();
        assert_eq!(got, vec![p]);
    }

    #[test]
    fn missing_path_errors() {
        let err = scan(Path::new("/does/not/exist/mosaic"), false);
        assert!(err.is_err());
    }
}
