// src-tauri/src/bin/mosaic_cli/run/format.rs
// Shared conversions between CLI clap enums and mosaic_lib's
// OutputFormat / ReelFormat / SheetTheme — used by multiple subcommands.

use mosaic_lib::output_path::{OutputFormat, ReelFormat, SheetTheme};

pub fn resolve_img_format(arg: &Option<crate::cli::ImgFormat>, cfg: Option<&str>, fallback: &str) -> OutputFormat {
    match arg {
        Some(crate::cli::ImgFormat::Png)  => OutputFormat::Png,
        Some(crate::cli::ImgFormat::Jpeg) => OutputFormat::Jpeg,
        None => parse_img_format(cfg)
            .or_else(|| parse_img_format(Some(fallback)))
            .expect("defaults value must be a valid format"),
    }
}

fn parse_img_format(s: Option<&str>) -> Option<OutputFormat> {
    match s {
        Some("jpeg") | Some("Jpeg") | Some("JPEG") => Some(OutputFormat::Jpeg),
        Some("png")  | Some("Png")  | Some("PNG")  => Some(OutputFormat::Png),
        _ => None,
    }
}

pub fn resolve_theme(arg: &Option<crate::cli::Theme>, cfg: Option<&str>, fallback: &str) -> SheetTheme {
    match arg {
        Some(crate::cli::Theme::Light) => SheetTheme::Light,
        Some(crate::cli::Theme::Dark)  => SheetTheme::Dark,
        None => parse_theme(cfg)
            .or_else(|| parse_theme(Some(fallback)))
            .expect("defaults value must be a valid theme"),
    }
}

fn parse_theme(s: Option<&str>) -> Option<SheetTheme> {
    match s {
        Some("light") | Some("Light") => Some(SheetTheme::Light),
        Some("dark")  | Some("Dark")  => Some(SheetTheme::Dark),
        _ => None,
    }
}

pub fn resolve_reel_format(arg: &Option<crate::cli::ReelFormat>, cfg: Option<&str>, fallback: &str) -> ReelFormat {
    match arg {
        Some(crate::cli::ReelFormat::Webp) => ReelFormat::Webp,
        Some(crate::cli::ReelFormat::Webm) => ReelFormat::Webm,
        Some(crate::cli::ReelFormat::Gif)  => ReelFormat::Gif,
        None => parse_reel_format(cfg)
            .or_else(|| parse_reel_format(Some(fallback)))
            .expect("defaults value must be a valid reel format"),
    }
}

fn parse_reel_format(s: Option<&str>) -> Option<ReelFormat> {
    match s {
        Some("webp") | Some("Webp") => Some(ReelFormat::Webp),
        Some("webm") | Some("Webm") => Some(ReelFormat::Webm),
        Some("gif")  | Some("Gif")  => Some(ReelFormat::Gif),
        _ => None,
    }
}

/// Unified 3-way boolean resolver used by sheet/animated_sheet for
/// the show_timestamps / show_header fields.
pub fn resolve_bool(on_flag: bool, off_flag: bool, cfg: Option<bool>, default: bool) -> bool {
    if off_flag { false }
    else if on_flag { true }
    else { cfg.unwrap_or(default) }
}
