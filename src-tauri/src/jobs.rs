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
        // Treat mutex poison as "the previous job panicked, nothing is
        // running now" — reset the flag so we don't block forever on a
        // stale `running == true` left over from the panicked holder.
        let mut running = match self.running.lock() {
            Ok(g) => g,
            Err(e) => {
                let mut g = e.into_inner();
                *g = false;
                g
            }
        };
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn begin_returns_err_when_already_running() {
        let state = JobState::default();
        state.begin().unwrap();
        assert!(state.begin().is_err());
    }

    #[test]
    fn begin_recovers_after_panic() {
        let state = Arc::new(JobState::default());
        let s2 = state.clone();
        let handle = thread::spawn(move || {
            s2.begin().unwrap();
            let _g = s2.running.lock().unwrap();
            panic!("simulated job panic");
        });
        assert!(handle.join().is_err());
        state.begin().expect("begin() should recover from mutex poison");
        state.end();
    }
}
