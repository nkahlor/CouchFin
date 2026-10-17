//! The blocking half of a load: reading and writing cache files, decoding.
//! Everything here runs on tokio's blocking pool.

use std::sync::Arc;

use super::disk::Disk;
use super::error::ImageError;
use super::key::Key;
use super::pixels::{Pixels, decode, into_pixels};

/// What to produce for one key, and where its files live.
#[derive(Debug, Clone)]
pub(super) struct Job {
    disk: Arc<Disk>,
    original: String,
}

impl Job {
    pub(super) fn new(disk: Arc<Disk>, key: &Key) -> Self {
        Self {
            disk,
            original: key.original_file(),
        }
    }

    /// Answers from the disk cache, or `None` if the network is needed. A file
    /// that won't decode is deleted, so the next step fetches it afresh.
    pub(super) fn cached(&self) -> Option<Pixels> {
        let bytes = self.disk.read(&self.original)?;
        match decode(&bytes) {
            Ok(image) => Some(into_pixels(image)),
            Err(error) => {
                tracing::warn!(%error, "deleting a corrupt cached image");
                self.disk.remove(&self.original);
                None
            }
        }
    }

    /// Decodes what the server sent and caches it. Bytes that don't decode
    /// are an error and are not cached.
    pub(super) fn downloaded(&self, bytes: &[u8]) -> Result<Pixels, ImageError> {
        let image = decode(bytes)?;
        self.disk.write(&self.original, bytes);
        Ok(into_pixels(image))
    }
}
