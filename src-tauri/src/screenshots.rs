use crate::ffmpeg::{run_cancellable, RunError};
use crate::jobs::ProgressReporter;
use crate::layout::sample_timestamps;
use crate::output_path::{jpeg_qv, screenshot_path, OutputFormat};
use crate::video_info::VideoInfo;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenshotsOptions {
    pub count: u32,
    pub format: OutputFormat,
    pub jpeg_quality: u32,
    #[serde(default)]
    pub suffix: String,
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
        (reporter.emit)(idx, total, &format!("Shot {}/{}", idx, total));

        let out = screenshot_path(source, out_dir, opts.format, &opts.suffix, idx, opts.count);
        let mut args = crate::ffmpeg::base_args();
        args.extend([
            "-ss".into(), format!("{}", ts),
            "-i".into(), source.to_string_lossy().into_owned(),
            "-vframes".into(), "1".into(),
        ]);
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", jpeg_qv(opts.jpeg_quality))]);
        }
        args.push(out.to_string_lossy().into_owned());

        run_cancellable(ffmpeg, &args, cancelled.clone()).await?;
        outputs.push(out);
    }

    Ok(outputs)
}
