// src-tauri/src/bin/mosaic_cli/progress.rs
// Thin wrapper around indicatif's MultiProgress used by every
// generate subcommand. `--quiet` yields a no-op callback.

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

pub struct Reporter {
    _mp: MultiProgress,
    pub file: ProgressBar,
    pub step: ProgressBar,
}

impl Reporter {
    pub fn new(total_files: u64, quiet: bool) -> Self {
        let mp = MultiProgress::new();
        if quiet { mp.set_draw_target(indicatif::ProgressDrawTarget::hidden()); }
        let file = mp.add(ProgressBar::new(total_files));
        file.set_style(ProgressStyle::with_template("{prefix} {wide_msg}").unwrap());
        let step = mp.add(ProgressBar::new(1));
        step.set_style(ProgressStyle::with_template("  {bar:30} {pos}/{len} {msg}").unwrap());
        Self { _mp: mp, file, step }
    }

    pub fn start_file(&self, idx: u64, total: u64, path: &std::path::Path) {
        self.file.set_position(idx);
        self.file.set_prefix(format!("{idx}/{total}"));
        self.file.set_message(format!("{}", path.display()));
        self.step.reset();
    }

    /// Returns a closure suitable for `ProgressReporter::emit`.
    pub fn emit_fn(&self) -> impl Fn(u32, u32, &str) + Send + Sync + 'static {
        let bar = self.step.clone();
        move |pos: u32, total: u32, label: &str| {
            bar.set_length(total as u64);
            bar.set_position(pos as u64);
            bar.set_message(label.to_string());
        }
    }

    pub fn finish(&self) { self.step.finish_and_clear(); self.file.finish_and_clear(); }
}
