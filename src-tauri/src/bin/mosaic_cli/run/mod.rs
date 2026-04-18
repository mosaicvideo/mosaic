// src-tauri/src/bin/mosaic_cli/run/mod.rs
// Per-subcommand implementations. Each submodule takes the parsed
// clap args + loaded config and returns a process exit code.

pub mod format;
pub mod inputs;
pub mod probe;
pub mod screenshots;
pub mod sheet;
pub mod reel;
pub mod animated_sheet;
pub mod suffix;
