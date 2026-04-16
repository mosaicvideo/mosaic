use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;

#[derive(Default)]
pub struct JobState {
    pub cancelled: Arc<AtomicBool>,
    pub running: Mutex<bool>,
}

impl JobState {
    pub fn begin(&self) -> Result<(), String> {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        if *running { return Err("a job is already running".into()); }
        self.cancelled.store(false, std::sync::atomic::Ordering::Relaxed);
        *running = true;
        Ok(())
    }
    pub fn end(&self) {
        let mut running = self.running.lock().unwrap_or_else(|e| e.into_inner());
        *running = false;
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Emits per-file progress events (step, total_steps, label) back to the UI.
pub struct ProgressReporter<'a> {
    pub emit: &'a (dyn Fn(u32, u32, &str) + Send + Sync),
}

/// Shared execution environment for all pipeline generate() functions.
pub struct PipelineContext<'a> {
    pub ffmpeg: &'a Path,
    pub cancelled: Arc<AtomicBool>,
    pub reporter: &'a ProgressReporter<'a>,
    /// Whether the ffmpeg binary supports zscale (libzimg) for HDR→SDR tonemapping.
    pub has_zscale: bool,
}
