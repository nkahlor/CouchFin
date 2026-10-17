//! The on-disk cache: a flat directory of files, trimmed to a size cap by
//! evicting the least recently used. All calls block, so callers run them on
//! the blocking pool.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::SystemTime;

use crate::fs_util::{remove_if_exists, write_bytes_atomically};

/// Cached files are private to the user, like settings.
const FILE_MODE: u32 = 0o600;

/// What `write_bytes_atomically` appends to a file it is still writing.
const TEMPORARY_SUFFIX: &str = ".tmp";

#[derive(Debug)]
pub(super) struct Disk {
    dir: PathBuf,
    limit: u64,
}

impl Disk {
    pub(super) fn new(dir: PathBuf, limit: u64) -> Self {
        Self { dir, limit }
    }

    /// The file's contents, and marks it as just used. A file that can't be
    /// read counts as missing: the cache is only an optimisation.
    pub(super) fn read(&self, name: &str) -> Option<Vec<u8>> {
        let read = || -> io::Result<Vec<u8>> {
            let mut file = File::open(self.dir.join(name))?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            // The modification time is the "last used" time eviction sorts by.
            let _ = file.set_modified(SystemTime::now());
            Ok(bytes)
        };
        match read() {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                tracing::warn!(%error, name, "could not read a cached image");
                None
            }
        }
    }

    /// Stores the file, then evicts down to the cap, never the file just
    /// written. A failure is logged, not returned: not caching is fine.
    pub(super) fn write(&self, name: &str, bytes: &[u8]) {
        let result = write_bytes_atomically(&self.dir.join(name), bytes, FILE_MODE)
            .and_then(|()| self.evict(name));
        if let Err(error) = result {
            tracing::warn!(%error, name, "could not cache an image on disk");
        }
    }

    pub(super) fn remove(&self, name: &str) {
        if let Err(error) = remove_if_exists(&self.dir.join(name)) {
            tracing::warn!(%error, name, "could not delete a cached image");
        }
    }

    /// Deletes every cached file.
    pub(super) fn clear(&self) -> io::Result<()> {
        match fs::read_dir(&self.dir) {
            Ok(mut entries) => entries.try_for_each(|entry| remove_if_exists(&entry?.path())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn evict(&self, keep: &str) -> io::Result<()> {
        let mut files = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = entry.metadata()?;
            // A `.tmp` file is another write in progress.
            if meta.is_file() && !name.ends_with(TEMPORARY_SUFFIX) {
                files.push((meta.modified()?, name, meta.len()));
            }
        }
        let mut total: u64 = files.iter().map(|(_, _, size)| size).sum();
        files.sort();
        for (_, name, size) in files {
            if total <= self.limit {
                break;
            }
            if name != keep {
                remove_if_exists(&self.dir.join(&name))?;
                total -= size;
            }
        }
        Ok(())
    }
}
