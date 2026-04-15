use crate::drawtext::format_hms_escaped;
use crate::ffmpeg::{run_cancellable, RunError};
use crate::header::build_header_lines;
use crate::jobs::ProgressReporter;
use crate::layout::{compute_sheet_layout, header_height, line_height, sample_timestamps};
use crate::output_path::{jpeg_qv, OutputFormat};
use crate::video_info::VideoInfo;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
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
}

pub async fn generate(
    source: &Path,
    info: &VideoInfo,
    output_path: &Path,
    opts: &SheetOptions,
    ffmpeg: &Path,
    font: &Path,
    cancelled: Arc<AtomicBool>,
    reporter: &ProgressReporter<'_>,
) -> Result<(), RunError> {
    let layout = compute_sheet_layout(opts.cols, opts.rows, opts.width, opts.gap);
    let timestamps = sample_timestamps(info.duration_secs, layout.total);
    let tmp = TempDir::new()?;
    let width_digits = crate::layout::pad_width_for_count(layout.total);

    let font_path = font_for_ffmpeg(font);
    let total_steps = layout.total + 2 + u32::from(opts.show_header); // extracts + tile + stack + header

    // 1. Extract thumbnails
    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        (reporter.emit)(idx, total_steps, &format!("Thumb {}/{}", idx, layout.total));

        let thumb = tmp.path().join(format!("thumb_{:0width$}.png", idx, width = width_digits));
        let mut vf = format!("scale={}:-2", layout.thumb_w);
        if opts.show_timestamps {
            let hms = format_hms_escaped(*ts);
            vf.push_str(&format!(
                ",drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:shadowcolor=black:shadowx=1:shadowy=1:x=5:y=h-th-5",
                hms, font_path, opts.thumb_font_size
            ));
        }
        let mut args = crate::ffmpeg::base_args();
        args.extend([
            "-ss".into(), format!("{}", ts),
            "-i".into(), source.to_string_lossy().into_owned(),
            "-vframes".into(), "1".into(),
            "-vf".into(), vf,
            thumb.to_string_lossy().into_owned(),
        ]);
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
    }

    // 2. Tile
    (reporter.emit)(layout.total + 1, total_steps, "Tiling grid");
    let grid = tmp.path().join("grid.png");
    let tile_input = tmp.path().join(format!("thumb_%0{}d.png", width_digits));
    let mut args = crate::ffmpeg::base_args();
    args.extend([
        "-framerate".into(), TILE_INPUT_FPS.into(),
        "-start_number".into(), "1".into(),
        "-i".into(), tile_input.to_string_lossy().into_owned(),
        "-vf".into(), format!(
            "tile={}x{}:margin={}:padding={}:color=0x000000",
            opts.cols, opts.rows, opts.gap, opts.gap
        ),
        "-frames:v".into(), "1".into(),
        grid.to_string_lossy().into_owned(),
    ]);
    run_cancellable(ffmpeg, &args, cancelled.clone()).await?;

    // 3. Header (optional) + 4. Finalize. The result of this block is a source
    // path on disk (`final_src`) that we then rename/copy into `output_path`.
    let final_src: PathBuf;
    if opts.show_header {
        (reporter.emit)(layout.total + 2, total_steps, "Header");
        let display = source.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let (l1, l2) = build_header_lines(info, &display);
        let h = header_height(opts.header_font_size, opts.gap);
        let line_h = line_height(opts.header_font_size);
        let vf = format!(
            "drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:x={}:y={},drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:x={}:y={}",
            l1, font_path, opts.header_font_size, opts.gap, opts.gap,
            l2, font_path, opts.header_font_size, opts.gap, opts.gap + line_h
        );
        let header = tmp.path().join("header.png");
        let mut args = crate::ffmpeg::base_args();
        args.extend([
            "-f".into(), "lavfi".into(),
            "-i".into(), format!("color=c=0x000000:s={}x{}:d=1", layout.grid_w, h),
            "-vf".into(), vf,
            "-frames:v".into(), "1".into(),
            header.to_string_lossy().into_owned(),
        ]);
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;

        (reporter.emit)(total_steps, total_steps, "Composing");
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
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
        final_src = final_tmp;
    } else {
        (reporter.emit)(total_steps, total_steps, "Finalizing");
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
                run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
                final_src = final_tmp;
            }
        }
    }

    // 5. Move final into place
    std::fs::create_dir_all(output_path.parent().unwrap_or(Path::new(".")))?;
    std::fs::rename(&final_src, output_path).or_else(|_| std::fs::copy(&final_src, output_path).map(|_| ()))?;
    Ok(())
}

fn font_for_ffmpeg(p: &Path) -> String {
    let mut s = p.to_string_lossy().into_owned();
    if cfg!(windows) {
        // Escape drive colon and normalise slashes for drawtext
        s = s.replace('\\', "/");
        if let Some(idx) = s.find(':') {
            s.replace_range(idx..idx + 1, r"\:");
        }
    }
    s
}
