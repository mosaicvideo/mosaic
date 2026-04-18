// Mirror of src-tauri/src/events.rs. A Rust unit test (`events::tests::
// js_mirror_matches`) asserts every export + literal here stays in sync.

export const FILE_START  = 'job:file-start';
export const STEP        = 'job:step';
export const FILE_DONE   = 'job:file-done';
export const FILE_FAILED = 'job:file-failed';
export const FINISHED    = 'job:finished';
