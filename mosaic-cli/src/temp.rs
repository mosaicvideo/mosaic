//! CLI-only temporary storage policy; shared pipelines keep their default constructors.

use std::io::{self, Write};
use std::path::Path;

pub fn configure(path: &Path) -> io::Result<()> {
    // Resolve relative paths once so later consumers all use the same directory.
    let path = path.canonicalize()?;
    if !path.is_dir() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a directory"));
    }

    // Exercise both operations we need instead of trusting permission bits (ACLs
    // and read-only mounts can disagree with those). Remove the probes immediately.
    let probe = tempfile::TempDir::new_in(&path)?;
    let mut file = tempfile::NamedTempFile::new_in(probe.path())?;
    file.write_all(b"mosaic temporary storage check")?;
    file.flush()?;
    file.close()?;
    probe.close()?;

    // This thread-safe tempfile override covers both media TempDirs and the
    // embedded font's NamedTempFile, without mutating process environment or GUI policy.
    tempfile::env::override_temp_dir(&path).map_err(|previous| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("temporary directory already configured as {}", previous.display()),
        )
    })
}
