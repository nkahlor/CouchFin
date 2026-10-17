//! Decoded images, and the codec that makes them.

use std::io::Cursor;
use std::sync::Arc;

use image::imageops::fast_blur;
use image::{ImageFormat, RgbaImage};

use super::ImageError;

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

/// Decodes a JPEG, PNG or WebP file.
pub(super) fn decode(bytes: &[u8]) -> Result<RgbaImage, ImageError> {
    image::load_from_memory(bytes)
        .map(image::DynamicImage::into_rgba8)
        .map_err(|error| decode_error(&error))
}

/// Blurs `image`, keeping its size.
pub(super) fn blur(image: &RgbaImage, sigma: f32) -> RgbaImage {
    fast_blur(image, sigma)
}

/// Encodes `image` as a PNG, the lossless format the blurred copy is stored in.
pub(super) fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, ImageError> {
    let mut out = Cursor::new(Vec::new());
    image
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|error| decode_error(&error))?;
    Ok(out.into_inner())
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
