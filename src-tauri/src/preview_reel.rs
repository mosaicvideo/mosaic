use crate::ffmpeg::RunError;
use crate::jobs::ProgressReporter;
use crate::video_info::VideoInfo;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreviewOptions {
    pub count: u32,
    pub clip_length_secs: u32,
    pub height: u32,
    pub fps: u32,
    pub quality: u32,
    #[serde(default)]
    pub suffix: String,
}

pub fn build_extract_args(
    source: &Path,
    info: &VideoInfo,
    timestamp: f64,
    clip_length_secs: u32,
    target_height: u32,
    output: &Path,
) -> Vec<String> {
    let desired = clip_length_secs as f64;
    let remaining = (info.duration_secs - timestamp).max(0.0);
    let duration = desired.min(remaining);

    let mut args: Vec<String> = vec![
        "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
        "-ss".into(), format!("{:.3}", timestamp),
        "-i".into(), source.to_string_lossy().into_owned(),
        "-t".into(), format!("{:.3}", duration),
        "-an".into(),
    ];
    if info.video.height > target_height {
        args.push("-vf".into());
        args.push(format!("scale=-2:{}", target_height));
    }
    args.extend([
        "-c:v".into(), "libx264".into(),
        "-preset".into(), "veryfast".into(),
        "-crf".into(), "23".into(),
        "-pix_fmt".into(), "yuv420p".into(),
    ]);
    args.push(output.to_string_lossy().into_owned());
    args
}

pub fn build_stitch_args(
    concat_list: &Path,
    fps: u32,
    quality: u32,
    output: &Path,
) -> Vec<String> {
    vec![
        "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
        "-f".into(), "concat".into(),
        "-safe".into(), "0".into(),
        "-i".into(), concat_list.to_string_lossy().into_owned(),
        "-vf".into(), format!("fps={}", fps),
        "-c:v".into(), "libwebp".into(),
        "-loop".into(), "0".into(),
        "-quality".into(), format!("{}", quality),
        output.to_string_lossy().into_owned(),
    ]
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
    ffmpeg: &Path,
    cancelled: Arc<AtomicBool>,
    reporter: &ProgressReporter<'_>,
) -> Result<(), RunError> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let timestamps = crate::layout::sample_timestamps(info.duration_secs, opts.count);
    if timestamps.is_empty() {
        return Err(RunError::NonZero {
            code: -1,
            stderr: "source duration too short for preview reel".into(),
        });
    }
    let total_steps = (timestamps.len() as u32) + 1;

    let tmp = tempfile::TempDir::new()?;
    let mut clips: Vec<std::path::PathBuf> = Vec::with_capacity(timestamps.len());

    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        (reporter.emit)(idx, total_steps, &format!("Reel clip {}/{}", idx, timestamps.len()));

        let clip = tmp.path().join(format!("clip_{:03}.mp4", idx));
        let args = build_extract_args(source, info, *ts, opts.clip_length_secs, opts.height, &clip);
        crate::ffmpeg::run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
        clips.push(clip);
    }

    (reporter.emit)(total_steps, total_steps, "Stitching reel");
    let concat_list = tmp.path().join("concat.txt");
    std::fs::write(&concat_list, render_concat_list(&clips))?;

    let args = build_stitch_args(&concat_list, opts.fps, opts.quality, out);
    crate::ffmpeg::run_cancellable(ffmpeg, &args, cancelled.clone()).await?;

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
            &PathBuf::from("/tmp/out/clip_01.mp4"),
        );
        assert_eq!(args[0], "-hide_banner");
        assert!(args.windows(2).any(|w| w[0] == "-ss" && w[1] == "12.500"));
        assert!(args.windows(2).any(|w| w[0] == "-i" && w[1] == "/v/movie.mkv"));
        assert!(args.windows(2).any(|w| w[0] == "-t" && w[1] == "5.000"));
        assert!(args.iter().any(|a| a == "-an"));
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
