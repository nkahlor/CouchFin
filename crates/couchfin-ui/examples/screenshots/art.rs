//! Generated artwork: posters, thumbnails and backdrops in varied colours,
//! patterns and shapes, so layouts meet pictures that fill their slot, and
//! pictures that do not. Nothing here is copyrighted artwork.

#![allow(clippy::many_single_char_names)]

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

/// What is drawn over the colour gradient, so neighbouring pictures differ.
#[derive(Clone, Copy)]
enum Pattern {
    Stripes,
    Rings,
    Checks,
    Diagonal,
}

fn channel(value: f32) -> u8 {
    // The clamp makes the cast exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = value.clamp(0.0, 255.0) as u8;
    byte
}

/// A colour from a hue in 0..6, at the given brightness.
fn hue(h: f32, light: f32) -> [f32; 3] {
    let h = h.rem_euclid(6.0);
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h {
        h if h < 1.0 => (1.0, x, 0.0),
        h if h < 2.0 => (x, 1.0, 0.0),
        h if h < 3.0 => (0.0, 1.0, x),
        h if h < 4.0 => (0.0, x, 1.0),
        h if h < 5.0 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    [r * light, g * light, b * light]
}

/// A picture of the given pixel size. `seed` picks hue and pattern.
pub fn picture(seed: u32, width: u32, height: u32) -> Image {
    let pattern = match seed % 4 {
        0 => Pattern::Stripes,
        1 => Pattern::Rings,
        2 => Pattern::Checks,
        _ => Pattern::Diagonal,
    };
    #[allow(clippy::cast_precision_loss)]
    let base = (seed * 7 % 12) as f32 / 2.0;
    let (top, bottom) = (hue(base, 230.0), hue(base + 1.5, 90.0));
    let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
    let size = width.min(height);
    let cell = |divisor: u32| (size / divisor).max(1);
    for (n, pixel) in pixels.make_mut_slice().iter_mut().enumerate() {
        let n = u32::try_from(n).unwrap_or(0);
        let (x, y) = (n % width, n / width);
        #[allow(clippy::cast_precision_loss)]
        let t = y as f32 / height as f32;
        let lit = match pattern {
            Pattern::Stripes => (x / cell(8)) % 2 == 0,
            Pattern::Checks => (x / cell(6) + y / cell(6)) % 2 == 0,
            Pattern::Diagonal => ((x + y) / cell(7)) % 2 == 0,
            Pattern::Rings => {
                let (dx, dy) = (x.abs_diff(width / 2), y.abs_diff(height / 2));
                #[allow(clippy::cast_precision_loss)]
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let ring = d as u32 / cell(8);
                ring.is_multiple_of(2)
            }
        };
        let boost = if lit { 1.0 } else { 0.72 };
        *pixel = Rgba8Pixel::new(
            channel((top[0] * (1.0 - t) + bottom[0] * t) * boost),
            channel((top[1] * (1.0 - t) + bottom[1] * t) * boost),
            channel((top[2] * (1.0 - t) + bottom[2] * t) * boost),
            255,
        );
    }
    Image::from_rgba8(pixels)
}

/// A 2:3 poster.
pub fn poster(seed: u32) -> Image {
    picture(seed, 300, 450)
}

/// A square picture, as some posters really are, for a 2:3 slot.
pub fn square(seed: u32) -> Image {
    picture(seed, 400, 400)
}

/// A very wide picture for a 2:3 slot.
pub fn banner(seed: u32) -> Image {
    picture(seed, 900, 300)
}

/// A 16:9 thumbnail.
pub fn thumb(seed: u32) -> Image {
    picture(seed, 480, 270)
}

/// A 16:9 backdrop.
pub fn backdrop(seed: u32) -> Image {
    picture(seed, 1280, 720)
}
