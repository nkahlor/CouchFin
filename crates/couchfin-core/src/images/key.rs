//! What identifies a cached image.

use couchfin_api::ImageRequest;

/// Item, kind, width and tag: everything that makes two images different.
/// Jellyfin changes an image's tag when the image changes, so a new tag is a
/// new key and the old files simply age out of the cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Key(String);

impl Key {
    pub(super) fn new(request: &ImageRequest) -> Self {
        // Item IDs and tags only contain `[A-Za-z0-9._~-]`, so this is a safe
        // file name, and `_` never occurs inside a part.
        Self(format!(
            "{}_{}_{}_{}",
            request.item,
            request.kind.as_str(),
            request.max_width,
            request.tag.as_ref().map_or("none", |tag| tag.as_str()),
        ))
    }

    /// The file holding the encoded original as the server sent it.
    pub(super) fn original_file(&self) -> String {
        format!("{}.orig", self.0)
    }

    /// The file holding the blurred copy as a PNG. The sigma is in the name,
    /// so changing it never serves a copy blurred differently.
    pub(super) fn blurred_file(&self, sigma: f32) -> String {
        format!("{}.blur{}.png", self.0, sigma.to_bits())
    }
}
