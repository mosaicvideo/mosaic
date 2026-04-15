use crate::drawtext::{font_for_ffmpeg, format_hms_escaped, header_overlay, timestamp_overlay};
use crate::ffmpeg::{run_cancellable, RunError};
use crate::header::build_header_lines;
use crate::jobs::ProgressReporter;
use crate::layout::{compute_sheet_layout, header_height, line_height, sample_timestamps, xstack_layout, SheetLayout};
use crate::output_path::SheetTheme;
use crate::video_info::VideoInfo;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// ffmpeg's xstack requires at least 2 inputs and accepts up to 32
/// (AV_FILTER_MAX_INPUTS in libavfilter). We fail fast outside this band
/// rather than let ffmpeg error cryptically.
const MIN_CELLS: u32 = 2;
const MAX_CELLS: u32 = 32;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimatedSheetOptions {
    pub cols: u32,
    pub rows: u32,
    pub width: u32,
    pub gap: u32,
    pub clip_length_secs: u32,
    pub fps: u32,
    pub quality: u32,
    pub thumb_font_size: u32,
    pub header_font_size: u32,
    pub show_timestamps: bool,
    pub show_header: bool,
    #[serde(default)]
    pub suffix: String,
    #[serde(default)]
    pub theme: SheetTheme,
}

/// Derive a thumb height from source aspect ratio, rounded down to an even
/// pixel count so yuv420p subsampling is happy.
pub fn thumb_height(thumb_w: u32, src_w: u32, src_h: u32) -> u32 {
    if src_w == 0 || src_h == 0 { return thumb_w.max(2) - (thumb_w % 2); }
    let raw = (thumb_w as f64 * src_h as f64 / src_w as f64).round() as u32;
    let even = raw - (raw % 2);
    even.max(2)
}

pub(crate) struct HeaderParams {
    pub(crate) line1: String,
    pub(crate) line2: String,
    pub(crate) height: u32,
    pub(crate) line_h: u32,
    pub(crate) font_size: u32,
    pub(crate) font_ffmpeg: String,
}

#[allow(clippy::too_many_arguments)]
pub fn build_extract_args(
    source: &Path,
    timestamp: f64,
    thumb_w: u32,
    thumb_h: u32,
    gap: u32,
    fps: u32,
    clip_length_secs: u32,
    show_timestamps: bool,
    thumb_font_size: u32,
    theme: SheetTheme,
    font: &Path,
    output: &Path,
) -> Vec<String> {
    let mut vf = format!("scale={}:{}", thumb_w, thumb_h);
    if show_timestamps {
        vf.push(',');
        vf.push_str(&timestamp_overlay(
            &format_hms_escaped(timestamp),
            &font_for_ffmpeg(font),
            thumb_font_size,
            theme.fontcolor(),
            theme.shadowcolor(),
        ));
    }
    vf.push_str(&format!(
        ",pad={}:{}:{}:{}:{}",
        thumb_w + gap, thumb_h + gap, gap / 2, gap / 2, theme.bg()
    ));

    let mut args = crate::ffmpeg::base_args();
    args.extend([
        "-ss".into(), format!("{:.3}", timestamp),
        "-i".into(), source.to_string_lossy().into_owned(),
        "-t".into(), format!("{}", clip_length_secs),
        "-an".into(),
        "-vf".into(), vf,
        "-r".into(), format!("{}", fps),
    ]);
    args.extend(crate::ffmpeg::h264_clip_encoder());
    args.push(output.to_string_lossy().into_owned());
    args
}

/// Build the single `-filter_complex` graph that:
///   1. xstacks N pre-padded cells into a grid,
///   2. pads the grid by gap/2 on all sides so outer margin matches the still
///      contact sheet (total width = `layout.grid_w`),
///   3. optionally composites a header panel (color source + two drawtexts) on
///      top via vstack.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_stitch_args(
    clips: &[PathBuf],
    layout: &SheetLayout,
    thumb_h: u32,
    gap: u32,
    theme: SheetTheme,
    header: Option<&HeaderParams>,
    clip_length_secs: u32,
    fps: u32,
    quality: u32,
    output: &Path,
) -> Vec<String> {
    let n = clips.len() as u32;
    let step_w = layout.thumb_w + gap;
    let step_h = thumb_h + gap;
    let grid_w = layout.grid_w;
    let grid_h = layout.rows * step_h + gap;
    let bg = theme.bg();

    let mut args = crate::ffmpeg::base_args();
    for clip in clips {
        args.extend(["-i".into(), clip.to_string_lossy().into_owned()]);
    }

    let inputs_tag: String = (0..n).map(|i| format!("[{}:v]", i)).collect();
    let layout_expr = xstack_layout(layout.cols, layout.rows, step_w, step_h);

    let mut graph = format!(
        "{}xstack=inputs={}:layout={}[xs];[xs]pad={}:{}:{}:{}:{}[grid]",
        inputs_tag, n, layout_expr, grid_w, grid_h, gap / 2, gap / 2, bg
    );

    let final_label = if let Some(h) = header {
        let hdr_draw = header_overlay(&h.line1, &h.line2, &h.font_ffmpeg, h.font_size, theme.fontcolor(), gap, h.line_h);
        graph.push_str(&format!(
            ";color=c={}:s={}x{}:d={}:r={},{}[hdr];[hdr][grid]vstack[out]",
            bg, grid_w, h.height, clip_length_secs, fps, hdr_draw
        ));
        "[out]"
    } else {
        "[grid]"
    };

    args.extend([
        "-filter_complex".into(), graph,
        "-map".into(), final_label.into(),
        "-c:v".into(), "libwebp".into(),
        "-loop".into(), "0".into(),
        "-quality".into(), format!("{}", quality),
        output.to_string_lossy().into_owned(),
    ]);
    args
}

#[allow(clippy::too_many_arguments)]
pub async fn generate(
    source: &Path,
    info: &VideoInfo,
    out: &Path,
    opts: &AnimatedSheetOptions,
    ffmpeg: &Path,
    font: &Path,
    cancelled: Arc<AtomicBool>,
    reporter: &ProgressReporter<'_>,
) -> Result<(), RunError> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let layout = compute_sheet_layout(opts.cols, opts.rows, opts.width, opts.gap);
    if layout.total < MIN_CELLS || layout.total > MAX_CELLS {
        return Err(RunError::NonZero {
            code: -1,
            stderr: format!(
                "animated contact sheet requires {}..={} cells; requested {} (cols={}, rows={})",
                MIN_CELLS, MAX_CELLS, layout.total, opts.cols, opts.rows
            ),
        });
    }

    let thumb_h = thumb_height(layout.thumb_w, info.video.width, info.video.height);
    let timestamps = sample_timestamps(info.duration_secs, layout.total);
    if timestamps.is_empty() {
        return Err(RunError::NonZero {
            code: -1,
            stderr: "source duration too short for animated contact sheet".into(),
        });
    }

    let total_steps = layout.total + 1;
    let tmp = tempfile::TempDir::new()?;

    let mut clips: Vec<PathBuf> = Vec::with_capacity(timestamps.len());
    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        (reporter.emit)(idx, total_steps, &format!("Cell {}/{}", idx, layout.total));

        let cell = tmp.path().join(format!("cell_{:03}.mp4", idx));
        let args = build_extract_args(
            source, *ts, layout.thumb_w, thumb_h, opts.gap, opts.fps,
            opts.clip_length_secs, opts.show_timestamps, opts.thumb_font_size,
            opts.theme, font, &cell,
        );
        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
        clips.push(cell);
    }

    (reporter.emit)(total_steps, total_steps, "Stitching sheet");

    let header_params = if opts.show_header {
        let display = source.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let (l1, l2) = build_header_lines(info, &display);
        Some(HeaderParams {
            line1: l1,
            line2: l2,
            height: header_height(opts.header_font_size, opts.gap),
            line_h: line_height(opts.header_font_size),
            font_size: opts.header_font_size,
            font_ffmpeg: font_for_ffmpeg(font),
        })
    } else {
        None
    };

    let args = build_stitch_args(
        &clips, &layout, thumb_h, opts.gap, opts.theme, header_params.as_ref(),
        opts.clip_length_secs, opts.fps, opts.quality, out,
    );
    run_cancellable(ffmpeg, &args, cancelled.clone()).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumb_height_16_by_9_source() {
        // 640 * 1080/1920 = 360 (even)
        assert_eq!(thumb_height(640, 1920, 1080), 360);
    }

    #[test]
    fn thumb_height_rounds_down_to_even() {
        // 100 * 601/1000 = 60.1 → 60 (already even)
        assert_eq!(thumb_height(100, 1000, 601), 60);
        // 100 * 603/1000 = 60.3 → 60 (rounded from 60, already even)
        assert_eq!(thumb_height(100, 1000, 603), 60);
        // 100 * 613/1000 = 61.3 → round to 61 → even 60
        assert_eq!(thumb_height(100, 1000, 613), 60);
    }

    #[test]
    fn thumb_height_handles_zero_source_dims() {
        assert_eq!(thumb_height(100, 0, 0), 100);
    }

    fn sample_info(duration: f64, w: u32, h: u32) -> VideoInfo {
        VideoInfo {
            filename: String::new(),
            duration_secs: duration,
            size_bytes: None,
            bit_rate: None,
            video: crate::video_info::VideoStream {
                codec: String::new(),
                profile: None,
                width: w,
                height: h,
                fps: 30.0,
                bit_rate: None,
            },
            audio: None,
        }
    }

    #[test]
    fn extract_args_shape() {
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            12.5,
            320, 180, 10, 12, 2,
            true, 18, SheetTheme::Dark,
            Path::new("/f/font.ttf"),
            Path::new("/tmp/cell.mp4"),
        );
        assert_eq!(args[0], "-hide_banner");
        assert!(args.windows(2).any(|w| w[0] == "-ss" && w[1] == "12.500"));
        assert!(args.windows(2).any(|w| w[0] == "-i" && w[1] == "/v/movie.mkv"));
        assert!(args.windows(2).any(|w| w[0] == "-t" && w[1] == "2"));
        assert!(args.iter().any(|a| a == "-an"));
        assert!(args.windows(2).any(|w| w[0] == "-r" && w[1] == "12"));
        assert!(args.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libx264"));
        assert!(args.windows(2).any(|w| w[0] == "-pix_fmt" && w[1] == "yuv420p"));
        let vf_pos = args.iter().position(|a| a == "-vf").unwrap();
        let vf = &args[vf_pos + 1];
        assert!(vf.contains("scale=320:180"));
        assert!(vf.contains("drawtext="));
        assert!(vf.contains("pad=330:190:5:5:0x000000"));
        assert_eq!(args.last().unwrap(), "/tmp/cell.mp4");
    }

    #[test]
    fn extract_args_light_theme_uses_white_bg_and_black_text() {
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            5.0,
            320, 180, 10, 12, 2,
            true, 18, SheetTheme::Light,
            Path::new("/f/font.ttf"),
            Path::new("/tmp/cell.mp4"),
        );
        let vf = args.iter().position(|a| a == "-vf").map(|i| &args[i + 1]).unwrap();
        assert!(vf.contains("pad=330:190:5:5:0xFFFFFF"));
        assert!(vf.contains("fontcolor=black"));
        assert!(vf.contains("shadowcolor=white"));
    }

    #[test]
    fn extract_args_omits_drawtext_when_timestamps_off() {
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            5.0,
            320, 180, 10, 12, 2,
            false, 18, SheetTheme::Dark,
            Path::new("/f/font.ttf"),
            Path::new("/tmp/cell.mp4"),
        );
        let vf = args.iter().position(|a| a == "-vf").map(|i| &args[i + 1]).unwrap();
        assert!(!vf.contains("drawtext"));
        assert!(vf.contains("scale=320:180"));
        assert!(vf.contains("pad=330:190:5:5:0x000000"));
    }

    #[test]
    fn stitch_args_no_header_maps_grid() {
        let layout = compute_sheet_layout(2, 2, 800, 10);
        let clips: Vec<PathBuf> = (0..4).map(|i| PathBuf::from(format!("/tmp/c{}.mp4", i))).collect();
        let args = build_stitch_args(
            &clips, &layout, 200, 10, SheetTheme::Dark, None, 3, 12, 75,
            Path::new("/out/sheet.webp"),
        );
        // 4 inputs
        assert_eq!(args.iter().filter(|a| *a == "-i").count(), 4);
        let fc = args.iter().position(|a| a == "-filter_complex").unwrap();
        let graph = &args[fc + 1];
        assert!(graph.contains("xstack=inputs=4:layout="));
        assert!(graph.contains("pad="));
        assert!(!graph.contains("vstack"));
        assert!(!graph.contains("color=c=0x000000"));
        let map_pos = args.iter().position(|a| a == "-map").unwrap();
        assert_eq!(args[map_pos + 1], "[grid]");
        assert!(args.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libwebp"));
        assert!(args.windows(2).any(|w| w[0] == "-loop" && w[1] == "0"));
        assert!(args.windows(2).any(|w| w[0] == "-quality" && w[1] == "75"));
        assert_eq!(args.last().unwrap(), "/out/sheet.webp");
    }

    #[test]
    fn stitch_args_with_header_maps_out_and_has_vstack() {
        let layout = compute_sheet_layout(2, 2, 800, 10);
        let clips: Vec<PathBuf> = (0..4).map(|i| PathBuf::from(format!("/tmp/c{}.mp4", i))).collect();
        let header = HeaderParams {
            line1: "movie.mkv".into(),
            line2: "Duration: 00:01:30 | 1920x1080".into(),
            height: 72, line_h: 26, font_size: 20,
            font_ffmpeg: "/f/font.ttf".into(),
        };
        let args = build_stitch_args(
            &clips, &layout, 200, 10, SheetTheme::Dark, Some(&header), 3, 12, 75,
            Path::new("/out/sheet.webp"),
        );
        let fc = args.iter().position(|a| a == "-filter_complex").unwrap();
        let graph = &args[fc + 1];
        assert!(graph.contains("color=c=0x000000"));
        assert!(graph.contains("drawtext=text='movie.mkv'"));
        assert!(graph.contains("vstack"));
        assert!(graph.ends_with("[out]"));
        let map_pos = args.iter().position(|a| a == "-map").unwrap();
        assert_eq!(args[map_pos + 1], "[out]");
    }

    #[test]
    fn stitch_args_light_theme_uses_white_bg() {
        let layout = compute_sheet_layout(2, 2, 800, 10);
        let clips: Vec<PathBuf> = (0..4).map(|i| PathBuf::from(format!("/tmp/c{}.mp4", i))).collect();
        let header = HeaderParams {
            line1: "m.mkv".into(), line2: "x".into(),
            height: 50, line_h: 20, font_size: 16,
            font_ffmpeg: "/f.ttf".into(),
        };
        let args = build_stitch_args(
            &clips, &layout, 200, 10, SheetTheme::Light, Some(&header), 3, 12, 75,
            Path::new("/out/sheet.webp"),
        );
        let fc = args.iter().position(|a| a == "-filter_complex").unwrap();
        let graph = &args[fc + 1];
        assert!(graph.contains("color=c=0xFFFFFF"));
        assert!(graph.contains("pad=798:430:5:5:0xFFFFFF"));
        assert!(graph.contains("fontcolor=black"));
    }

    #[test]
    fn stitch_args_includes_all_cells_as_xstack_inputs() {
        let layout = compute_sheet_layout(3, 2, 900, 10);
        let clips: Vec<PathBuf> = (0..6).map(|i| PathBuf::from(format!("/tmp/c{}.mp4", i))).collect();
        let args = build_stitch_args(
            &clips, &layout, 150, 10, SheetTheme::Dark, None, 2, 10, 75,
            Path::new("/out/sheet.webp"),
        );
        let fc = args.iter().position(|a| a == "-filter_complex").unwrap();
        let graph = &args[fc + 1];
        assert!(graph.starts_with("[0:v][1:v][2:v][3:v][4:v][5:v]xstack=inputs=6:"));
    }

    fn invoke_generate_with_grid(cols: u32, rows: u32) -> Result<(), RunError> {
        let info = sample_info(60.0, 1920, 1080);
        let opts = AnimatedSheetOptions {
            cols, rows, width: 1280, gap: 4,
            clip_length_secs: 1, fps: 8, quality: 75,
            thumb_font_size: 14, header_font_size: 18,
            show_timestamps: false, show_header: false,
            suffix: String::new(),
            theme: SheetTheme::Dark,
        };
        let out = PathBuf::from("/tmp/_never_written.webp");
        let ffmpeg = Path::new("/bin/false");
        let font = Path::new("/bin/false");
        let cancelled = Arc::new(AtomicBool::new(false));
        let reporter = ProgressReporter { emit: &|_, _, _| {} };
        let fut = generate(Path::new("/x.mp4"), &info, &out, &opts, ffmpeg, font, cancelled, &reporter);
        tokio::runtime::Builder::new_current_thread()
            .build().unwrap().block_on(fut)
    }

    #[test]
    fn generate_rejects_over_32_cells() {
        match invoke_generate_with_grid(6, 6) {
            Err(RunError::NonZero { stderr, .. }) => {
                assert!(stderr.contains("requires 2..=32"), "stderr: {}", stderr);
                assert!(stderr.contains("requested 36"), "stderr: {}", stderr);
            }
            other => panic!("expected guard error, got {:?}", other),
        }
    }

    #[test]
    fn generate_rejects_single_cell() {
        match invoke_generate_with_grid(1, 1) {
            Err(RunError::NonZero { stderr, .. }) => {
                assert!(stderr.contains("requires 2..=32"), "stderr: {}", stderr);
                assert!(stderr.contains("requested 1"), "stderr: {}", stderr);
            }
            other => panic!("expected guard error, got {:?}", other),
        }
    }
}
