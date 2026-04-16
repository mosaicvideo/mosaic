use crate::ffmpeg::{run_batch_cancellable, RunError};
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

    let mut batch = Vec::with_capacity(timestamps.len());
    let mut outputs = Vec::with_capacity(timestamps.len());
    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        let out = screenshot_path(source, out_dir, opts.format, &opts.suffix, idx, opts.count);
        let mut args = crate::ffmpeg::base_args();
        args.extend(crate::ffmpeg::seek_input_args(source, *ts));
        args.extend(["-frames:v".into(), "1".into()]);
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", jpeg_qv(opts.jpeg_quality))]);
        }
        args.push(out.to_string_lossy().into_owned());
        batch.push(args);
        outputs.push(out);
    }

    let mut done = 0u32;
    run_batch_cancellable(ffmpeg, batch, cancelled.clone(), |_| {
        done += 1;
        (reporter.emit)(done, total, &format!("Shot {}/{}", done, total));
    }).await?;

    Ok(outputs)
}
