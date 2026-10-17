//! Small file helpers shared by the settings and token files.

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;

/// Reads `path`, or `None` if it doesn't exist.
pub(crate) fn read_if_exists(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Replaces `path` with `contents` atomically: a reader sees the old file or
/// the new one, never half of one. The new file has permissions `mode` from
/// the moment it is created, so a secret is never readable by others, even
/// briefly. A missing parent directory is created readable by the user only.
pub(crate) fn write_atomically(path: &Path, contents: &str, mode: u32) -> io::Result<()> {
    write_bytes_atomically(path, contents.as_bytes(), mode)
}

/// [`write_atomically`] for contents that aren't text, such as images.
pub(crate) fn write_bytes_atomically(path: &Path, contents: &[u8], mode: u32) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = Path::new(&temporary);
    // A leftover from a crash may have other permissions; `create_new` below
    // must make a fresh file so `mode` applies.
    match fs::remove_file(temporary) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    fs::rename(temporary, path)
}

/// Removes `path`; a file that is already gone is fine.
pub(crate) fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
