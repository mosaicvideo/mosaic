use std::path::PathBuf;

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
    let extra_paths: &[&str] = if cfg!(target_os = "macos") {
        &["/opt/homebrew/bin", "/usr/local/bin"]
    } else {
        &[]
    };

    let find = |name: &str| -> Option<PathBuf> {
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
