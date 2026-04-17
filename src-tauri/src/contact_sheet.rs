use crate::drawtext::{font_for_ffmpeg, format_hms_escaped, header_overlay, timestamp_overlay};
use crate::ffmpeg::{run_batch_cancellable, run_cancellable, RunError};
use crate::header::build_header_lines;
use crate::jobs::PipelineContext;
use crate::layout::{compute_sheet_layout, header_height, line_height, sample_timestamps, thumb_height};
use crate::output_path::{jpeg_qv, OutputFormat, SheetTheme};
use crate::video_info::VideoInfo;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// ffmpeg's tile filter consumes its inputs as a video stream; this is the input
/// framerate for the per-thumbnail stills going into `tile`, not a user-facing knob.
const TILE_INPUT_FPS: &str = "1";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SheetOptions {
    pub cols: u32,
    pub rows: u32,
    pub width: u32,
    pub gap: u32,
    pub thumb_font_size: u32,
    pub header_font_size: u32,
    pub show_timestamps: bool,
    pub show_header: bool,
    pub format: OutputFormat,
    pub jpeg_quality: u32,
    #[serde(default)]
    pub suffix: String,
    #[serde(default)]
    pub theme: SheetTheme,
}

pub async fn generate(
    source: &Path,
    info: &VideoInfo,
    output_path: &Path,
    opts: &SheetOptions,
    font: &Path,
    ctx: &PipelineContext<'_>,
) -> Result<(), RunError> {
    let layout = compute_sheet_layout(opts.cols, opts.rows, opts.width, opts.gap);
    let timestamps = sample_timestamps(info.duration_secs, layout.total);
    let tmp = TempDir::new()?;
    let width_digits = crate::layout::pad_width_for_count(layout.total);

    let font_path = font_for_ffmpeg(font);
    let total_steps = layout.total + 2 + u32::from(opts.show_header); // extracts + tile + stack + header

    // Explicit W:H from displayed dims: ffmpeg's `scale=W:-2` auto-height uses
    // the encoded pixel grid and ignores SAR, so anamorphic sources (e.g.
    // phone-shot 9:16 encoded in a 1:1 frame) come out square with the
    // content stretched. `VideoStream.width`/`height` already carry the
    // square-pixel displayed dims, so the same formula the animated sheet
    // uses produces cells with the correct aspect for both normal and
    // anamorphic / rotated sources.
    let thumb_h = thumb_height(layout.thumb_w, info.video.width, info.video.height);

    // 1. Extract thumbnails (parallel)
    let tonemap = crate::ffmpeg::tonemap_filter(ctx.has_zscale, info.video.color_transfer.as_deref(), info.video.dv_profile);
    let mut batch = Vec::with_capacity(timestamps.len());
    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        let thumb = tmp.path().join(format!("thumb_{:0width$}.png", idx, width = width_digits));
        let mut vf = String::new();
        if let Some(ref tm) = tonemap {
            vf.push_str(tm);
            vf.push(',');
        }
        vf.push_str(&format!("scale={}:{}", layout.thumb_w, thumb_h));
        if opts.show_timestamps {
            vf.push(',');
            vf.push_str(&timestamp_overlay(
                &format_hms_escaped(*ts),
                &font_path,
                opts.thumb_font_size,
                opts.theme.fontcolor(),
                opts.theme.shadowcolor(),
            ));
        }
        let mut args = crate::ffmpeg::base_args();
        args.extend(crate::ffmpeg::seek_input_args(source, *ts));
        args.extend([
            "-frames:v".into(), "1".into(),
            "-vf".into(), vf,
            thumb.to_string_lossy().into_owned(),
        ]);
        batch.push(args);
    }

    let mut done = 0u32;
    run_batch_cancellable(ctx.ffmpeg, batch, ctx.cancelled.clone(), |_| {
        done += 1;
        (ctx.reporter.emit)(done, total_steps, &format!("Thumb {}/{}", done, layout.total));
    }).await?;

    // 2. Tile
    (ctx.reporter.emit)(layout.total + 1, total_steps, "Tiling grid");
    let grid = tmp.path().join("grid.png");
    let tile_input = tmp.path().join(format!("thumb_%0{}d.png", width_digits));
    let mut args = crate::ffmpeg::base_args();
    args.extend([
        "-framerate".into(), TILE_INPUT_FPS.into(),
        "-start_number".into(), "1".into(),
        "-i".into(), tile_input.to_string_lossy().into_owned(),
        "-vf".into(), format!(
            "tile={}x{}:margin={}:padding={}:color={}",
            opts.cols, opts.rows, opts.gap, opts.gap, opts.theme.bg()
        ),
        "-frames:v".into(), "1".into(),
        grid.to_string_lossy().into_owned(),
    ]);
    run_cancellable(ctx.ffmpeg, &args, ctx.cancelled.clone()).await?;

    // 3. Header (optional) + 4. Finalize. The result of this block is a source
    // path on disk (`final_src`) that we then rename/copy into `output_path`.
    let final_src: PathBuf;
    if opts.show_header {
        (ctx.reporter.emit)(layout.total + 2, total_steps, "Header");
        let display = source.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let lines = build_header_lines(info, &display);
        let h = header_height(opts.header_font_size, opts.gap, lines.len() as u32);
        let line_h = line_height(opts.header_font_size);
        let vf = header_overlay(&lines, &font_path, opts.header_font_size, opts.theme.fontcolor(), opts.gap, line_h);
        let header = tmp.path().join("header.png");
        let mut args = crate::ffmpeg::base_args();
        args.extend([
            "-f".into(), "lavfi".into(),
            "-i".into(), format!("color=c={}:s={}x{}:d=1", opts.theme.bg(), layout.grid_w, h),
            "-vf".into(), vf,
            "-frames:v".into(), "1".into(),
            header.to_string_lossy().into_owned(),
        ]);
        run_cancellable(ctx.ffmpeg, &args, ctx.cancelled.clone()).await?;

        (ctx.reporter.emit)(total_steps, total_steps, "Composing");
        let final_tmp = tmp.path().join(format!("final.{}", opts.format.ext()));
        let mut args = crate::ffmpeg::base_args();
        args.extend([
            "-i".into(), header.to_string_lossy().into_owned(),
            "-i".into(), grid.to_string_lossy().into_owned(),
            "-filter_complex".into(), "vstack".into(),
            "-frames:v".into(), "1".into(),
        ]);
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", jpeg_qv(opts.jpeg_quality))]);
        }
        args.push(final_tmp.to_string_lossy().into_owned());
        run_cancellable(ctx.ffmpeg, &args, ctx.cancelled.clone()).await?;
        final_src = final_tmp;
    } else {
        (ctx.reporter.emit)(total_steps, total_steps, "Finalizing");
        match opts.format {
            OutputFormat::Png => {
                // Grid is already PNG; no re-encode needed. Reuse it as-is.
                final_src = grid;
            }
            OutputFormat::Jpeg => {
                // Convert PNG grid to JPEG with the requested quality.
                let final_tmp = tmp.path().join(format!("final.{}", opts.format.ext()));
                let mut args = crate::ffmpeg::base_args();
                args.extend([
                    "-i".into(), grid.to_string_lossy().into_owned(),
                    "-frames:v".into(), "1".into(),
                    "-q:v".into(), format!("{}", jpeg_qv(opts.jpeg_quality)),
                    final_tmp.to_string_lossy().into_owned(),
                ]);
                run_cancellable(ctx.ffmpeg, &args, ctx.cancelled.clone()).await?;
                final_src = final_tmp;
            }
        }
    }

    // 5. Move final into place
    std::fs::create_dir_all(output_path.parent().unwrap_or(Path::new(".")))?;
    std::fs::rename(&final_src, output_path).or_else(|_| std::fs::copy(&final_src, output_path).map(|_| ()))?;
    Ok(())
}

