// src-tauri/src/bin/mosaic_cli/config.rs
// Resolves ~/.mosaic-cli.toml (or $MOSAIC_CLI_CONFIG), auto-creates a
// commented template on first run, parses it into a Config with
// Option<T> fields, and warns on unknown keys.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ConfigError {
    ParseFailed { path: PathBuf, msg: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseFailed { path, msg } => {
                write!(f, "config parse failed: {}: {}", path.display(), msg)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

// Cfg fields are feature-surface for users; not every field is consulted by
// every subcommand (e.g., screenshots doesn't read ReelCfg).
#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub screenshots: ScreenshotsCfg,
    #[serde(default)]
    pub sheet: SheetCfg,
    #[serde(default)]
    pub reel: ReelCfg,
    #[serde(default)]
    pub animated_sheet: AnimatedSheetCfg,
}

#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
pub struct ScreenshotsCfg {
    pub count: Option<u32>,
    pub format: Option<String>,
    pub quality: Option<u32>,
    pub suffix: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
pub struct SheetCfg {
    pub cols: Option<u32>,
    pub rows: Option<u32>,
    pub width: Option<u32>,
    pub gap: Option<u32>,
    pub thumb_font: Option<u32>,
    pub header_font: Option<u32>,
    pub show_timestamps: Option<bool>,
    pub show_header: Option<bool>,
    pub format: Option<String>,
    pub quality: Option<u32>,
    pub theme: Option<String>,
    pub suffix: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
pub struct ReelCfg {
    pub count: Option<u32>,
    pub clip_length_secs: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub quality: Option<u32>,
    pub format: Option<String>,
    pub suffix: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Default, Deserialize)]
pub struct AnimatedSheetCfg {
    pub cols: Option<u32>,
    pub rows: Option<u32>,
    pub width: Option<u32>,
    pub gap: Option<u32>,
    pub clip_length_secs: Option<u32>,
    pub fps: Option<u32>,
    pub quality: Option<u32>,
    pub thumb_font: Option<u32>,
    pub header_font: Option<u32>,
    pub show_timestamps: Option<bool>,
    pub show_header: Option<bool>,
    pub theme: Option<String>,
    pub suffix: Option<String>,
}

/// Default path: `$HOME/.mosaic-cli.toml` (Unix) or
/// `%USERPROFILE%\.mosaic-cli.toml` (Windows). Overridable via
/// `$MOSAIC_CLI_CONFIG`. Returns `None` when no home is set (sandboxed CI).
/// The bool flag is `true` when the path came from `$MOSAIC_CLI_CONFIG`
/// (user explicitly set it) — callers use this to decide whether a
/// write-fail on auto-create warrants a warning.
pub fn resolve_path() -> Option<(PathBuf, bool)> {
    if let Ok(p) = std::env::var("MOSAIC_CLI_CONFIG") {
        return Some((PathBuf::from(p), true));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some((PathBuf::from(home).join(".mosaic-cli.toml"), false))
}

/// Load config from `path` if it exists, otherwise create a commented
/// template (printing a one-time notice to stderr) and return defaults.
/// On any I/O error during creation, skip silently and return defaults.
/// Returns `Err(ConfigError)` if the file exists but cannot be parsed.
/// `is_explicit` should be `true` when the path came from `$MOSAIC_CLI_CONFIG`
/// so that a failed write emits a warning rather than failing silently.
pub fn load_or_create(path: &Path, is_explicit: bool) -> Result<Config, ConfigError> {
    if path.exists() {
        match std::fs::read_to_string(path) {
            Ok(body) => return parse(&body, path),
            Err(e) => {
                eprintln!("warning: could not read {}: {e}", path.display());
                return Ok(Config::default());
            }
        }
    }
    match std::fs::write(path, template()) {
        Ok(()) => eprintln!("Created {}", path.display()),
        Err(e) => {
            if is_explicit {
                eprintln!("warning: MOSAIC_CLI_CONFIG={} could not be created: {e}", path.display());
            }
            // read-only $HOME / sandbox: silently fall back to defaults
        }
    }
    Ok(Config::default())
}

fn parse(body: &str, path: &Path) -> Result<Config, ConfigError> {
    // First pass: permissive parse into a raw toml::Value so we can diff
    // against known keys and warn without failing the run.
    if let Ok(raw) = body.parse::<toml::Value>() {
        warn_unknown_keys(&raw, path);
    }
    toml::from_str::<Config>(body).map_err(|e| ConfigError::ParseFailed {
        path: path.to_path_buf(),
        msg: e.to_string(),
    })
}

fn warn_unknown_keys(raw: &toml::Value, path: &Path) {
    let known_sections = ["screenshots", "sheet", "reel", "animated_sheet"];
    let known_keys: &[(&str, &[&str])] = &[
        ("screenshots", &["count", "format", "quality", "suffix"]),
        (
            "sheet",
            &[
                "cols",
                "rows",
                "width",
                "gap",
                "thumb_font",
                "header_font",
                "show_timestamps",
                "show_header",
                "format",
                "quality",
                "theme",
                "suffix",
            ],
        ),
        (
            "reel",
            &[
                "count",
                "clip_length_secs",
                "height",
                "fps",
                "quality",
                "format",
                "suffix",
            ],
        ),
        (
            "animated_sheet",
            &[
                "cols",
                "rows",
                "width",
                "gap",
                "clip_length_secs",
                "fps",
                "quality",
                "thumb_font",
                "header_font",
                "show_timestamps",
                "show_header",
                "theme",
                "suffix",
            ],
        ),
    ];
    let Some(table) = raw.as_table() else {
        return;
    };
    for (section, value) in table {
        if !known_sections.contains(&section.as_str()) {
            eprintln!(
                "warning: unknown section '{section}' in {}",
                path.display()
            );
            continue;
        }
        let Some(sub) = value.as_table() else {
            continue;
        };
        let allowed = known_keys
            .iter()
            .find(|(s, _)| *s == section)
            .map(|(_, k)| *k)
            .unwrap_or(&[]);
        for key in sub.keys() {
            if !allowed.contains(&key.as_str()) {
                eprintln!(
                    "warning: unknown key '{section}.{key}' in {}",
                    path.display()
                );
            }
        }
    }
}

fn template() -> &'static str {
    // Keep in sync with the Cfg structs above. Every field appears commented
    // so users can uncomment and edit individual values.
    concat!(
        "# ~/.mosaic-cli.toml — per-user defaults for mosaic-cli.\n",
        "# Uncomment any key to override the built-in default.\n",
        "# CLI flags always override this file.\n",
        "\n",
        "[screenshots]\n",
        "# count = 8\n",
        "# format = \"png\"        # png | jpeg\n",
        "# quality = 92           # 50-100, JPEG only\n",
        "# suffix = \"_screens_\"\n",
        "\n",
        "[sheet]\n",
        "# cols = 3\n",
        "# rows = 6\n",
        "# width = 1920\n",
        "# gap = 10\n",
        "# thumb_font = 18\n",
        "# header_font = 20\n",
        "# show_timestamps = true\n",
        "# show_header = true\n",
        "# format = \"png\"        # png | jpeg\n",
        "# quality = 92\n",
        "# theme = \"dark\"        # dark | light\n",
        "# suffix = \"_sheet\"\n",
        "\n",
        "[reel]\n",
        "# count = 15\n",
        "# clip_length_secs = 2\n",
        "# height = 360\n",
        "# fps = 24\n",
        "# quality = 75\n",
        "# format = \"webp\"       # webp | webm | gif\n",
        "# suffix = \"_reel\"\n",
        "\n",
        "[animated_sheet]\n",
        "# cols = 3\n",
        "# rows = 6\n",
        "# width = 1280\n",
        "# gap = 8\n",
        "# clip_length_secs = 2\n",
        "# fps = 12\n",
        "# quality = 75\n",
        "# thumb_font = 14\n",
        "# header_font = 18\n",
        "# show_timestamps = true\n",
        "# show_header = true\n",
        "# theme = \"dark\"\n",
        "# suffix = \"_animated_sheet\"\n",
    )
}

impl Config {
    /// Validate numeric fields against the same ranges enforced by the
    /// clap `value_parser!` declarations. CLI flags are already bounded
    /// at parse time; config values come through serde and need their own
    /// bounds check.
    pub fn validate(&self) -> Result<(), String> {
        check_range("screenshots.count", self.screenshots.count, 1, 1000)?;
        check_range("screenshots.quality", self.screenshots.quality, 50, 100)?;

        check_range("sheet.cols", self.sheet.cols, 1, 32)?;
        check_range("sheet.rows", self.sheet.rows, 1, 32)?;
        check_range("sheet.width", self.sheet.width, 320, 8192)?;
        check_range("sheet.gap", self.sheet.gap, 0, 200)?;
        check_range("sheet.thumb_font", self.sheet.thumb_font, 8, 72)?;
        check_range("sheet.header_font", self.sheet.header_font, 8, 72)?;
        check_range("sheet.quality", self.sheet.quality, 50, 100)?;

        check_range("reel.count", self.reel.count, 1, 100)?;
        check_range("reel.clip_length_secs", self.reel.clip_length_secs, 1, 60)?;
        check_range("reel.height", self.reel.height, 120, 4320)?;
        check_range("reel.fps", self.reel.fps, 1, 120)?;
        check_range("reel.quality", self.reel.quality, 0, 100)?;

        check_range("animated_sheet.cols", self.animated_sheet.cols, 1, 32)?;
        check_range("animated_sheet.rows", self.animated_sheet.rows, 1, 32)?;
        check_range("animated_sheet.width", self.animated_sheet.width, 320, 8192)?;
        check_range("animated_sheet.gap", self.animated_sheet.gap, 0, 200)?;
        check_range("animated_sheet.clip_length_secs", self.animated_sheet.clip_length_secs, 1, 60)?;
        check_range("animated_sheet.fps", self.animated_sheet.fps, 1, 120)?;
        check_range("animated_sheet.quality", self.animated_sheet.quality, 0, 100)?;
        check_range("animated_sheet.thumb_font", self.animated_sheet.thumb_font, 8, 72)?;
        check_range("animated_sheet.header_font", self.animated_sheet.header_font, 8, 72)?;
        Ok(())
    }
}

fn check_range(key: &str, value: Option<u32>, min: u32, max: u32) -> Result<(), String> {
    match value {
        Some(v) if v < min || v > max => {
            Err(format!("config: {key} = {v} out of range ({min}..={max})"))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_missing_creates_template() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join(".mosaic-cli.toml");
        let cfg = load_or_create(&p, false).unwrap();
        assert!(p.exists(), "template should have been written");
        assert!(cfg.screenshots.count.is_none());
    }

    #[test]
    fn parse_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join(".mosaic-cli.toml");
        std::fs::write(&p, "[sheet]\ncols = 5\nrows = 4\n").unwrap();
        let cfg = load_or_create(&p, false).unwrap();
        assert_eq!(cfg.sheet.cols, Some(5));
        assert_eq!(cfg.sheet.rows, Some(4));
    }

    #[test]
    fn unknown_key_does_not_fail() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join(".mosaic-cli.toml");
        std::fs::write(&p, "[sheet]\ncols = 3\nbogus = 99\n").unwrap();
        let cfg = load_or_create(&p, false).unwrap(); // stderr warning is side-effect, not asserted here
        assert_eq!(cfg.sheet.cols, Some(3));
    }

    #[test]
    fn parse_failure_returns_err() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join(".mosaic-cli.toml");
        std::fs::write(&p, "[sheet]\ncols = \"not a number\"\n").unwrap();
        let err = load_or_create(&p, false).unwrap_err();
        assert!(matches!(err, ConfigError::ParseFailed { .. }));
    }

    #[test]
    fn validate_passes_on_empty_config() {
        let cfg = Config::default();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_rejects_zero_cols() {
        let mut cfg = Config::default();
        cfg.sheet.cols = Some(0);
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("sheet.cols"), "got: {err}");
        assert!(err.contains("out of range"));
    }

    #[test]
    fn validate_rejects_zero_clip_length() {
        let mut cfg = Config::default();
        cfg.reel.clip_length_secs = Some(0);
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_over_range_quality() {
        let mut cfg = Config::default();
        cfg.sheet.quality = Some(200);
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn explicit_path_nonexistent_parent_does_not_crash() {
        // $MOSAIC_CLI_CONFIG pointing at a path whose parent dir doesn't exist.
        // Should return defaults, not error.
        let p = std::path::PathBuf::from("/nonexistent-parent-12345/cfg.toml");
        let cfg = load_or_create(&p, true).unwrap();
        assert!(cfg.screenshots.count.is_none());
    }
}
