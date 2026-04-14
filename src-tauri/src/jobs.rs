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
