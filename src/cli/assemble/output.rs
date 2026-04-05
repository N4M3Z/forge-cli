use commands::error::{Error, ErrorKind};
use std::fs;
use std::path::Path;

/// Write assembled content to the build directory, creating parent dirs.
/// Always ensures a trailing newline (POSIX text file convention).
pub fn write_file(output_path: &Path, content: &str) -> Result<(), Error> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }
    let mut bytes = content.as_bytes().to_vec();
    if bytes.last() != Some(&b'\n') {
        bytes.push(b'\n');
    }
    fs::write(output_path, bytes).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {e}", output_path.display()),
        )
    })
}
