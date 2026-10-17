//! The sizes images are requested at, and BlurHash placeholders.
//!
//! A server resizes each distinct size once and then caches it, and so does
//! the HTTP cache and ours. So the size must not follow the window, a row's
//! live card size or a rounding of it: jellyfin-web asks for one fixed box per
//! card shape, times the device pixel ratio, and so does Couchfin.

use couchfin_api::BlurHash;

use super::Pixels;

/// What a picture is for. It decides the box requested from the server;
/// library tiles share the thumbnail's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageShape {
    /// A 2:3 poster, on Home and in the library grid.
    Poster,
    /// A 16:9 thumbnail (also a library tile).
    Thumb,
    /// A wide background.
    Backdrop,
}

/// The pixel box to request an image at.
///
/// # Examples
///
/// ```
/// use couchfin_core::{ImageShape, ImageSize};
///
/// let size = ImageSize::of(ImageShape::Poster, 1.5);
/// assert_eq!((size.width, size.height), (420, 630));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageSize {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// The scale factors images are requested at. A screen's factor rounds up to
/// the next of these, so a handful of URLs cover every display: 1.0 is a
/// 1080p TV at 100 %, 1.5 and 2.0 are 1440p, 4K and Steam Deck docks.
const SCALES: [f32; 3] = [1.0, 1.5, 2.0];

impl ImageSize {
    /// The box for `shape` on a display with this scale factor.
    #[must_use]
    pub fn of(shape: ImageShape, scale: f32) -> Self {
        let (width, height) = match shape {
            ImageShape::Poster => (280.0, 420.0),
            ImageShape::Thumb => (448.0, 252.0),
            ImageShape::Backdrop => (1280.0, 720.0),
        };
        let scale = if scale.is_nan() { SCALES[0] } else { scale };
        let scale = SCALES
            .into_iter()
            .find(|step| scale <= *step)
            .unwrap_or(SCALES[SCALES.len() - 1]);
        // Small positive whole-ish numbers.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let px = |logical: f32| (logical * scale).round() as u32;
        Self {
            width: px(width),
            height: px(height),
        }
    }
}

/// Decodes `hash` to a tiny RGBA picture with the proportions of `shape`, to
/// show scaled up while the real image loads. It is a few hundred pixels, so
/// this takes microseconds, but call it off the UI thread anyway.
///
/// `None` if the decoder rejects the hash.
///
/// # Examples
///
/// ```
/// use couchfin_core::{ImageShape, placeholder};
///
/// let hash = "LEHV6nWB2yk8pyo0adR*.7kCMdnj".parse()?;
/// let tiny = placeholder(&hash, ImageShape::Poster).ok_or("decodes")?;
/// assert_eq!((tiny.width(), tiny.height()), (24, 36));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn placeholder(hash: &BlurHash, shape: ImageShape) -> Option<Pixels> {
    // jellyfin-web decodes to about 20x20; keeping the card's proportions
    // makes the stretch to the card uniform.
    let (width, height) = match shape {
        ImageShape::Poster => (24, 36),
        ImageShape::Thumb | ImageShape::Backdrop => (32, 18),
    };
    let rgba = blurhash::decode(hash.as_str(), width, height, 1.0).ok()?;
    Pixels::from_rgba(width, height, rgba)
}
