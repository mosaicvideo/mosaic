use std::collections::HashSet;
use std::path::PathBuf;

pub(crate) const BASE_ARGS: &[&str] = &["-hide_banner", "-loglevel", "error", "-y"];

pub(crate) fn base_args() -> Vec<String> {
    BASE_ARGS.iter().map(|s| s.to_string()).collect()
}

/// Join a list of optional filter components into a single `-vf` value,
/// dropping `None`s. Empty input yields an empty string; callers decide
/// whether to emit `-vf` at all based on the result.
pub(crate) fn vf_chain(parts: &[Option<&str>]) -> String {
    parts.iter().copied().flatten().collect::<Vec<_>>().join(",")
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

/// IPT-PQ-C2 → BT.709 color correction matrix for Dolby Vision Profile 5.
/// Derived from libplacebo's IPT coefficients (Ebner & Fairchild 1998 inverse
/// matrix, BT.2020 LMS→RGB Hunt-Pointer-Estevez transform, 2% crosstalk).
/// Correct hues, slightly washed out (PQ gamma not inverted — acceptable for
/// thumbnails). Works on any ffmpeg build, no zscale/GPU required.
const DV_P5_COLOR_MATRIX: &str = "colorchannelmixer=\
    rr=0.2938:rg=0.3557:rb=0.3504:\
    gr=0.3508:gg=0.7312:gb=-0.0821:\
    br=-0.1610:bg=1.0337:bb=0.1275";

/// Returns the video filter for HDR/DV color correction, or `None` for SDR.
///
/// - **DV Profile 5**: colorchannelmixer (IPT-PQ-C2 → BT.709, no zscale needed)
/// - **PQ/HLG with zscale**: full zscale tonemap chain
/// - **Everything else**: None
pub fn tonemap_filter(has_zscale: bool, color_transfer: Option<&str>, dv_profile: Option<u8>) -> Option<String> {
    // DV Profile 5 uses IPT-PQ-C2 color space that ffmpeg misinterprets as
    // YCbCr, producing green/purple output. Apply the correction matrix first
    // since DV P5 has color_transfer=None which would fall through to None.
    if dv_profile == Some(5) {
        return Some(DV_P5_COLOR_MATRIX.into());
    }

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
    pub mediainfo: PathBuf,
}

impl Tools {
    /// Lists the located ffmpeg's filters and encoders. Spawns ffmpeg twice,
    /// so call it once per batch, not in per-file hot paths.
    pub fn detect_caps(&self) -> FfmpegCaps {
        FfmpegCaps {
            filters: list_names(&self.ffmpeg, "-filters"),
            encoders: list_names(&self.ffmpeg, "-encoders"),
        }
    }
}

/// An ffmpeg feature some output needs. Not every ffmpeg build has them all —
/// Homebrew's default `ffmpeg` bottle ships without drawtext and libwebp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    Drawtext,
    Libx264,
    Libwebp,
    LibvpxVp9,
}

impl Need {
    fn present_in(self, caps: &FfmpegCaps) -> bool {
        match self {
            Self::Drawtext => caps.filters.contains("drawtext"),
            Self::Libx264 => caps.encoders.contains("libx264"),
            Self::Libwebp => caps.encoders.contains("libwebp"),
            Self::LibvpxVp9 => caps.encoders.contains("libvpx-vp9"),
        }
    }

    pub fn describe(self) -> &'static str {
        match self {
            Self::Drawtext => "drawtext filter (timestamps and header text)",
            Self::Libx264 => "libx264 encoder (animated clips)",
            Self::Libwebp => "libwebp encoder (WebP previews and animated sheets)",
            Self::LibvpxVp9 => "libvpx-vp9 encoder (WebM previews)",
        }
    }
}

/// Every feature the shipping defaults use. The GUI warns at startup when any is missing.
pub const DEFAULT_NEEDS: &[Need] = &[Need::Drawtext, Need::Libx264, Need::Libwebp];

#[derive(Debug, Default, Clone)]
pub struct FfmpegCaps {
    filters: HashSet<String>,
    encoders: HashSet<String>,
}

impl FfmpegCaps {
    /// Whether ffmpeg has `zscale` (libzimg). Without it, HDR→SDR tonemapping is skipped.
    pub fn has_zscale(&self) -> bool {
        self.filters.contains("zscale")
    }

    pub fn missing(&self, needs: &[Need]) -> Vec<Need> {
        needs.iter().copied().filter(|n| !n.present_in(self)).collect()
    }

    /// `Err` naming every missing feature and how to get them, or `Ok` when all are present.
    pub fn require(&self, ffmpeg: &std::path::Path, needs: &[Need]) -> Result<(), String> {
        let missing = self.missing(needs);
        if missing.is_empty() { return Ok(()); }
        Err(missing_message(ffmpeg, &missing))
    }
}

pub fn missing_message(ffmpeg: &std::path::Path, missing: &[Need]) -> String {
    let list = missing.iter().map(|n| n.describe()).collect::<Vec<_>>().join(", ");
    let fix = if cfg!(target_os = "macos") {
        "Install a full build with `brew install ffmpeg-full`; Mosaic picks it up automatically."
    } else {
        "Install an ffmpeg build that includes them."
    };
    format!("{} is missing: {}. {}", ffmpeg.display(), list, fix)
}

/// Names from `ffmpeg -filters` / `ffmpeg -encoders` output: each entry line
/// is a flags column followed by the name. Header lines never have a second
/// token that collides with a real filter or encoder name.
pub(crate) fn parse_names(stdout: &str) -> HashSet<String> {
    stdout.lines()
        .filter_map(|l| l.split_whitespace().nth(1))
        .map(str::to_owned)
        .collect()
}

#[derive(Debug, thiserror::Error, serde::Serialize)]
pub enum ToolsError {
    #[error("ffmpeg not found on PATH")]
    Ffmpeg,
    #[error("ffprobe not found on PATH")]
    Ffprobe,
    #[error("mediainfo not found on PATH")]
    MediaInfo,
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

    let ffmpeg = find("ffmpeg").ok_or(ToolsError::Ffmpeg)?;
    let ffprobe = find("ffprobe").ok_or(ToolsError::Ffprobe)?;
    let mediainfo = find("mediainfo").ok_or(ToolsError::MediaInfo)?;
    Ok(Tools { ffmpeg, ffprobe, mediainfo })
}

fn list_names(ffmpeg: &std::path::Path, flag: &str) -> HashSet<String> {
    let mut cmd = std::process::Command::new(ffmpeg);
    cmd.args([flag, "-hide_banner"])
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
        .map(|o| parse_names(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
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
        let chain = tonemap_filter(true, Some("smpte2084"), None).unwrap();
        assert!(chain.contains("tonemap=hable"));
        assert!(chain.contains("tin=smpte2084"));
        assert!(chain.contains("min=bt2020nc"));
        assert!(chain.contains("pin=bt2020"));
    }

    #[test]
    fn tonemap_filter_returns_chain_for_hlg() {
        let chain = tonemap_filter(true, Some("arib-std-b67"), None).unwrap();
        assert!(chain.contains("tin=arib-std-b67"));
    }

    #[test]
    fn tonemap_filter_skips_when_transfer_missing() {
        assert!(tonemap_filter(true, None, None).is_none());
    }

    #[test]
    fn tonemap_filter_skips_when_transfer_unknown() {
        assert!(tonemap_filter(true, Some("unknown"), None).is_none());
    }

    #[test]
    fn tonemap_filter_skips_sdr_transfer() {
        assert!(tonemap_filter(true, Some("bt709"), None).is_none());
    }

    #[test]
    fn tonemap_filter_returns_none_when_zscale_missing() {
        assert!(tonemap_filter(false, Some("smpte2084"), None).is_none());
    }

    #[test]
    fn tonemap_filter_returns_ccm_for_dv_p5() {
        let chain = tonemap_filter(false, None, Some(5)).unwrap();
        assert!(chain.contains("colorchannelmixer"));
        assert!(chain.contains("rr=0.2938"));
        assert!(!chain.contains("tonemap"));
    }

    #[test]
    fn tonemap_filter_dv_p5_ignores_zscale() {
        // DV P5 correction works without zscale
        let a = tonemap_filter(false, None, Some(5)).unwrap();
        let b = tonemap_filter(true, None, Some(5)).unwrap();
        assert_eq!(a, b);
    }

    const FILTERS_SAMPLE: &str = "Filters:\n  T.. = Timeline support\n  ------\n .S xstack            N->V       Stack video inputs into custom layout.\n TSC zscale            V->V       Apply resizing, colorspace and bit depth conversion.\n";
    const ENCODERS_SAMPLE: &str = "Encoders:\n V..... = Video\n ------\n V....D libx264              libx264 H.264\n V....D libvpx-vp9           libvpx VP9 (codec vp9)\n";

    fn sample_caps() -> FfmpegCaps {
        FfmpegCaps { filters: parse_names(FILTERS_SAMPLE), encoders: parse_names(ENCODERS_SAMPLE) }
    }

    #[test]
    fn parse_names_reads_second_column() {
        let names = parse_names(FILTERS_SAMPLE);
        assert!(names.contains("xstack"));
        assert!(names.contains("zscale"));
        assert!(!names.contains("drawtext"));
    }

    #[test]
    fn missing_reports_only_absent_features() {
        let caps = sample_caps();
        assert!(caps.has_zscale());
        assert_eq!(caps.missing(DEFAULT_NEEDS), vec![Need::Drawtext, Need::Libwebp]);
        assert!(caps.missing(&[Need::Libx264, Need::LibvpxVp9]).is_empty());
    }

    #[test]
    fn require_names_every_missing_feature() {
        let err = sample_caps()
            .require(std::path::Path::new("/usr/bin/ffmpeg"), DEFAULT_NEEDS)
            .unwrap_err();
        assert!(err.starts_with("/usr/bin/ffmpeg is missing: "));
        assert!(err.contains("drawtext"));
        assert!(err.contains("libwebp"));
        assert!(!err.contains("libx264"));
        assert!(sample_caps().require(std::path::Path::new("ffmpeg"), &[]).is_ok());
    }

    #[test]
    fn locate_tools_populates_mediainfo_when_installed() {
        // Smoke test: on a dev machine with all three tools, `Tools.mediainfo`
        // should resolve to an executable path.
        if which::which("ffmpeg").is_err() || which::which("ffprobe").is_err() || which::which("mediainfo").is_err() {
            eprintln!("skipping: ffmpeg/ffprobe/mediainfo not all installed");
            return;
        }
        let t = locate_tools().unwrap();
        assert!(t.mediainfo.exists());
    }
}

use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);

/// Enable argv-print-before-spawn globally (used by `mosaic-cli --verbose`).
#[allow(dead_code)]
pub fn set_verbose(on: bool) {
    VERBOSE.store(on, Ordering::Relaxed);
}

fn maybe_log_argv(bin: &std::path::Path, args: &[impl AsRef<std::ffi::OsStr>]) {
    if !VERBOSE.load(Ordering::Relaxed) { return; }
    let mut line = bin.display().to_string();
    for a in args {
        line.push(' ');
        line.push_str(&a.as_ref().to_string_lossy());
    }
    eprintln!("+ {line}");
}

use std::process::Stdio;
use tokio::process::Command;

/// Apply platform-specific flags to prevent a visible console window on Windows.
#[cfg(target_os = "windows")]
fn hide_window(cmd: &mut Command) -> &mut Command {
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
    #[error("{0}")]
    Invalid(String),
}

pub async fn run_capture(exe: &std::path::Path, args: &[&str]) -> Result<String, RunError> {
    maybe_log_argv(exe, args);
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

pub async fn run_cancellable(
    exe: &std::path::Path,
    args: &[String],
    cancelled: Arc<AtomicBool>,
) -> Result<(), RunError> {
    if cancelled.load(Ordering::Relaxed) { return Err(RunError::Killed); }
    maybe_log_argv(exe, args);
    let mut cmd = Command::new(exe);
    cmd.args(args.iter().map(|s| s.as_str()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
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
