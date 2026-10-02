use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct JobState {
    pub cancelled: Arc<AtomicBool>,
    running: AtomicBool,
}

/// Held for the lifetime of a job. Dropping it — on success, error, or a
/// panic unwinding the command's task — marks the job finished, so a crashed
/// job can never leave `running` stuck at true.
pub struct JobGuard<'a> {
    state: &'a JobState,
}

impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        // Cancel is cleared at job end rather than job start: the frontend runs
        // one job per output type, and a Cancel clicked between two of them
        // must still stop the next one instead of being wiped by its start.
        self.state.cancelled.store(false, Ordering::SeqCst);
        self.state.running.store(false, Ordering::SeqCst);
    }
}

impl JobState {
    pub fn begin(&self) -> Result<JobGuard<'_>, String> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err("a job is already running".into());
        }
        Ok(JobGuard { state: self })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

/// Moves a finished output from the job's temp dir to its final path. Every
/// pipeline renders into a temp dir first so a cancelled or failed run never
/// leaves a truncated file at the user-visible path.
pub(crate) fn move_into_place(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // rename fails across filesystems (temp dir vs. an external drive).
    std::fs::rename(src, dst).or_else(|_| std::fs::copy(src, dst).map(|_| ()))
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

    #[test]
    fn begin_returns_err_when_already_running() {
        let state = JobState::default();
        let _guard = state.begin().unwrap();
        assert!(state.begin().is_err());
    }

    #[test]
    fn dropping_guard_allows_next_job() {
        let state = JobState::default();
        drop(state.begin().unwrap());
        assert!(state.begin().is_ok());
    }

    #[tokio::test]
    async fn panicking_job_releases_running_flag() {
        let state = Arc::new(JobState::default());
        let s2 = state.clone();
        let res = tokio::spawn(async move {
            let _guard = s2.begin().unwrap();
            panic!("simulated pipeline panic");
        }).await;
        assert!(res.is_err());
        assert!(state.begin().is_ok(), "a panicked job must not block the next one");
    }

    #[test]
    fn cancel_between_jobs_survives_until_next_job_ends() {
        let state = JobState::default();
        drop(state.begin().unwrap());
        state.cancel();
        let guard = state.begin().unwrap();
        assert!(state.cancelled.load(Ordering::SeqCst), "next job must see the pending cancel");
        drop(guard);
        assert!(!state.cancelled.load(Ordering::SeqCst));
    }
}
