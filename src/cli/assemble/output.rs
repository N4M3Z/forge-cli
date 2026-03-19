use commands::error::{Error, ErrorKind};
use std::fs;
use std::path::Path;

/// Write assembled content to the build directory, creating parent dirs.
pub fn write_file(output_path: &Path, content: &str) -> Result<(), Error> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }
    fs::write(output_path, content).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {e}", output_path.display()),
        )
    })
}
