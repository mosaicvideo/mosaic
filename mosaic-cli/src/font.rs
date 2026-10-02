// mosaic-cli/src/font.rs
// Embeds DejaVuSans.ttf into the binary via include_bytes! and lazily
// writes it to a tempfile on first use. Only `sheet` and
// `animated-sheet` subcommands call this — `screenshots` and `reel`
// pipelines render no drawtext.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use tempfile::{NamedTempFile, TempPath};

// A TempPath, not a NamedTempFile, so no write handle stays open while ffmpeg
// reads the font on Windows; the file is still deleted on drop.
static FONT_FILE: OnceLock<TempPath> = OnceLock::new();

const FONT_BYTES: &[u8] = include_bytes!("../../src-tauri/assets/fonts/DejaVuSans.ttf");

pub fn path() -> Result<PathBuf, std::io::Error> {
    // Returns an Err only on the first-time extraction attempt; subsequent
    // calls hit the OnceLock hot path and always return Ok.
    if let Some(p) = FONT_FILE.get() { return Ok(p.to_path_buf()); }
    let mut tf = NamedTempFile::new()?;
    tf.write_all(FONT_BYTES)?;
    tf.flush()?;
    // If set() loses a race, the losing TempPath is dropped and deleted.
    let _ = FONT_FILE.set(tf.into_temp_path());
    Ok(FONT_FILE.get().unwrap().to_path_buf())
}
