//! Rust mirror of `src/events.js`; the parity test below enforces both stay in sync.

pub const FILE_START:  &str = "job:file-start";
pub const STEP:        &str = "job:step";
pub const FILE_DONE:   &str = "job:file-done";
pub const FILE_FAILED: &str = "job:file-failed";
pub const FINISHED:    &str = "job:finished";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_mirror_matches() {
        let js = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../src/events.js")
        ).expect("src/events.js must exist");
        for (name, value) in [
            ("FILE_START",  FILE_START),
            ("STEP",        STEP),
            ("FILE_DONE",   FILE_DONE),
            ("FILE_FAILED", FILE_FAILED),
            ("FINISHED",    FINISHED),
        ] {
            assert!(js.contains(name),
                "src/events.js missing export name `{name}`");
            assert!(js.contains(&format!("'{value}'")),
                "src/events.js missing literal '{value}' for `{name}`");
        }
    }
}
