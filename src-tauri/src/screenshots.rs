use crate::ffmpeg::{run_batch_cancellable, RunError};
use crate::jobs::{move_into_place, PipelineContext};
use crate::layout::{require_nonzero, sample_timestamps};
use crate::output_path::{jpeg_qv, screenshot_path, OutputFormat};
use crate::video_info::VideoInfo;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenshotsOptions {
    pub count: u32,
    pub format: OutputFormat,
    pub jpeg_quality: u32,
    #[serde(default)]
    pub suffix: String,
}

impl ScreenshotsOptions {
    pub fn validate(&self) -> Result<(), String> {
        require_nonzero("screenshot count", self.count)
    }
}

pub async fn generate(
    source: &Path,
    info: &VideoInfo,
    out_dir: &Path,
    opts: &ScreenshotsOptions,
    ctx: &PipelineContext<'_>,
) -> Result<Vec<std::path::PathBuf>, RunError> {
    opts.validate().map_err(RunError::Invalid)?;
    let tmp = tempfile::TempDir::new()?;
    let timestamps = sample_timestamps(info.duration_secs, opts.count);
    let total = opts.count;

    let tonemap = crate::ffmpeg::tonemap_filter(ctx.has_zscale, info.video.color_transfer.as_deref(), info.video.dv_profile);
    // Anamorphic sources (non-square SAR) need an explicit resize to the
    // displayed dims because PNG and JPEG carry no pixel-aspect metadata
    // most viewers honour. Rotation alone is handled by ffmpeg's autorotate
    // (decoded frame is already upright), so we gate on `sar` being Some.
    let sar_scale: Option<String> = info.video.sar
        .map(|_| format!("scale={}:{}", info.video.width, info.video.height));

    let mut batch = Vec::with_capacity(timestamps.len());
    let mut outputs = Vec::with_capacity(timestamps.len());
    let mut rendered = Vec::with_capacity(timestamps.len());
    for (i, ts) in timestamps.iter().enumerate() {
        let idx = (i as u32) + 1;
        let out = screenshot_path(source, out_dir, opts.format, &opts.suffix, idx, opts.count, &|p| p.exists());
        let mut args = crate::ffmpeg::base_args();
        args.extend(crate::ffmpeg::seek_input_args(source, *ts));
        args.extend(["-frames:v".into(), "1".into()]);
        let vf: Vec<&str> = [tonemap.as_deref(), sar_scale.as_deref()]
            .into_iter().flatten().collect();
        if !vf.is_empty() {
            args.extend(["-vf".into(), vf.join(",")]);
        }
        if matches!(opts.format, OutputFormat::Jpeg) {
            args.extend(["-q:v".into(), format!("{}", jpeg_qv(opts.jpeg_quality))]);
        }
        let staged = tmp.path().join(out.file_name().unwrap_or_default());
        args.push(staged.to_string_lossy().into_owned());
        batch.push(args);
        rendered.push(staged);
        outputs.push(out);
    }

    let mut done = 0u32;
    run_batch_cancellable(ctx.ffmpeg, batch, ctx.cancelled.clone(), |_| {
        done += 1;
        (ctx.reporter.emit)(done, total, &format!("Shot {}/{}", done, total));
    }).await?;

    for (staged, out) in rendered.iter().zip(&outputs) {
        move_into_place(staged, out)?;
    }
    Ok(outputs)
}
