use crate::animated_sheet::{self, AnimatedSheetOptions};
use crate::contact_sheet::{self, SheetOptions};
use crate::events;
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
    probe(&tools, &path).await
}

#[tauri::command]
pub fn check_tools() -> Result<(), String> {
    locate_tools().map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_mediainfo(path: String) -> Result<String, String> {
    let tools = locate_tools().map_err(|e| e.to_string())?;
    run_capture(&tools.mediainfo, &[&path]).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_video_exts() -> Vec<String> {
    crate::input_scan::VIDEO_EXTS.iter().map(|s| s.to_string()).collect()
}

#[tauri::command]
pub fn scan_folder(path: String, recursive: bool) -> Result<Vec<String>, String> {
    let root = std::path::PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("not a directory: {}", path));
    }
    let found = crate::input_scan::scan(&root, recursive)?;
    Ok(found.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    use std::process::Command;
    let p = std::path::PathBuf::from(&path);

    // `-R` / `/select,` select the path in its PARENT (Finder/Explorer
    // window opens one level up with the target highlighted). That's right
    // for files but wrong for directories — passing a dir would open the
    // grandparent. Drop the select flag when the path is a directory.
    #[cfg(target_os = "macos")]
    {
        // Pass the OsStr directly so non-UTF8 paths (rare on APFS but legal)
        // aren't rejected by a `to_str()` guard.
        let mut cmd = Command::new("open");
        if !p.is_dir() { cmd.arg("-R"); }
        cmd.arg(p.as_os_str()).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = Command::new("explorer");
        if p.is_dir() {
            cmd.arg(p.as_os_str());
        } else {
            // explorer parses `/select,<path>` by splitting on the first
            // comma. A filename containing `,` (e.g. `foo,bar.mkv`) would
            // otherwise select the wrong sibling. Quote the path so
            // explorer treats it as one token. Filenames can't contain `"`
            // on Windows, so there's nothing to escape inside.
            //
            // `raw_arg` preserves our literal quotes — regular `arg()` would
            // re-escape them and defeat the fix.
            let mut token = std::ffi::OsString::from("/select,\"");
            token.push(p.as_os_str());
            token.push("\"");
            cmd.raw_arg(&token);
        }
        cmd.creation_flags(CREATE_NO_WINDOW).spawn().map_err(|e| e.to_string())?;
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
    /// Frontend-cached probe result. When present, backend skips the probe()
    /// round-trip for this file, avoiding the N+1 pattern across passes.
    #[serde(default)]
    pub info: Option<VideoInfo>,
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
    // Resolve zscale support once per batch rather than per file — spawning
    // `ffmpeg -filters` is relatively cheap but not free, and the answer is
    // invariant across a batch.
    let has_zscale = tools.detect_has_zscale();
    let total = items.len();
    let mut completed = 0u32;
    let mut failed = 0u32;
    let mut cancelled_count = 0u32;

    for (i, item) in items.iter().enumerate() {
        if state.cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            cancelled_count = (total - i) as u32;
            break;
        }
        let _ = app.emit(events::FILE_START, serde_json::json!({
            "fileId": item.id, "index": i + 1, "total": total
        }));

        let source = PathBuf::from(&item.path);
        let out_dir = resolve_out_dir(&source, &output);
        let info = match item.info.clone() {
            Some(cached) => cached,
            None => match probe(&tools, &item.path).await {
                Ok(i) => i,
                Err(e) => {
                    failed += 1;
                    let _ = app.emit(events::FILE_FAILED, serde_json::json!({ "fileId": item.id, "error": e }));
                    continue;
                }
            },
        };

        let id = item.id.clone();
        let app2 = app.clone();
        let step_fn = move |step: u32, total_steps: u32, label: &str| {
            let _ = app2.emit(events::STEP, serde_json::json!({
                "fileId": id, "step": step, "totalSteps": total_steps, "label": label
            }));
        };
        let reporter = ProgressReporter { emit: &step_fn };
        let ctx = PipelineContext { ffmpeg: &tools.ffmpeg, cancelled: state.cancelled.clone(), reporter: &reporter, has_zscale };

        match per_file(&source, &info, &out_dir, &ctx).await {
            Ok(out) => {
                completed += 1;
                let _ = app.emit(events::FILE_DONE, serde_json::json!({
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
                let _ = app.emit(events::FILE_FAILED, serde_json::json!({
                    "fileId": item.id, "error": e.to_string()
                }));
            }
        }
    }

    state.end();
    let _ = app.emit(events::FINISHED, serde_json::json!({
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

/// Canonical probe pipeline: ffprobe for structured metadata, MediaInfo for
/// enrichment (HDR format, commercial audio name, bit depth, etc.). Both
/// tools are required at startup; a non-zero exit or malformed MediaInfo
/// output still degrades gracefully (`enrichment = None`) so a single bad
/// file doesn't block the whole queue.
///
/// The two probes are independent I/O against the same file, so they run
/// concurrently — roughly halves per-file probe latency on drag-and-drop.
pub(crate) async fn probe(tools: &Tools, path: &str) -> Result<VideoInfo, String> {
    let ffprobe_args = [
        "-v", "error",
        "-show_entries", "format=filename,duration,size,bit_rate",
        "-show_entries", "stream=codec_name,codec_type,width,height,r_frame_rate,sample_rate,channels,bit_rate,profile,color_transfer,sample_aspect_ratio",
        "-show_entries", "stream_side_data=side_data_type,dv_profile,rotation",
        "-of", "json",
        path,
    ];
    let (ffprobe_res, enrichment) = tokio::join!(
        run_capture(&tools.ffprobe, &ffprobe_args),
        probe_mediainfo(&tools.mediainfo, path),
    );
    let json = ffprobe_res.map_err(|e| e.to_string())?;
    let mut info = parse(&json).map_err(|e| e.to_string())?;
    info.enrichment = enrichment;
    Ok(info)
}

/// Best-effort MediaInfo enrichment: `None` if the binary errors or emits
/// output we can't parse.
async fn probe_mediainfo(bin: &std::path::Path, path: &str) -> Option<crate::mediainfo::Enrichment> {
    let json = run_capture(bin, &["--Output=JSON", path]).await.ok()?;
    crate::mediainfo::parse_enrichment(&json)
}

fn resolve_out_dir(source: &std::path::Path, output: &OutputLocation) -> PathBuf {
    match output {
        OutputLocation::NextToSource => source.parent().map(PathBuf::from).unwrap_or_default(),
        OutputLocation::Custom { custom } => PathBuf::from(custom),
    }
}
