use crate::ffmpeg::{locate_tools, run_capture};
use crate::video_info::{parse, VideoInfo};

#[tauri::command]
pub async fn probe_video(path: String) -> Result<VideoInfo, String> {
    let tools = locate_tools().map_err(|e| e.to_string())?;
    let args = [
        "-v", "error",
        "-show_entries", "format=filename,duration,size,bit_rate",
        "-show_entries", "stream=codec_name,codec_type,width,height,r_frame_rate,sample_rate,channels,bit_rate,profile",
        "-of", "json",
        &path,
    ];
    let json = run_capture(&tools.ffprobe, &args).await.map_err(|e| e.to_string())?;
    parse(&json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_tools() -> Result<(), String> {
    locate_tools().map(|_| ()).map_err(|e| e.to_string())
}
