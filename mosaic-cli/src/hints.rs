// CLI-only install hint text for tool-missing errors. The GUI has its
// own tools-missing UI; this is the equivalent for terminal users.

use mosaic_lib::ffmpeg::missing_tools;
use mosaic_lib::install_hint::{install_plan, Platform};

pub fn print_tool_install_hint() {
    let plan = install_plan(Platform::current(), &missing_tools());
    eprintln!();
    eprintln!("Mosaic needs ffmpeg, ffprobe, and mediainfo on your PATH.");
    if plan.steps.is_empty() {
        eprintln!("Install ffmpeg and mediainfo with your package manager.");
        return;
    }
    eprintln!("Install:");
    for step in &plan.steps {
        eprintln!("  {}", step.display());
    }
}
