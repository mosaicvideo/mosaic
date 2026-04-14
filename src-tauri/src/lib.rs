#[cfg(any(test, feature = "test-api"))]
pub mod video_info;
#[cfg(not(any(test, feature = "test-api")))]
mod video_info;

mod drawtext;
mod layout;

#[cfg(any(test, feature = "test-api"))]
pub mod output_path;
#[cfg(not(any(test, feature = "test-api")))]
mod output_path;

mod header;

#[cfg(any(test, feature = "test-api"))]
pub mod ffmpeg;
#[cfg(not(any(test, feature = "test-api")))]
mod ffmpeg;

#[cfg(any(test, feature = "test-api"))]
pub mod contact_sheet;
#[cfg(not(any(test, feature = "test-api")))]
mod contact_sheet;

#[cfg(any(test, feature = "test-api"))]
pub mod screenshots;
#[cfg(not(any(test, feature = "test-api")))]
mod screenshots;

#[cfg(any(test, feature = "test-api"))]
pub mod jobs;
#[cfg(not(any(test, feature = "test-api")))]
mod jobs;

mod commands;

#[cfg(any(test, feature = "test-api"))]
pub fn ffmpeg_test_hook_locate() -> Result<ffmpeg::Tools, ffmpeg::ToolsError> {
    ffmpeg::locate_tools()
}

#[cfg(any(test, feature = "test-api"))]
pub async fn ffmpeg_test_hook_probe(exe: &std::path::Path, path: &str) -> Result<video_info::VideoInfo, String> {
    commands::probe(exe, path).await
}

#[cfg(any(test, feature = "test-api"))]
pub fn video_info_test_hook_parse(json: &str) -> Result<video_info::VideoInfo, video_info::ProbeParseError> {
    video_info::parse(json)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(std::sync::Arc::new(crate::jobs::JobState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::probe_video,
            commands::check_tools,
            commands::generate_contact_sheets,
            commands::generate_screenshots,
            commands::cancel_job,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
