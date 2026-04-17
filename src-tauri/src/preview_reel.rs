use crate::ffmpeg::{run_batch_cancellable, RunError};
use crate::jobs::PipelineContext;
use crate::output_path::{vp9_crf, ReelFormat};
use crate::video_info::VideoInfo;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreviewOptions {
    pub count: u32,
    pub clip_length_secs: u32,
    pub height: u32,
    pub fps: u32,
    pub quality: u32,
    #[serde(default)]
    pub suffix: String,
    #[serde(default)]
    pub format: ReelFormat,
}

pub fn build_extract_args(
    source: &Path,
    info: &VideoInfo,
    timestamp: f64,
    clip_length_secs: u32,
    target_height: u32,
    has_zscale: bool,
    output: &Path,
) -> Vec<String> {
    let desired = clip_length_secs as f64;
    let remaining = (info.duration_secs - timestamp).max(0.0);
    let duration = desired.min(remaining);

    let mut args = crate::ffmpeg::base_args();
    args.extend(crate::ffmpeg::seek_input_args_clip(source, timestamp));
    args.extend([
        "-t".into(), format!("{:.3}", duration),
    ]);
    let tonemap = crate::ffmpeg::tonemap_filter(has_zscale, info.video.color_transfer.as_deref(), info.video.dv_profile);
    if tonemap.is_some() || info.video.height > target_height {
        let mut vf = String::new();
        if let Some(tm) = tonemap {
            vf.push_str(&tm);
        }
        if info.video.height > target_height {
            if !vf.is_empty() { vf.push(','); }
            vf.push_str(&format!("scale=-2:{}", target_height));
        }
        args.push("-vf".into());
        args.push(vf);
    }
    args.extend(crate::ffmpeg::h264_clip_encoder());
    args.push(output.to_string_lossy().into_owned());
    args
}

/// Hard cap on GIF frame rate. GIF has no inter-frame compression so output
/// size scales roughly linearly with frame count; capping avoids absurd files.
const GIF_FPS_CAP: u32 = 12;
/// Palette size cap for GIF. 128 colors looks nearly identical to 256 on
/// typical footage while meaningfully shrinking each frame's LZW dictionary.
const GIF_MAX_COLORS: u32 = 128;

/// Build the final stitch/encode invocation for the selected reel format.
///
/// - **WebP** (`libwebp`): `quality` maps directly to `-quality`; `-loop 0`.
/// - **WebM** (`libvpx-vp9`): `quality` maps to CRF via [`crate::output_path::vp9_crf`].
///   VP9 loops natively in browsers, so no `-loop` flag. `-b:v 0` enables pure CRF mode.
/// - **GIF**: palette-based, single pass via `-filter_complex` (required
///   because `split` emits labeled outputs). `quality` has no knob; ignored.
///   Frame rate is hard-capped at [`GIF_FPS_CAP`] and palette at
///   [`GIF_MAX_COLORS`] to keep output sizes reasonable.
pub fn build_stitch_args(
    concat_list: &Path,
    fps: u32,
    quality: u32,
    format: ReelFormat,
    output: &Path,
) -> Vec<String> {
    let mut args = crate::ffmpeg::base_args();
    args.extend([
        "-f".into(), "concat".into(),
        "-safe".into(), "0".into(),
        "-i".into(), concat_list.to_string_lossy().into_owned(),
    ]);
    match format {
        ReelFormat::Webp => {
            args.extend([
                "-vf".into(), format!("fps={}", fps),
                "-c:v".into(), "libwebp".into(),
                "-loop".into(), "0".into(),
                "-quality".into(), format!("{}", quality),
            ]);
        }
        ReelFormat::Webm => {
            args.extend([
                "-vf".into(), format!("fps={}", fps),
                "-c:v".into(), "libvpx-vp9".into(),
                "-b:v".into(), "0".into(),
                "-crf".into(), format!("{}", vp9_crf(quality)),
                "-pix_fmt".into(), "yuv420p".into(),
            ]);
        }
        ReelFormat::Gif => {
            let gif_fps = fps.min(GIF_FPS_CAP);
            args.extend([
                "-filter_complex".into(),
                format!(
                    "fps={},split[a][b];[a]palettegen=stats_mode=diff:max_colors={}[p];[b][p]paletteuse=dither=sierra2_4a",
                    gif_fps, GIF_MAX_COLORS
                ),
                "-loop".into(), "0".into(),
            ]);
        }
    }
    args.push(output.to_string_lossy().into_owned());
    args
}

pub fn render_concat_list(paths: &[PathBuf]) -> String {
    let mut out = String::new();
    for p in paths {
        let s = p.to_string_lossy();
        let escaped = s.replace('\\', "\\\\").replace('\'', "\\'");
        out.push_str("file '");
        out.push_str(&escaped);
        out.push_str("'\n");
    }
    out
}

pub async fn generate(
    source: &Path,
    info: &VideoInfo,
    out: &Path,
    opts: &PreviewOptions,
    ctx: &PipelineContext<'_>,
) -> Result<(), RunError> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let timestamps = crate::layout::sample_clip_timestamps(info.duration_secs, opts.count, opts.clip_length_secs as f64);
    if timestamps.is_empty() {
        return Err(RunError::NonZero {
            code: -1,
            stderr: format!(
                "source duration ({:.2}s) is too short for {} clips of {}s each",
                info.duration_secs, opts.count, opts.clip_length_secs
            ),
        });
    }
    let total_steps = (timestamps.len() as u32) + 1;

    let tmp = tempfile::TempDir::new()?;
    let mut clips: Vec<std::path::PathBuf> = Vec::with_capacity(timestamps.len());
    let mut batch = Vec::with_capacity(timestamps.len());

    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        let clip = tmp.path().join(format!("clip_{:03}.mp4", idx));
        let args = build_extract_args(source, info, *ts, opts.clip_length_secs, opts.height, ctx.has_zscale, &clip);
        batch.push(args);
        clips.push(clip);
    }

    let mut done = 0u32;
    run_batch_cancellable(ctx.ffmpeg, batch, ctx.cancelled.clone(), |_| {
        done += 1;
        (ctx.reporter.emit)(done, total_steps, &format!("Reel clip {}/{}", done, timestamps.len()));
    }).await?;

    (ctx.reporter.emit)(total_steps, total_steps, "Stitching reel");
    let concat_list = tmp.path().join("concat.txt");
    std::fs::write(&concat_list, render_concat_list(&clips))?;

    let args = build_stitch_args(&concat_list, opts.fps, opts.quality, opts.format, out);
    crate::ffmpeg::run_cancellable(ctx.ffmpeg, &args, ctx.cancelled.clone()).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_info(duration: f64, height: u32) -> VideoInfo {
        VideoInfo {
            filename: String::new(),
            duration_secs: duration,
            size_bytes: None,
            bit_rate: None,
            video: crate::video_info::VideoStream {
                codec: String::new(),
                profile: None,
                width: 1920,
                height,
                fps: 30.0,
                bit_rate: None,
                is_hdr: false,
                color_transfer: None,
                dv_profile: None,
            },
            audio: None,
        }
    }

    #[test]
    fn extract_args_basic() {
        let info = sample_info(60.0, 1080);
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            &info,
            12.5,
            5,
            480,
            false,
            &PathBuf::from("/tmp/out/clip_01.mp4"),
        );
        assert_eq!(args[0], "-hide_banner");
        assert!(args.iter().any(|a| a == "-an"));
        // Clip seeking: single -ss (no -copyts) for compatibility with TS containers
        assert!(!args.iter().any(|a| a == "-copyts"));
        assert!(args.windows(2).any(|w| w[0] == "-ss" && w[1] == "12.500"));
        assert!(args.windows(2).any(|w| w[0] == "-i" && w[1] == "/v/movie.mkv"));
        assert!(args.windows(2).any(|w| w[0] == "-t" && w[1] == "5.000"));
        assert!(args.windows(2).any(|w| w[0] == "-vf" && w[1] == "scale=-2:480"));
        assert_eq!(args.last().unwrap(), "/tmp/out/clip_01.mp4");
    }

    #[test]
    fn extract_args_skips_scale_when_source_smaller_or_equal() {
        let info = sample_info(60.0, 480);
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            &info,
            12.0,
            5,
            480,
            false,
            &PathBuf::from("/tmp/out/clip_01.mp4"),
        );
        assert!(!args.iter().any(|a| a == "-vf"));
    }

    #[test]
    fn extract_args_clamps_duration_when_overshoots_end() {
        let info = sample_info(10.0, 1080);
        let args = build_extract_args(
            Path::new("/v/movie.mkv"),
            &info,
            8.0,
            5,
            480,
            false,
            &PathBuf::from("/tmp/out/clip_03.mp4"),
        );
        // 10.0 - 8.0 = 2.0 (less than requested 5)
        assert!(args.windows(2).any(|w| w[0] == "-t" && w[1] == "2.000"));
    }

    #[test]
    fn stitch_args_uses_concat_demuxer_and_libwebp() {
        let args = build_stitch_args(
            Path::new("/tmp/concat.txt"),
            24,
            75,
            ReelFormat::Webp,
            Path::new("/out/movie - reel.webp"),
        );
        assert_eq!(args[0], "-hide_banner");
        assert!(args.windows(2).any(|w| w[0] == "-f" && w[1] == "concat"));
        assert!(args.windows(2).any(|w| w[0] == "-safe" && w[1] == "0"));
        assert!(args.windows(2).any(|w| w[0] == "-i" && w[1] == "/tmp/concat.txt"));
        assert!(args.windows(2).any(|w| w[0] == "-vf" && w[1] == "fps=24"));
        assert!(args.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libwebp"));
        assert!(args.windows(2).any(|w| w[0] == "-loop" && w[1] == "0"));
        assert!(args.windows(2).any(|w| w[0] == "-quality" && w[1] == "75"));
        assert_eq!(args.last().unwrap(), "/out/movie - reel.webp");
    }

    #[test]
    fn stitch_args_webm_uses_libvpx_vp9_crf_mode() {
        let args = build_stitch_args(
            Path::new("/tmp/concat.txt"),
            24,
            75,
            ReelFormat::Webm,
            Path::new("/out/movie - reel.webm"),
        );
        assert!(args.windows(2).any(|w| w[0] == "-vf" && w[1] == "fps=24"));
        assert!(args.windows(2).any(|w| w[0] == "-c:v" && w[1] == "libvpx-vp9"));
        assert!(args.windows(2).any(|w| w[0] == "-b:v" && w[1] == "0"));
        assert!(args.windows(2).any(|w| w[0] == "-crf"));
        assert!(args.windows(2).any(|w| w[0] == "-pix_fmt" && w[1] == "yuv420p"));
        // No -loop for VP9 — browsers loop WebM natively.
        assert!(!args.iter().any(|a| a == "-loop"));
        // Quality must NOT appear verbatim (webm uses CRF, not -quality).
        assert!(!args.iter().any(|a| a == "-quality"));
        assert_eq!(args.last().unwrap(), "/out/movie - reel.webm");
    }

    #[test]
    fn stitch_args_gif_uses_filter_complex_palettegen() {
        let args = build_stitch_args(
            Path::new("/tmp/concat.txt"),
            12,
            75,
            ReelFormat::Gif,
            Path::new("/out/movie - reel.gif"),
        );
        // GIF path must use -filter_complex (not -vf) because `split` emits labels.
        assert!(!args.iter().any(|a| a == "-vf"));
        let fc_pos = args.iter().position(|a| a == "-filter_complex")
            .expect("gif branch must use -filter_complex");
        let graph = &args[fc_pos + 1];
        assert!(graph.contains("fps=12"));
        assert!(graph.contains("split"));
        assert!(graph.contains("palettegen=stats_mode=diff"));
        assert!(graph.contains("max_colors=128"));
        assert!(graph.contains("paletteuse=dither=sierra2_4a"));
        assert!(args.windows(2).any(|w| w[0] == "-loop" && w[1] == "0"));
        // No codec selection flags — ffmpeg picks GIF encoder from the .gif extension.
        assert!(!args.iter().any(|a| a == "-c:v"));
        assert_eq!(args.last().unwrap(), "/out/movie - reel.gif");
    }

    #[test]
    fn stitch_args_gif_caps_fps_at_12() {
        // User requests 30 fps — GIF branch must clamp to the hard cap.
        let args = build_stitch_args(
            Path::new("/tmp/concat.txt"),
            30,
            75,
            ReelFormat::Gif,
            Path::new("/out/movie - reel.gif"),
        );
        let fc_pos = args.iter().position(|a| a == "-filter_complex").unwrap();
        let graph = &args[fc_pos + 1];
        assert!(graph.contains("fps=12"), "expected fps=12 cap, got graph: {}", graph);
        assert!(!graph.contains("fps=30"), "requested 30fps must be clamped for GIF");
    }

    #[test]
    fn stitch_args_gif_keeps_lower_requested_fps() {
        // User requests 8 fps — below the cap, must pass through unchanged.
        let args = build_stitch_args(
            Path::new("/tmp/concat.txt"),
            8,
            75,
            ReelFormat::Gif,
            Path::new("/out/movie - reel.gif"),
        );
        let fc_pos = args.iter().position(|a| a == "-filter_complex").unwrap();
        let graph = &args[fc_pos + 1];
        assert!(graph.contains("fps=8"), "fps below cap must pass through; got: {}", graph);
    }

    #[test]
    fn concat_list_basic() {
        let list = render_concat_list(&[
            PathBuf::from("/tmp/a/clip_01.mp4"),
            PathBuf::from("/tmp/a/clip_02.mp4"),
        ]);
        assert_eq!(
            list,
            "file '/tmp/a/clip_01.mp4'\nfile '/tmp/a/clip_02.mp4'\n"
        );
    }

    #[test]
    fn concat_list_escapes_single_quote_and_backslash() {
        // ffmpeg concat demuxer: inside single-quoted values, backslash and
        // single quote must each be backslash-escaped.
        let list = render_concat_list(&[
            PathBuf::from(r"/tmp/o'brien\videos/clip.mp4"),
        ]);
        assert_eq!(list, "file '/tmp/o\\'brien\\\\videos/clip.mp4'\n");
    }
}
