// src-tauri/src/bin/mosaic_cli/run/reel.rs
use crate::cli::ReelArgs;
use crate::config::Config;
use crate::progress::Reporter;
use crate::run::{format::resolve_reel_format, inputs};
use mosaic_lib::{
    defaults,
    ffmpeg::{RunError},
    ffmpeg_test_hook_locate, ffmpeg_test_hook_probe,
    jobs::{PipelineContext, ProgressReporter},
    output_path::{preview_reel_path, DEFAULT_PREVIEW_SUFFIX},
    preview_reel::{generate, PreviewOptions},
};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub async fn run(args: ReelArgs, cfg: &Config) -> i32 {
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

    let suffix_raw = args.suffix.clone()
        .or_else(|| cfg.reel.suffix.clone())
        .unwrap_or_else(|| DEFAULT_PREVIEW_SUFFIX.to_string());
    let suffix = match crate::run::suffix::validate(&suffix_raw) {
        Ok(s) => s,
        Err(e) => { eprintln!("{e}"); return 2; }
    };

    let opts = PreviewOptions {
        count:            args.count.or(cfg.reel.count).unwrap_or(defaults::reel::COUNT),
        clip_length_secs: args.clip_length.or(cfg.reel.clip_length_secs).unwrap_or(defaults::reel::CLIP_LENGTH_SECS),
        height:           args.height.or(cfg.reel.height).unwrap_or(defaults::reel::HEIGHT),
        fps:              args.fps.or(cfg.reel.fps).unwrap_or(defaults::reel::FPS),
        quality:          args.quality.or(cfg.reel.quality).unwrap_or(defaults::reel::QUALITY),
        suffix,
        format:           resolve_reel_format(&args.format, cfg.reel.format.as_deref(), defaults::reel::FORMAT),
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
        let out_path = preview_reel_path(src, &out_dir, opts.format, &opts.suffix, &exists);
        let ctx = PipelineContext {
            ffmpeg: &tools.ffmpeg,
            cancelled: cancelled.clone(),
            reporter: &pr,
            has_zscale,
        };
        match generate(src, &info, &out_path, &opts, &ctx).await {
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
