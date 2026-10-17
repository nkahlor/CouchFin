//! Typed image requests and the bytes they return.

use crate::ids::{ImageTag, ItemId};

/// Which of an item's images to fetch.
///
/// # Examples
///
/// ```
/// use couchfin_api::ImageKind;
///
/// assert_eq!(ImageKind::Backdrop.as_str(), "Backdrop");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageKind {
    /// The poster (2:3), or an episode's still.
    Primary,
    /// The wide background image shown behind a detail page.
    Backdrop,
    /// The 16:9 thumbnail used on Continue Watching and Next Up.
    Thumb,
}

impl ImageKind {
    /// Jellyfin's name for the image type, as used in the URL path.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "Primary",
            Self::Backdrop => "Backdrop",
            Self::Thumb => "Thumb",
        }
    }
}

/// Which image to fetch and how large.
///
/// Pass the item's tag when you have it: the server then lets the image be
/// cached, and a changed image has a new tag, so a cache keyed by
/// `(item, kind, max_width, tag)` never serves a stale one.
///
/// # Examples
///
/// ```
/// use couchfin_api::{ImageKind, ImageRequest};
///
/// let request = ImageRequest::new("abc".parse()?, ImageKind::Thumb, 400)
///     .with_tag("0123".parse()?);
/// assert_eq!(request.max_width, 400);
/// assert!(request.tag.is_some());
/// # Ok::<(), couchfin_api::InvalidValueError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageRequest {
    /// The item the image belongs to.
    pub item: ItemId,
    /// Which image.
    pub kind: ImageKind,
    /// The server scales the image down to at most this many pixels wide.
    pub max_width: u32,
    /// The tag from the item's `image_tags`, for cache-busting.
    pub tag: Option<ImageTag>,
}

impl ImageRequest {
    /// A request without a tag.
    #[must_use]
    pub fn new(item: ItemId, kind: ImageKind, max_width: u32) -> Self {
        Self {
            item,
            kind,
            max_width,
            tag: None,
        }
    }

    /// Adds the image's tag.
    #[must_use]
    pub fn with_tag(mut self, tag: ImageTag) -> Self {
        self.tag = Some(tag);
        self
    }
}

/// A downloaded image, still encoded.
///
/// # Examples
///
/// ```
/// use couchfin_api::Image;
///
/// let image = Image { bytes: vec![0xFF, 0xD8], content_type: "image/jpeg".to_owned() };
/// assert_eq!(image.content_type, "image/jpeg");
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    /// The encoded image file (JPEG, PNG or WebP).
    pub bytes: Vec<u8>,
    /// The `Content-Type` the server sent, such as `image/jpeg`.
    pub content_type: String,
}

// Not derived: a Debug dump of several hundred kilobytes would flood a log.
impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("bytes", &format_args!("{} bytes", self.bytes.len()))
            .field("content_type", &self.content_type)
            .finish()
    }
}
