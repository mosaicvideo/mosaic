// src-tauri/src/bin/mosaic_cli/run/animated_sheet.rs
use crate::cli::AnimatedSheetArgs;
use crate::config::Config;
use crate::font;
use crate::progress::Reporter;
use crate::run::{format::{resolve_bool, resolve_theme}, inputs};
use mosaic_lib::{
    animated_sheet::{generate, AnimatedSheetOptions},
    defaults,
    ffmpeg::{RunError},
    ffmpeg_test_hook_locate, ffmpeg_test_hook_probe,
    jobs::{PipelineContext, ProgressReporter},
    output_path::{animated_sheet_path, DEFAULT_ANIMATED_SHEET_SUFFIX},
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub async fn run(args: AnimatedSheetArgs, cfg: &Config) -> i32 {
    let tools = match ffmpeg_test_hook_locate() {
        Ok(t) => t,
        Err(e) => { eprintln!("{e}"); crate::hints::print_tool_install_hint(); return 2; }
    };
    let inputs = match inputs::expand(&args.shared.inputs, !args.shared.no_recursive) {
        Ok(v) => v, Err(e) => { eprintln!("{e}"); return 2; }
    };
    if inputs.is_empty() { eprintln!("no input files"); return 2; }

    let font_path = match font::path() {
        Ok(p) => p, Err(e) => { eprintln!("font extract failed: {e}"); return 2; }
    };

    let has_zscale = tools.detect_has_zscale();
    let cancelled = Arc::new(AtomicBool::new(false));
    crate::signals::install(cancelled.clone());

    let total = inputs.len() as u64;
    let reporter = Reporter::new(total, args.shared.quiet);
    let emit = reporter.emit_fn();
    let pr = ProgressReporter { emit: &emit };

    let suffix_raw = args.suffix.clone()
        .or_else(|| cfg.animated_sheet.suffix.clone())
        .unwrap_or_else(|| DEFAULT_ANIMATED_SHEET_SUFFIX.to_string());
    let suffix = match crate::run::suffix::validate(&suffix_raw) {
        Ok(s) => s,
        Err(e) => { eprintln!("{e}"); return 2; }
    };

    let opts = AnimatedSheetOptions {
        cols:             args.cols.or(cfg.animated_sheet.cols).unwrap_or(defaults::animated_sheet::COLS),
        rows:             args.rows.or(cfg.animated_sheet.rows).unwrap_or(defaults::animated_sheet::ROWS),
        width:            args.width.or(cfg.animated_sheet.width).unwrap_or(defaults::animated_sheet::WIDTH),
        gap:              args.gap.or(cfg.animated_sheet.gap).unwrap_or(defaults::animated_sheet::GAP),
        clip_length_secs: args.clip_length.or(cfg.animated_sheet.clip_length_secs).unwrap_or(defaults::animated_sheet::CLIP_LENGTH_SECS),
        fps:              args.fps.or(cfg.animated_sheet.fps).unwrap_or(defaults::animated_sheet::FPS),
        quality:          args.quality.or(cfg.animated_sheet.quality).unwrap_or(defaults::animated_sheet::QUALITY),
        thumb_font_size:  args.thumb_font.or(cfg.animated_sheet.thumb_font).unwrap_or(defaults::animated_sheet::THUMB_FONT),
        header_font_size: args.header_font.or(cfg.animated_sheet.header_font).unwrap_or(defaults::animated_sheet::HEADER_FONT),
        show_timestamps:  resolve_bool(args.timestamps, args.no_timestamps, cfg.animated_sheet.show_timestamps, true),
        show_header:      resolve_bool(args.header, args.no_header, cfg.animated_sheet.show_header, true),
        suffix,
        theme:            resolve_theme(&args.theme, cfg.animated_sheet.theme.as_deref(), defaults::animated_sheet::THEME),
    };

    let mut done = 0u64;
    let mut failed = 0u64;
    let exists = |p: &std::path::Path| p.exists();
    for (i, src) in inputs.iter().enumerate() {
        let idx = i as u64 + 1;
        reporter.start_file(idx, total, src);
        let info = match ffmpeg_test_hook_probe(&tools, &src.to_string_lossy()).await {
            Ok(v) => v,
            Err(e) => { eprintln!("{}: {e}", src.display()); failed += 1; continue; }
        };
        let out_dir = inputs::out_dir(&args.shared, src);
        let out_path = animated_sheet_path(src, &out_dir, &opts.suffix, &exists);
        let ctx = PipelineContext {
            ffmpeg: &tools.ffmpeg,
            cancelled: cancelled.clone(),
            reporter: &pr,
            has_zscale,
        };
        match generate(src, &info, &out_path, &opts, &font_path, &ctx).await {
            Ok(()) => { println!("{}", out_path.display()); done += 1; }
            Err(RunError::Killed) => { /* cancellation — don't count as failed */ }
            Err(e) => { eprintln!("{}: {e}", src.display()); failed += 1; }
        }
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) { break; }
    }
    reporter.finish();
    if !args.shared.quiet {
        eprintln!("{done} done · {failed} failed · {} cancelled",
                  if cancelled.load(std::sync::atomic::Ordering::Relaxed) { total - done - failed } else { 0 });
    }
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) { return 130; }
    if failed > 0 { 1 } else { 0 }
}
