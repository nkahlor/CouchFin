//! The blocking half of a load: reading and writing cache files, decoding
//! and blurring. Everything here runs on tokio's blocking pool.

use std::sync::Arc;

use image::RgbaImage;

use super::cache::Variant;
use super::disk::Disk;
use super::error::ImageError;
use super::key::Key;
use super::pixels::{Pixels, blur, decode, encode_png, into_pixels};

/// What to produce for one key, and where its files live.
#[derive(Debug, Clone)]
pub(super) struct Job {
    disk: Arc<Disk>,
    original: String,
    blurred: String,
    variant: Variant,
    sigma: f32,
}

impl Job {
    pub(super) fn new(disk: Arc<Disk>, key: &Key, variant: Variant, sigma: f32) -> Self {
        Self {
            disk,
            original: key.original_file(),
            blurred: key.blurred_file(sigma),
            variant,
            sigma,
        }
    }

    /// Answers from the disk cache, or `None` if the network is needed. A file
    /// that won't decode is deleted, so the next step fetches it afresh.
    pub(super) fn cached(&self) -> Option<Pixels> {
        if self.variant == Variant::Blurred
            && let Some(pixels) = self.cached_blur()
        {
            return Some(pixels);
        }
        let bytes = self.disk.read(&self.original)?;
        match decode(&bytes) {
            Ok(image) => Some(self.render(image)),
            Err(error) => {
                tracing::warn!(%error, "deleting a corrupt cached image");
                self.disk.remove(&self.original);
                None
            }
        }
    }

    fn cached_blur(&self) -> Option<Pixels> {
        let bytes = self.disk.read(&self.blurred)?;
        match decode(&bytes) {
            Ok(image) => Some(into_pixels(image)),
            Err(error) => {
                tracing::warn!(%error, "deleting a corrupt blurred image");
                self.disk.remove(&self.blurred);
                None
            }
        }
    }

    /// Decodes what the server sent and caches it. Bytes that don't decode
    /// are an error and are not cached.
    pub(super) fn downloaded(&self, bytes: &[u8]) -> Result<Pixels, ImageError> {
        let image = decode(bytes)?;
        self.disk.write(&self.original, bytes);
        Ok(self.render(image))
    }

    /// The pixels for the wanted variant; a blurred copy is cached too.
    fn render(&self, image: RgbaImage) -> Pixels {
        match self.variant {
            Variant::Sharp => into_pixels(image),
            Variant::Blurred => {
                let blurred = blur(&image, self.sigma);
                match encode_png(&blurred) {
                    Ok(png) => self.disk.write(&self.blurred, &png),
                    Err(error) => tracing::warn!(%error, "could not store a blurred image"),
                }
                into_pixels(blurred)
            }
        }
    }
}
