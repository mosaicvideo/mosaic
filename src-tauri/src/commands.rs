use crate::contact_sheet::{self, SheetOptions};
use crate::ffmpeg::{locate_tools, run_capture, RunError};
use crate::jobs::{JobState, ProgressReporter};
use crate::output_path::{contact_sheet_path, preview_reel_path};
use crate::preview_reel::{self, PreviewOptions};
use crate::screenshots::{self, ScreenshotsOptions};
use crate::video_info::{parse, VideoInfo};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub async fn probe_video(path: String) -> Result<VideoInfo, String> {
    let tools = locate_tools().map_err(|e| e.to_string())?;
    probe(&tools.ffprobe, &path).await
}

#[tauri::command]
pub fn check_tools() -> Result<(), String> {
    locate_tools().map(|_| ()).map_err(|e| e.to_string())
}

const VIDEO_EXTS: &[&str] = &[
    "mp4", "mkv", "mov", "avi", "webm", "wmv", "flv", "m4v", "mpg", "mpeg", "ts", "m2ts",
];

/// Cap folder-scan recursion depth so a pathological symlink loop can't hang the UI.
const MAX_SCAN_DEPTH: u32 = 16;

#[tauri::command]
pub fn scan_folder(path: String, recursive: bool) -> Result<Vec<String>, String> {
    let root = std::path::PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("not a directory: {}", path));
    }
    let mut out = Vec::new();
    walk(&root, recursive, 0, &mut out);
    out.sort();
    Ok(out.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
}

fn walk(dir: &std::path::Path, recursive: bool, depth: u32, out: &mut Vec<std::path::PathBuf>) {
    if depth > MAX_SCAN_DEPTH { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let p = entry.path();
        if ft.is_dir() {
            if recursive { walk(&p, recursive, depth + 1, out); }
        } else if ft.is_file() {
            let ext_ok = p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .map(|e| VIDEO_EXTS.contains(&e.as_str()))
                .unwrap_or(false);
            if ext_ok { out.push(p); }
        }
    }
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    use std::process::Command;
    let p = std::path::PathBuf::from(&path);

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", p.to_str().ok_or("non-utf8 path")?])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(format!("/select,{}", p.display()))
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        let target = if p.is_file() { p.parent().map(|x| x.to_path_buf()).unwrap_or(p.clone()) } else { p.clone() };
        Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(serde::Deserialize)]
pub struct QueueItem {
    pub id: String,
    pub path: String,
}

#[derive(serde::Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum OutputLocation {
    NextToSource,
    Custom { custom: String },
}

type PerFileFut<'a> =
    Pin<Box<dyn Future<Output = Result<Option<PathBuf>, RunError>> + Send + 'a>>;

async fn run_job_loop<F>(
    app: AppHandle,
    state: Arc<JobState>,
    ffprobe: PathBuf,
    items: Vec<QueueItem>,
    output: OutputLocation,
    per_file: F,
) -> Result<(), String>
where
    F: for<'a> Fn(&'a Path, &'a VideoInfo, &'a Path, Arc<AtomicBool>, &'a ProgressReporter<'a>) -> PerFileFut<'a>,
{
    state.begin()?;
    let total = items.len();
    let mut completed = 0u32;
    let mut failed = 0u32;
    let mut cancelled_count = 0u32;

    for (i, item) in items.iter().enumerate() {
        if state.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            cancelled_count = (total - i) as u32;
            break;
        }
        let _ = app.emit("job:file-start", serde_json::json!({
            "fileId": item.id, "index": i + 1, "total": total
        }));

        let source = PathBuf::from(&item.path);
        let out_dir = resolve_out_dir(&source, &output);
        let info = match probe(&ffprobe, &item.path).await {
            Ok(i) => i,
            Err(e) => {
                failed += 1;
                let _ = app.emit("job:file-failed", serde_json::json!({ "fileId": item.id, "error": e }));
                continue;
            }
        };

        let id = item.id.clone();
        let app2 = app.clone();
        let step_fn = move |step: u32, total_steps: u32, label: &str| {
            let _ = app2.emit("job:step", serde_json::json!({
                "fileId": id, "step": step, "totalSteps": total_steps, "label": label
            }));
        };
        let reporter = ProgressReporter { emit: &step_fn };

        match per_file(&source, &info, &out_dir, state.cancelled.clone(), &reporter).await {
            Ok(out) => {
                completed += 1;
                let _ = app.emit("job:file-done", serde_json::json!({
                    "fileId": item.id,
                    "outputPath": out.map(|p| p.to_string_lossy().into_owned()),
                }));
            }
            Err(RunError::Killed) => {
                cancelled_count += 1;
                break;
            }
            Err(e) => {
                failed += 1;
                let _ = app.emit("job:file-failed", serde_json::json!({
                    "fileId": item.id, "error": e.to_string()
                }));
            }
        }
    }

    state.end();
    let _ = app.emit("job:finished", serde_json::json!({
        "completed": completed, "failed": failed, "cancelled": cancelled_count
    }));
    Ok(())
}

#[tauri::command]
pub async fn generate_contact_sheets(
    app: AppHandle,
    state: State<'_, Arc<JobState>>,
    items: Vec<QueueItem>,
    opts: SheetOptions,
    output: OutputLocation,
) -> Result<(), String> {
    let tools = locate_tools().map_err(|e| e.to_string())?;
    let font = app.path().resolve("assets/fonts/DejaVuSans.ttf", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let state_inner = Arc::clone(state.inner());
    let ffmpeg = tools.ffmpeg.clone();

    run_job_loop(app.clone(), state_inner, tools.ffprobe, items, output,
        move |source, info, out_dir, cancelled, reporter| {
            let out = contact_sheet_path(source, out_dir, opts.format, &opts.suffix, &|p| p.exists());
            let opts = opts.clone();
            let ffmpeg = ffmpeg.clone();
            let font = font.clone();
            Box::pin(async move {
                contact_sheet::generate(source, info, &out, &opts, &ffmpeg, &font, cancelled, reporter).await?;
                Ok(Some(out))
            })
        }).await
}

#[tauri::command]
pub async fn generate_screenshots(
    app: AppHandle,
    state: State<'_, Arc<JobState>>,
    items: Vec<QueueItem>,
    opts: ScreenshotsOptions,
    output: OutputLocation,
) -> Result<(), String> {
    let tools = locate_tools().map_err(|e| e.to_string())?;
    let state_inner = Arc::clone(state.inner());
    let ffmpeg = tools.ffmpeg.clone();

    run_job_loop(app.clone(), state_inner, tools.ffprobe, items, output,
        move |source, info, out_dir, cancelled, reporter| {
            let opts = opts.clone();
            let ffmpeg = ffmpeg.clone();
            Box::pin(async move {
                let paths = screenshots::generate(source, info, out_dir, &opts, &ffmpeg, cancelled, reporter).await?;
                Ok(paths.into_iter().next())
            })
        }).await
}

#[tauri::command]
pub async fn generate_preview_reels(
    app: AppHandle,
    state: State<'_, Arc<JobState>>,
    items: Vec<QueueItem>,
    opts: PreviewOptions,
    output: OutputLocation,
) -> Result<(), String> {
    let tools = locate_tools().map_err(|e| e.to_string())?;
    let state_inner = Arc::clone(state.inner());
    let ffmpeg = tools.ffmpeg.clone();

    run_job_loop(app.clone(), state_inner, tools.ffprobe, items, output,
        move |source, info, out_dir, cancelled, reporter| {
            let out = preview_reel_path(source, out_dir, &opts.suffix, &|p| p.exists());
            let opts = opts.clone();
            let ffmpeg = ffmpeg.clone();
            Box::pin(async move {
                preview_reel::generate(source, info, &out, &opts, &ffmpeg, cancelled, reporter).await?;
                Ok(Some(out))
            })
        }).await
}

#[tauri::command]
pub fn cancel_job(state: State<'_, Arc<JobState>>) {
    state.cancel();
}

/// Canonical ffprobe pipeline: run ffprobe with our arg list, parse into `VideoInfo`.
pub(crate) async fn probe(ffprobe: &std::path::Path, path: &str) -> Result<VideoInfo, String> {
    let args = [
        "-v", "error",
        "-show_entries", "format=filename,duration,size,bit_rate",
        "-show_entries", "stream=codec_name,codec_type,width,height,r_frame_rate,sample_rate,channels,bit_rate,profile",
        "-of", "json",
        path,
    ];
    let json = run_capture(ffprobe, &args).await.map_err(|e| e.to_string())?;
    parse(&json).map_err(|e| e.to_string())
}

fn resolve_out_dir(source: &std::path::Path, output: &OutputLocation) -> PathBuf {
    match output {
        OutputLocation::NextToSource => source.parent().map(PathBuf::from).unwrap_or_default(),
        OutputLocation::Custom { custom } => PathBuf::from(custom),
    }
}
