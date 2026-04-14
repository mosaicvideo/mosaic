mod video_info;
mod drawtext;
mod layout;
mod output_path;
mod header;
mod ffmpeg;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            commands::probe_video,
            commands::check_tools,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
