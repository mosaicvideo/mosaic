// src-tauri/src/defaults.rs
// Source of truth for every shipping default shown to users.
// The GUI's `src/index.html` values are kept in sync via
// `scripts/sync-defaults.mjs`; the CLI reads these directly.

// The module is feature-gated (public under `test-api` / `cli`, private
// otherwise), so in a plain GUI `cargo build` nothing inside the crate
// consumes these consts — they're read by the CLI binary (separate
// compilation unit) and by the build-time sync script. Silence the
// resulting false-positive `dead_code` warnings at the file level.
#![allow(dead_code)]

pub mod screenshots {
    pub const COUNT: u32 = 8;
    pub const FORMAT: &str = "png";
    pub const JPEG_QUALITY: u32 = 92;
}

pub mod sheet {
    pub const COLS: u32 = 3;
    pub const ROWS: u32 = 6;
    pub const WIDTH: u32 = 1920;
    pub const GAP: u32 = 10;
    pub const THUMB_FONT: u32 = 18;
    pub const HEADER_FONT: u32 = 20;
    pub const FORMAT: &str = "png";
    pub const JPEG_QUALITY: u32 = 92;
    pub const THEME: &str = "dark";
}

pub mod reel {
    pub const COUNT: u32 = 15;
    pub const CLIP_LENGTH_SECS: u32 = 2;
    pub const HEIGHT: u32 = 360;
    pub const FPS: u32 = 24;
    pub const QUALITY: u32 = 75;
    pub const FORMAT: &str = "webp";
}

pub mod animated_sheet {
    pub const COLS: u32 = 3;
    pub const ROWS: u32 = 6;
    pub const WIDTH: u32 = 1280;
    pub const GAP: u32 = 8;
    pub const CLIP_LENGTH_SECS: u32 = 2;
    pub const FPS: u32 = 12;
    pub const QUALITY: u32 = 75;
    pub const THUMB_FONT: u32 = 14;
    pub const HEADER_FONT: u32 = 18;
    pub const THEME: &str = "dark";
}

#[cfg(test)]
mod tests {
    use super::*;

    // Spot-checks for constant values. HTML/Rust drift is guarded separately
    // by scripts/sync-defaults.mjs + the defaults-sync-check CI workflow.
    #[test]
    fn screenshots_has_expected_values() {
        assert_eq!(screenshots::COUNT, 8);
        assert_eq!(screenshots::JPEG_QUALITY, 92);
    }
    #[test]
    fn sheet_has_expected_values() {
        assert_eq!(sheet::COLS, 3);
        assert_eq!(sheet::ROWS, 6);
        assert_eq!(sheet::WIDTH, 1920);
    }
    #[test]
    fn reel_has_expected_values() {
        assert_eq!(reel::COUNT, 15);
        assert_eq!(reel::FPS, 24);
    }
    #[test]
    fn animated_sheet_has_expected_values() {
        assert_eq!(animated_sheet::COLS, 3);
        assert_eq!(animated_sheet::FPS, 12);
    }
}
