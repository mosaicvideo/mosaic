use crate::drawtext::{escape_drawtext, format_hms_escaped};
use crate::ffmpeg::{run_cancellable, RunError};
use crate::header::build_header_lines;
use crate::layout::{compute_sheet_layout, header_height, sample_timestamps};
use crate::output_path::OutputFormat;
use crate::video_info::VideoInfo;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tempfile::TempDir;

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
}

pub struct ProgressReporter<'a> {
    pub emit: &'a (dyn Fn(u32, u32, &str) + Send + Sync),
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
    let width_digits = layout.total.to_string().len().max(2);

    let font_path = font_for_ffmpeg(font);
    let total_steps = layout.total + 2 + u32::from(opts.show_header); // extracts + tile + stack + header

    // 1. Extract thumbnails
    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        (reporter.emit)(idx, total_steps, &format!("Extracting thumb {}/{}", idx, layout.total));

        let thumb = tmp.path().join(format!("thumb_{:0width$}.png", idx, width = width_digits));
        let mut vf = format!("scale={}:-2", layout.thumb_w);
        if opts.show_timestamps {
            let hms = format_hms_escaped(*ts);
            vf.push_str(&format!(
                ",drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:shadowcolor=black:shadowx=1:shadowy=1:x=5:y=h-th-5",
                hms, font_path, opts.thumb_font_size
            ));
        }
        let args: Vec<String> = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
            "-ss".into(), format!("{}", ts),
            "-i".into(), source.to_string_lossy().into_owned(),
            "-vframes".into(), "1".into(),
            "-vf".into(), vf,
            thumb.to_string_lossy().into_owned(),
        ];
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
    }

    // 2. Tile
    (reporter.emit)(layout.total + 1, total_steps, "Building grid");
    let grid = tmp.path().join("grid.png");
    let tile_input = tmp.path().join(format!("thumb_%0{}d.png", width_digits));
    let args: Vec<String> = vec![
        "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
        "-framerate".into(), "1".into(),
        "-start_number".into(), "1".into(),
        "-i".into(), tile_input.to_string_lossy().into_owned(),
        "-vf".into(), format!(
            "tile={}x{}:margin={}:padding={}:color=0x000000",
            opts.cols, opts.rows, opts.gap, opts.gap
        ),
        "-frames:v".into(), "1".into(),
        grid.to_string_lossy().into_owned(),
    ];
    run_cancellable(ffmpeg, &args, cancelled.clone()).await?;

    // 3. Header (optional)
    let final_tmp: PathBuf;
    if opts.show_header {
        (reporter.emit)(layout.total + 2, total_steps, "Rendering header");
        let display = source.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let (l1, l2) = build_header_lines(info, &display);
        let h = header_height(opts.header_font_size, opts.gap);
        let line_h = ((opts.header_font_size as f64) * 1.3).round() as u32;
        let vf = format!(
            "drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:x={}:y={},drawtext=text='{}':fontfile='{}':fontsize={}:fontcolor=white:x={}:y={}",
            l1, font_path, opts.header_font_size, opts.gap, opts.gap,
            l2, font_path, opts.header_font_size, opts.gap, opts.gap + line_h
        );
        let header = tmp.path().join("header.png");
        let args: Vec<String> = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
            "-f".into(), "lavfi".into(),
            "-i".into(), format!("color=c=0x000000:s={}x{}:d=1", layout.grid_w, h),
            "-vf".into(), vf,
            "-frames:v".into(), "1".into(),
            header.to_string_lossy().into_owned(),
        ];
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;

        (reporter.emit)(total_steps, total_steps, "Composing final image");
        final_tmp = tmp.path().join(format!("final.{}", opts.format.ext()));
        let mut args: Vec<String> = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
            "-i".into(), header.to_string_lossy().into_owned(),
            "-i".into(), grid.to_string_lossy().into_owned(),
            "-filter_complex".into(), "vstack".into(),
            "-frames:v".into(), "1".into(),
        ];
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", jpeg_qv(opts.jpeg_quality))]);
        }
        args.push(final_tmp.to_string_lossy().into_owned());
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
    } else {
        (reporter.emit)(total_steps, total_steps, "Finalizing");
        // No header: re-encode grid to the target format so extension matches content.
        final_tmp = tmp.path().join(format!("final.{}", opts.format.ext()));
        let mut args: Vec<String> = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
            "-i".into(), grid.to_string_lossy().into_owned(),
            "-frames:v".into(), "1".into(),
        ];
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", jpeg_qv(opts.jpeg_quality))]);
        }
        args.push(final_tmp.to_string_lossy().into_owned());
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
    }

    // 4. Move final into place
    std::fs::create_dir_all(output_path.parent().unwrap_or(Path::new(".")))?;
    std::fs::rename(&final_tmp, output_path).or_else(|_| std::fs::copy(&final_tmp, output_path).map(|_| ()))?;
    Ok(())
}

pub fn jpeg_qv(q: u32) -> u32 {
    // libmjpeg: 2 (best) .. 31 (worst). Map 100→2, 50→15.
    let q = q.clamp(50, 100) as i64;
    (2 + ((100 - q) * 13 / 50)).max(2) as u32
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
