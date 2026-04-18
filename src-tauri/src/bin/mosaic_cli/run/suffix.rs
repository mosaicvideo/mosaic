// src-tauri/src/bin/mosaic_cli/run/suffix.rs
// Validator for user-supplied filename suffixes. Rejects anything that
// could escape out_dir or produce surprising filenames.

/// Validate a suffix from CLI or config. Returns the suffix unchanged
/// on success, or an error string on rejection.
///
/// Empty strings are allowed — downstream `output_path::resolved()`
/// substitutes the built-in default when empty. That's why we only
/// reject *whitespace-only* strings, not empties.
pub fn validate(s: &str) -> Result<String, String> {
    if s.is_empty() {
        return Ok(String::new());
    }
    if s.trim().is_empty() {
        return Err(format!("suffix must not be whitespace-only: {s:?}"));
    }
    if s.contains('/') || s.contains('\\') {
        return Err(format!("suffix must not contain path separators: {s:?}"));
    }
    if s.split(['/', '\\']).any(|c| c == "..") || s == ".." || s.contains("..") {
        return Err(format!("suffix must not contain '..': {s:?}"));
    }
    Ok(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn empty_ok() { assert_eq!(validate("").unwrap(), ""); }
    #[test] fn normal_ok() { assert_eq!(validate("_sheet").unwrap(), "_sheet"); }
    #[test] fn slash_rejected() { assert!(validate("/").is_err()); }
    #[test] fn backslash_rejected() { assert!(validate("\\").is_err()); }
    #[test] fn dotdot_rejected() { assert!(validate("..").is_err()); }
    #[test] fn dotdot_embedded_rejected() { assert!(validate("x..y").is_err()); }
    #[test] fn traversal_rejected() { assert!(validate("../../escape").is_err()); }
    #[test] fn whitespace_only_rejected() { assert!(validate(" ").is_err()); assert!(validate("\t").is_err()); }
    #[test] fn leading_trailing_space_ok() { assert!(validate(" _ok").is_ok()); }
}
