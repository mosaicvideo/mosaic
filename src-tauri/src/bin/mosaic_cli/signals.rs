// src-tauri/src/bin/mosaic_cli/signals.rs
// Installs a tokio Ctrl-C handler that sets the shared cancel flag.
// Second Ctrl-C exits the process immediately without draining ffmpeg.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn install(cancelled: Arc<AtomicBool>) {
    tokio::spawn(async move {
        // First signal: request graceful cancel.
        let _ = tokio::signal::ctrl_c().await;
        cancelled.store(true, Ordering::Relaxed);
        eprintln!("cancelling…");
        // Second signal: hard exit.
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("forced exit");
        std::process::exit(130);
    });
}
