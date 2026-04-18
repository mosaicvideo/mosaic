// src-tauri/src/bin/mosaic_cli/run/inputs.rs
// Turns clap positional inputs (mix of files and dirs) into the final
// per-file worklist, using input_scan for directory expansion.

use std::path::{Path, PathBuf};

pub fn expand(inputs: &[PathBuf], recursive: bool) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for p in inputs {
        let found = mosaic_lib::input_scan::scan(Path::new(p), recursive)?;
        out.extend(found);
    }
    // Pragmatic stopgap: reject non-UTF-8 paths early with a clear error
    // rather than silently mangling them via to_string_lossy() and confusing
    // ffmpeg with a "No such file or directory" from a corrupted path string.
    // A proper fix would thread OsStr through all ffmpeg arg vectors, but that
    // is a multi-file refactor. This keeps the common case correct.
    for p in &out {
        if p.to_str().is_none() {
            return Err(format!("path is not valid UTF-8: {}", p.display()));
        }
    }
    Ok(out)
}

/// Resolve the output directory for a given source file: explicit
/// `--output` wins, otherwise emit alongside the source (fallback to `.` for bare-root paths).
pub fn out_dir(shared: &crate::cli::Shared, src: &Path) -> PathBuf {
    shared.output.clone().unwrap_or_else(|| {
        src.parent().unwrap_or(Path::new(".")).to_path_buf()
    })
}
