#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod video_info;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod video_info;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod defaults;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod defaults;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod input_scan;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod input_scan;

mod drawtext;
mod layout;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod mediainfo;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod mediainfo;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod output_path;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod output_path;

mod header;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod ffmpeg;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod ffmpeg;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod contact_sheet;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod contact_sheet;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod screenshots;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod screenshots;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod preview_reel;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod preview_reel;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod animated_sheet;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod animated_sheet;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub mod jobs;
#[cfg(not(any(test, feature = "test-api", feature = "cli")))]
mod jobs;

mod commands;
pub mod events;

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub fn ffmpeg_test_hook_locate() -> Result<ffmpeg::Tools, ffmpeg::ToolsError> {
    ffmpeg::locate_tools()
}

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub async fn ffmpeg_test_hook_probe(tools: &ffmpeg::Tools, path: &str) -> Result<video_info::VideoInfo, String> {
    commands::probe(tools, path).await
}

#[cfg(any(test, feature = "test-api", feature = "cli"))]
pub fn video_info_test_hook_parse(json: &str) -> Result<video_info::VideoInfo, video_info::ProbeParseError> {
    video_info::parse(json)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build());
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }
    builder
        .setup(|app| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let version = app.package_info().version.to_string();
                let _ = window.set_title(&format!("Mosaic {version}"));
            }
            Ok(())
        })
        .manage(std::sync::Arc::new(crate::jobs::JobState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::probe_video,
            commands::check_tools,
            commands::run_mediainfo,
            commands::get_video_exts,
            commands::reveal_in_finder,
            commands::scan_folder,
            commands::generate_contact_sheets,
            commands::generate_screenshots,
            commands::generate_preview_reels,
            commands::generate_animated_sheets,
            commands::cancel_job,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
