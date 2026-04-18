// src-tauri/src/bin/mosaic_cli/font.rs
// Embeds DejaVuSans.ttf into the binary via include_bytes! and lazily
// writes it to a tempfile on first use. Only `sheet` and
// `animated-sheet` subcommands call this — `screenshots` and `reel`
// pipelines render no drawtext.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use tempfile::NamedTempFile;

static FONT_FILE: OnceLock<NamedTempFile> = OnceLock::new();

const FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/DejaVuSans.ttf");

pub fn path() -> Result<PathBuf, std::io::Error> {
    // Returns an Err only on the first-time extraction attempt; subsequent
    // calls hit the OnceLock hot path and always return Ok.
    if let Some(f) = FONT_FILE.get() { return Ok(f.path().to_path_buf()); }
    let mut tf = NamedTempFile::new()?;
    tf.write_all(FONT_BYTES)?;
    // If set() fails, another thread won the race — use whichever tempfile is now in the lock.
    match FONT_FILE.set(tf) {
        Ok(()) => Ok(FONT_FILE.get().unwrap().path().to_path_buf()),
        Err(_losing_tempfile) => Ok(FONT_FILE.get().unwrap().path().to_path_buf()),
    }
}
