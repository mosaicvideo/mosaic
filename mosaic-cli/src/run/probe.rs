// src-tauri/src/bin/mosaic_cli/run/probe.rs
use crate::cli::ProbeArgs;
use mosaic_lib::{ffmpeg_test_hook_locate, ffmpeg_test_hook_probe};
use serde_json::json;

pub async fn run(args: ProbeArgs) -> i32 {
    let tools = match ffmpeg_test_hook_locate() {
        Ok(t) => t,
        Err(e) => { eprintln!("{e}"); crate::hints::print_tool_install_hint(); return 2; }
    };
    if !args.input.exists() {
        eprintln!("path does not exist: {}", args.input.display());
        return 2;
    }
    let path_str = args.input.to_string_lossy().into_owned();
    let info = match ffmpeg_test_hook_probe(&tools, &path_str).await {
        Ok(i) => i,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    if args.mediainfo {
        let path_str = args.input.to_string_lossy();
        // Second invocation: fetch human-readable mediainfo for the wrapped output.
        // The first (inside `commands::probe`) runs --Output=JSON for enrichment.
        let mi = match mosaic_lib::ffmpeg::run_capture(tools.mediainfo.as_path(), &[path_str.as_ref()]).await {
            Ok(s) => s,
            Err(e) => { eprintln!("mediainfo: {e}"); return 1; }
        };
        let out = json!({ "ffprobe": info, "mediainfo": mi });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("{}", serde_json::to_string_pretty(&info).unwrap());
    }
    0
}
