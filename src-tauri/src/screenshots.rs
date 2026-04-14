use crate::contact_sheet::ProgressReporter;
use crate::ffmpeg::{run_cancellable, RunError};
use crate::layout::sample_timestamps;
use crate::output_path::{screenshot_path, OutputFormat};
use crate::video_info::VideoInfo;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenshotsOptions {
    pub count: u32,
    pub width: u32, // 0 = keep source
    pub format: OutputFormat,
    pub jpeg_quality: u32,
}

pub async fn generate(
    source: &Path,
    info: &VideoInfo,
    out_dir: &Path,
    opts: &ScreenshotsOptions,
    ffmpeg: &Path,
    cancelled: Arc<AtomicBool>,
    reporter: &ProgressReporter<'_>,
) -> Result<Vec<std::path::PathBuf>, RunError> {
    std::fs::create_dir_all(out_dir)?;
    let timestamps = sample_timestamps(info.duration_secs, opts.count);
    let total = opts.count;
    let mut outputs = Vec::with_capacity(timestamps.len());

    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        (reporter.emit)(idx, total, &format!("Screenshot {}/{}", idx, total));

        let out = screenshot_path(source, out_dir, opts.format, idx, opts.count);
        let mut args: Vec<String> = vec![
            "-hide_banner".into(), "-loglevel".into(), "error".into(), "-y".into(),
            "-ss".into(), format!("{}", ts),
            "-i".into(), source.to_string_lossy().into_owned(),
            "-vframes".into(), "1".into(),
        ];
        if opts.width > 0 {
            args.extend(["-vf".into(), format!("scale={}:-2", opts.width)]);
        }
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", crate::contact_sheet::jpeg_qv(opts.jpeg_quality))]);
        }
        args.push(out.to_string_lossy().into_owned());

        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
        outputs.push(out);
    }

    Ok(outputs)
}
