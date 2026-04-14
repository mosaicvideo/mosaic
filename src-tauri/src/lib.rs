mod video_info;
mod drawtext;
mod layout;
mod output_path;
mod header;
mod ffmpeg;
mod contact_sheet;
mod screenshots;
mod jobs;
mod commands;

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
