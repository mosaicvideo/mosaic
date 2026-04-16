use crate::animated_sheet::{self, AnimatedSheetOptions};
use crate::contact_sheet::{self, SheetOptions};
use crate::ffmpeg::{locate_tools, run_capture, RunError, Tools};
use crate::jobs::{JobState, PipelineContext, ProgressReporter};
use crate::output_path::{animated_sheet_path, contact_sheet_path, preview_reel_path};
use crate::preview_reel::{self, PreviewOptions};
use crate::screenshots::{self, ScreenshotsOptions};
use crate::video_info::{parse, VideoInfo};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
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

#[tauri::command]
pub fn check_mediainfo() -> bool {
    crate::ffmpeg::locate_mediainfo().is_some()
}

#[tauri::command]
pub async fn run_mediainfo(path: String) -> Result<String, String> {
    let bin = crate::ffmpeg::locate_mediainfo().ok_or_else(|| {
        "MediaInfo not found.\n\nInstall it:\n  macOS:   brew install mediainfo\n  Windows: winget install MediaArea.MediaInfo.CLI\n  Linux:   apt install mediainfo".to_string()
    })?;
    run_capture(&bin, &[&path]).await.map_err(|e| e.to_string())
}

const VIDEO_EXTS: &[&str] = &[
    // Common containers
    "mp4", "mkv", "mov", "avi", "webm", "wmv", "flv", "m4v", "mpg", "mpeg",
    "ts", "m2ts", "mts", "vob", "iso", "ogv", "ogm", "qt", "asf",
    // Mobile / MP4 family
    "3gp", "3g2", "f4v", "mj2",
    // Legacy / regional
    "rm", "rmvb", "divx", "swf", "nsv",
    // Broadcast / professional
    "mxf", "gxf", "r3d",
    // Camcorder / capture / recording
    "dv", "dif", "wtv", "nuv", "pva",
    // Other containers
    "nut", "vro", "m1v", "m2v", "mk3d", "fli", "flc", "ivf", "y4m",
];

/// Cap folder-scan recursion depth so a pathological symlink loop can't hang the UI.
const MAX_SCAN_DEPTH: u32 = 16;

#[tauri::command]
pub fn get_video_exts() -> Vec<String> {
    VIDEO_EXTS.iter().map(|s| s.to_string()).collect()
}

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
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        Command::new("explorer")
            .arg(format!("/select,{}", p.display()))
            .creation_flags(CREATE_NO_WINDOW)
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
    tools: Tools,
    items: Vec<QueueItem>,
    output: OutputLocation,
    per_file: F,
) -> Result<(), String>
where
    F: for<'a> Fn(&'a Path, &'a VideoInfo, &'a Path, &'a PipelineContext<'a>) -> PerFileFut<'a>,
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
        let info = match probe(&tools.ffprobe, &item.path).await {
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
        let ctx = PipelineContext { ffmpeg: &tools.ffmpeg, cancelled: state.cancelled.clone(), reporter: &reporter, has_zscale: tools.has_zscale };

        match per_file(&source, &info, &out_dir, &ctx).await {
            Ok(out) => {
                completed += 1;
                let _ = app.emit("job:file-done", serde_json::json!({
                    "fileId": item.id,
                    "index": i + 1, "total": total,
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

    run_job_loop(app.clone(), state_inner, tools, items, output,
        move |source, info, out_dir, ctx| {
            let out = contact_sheet_path(source, out_dir, opts.format, &opts.suffix, &|p| p.exists());
            let opts = opts.clone();
            let font = font.clone();
            Box::pin(async move {
                contact_sheet::generate(source, info, &out, &opts, &font, ctx).await?;
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

    run_job_loop(app.clone(), state_inner, tools, items, output,
        move |source, info, out_dir, ctx| {
            let opts = opts.clone();
            Box::pin(async move {
                let paths = screenshots::generate(source, info, out_dir, &opts, ctx).await?;
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

    run_job_loop(app.clone(), state_inner, tools, items, output,
        move |source, info, out_dir, ctx| {
            let out = preview_reel_path(source, out_dir, opts.format, &opts.suffix, &|p| p.exists());
            let opts = opts.clone();
            Box::pin(async move {
                preview_reel::generate(source, info, &out, &opts, ctx).await?;
                Ok(Some(out))
            })
        }).await
}

#[tauri::command]
pub async fn generate_animated_sheets(
    app: AppHandle,
    state: State<'_, Arc<JobState>>,
    items: Vec<QueueItem>,
    opts: AnimatedSheetOptions,
    output: OutputLocation,
) -> Result<(), String> {
    let tools = locate_tools().map_err(|e| e.to_string())?;
    let font = app.path().resolve("assets/fonts/DejaVuSans.ttf", tauri::path::BaseDirectory::Resource)
        .map_err(|e| e.to_string())?;
    let state_inner = Arc::clone(state.inner());

    run_job_loop(app.clone(), state_inner, tools, items, output,
        move |source, info, out_dir, ctx| {
            let out = animated_sheet_path(source, out_dir, &opts.suffix, &|p| p.exists());
            let opts = opts.clone();
            let font = font.clone();
            Box::pin(async move {
                animated_sheet::generate(source, info, &out, &opts, &font, ctx).await?;
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
        "-show_entries", "stream=codec_name,codec_type,width,height,r_frame_rate,sample_rate,channels,bit_rate,profile,color_transfer",
        "-show_entries", "stream_side_data=side_data_type,dv_profile",
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
