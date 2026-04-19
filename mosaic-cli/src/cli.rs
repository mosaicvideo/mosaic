// src-tauri/src/bin/mosaic_cli/cli.rs
// Clap v4 derive surface for every subcommand. Defaults reference
// mosaic_lib::defaults so GUI and CLI stay in lockstep.

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "mosaic-cli",
    version,
    about = "Video contact sheets, screenshots, previews, and animated sheets",
    after_help = "Config file: ~/.mosaic-cli.toml (auto-created on first run; override path with $MOSAIC_CLI_CONFIG).\nDocs: https://mosaicvideo.github.io/mosaic/cli.html"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Screenshots(ScreenshotsArgs),
    Sheet(SheetArgs),
    Reel(ReelArgs),
    AnimatedSheet(AnimatedSheetArgs),
    Probe(ProbeArgs),
    Completions(CompletionsArgs),
    #[command(about = "Emit a roff man page to stdout")]
    Manpage,
}

#[derive(Parser)]
pub struct Shared {
    /// Output directory (defaults to next to each source file).
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Suppress progress bars.
    #[arg(short, long)]
    pub quiet: bool,
    /// Print ffmpeg args before each invocation.
    #[arg(short, long, conflicts_with = "quiet")]
    pub verbose: bool,
    /// Do not descend into subdirectories when inputs are directories.
    #[arg(long)]
    pub no_recursive: bool,
    /// Input files or directories (at least one required).
    #[arg(required = true)]
    pub inputs: Vec<PathBuf>,
}

#[derive(Copy, Clone, ValueEnum)]
pub enum ImgFormat { Png, Jpeg }

#[derive(Copy, Clone, ValueEnum)]
pub enum ReelFormat { Webp, Webm, Gif }

#[derive(Copy, Clone, ValueEnum)]
pub enum Theme { Dark, Light }

#[derive(Parser)]
#[command(about = "Capture individual frames from a video")]
pub struct ScreenshotsArgs {
    /// Number of screenshots to capture, evenly spaced across the video.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=1000))]
    pub count: Option<u32>,
    /// Output image format.
    #[arg(long, value_enum)]
    pub format: Option<ImgFormat>,
    /// JPEG quality 50–100 (ignored for PNG).
    #[arg(long, value_parser = clap::value_parser!(u32).range(50..=100))]
    pub quality: Option<u32>,
    /// Filename infix between the source stem and the index.
    #[arg(long)]
    pub suffix: Option<String>,
    #[command(flatten)]
    pub shared: Shared,
}

#[derive(Parser)]
#[command(about = "Generate a still contact sheet (grid of thumbnails)")]
pub struct SheetArgs {
    /// Number of columns in the thumbnail grid.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=32))]
    pub cols: Option<u32>,
    /// Number of rows in the thumbnail grid.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=32))]
    pub rows: Option<u32>,
    /// Total sheet width in pixels. Thumbnails are sized to fit.
    #[arg(long, value_parser = clap::value_parser!(u32).range(320..=8192))]
    pub width: Option<u32>,
    /// Gap in pixels between adjacent thumbnails.
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..=200))]
    pub gap: Option<u32>,
    /// Timestamp-overlay font size (points).
    #[arg(long = "thumb-font", value_parser = clap::value_parser!(u32).range(8..=72))]
    pub thumb_font: Option<u32>,
    /// Header text font size (points).
    #[arg(long = "header-font", value_parser = clap::value_parser!(u32).range(8..=72))]
    pub header_font: Option<u32>,
    /// Force timestamp overlays on (overrides config).
    #[arg(long = "timestamps", conflicts_with = "no_timestamps")]
    pub timestamps: bool,
    /// Omit timestamp overlays on each thumbnail.
    #[arg(long = "no-timestamps")]
    pub no_timestamps: bool,
    /// Force header on (overrides config).
    #[arg(long = "header", conflicts_with = "no_header")]
    pub header: bool,
    /// Omit the metadata header above the grid.
    #[arg(long = "no-header")]
    pub no_header: bool,
    /// Output image format.
    #[arg(long, value_enum)]
    pub format: Option<ImgFormat>,
    /// JPEG quality 50–100 (ignored for PNG).
    #[arg(long, value_parser = clap::value_parser!(u32).range(50..=100))]
    pub quality: Option<u32>,
    /// Color theme for background and text.
    #[arg(long, value_enum)]
    pub theme: Option<Theme>,
    /// Filename infix inserted between stem and extension.
    #[arg(long)]
    pub suffix: Option<String>,
    #[command(flatten)]
    pub shared: Shared,
}

#[derive(Parser)]
#[command(about = "Generate an animated preview reel from short clips")]
pub struct ReelArgs {
    /// Number of clips to stitch into the reel.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=100))]
    pub count: Option<u32>,
    /// Each clip's duration in seconds.
    #[arg(long = "clip-length", value_parser = clap::value_parser!(u32).range(1..=60))]
    pub clip_length: Option<u32>,
    /// Output height in pixels. Width follows aspect ratio.
    #[arg(long, value_parser = clap::value_parser!(u32).range(120..=4320))]
    pub height: Option<u32>,
    /// Output frame rate (capped at source fps).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=120))]
    pub fps: Option<u32>,
    /// Encoder quality 0–100 (higher = better, webp/webm only).
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..=100))]
    pub quality: Option<u32>,
    /// Output container format.
    #[arg(long, value_enum)]
    pub format: Option<ReelFormat>,
    /// Filename infix inserted between stem and extension.
    #[arg(long)]
    pub suffix: Option<String>,
    #[command(flatten)]
    pub shared: Shared,
}

#[derive(Parser)]
#[command(about = "Generate an animated contact sheet (grid of animated clips)")]
pub struct AnimatedSheetArgs {
    /// Number of columns in the thumbnail grid.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=32))]
    pub cols: Option<u32>,
    /// Number of rows in the thumbnail grid.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=32))]
    pub rows: Option<u32>,
    /// Total sheet width in pixels. Thumbnails are sized to fit.
    #[arg(long, value_parser = clap::value_parser!(u32).range(320..=8192))]
    pub width: Option<u32>,
    /// Gap in pixels between adjacent thumbnails.
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..=200))]
    pub gap: Option<u32>,
    /// Each animated cell's duration in seconds.
    #[arg(long = "clip-length", value_parser = clap::value_parser!(u32).range(1..=60))]
    pub clip_length: Option<u32>,
    /// Animated frame rate (capped at source fps).
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=120))]
    pub fps: Option<u32>,
    /// Webp encoder quality 0–100 (higher = better).
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..=100))]
    pub quality: Option<u32>,
    /// Timestamp-overlay font size (points).
    #[arg(long = "thumb-font", value_parser = clap::value_parser!(u32).range(8..=72))]
    pub thumb_font: Option<u32>,
    /// Header text font size (points).
    #[arg(long = "header-font", value_parser = clap::value_parser!(u32).range(8..=72))]
    pub header_font: Option<u32>,
    /// Force timestamp overlays on (overrides config).
    #[arg(long = "timestamps", conflicts_with = "no_timestamps")]
    pub timestamps: bool,
    /// Omit timestamp overlays on each thumbnail.
    #[arg(long = "no-timestamps")]
    pub no_timestamps: bool,
    /// Force header on (overrides config).
    #[arg(long = "header", conflicts_with = "no_header")]
    pub header: bool,
    /// Omit the metadata header above the grid.
    #[arg(long = "no-header")]
    pub no_header: bool,
    /// Color theme for background and text.
    #[arg(long, value_enum)]
    pub theme: Option<Theme>,
    /// Filename infix inserted between stem and extension.
    #[arg(long)]
    pub suffix: Option<String>,
    #[command(flatten)]
    pub shared: Shared,
}

#[derive(Parser)]
#[command(about = "Print ffprobe/mediainfo metadata as JSON")]
pub struct ProbeArgs {
    /// Also include raw mediainfo output under a "mediainfo" key.
    #[arg(long)]
    pub mediainfo: bool,
    /// Video file to inspect.
    #[arg(required = true)]
    pub input: PathBuf,
}

#[derive(Parser)]
#[command(about = "Emit a shell-completion script to stdout")]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    pub shell: Shell,
}
