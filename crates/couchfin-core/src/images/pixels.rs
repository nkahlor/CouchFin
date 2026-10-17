//! Decoded images, and the codec that makes them.

use std::sync::Arc;

use image::RgbaImage;

use super::ImageError;
use super::complete::incomplete;

/// A decoded image: straight (non-premultiplied) RGBA, 8 bits per channel,
/// row by row from the top left.
///
/// Clones share the pixel bytes, so handing one to the UI is cheap. The UI
/// copies them into a `slint::SharedPixelBuffer::<Rgba8Pixel>`; this crate
/// does not depend on Slint.
///
/// # Examples
///
/// ```
/// use couchfin_core::Pixels;
///
/// let pixels = Pixels::from_rgba(1, 2, vec![0; 8]).ok_or("wrong length")?;
/// assert_eq!((pixels.width(), pixels.height()), (1, 2));
/// assert_eq!(pixels.rgba().len(), 8);
/// assert!(Pixels::from_rgba(1, 2, vec![0; 7]).is_none());
/// # Ok::<(), &str>(())
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Pixels {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
}

impl Pixels {
    /// Wraps `rgba`, which must be exactly `width * height * 4` bytes.
    #[must_use]
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        let expected = u64::from(width) * u64::from(height) * 4;
        (expected == rgba.len() as u64).then(|| Self {
            width,
            height,
            rgba: rgba.into(),
        })
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The pixel bytes, four per pixel.
    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

// Not derived: a dump of the pixel bytes would flood a log.
impl std::fmt::Debug for Pixels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pixels({}x{})", self.width, self.height)
    }
}

fn decode_error(error: &image::ImageError) -> ImageError {
    ImageError::Decode {
        detail: error.to_string(),
    }
}

/// Decodes a whole JPEG, PNG or WebP file that is at most `max_width` wide,
/// when there is a limit (a box to fill may be exceeded in one direction).
///
/// Anything else is an error, never an image with holes in it: a partial
/// file, or a width the server was not asked for (the server scales down,
/// never up, so a wider image is not the one requested).
pub(super) fn decode(bytes: &[u8], max_width: Option<u32>) -> Result<RgbaImage, ImageError> {
    if let Some(reason) = incomplete(bytes) {
        return Err(ImageError::Decode {
            detail: reason.to_owned(),
        });
    }
    let image = image::load_from_memory(bytes)
        .map(image::DynamicImage::into_rgba8)
        .map_err(|error| decode_error(&error))?;
    if let Some(max_width) = max_width
        && image.width() > max_width
    {
        return Err(ImageError::Decode {
            detail: format!(
                "the image is {} px wide, but at most {max_width} were requested",
                image.width()
            ),
        });
    }
    Ok(image)
}

/// Moves a decoded image into [`Pixels`].
pub(super) fn into_pixels(image: RgbaImage) -> Pixels {
    let (width, height) = image.dimensions();
    Pixels {
        width,
        height,
        rgba: image.into_raw().into(),
    }
}
