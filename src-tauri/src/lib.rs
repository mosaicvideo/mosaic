pub mod video_info;
mod drawtext;
mod layout;
pub mod output_path;
mod header;
pub mod ffmpeg;
pub mod contact_sheet;
pub mod screenshots;
mod jobs;
mod commands;

pub fn ffmpeg_test_hook_locate() -> Result<ffmpeg::Tools, ffmpeg::ToolsError> {
    ffmpeg::locate_tools()
}

pub async fn ffmpeg_test_hook_probe(exe: &std::path::Path, path: &str) -> Result<String, String> {
    let args = [
        "-v", "error",
        "-show_entries", "format=filename,duration,size,bit_rate",
        "-show_entries", "stream=codec_name,codec_type,width,height,r_frame_rate,sample_rate,channels,bit_rate,profile",
        "-of", "json", path,
    ];
    ffmpeg::run_capture(exe, &args).await.map_err(|e| e.to_string())
}

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
