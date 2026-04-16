use std::path::PathBuf;

pub(crate) const BASE_ARGS: &[&str] = &["-hide_banner", "-loglevel", "error", "-y"];

pub(crate) fn base_args() -> Vec<String> {
    BASE_ARGS.iter().map(|s| s.to_string()).collect()
}

/// Frame-accurate seeking for single-frame extraction (contact sheets, screenshots).
/// Uses dual `-ss` with `-copyts`: input-level `-ss` does fast keyframe seek,
/// `-copyts` preserves original stream timestamps, output-level `-ss` trims to
/// the exact frame. `-an` strips audio since no extraction pipeline produces audio.
pub fn seek_input_args(source: &std::path::Path, timestamp: f64) -> Vec<String> {
    vec![
        "-ss".into(), format!("{:.3}", timestamp),
        "-copyts".into(),
        "-i".into(), source.to_string_lossy().into_owned(),
        "-ss".into(), format!("{:.3}", timestamp),
        "-an".into(),
    ]
}

/// Fast seeking for multi-second clip extraction (preview reels, animated sheets).
/// Uses simple input-level `-ss` without `-copyts` — avoids reference-frame loss
/// on transport streams and other containers with sparse keyframes. A slightly
/// imprecise clip start (nearest keyframe) is acceptable for clips.
pub fn seek_input_args_clip(source: &std::path::Path, timestamp: f64) -> Vec<String> {
    vec![
        "-ss".into(), format!("{:.3}", timestamp),
        "-i".into(), source.to_string_lossy().into_owned(),
        "-an".into(),
    ]
}

/// Returns the zscale/tonemap filter chain for HDR→SDR conversion, or `None`
/// when the ffmpeg build lacks zscale or the stream has no confirmed HDR
/// transfer function.
///
/// Only tonemaps when `color_transfer` is an explicit HDR transfer (PQ or HLG).
/// Streams with `"unknown"` or absent transfer — common in DV Profile 5 where
/// the base layer is SDR-compatible — are left untouched. Note: DV Profile 5
/// files may still exhibit color distortion (green/purple tint) due to ffmpeg's
/// HEVC decoder applying DV RPU reshaping at decode time; this is a decoder
/// limitation that no post-decode filter can fix.
pub fn tonemap_filter(has_zscale: bool, color_transfer: Option<&str>) -> Option<String> {
    use crate::video_info::{PQ_TRANSFER, HLG_TRANSFER};
    if !has_zscale { return None; }

    let tin = match color_transfer {
        Some(PQ_TRANSFER) => PQ_TRANSFER,
        Some(HLG_TRANSFER) => HLG_TRANSFER,
        _ => return None,
    };
    Some(format!(
        "zscale=tin={tin}:min=bt2020nc:pin=bt2020:t=linear:npl=100,\
         format=gbrpf32le,zscale=p=bt709,\
         tonemap=hable:desat=0,\
         zscale=t=bt709:m=bt709:r=tv,format=yuv420p"
    ))
}

/// Encoder flags used by every intermediate H.264 clip we produce for later
/// filter-graph consumption (preview reel, animated contact sheet). Chosen
/// for cheap re-encode + filter-graph compatibility: `yuv420p` for universal
/// decoder support, `veryfast` + CRF 23 for speed over size.
pub(crate) fn h264_clip_encoder() -> [String; 8] {
    [
        "-c:v".into(), "libx264".into(),
        "-preset".into(), "veryfast".into(),
        "-crf".into(), "23".into(),
        "-pix_fmt".into(), "yuv420p".into(),
    ]
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    /// Whether the located ffmpeg has the `zscale` filter (libzimg).
    /// When false, HDR→SDR tonemapping is silently skipped.
    pub has_zscale: bool,
}

#[derive(Debug, thiserror::Error, serde::Serialize)]
pub enum ToolsError {
    #[error("ffmpeg not found on PATH")]
    FfmpegMissing,
    #[error("ffprobe not found on PATH")]
    FfprobeMissing,
}

pub fn locate_tools() -> Result<Tools, ToolsError> {
    // ffmpeg-full (brew keg-only) first on macOS — it has drawtext/libfreetype,
    // which the default brew ffmpeg bottle lacks.
    let priority_paths: &[&str] = if cfg!(target_os = "macos") {
        &["/opt/homebrew/opt/ffmpeg-full/bin", "/usr/local/opt/ffmpeg-full/bin"]
    } else {
        &[]
    };
    let extra_paths: &[&str] = if cfg!(target_os = "macos") {
        &["/opt/homebrew/bin", "/usr/local/bin"]
    } else {
        &[]
    };

    let find = |name: &str| -> Option<PathBuf> {
        for ep in priority_paths {
            let candidate = std::path::Path::new(ep).join(name);
            if candidate.is_file() { return Some(candidate); }
        }
        if let Ok(p) = which::which(name) { return Some(p); }
        for ep in extra_paths {
            let candidate = std::path::Path::new(ep).join(name);
            if candidate.is_file() { return Some(candidate); }
        }
        None
    };

    let ffmpeg = find("ffmpeg").ok_or(ToolsError::FfmpegMissing)?;
    let ffprobe = find("ffprobe").ok_or(ToolsError::FfprobeMissing)?;
    let has_zscale = has_filter(&ffmpeg, "zscale");
    Ok(Tools { ffmpeg, ffprobe, has_zscale })
}

/// Locate the `mediainfo` CLI binary. Returns `None` if not installed.
/// MediaInfo is optional — the app works without it, but the info modal
/// shows install instructions instead of output.
pub fn locate_mediainfo() -> Option<PathBuf> {
    which::which("mediainfo").ok()
}

/// Check whether the given ffmpeg binary supports a specific filter.
fn has_filter(ffmpeg: &std::path::Path, name: &str) -> bool {
    let mut cmd = std::process::Command::new(ffmpeg);
    cmd.args(["-filters", "-hide_banner"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| {
            l.split_whitespace().nth(1) == Some(name)
        }))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_args_returns_standard_ffmpeg_prelude() {
        let args = base_args();
        assert_eq!(args, vec!["-hide_banner", "-loglevel", "error", "-y"]);
    }

    #[test]
    fn returns_ok_when_tools_present() {
        // This test is a smoke test: assume dev machine has both.
        // If absent, the test is skipped with a message.
        if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() {
            eprintln!("skipping: ffmpeg/ffprobe not installed");
            return;
        }
        let t = locate_tools().unwrap();
        assert!(t.ffmpeg.exists());
        assert!(t.ffprobe.exists());
    }

    #[test]
    fn seek_input_args_produces_dual_ss_with_copyts() {
        let args = seek_input_args(std::path::Path::new("/v/movie.mkv"), 42.5);
        assert_eq!(args, vec![
            "-ss", "42.500",
            "-copyts",
            "-i", "/v/movie.mkv",
            "-ss", "42.500",
            "-an",
        ]);
    }

    #[test]
    fn seek_input_args_clip_has_no_copyts() {
        let args = seek_input_args_clip(std::path::Path::new("/v/movie.mkv"), 42.5);
        assert_eq!(args, vec![
            "-ss", "42.500",
            "-i", "/v/movie.mkv",
            "-an",
        ]);
    }

    #[test]
    fn tonemap_filter_returns_chain_for_pq() {
        let chain = tonemap_filter(true, Some("smpte2084")).unwrap();
        assert!(chain.contains("tonemap=hable"));
        assert!(chain.contains("tin=smpte2084"));
        assert!(chain.contains("min=bt2020nc"));
        assert!(chain.contains("pin=bt2020"));
    }

    #[test]
    fn tonemap_filter_returns_chain_for_hlg() {
        let chain = tonemap_filter(true, Some("arib-std-b67")).unwrap();
        assert!(chain.contains("tin=arib-std-b67"));
    }

    #[test]
    fn tonemap_filter_skips_when_transfer_missing() {
        assert!(tonemap_filter(true, None).is_none());
    }

    #[test]
    fn tonemap_filter_skips_when_transfer_unknown() {
        // DV Profile 5: color_transfer is "unknown" — base layer is SDR.
        assert!(tonemap_filter(true, Some("unknown")).is_none());
    }

    #[test]
    fn tonemap_filter_skips_sdr_transfer() {
        assert!(tonemap_filter(true, Some("bt709")).is_none());
    }

    #[test]
    fn tonemap_filter_returns_none_when_zscale_missing() {
        assert!(tonemap_filter(false, Some("smpte2084")).is_none());
    }

    #[test]
    fn locate_mediainfo_returns_some_when_installed() {
        // Smoke test: if mediainfo is on this machine, we find it.
        // If not installed, the test still passes (returns None).
        let result = locate_mediainfo();
        if which::which("mediainfo").is_ok() {
            assert!(result.is_some());
            assert!(result.unwrap().exists());
        } else {
            assert!(result.is_none());
        }
    }
}

use std::process::Stdio;
use tokio::process::Command;

/// Apply platform-specific flags to prevent a visible console window on Windows.
#[cfg(target_os = "windows")]
fn hide_window(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    cmd.creation_flags(CREATE_NO_WINDOW)
}

#[cfg(not(target_os = "windows"))]
fn hide_window(cmd: &mut Command) -> &mut Command {
    cmd
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("process exited with code {code}: {stderr}")]
    NonZero { code: i32, stderr: String },
    #[error("process killed")]
    Killed,
}

pub async fn run_capture(exe: &std::path::Path, args: &[&str]) -> Result<String, RunError> {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut cmd);
    let output = cmd.output().await?;
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(RunError::NonZero {
            code,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub async fn run_cancellable(
    exe: &std::path::Path,
    args: &[String],
    cancelled: Arc<AtomicBool>,
) -> Result<(), RunError> {
    if cancelled.load(Ordering::Relaxed) { return Err(RunError::Killed); }

    let mut cmd = Command::new(exe);
    cmd.args(args.iter().map(|s| s.as_str()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    hide_window(&mut cmd);
    let mut child = cmd.spawn()?;

    // Drain stderr concurrently so ffmpeg never blocks on a full pipe buffer
    // (~64 KiB on macOS/Linux). With `-loglevel error` stderr is usually tiny,
    // but an unexpected panic can flood it and deadlock `child.wait()`.
    let stderr_task = child.stderr.take().map(|mut err| {
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = Vec::new();
            let _ = err.read_to_end(&mut buf).await;
            buf
        })
    });

    tokio::select! {
        status = child.wait() => {
            let status = status?;
            let stderr_bytes = match stderr_task {
                Some(h) => h.await.unwrap_or_default(),
                None => Vec::new(),
            };
            if !status.success() {
                let stderr = String::from_utf8_lossy(&stderr_bytes).into_owned();
                return Err(RunError::NonZero { code: status.code().unwrap_or(-1), stderr });
            }
            Ok(())
        }
        _ = async {
            while !cancelled.load(Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        } => {
            let _ = child.kill().await;
            if let Some(h) = stderr_task { h.abort(); }
            Err(RunError::Killed)
        }
    }
}

/// Run multiple ffmpeg commands concurrently with bounded parallelism.
/// `on_done` fires in the caller's context with the original task index
/// each time a command completes. First error aborts all remaining tasks.
pub async fn run_batch_cancellable<F>(
    exe: &std::path::Path,
    batch: Vec<Vec<String>>,
    cancelled: Arc<AtomicBool>,
    mut on_done: F,
) -> Result<(), RunError>
where
    F: FnMut(usize),
{
    let concurrency = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);
    let sem = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut set = tokio::task::JoinSet::new();
    let exe = exe.to_path_buf();

    for (i, args) in batch.into_iter().enumerate() {
        let sem = sem.clone();
        let exe = exe.clone();
        let cancelled = cancelled.clone();
        set.spawn(async move {
            let _permit = sem.acquire().await.map_err(|_| RunError::Killed)?;
            run_cancellable(&exe, &args, cancelled).await?;
            Ok::<usize, RunError>(i)
        });
    }

    while let Some(result) = set.join_next().await {
        match result {
            Ok(Ok(i)) => on_done(i),
            Ok(Err(e)) => {
                set.abort_all();
                return Err(e);
            }
            Err(join_err) => {
                set.abort_all();
                return Err(RunError::Io(std::io::Error::other(join_err)));
            }
        }
    }
    Ok(())
}
