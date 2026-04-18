// src-tauri/src/bin/mosaic_cli/hints.rs
// CLI-only install hint text for tool-missing errors. The GUI has its
// own tools-missing UI; this is the equivalent for terminal users.

pub fn print_tool_install_hint() {
    eprintln!();
    eprintln!("Mosaic needs ffmpeg, ffprobe, and mediainfo on your PATH.");
    eprintln!("Install:");
    eprintln!("  macOS:   brew install ffmpeg-full mediainfo");
    eprintln!("  Windows: winget install ffmpeg MediaArea.MediaInfo.CLI");
    eprintln!("  Linux:   apt install ffmpeg mediainfo   (or your distro equivalent)");
}
