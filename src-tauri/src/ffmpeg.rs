use std::path::PathBuf;

pub(crate) const BASE_ARGS: &[&str] = &["-hide_banner", "-loglevel", "error", "-y"];

pub(crate) fn base_args() -> Vec<String> {
    BASE_ARGS.iter().map(|s| s.to_string()).collect()
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
    Ok(Tools { ffmpeg, ffprobe })
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
}

use std::process::Stdio;
use tokio::process::Command;

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
    let output = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;
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

    let mut child = Command::new(exe)
        .args(args.iter().map(|s| s.as_str()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

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
