// src-tauri/src/bin/mosaic_cli/run/screenshots.rs
use crate::cli::ScreenshotsArgs;
use crate::config::Config;
use crate::progress::Reporter;
use crate::run::{format::resolve_img_format, inputs};
use mosaic_lib::{
    defaults,
    ffmpeg::RunError,
    ffmpeg_test_hook_locate, ffmpeg_test_hook_probe,
    jobs::{PipelineContext, ProgressReporter},
    output_path::DEFAULT_SHOTS_SUFFIX,
    screenshots::{generate, ScreenshotsOptions},
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub async fn run(args: ScreenshotsArgs, cfg: &Config) -> i32 {
    let tools = match ffmpeg_test_hook_locate() {
        Ok(t) => t,
        Err(e) => { eprintln!("{e}"); crate::hints::print_tool_install_hint(); return 2; }
    };
    let inputs = match inputs::expand(&args.shared.inputs, !args.shared.no_recursive) {
        Ok(v) => v, Err(e) => { eprintln!("{e}"); return 2; }
    };
    if inputs.is_empty() { eprintln!("no input files"); return 2; }

    let has_zscale = tools.detect_has_zscale();
    let cancelled = Arc::new(AtomicBool::new(false));
    crate::signals::install(cancelled.clone());

    let total = inputs.len() as u64;
    let reporter = Reporter::new(total, args.shared.quiet);
    let emit = reporter.emit_fn();
    let pr = ProgressReporter { emit: &emit };

    let count   = args.count.or(cfg.screenshots.count).unwrap_or(defaults::screenshots::COUNT);
    let fmt     = resolve_img_format(&args.format, cfg.screenshots.format.as_deref(), defaults::screenshots::FORMAT);
    let quality = args.quality
        .or(cfg.screenshots.quality)
        .unwrap_or(defaults::screenshots::JPEG_QUALITY);
    let suffix_raw = args.suffix.clone()
        .or_else(|| cfg.screenshots.suffix.clone())
        .unwrap_or_else(|| DEFAULT_SHOTS_SUFFIX.to_string());
    let suffix = match crate::run::suffix::validate(&suffix_raw) {
        Ok(s) => s,
        Err(e) => { eprintln!("{e}"); return 2; }
    };

    let opts = ScreenshotsOptions {
        count, format: fmt, jpeg_quality: quality, suffix,
    };

    let mut done = 0u64;
    let mut failed = 0u64;
    for (i, src) in inputs.iter().enumerate() {
        let idx = i as u64 + 1;
        reporter.start_file(idx, total, src);
        let info = match ffmpeg_test_hook_probe(&tools, &src.to_string_lossy()).await {
            Ok(v) => v,
            Err(e) => { eprintln!("{}: {e}", src.display()); failed += 1; continue; }
        };
        let out_dir = inputs::out_dir(&args.shared, src);
        let ctx = PipelineContext {
            ffmpeg: &tools.ffmpeg,
            cancelled: cancelled.clone(),
            reporter: &pr,
            has_zscale,
        };
        match generate(src, &info, &out_dir, &opts, &ctx).await {
            Ok(paths) => {
                for p in paths { println!("{}", p.display()); }
                done += 1;
            }
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
