//! How big things are on screen, to ask the image cache for pictures that
//! are sharp without being wasteful.

use couchfin_core::CardShape;
use slint::ComponentHandle;

use crate::generated::{HomeView, MainWindow, Theme};

/// Request widths are rounded up to a multiple of this, so a window that
/// resizes a little still finds its pictures in the cache.
const WIDTH_STEP: f32 = 32.0;

/// The sizes the window shows, in logical pixels, and its scale factor.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Geometry {
    scale: f32,
    /// The window's width.
    width: f32,
    /// The poster height of a Home row (thumbnails follow from it).
    poster_height: f32,
    poster_aspect: f32,
    thumb_aspect: f32,
    thumb_ratio: f32,
    grid_poster_height: f32,
    inset: f32,
    gap: f32,
}

impl Geometry {
    pub(crate) fn of(window: &MainWindow) -> Self {
        let theme = window.global::<Theme>();
        let scale = window.window().scale_factor();
        let width = window.window().size().to_logical(scale).width;
        let shown = window.global::<HomeView>().get_poster_height();
        Self {
            scale,
            width,
            // Zero until the Home page has been laid out once.
            poster_height: if shown > 0.0 {
                shown
            } else {
                theme.get_poster_max_height()
            },
            poster_aspect: theme.get_poster_aspect(),
            thumb_aspect: theme.get_thumb_aspect(),
            thumb_ratio: theme.get_thumb_height_ratio(),
            grid_poster_height: theme.get_grid_poster_height(),
            inset: theme.get_carousel_inset(),
            gap: theme.get_card_gap(),
        }
    }

    /// Pixels to ask for to fill `logical` points.
    fn request(self, logical: f32) -> u32 {
        let pixels = (logical * self.scale / WIDTH_STEP).ceil() * WIDTH_STEP;
        // Positive and far below u32::MAX.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let pixels = pixels.max(WIDTH_STEP) as u32;
        pixels
    }

    fn card_logical_width(self, shape: CardShape) -> f32 {
        match shape {
            CardShape::Poster => self.poster_height * self.poster_aspect,
            CardShape::Thumb => self.poster_height * self.thumb_ratio * self.thumb_aspect,
        }
    }

    /// The width to request for a Home card's picture.
    pub(crate) fn card_width(self, shape: CardShape) -> u32 {
        self.request(self.card_logical_width(shape))
    }

    /// How many cards of a row fit on screen from the current one.
    pub(crate) fn cards_across(self, shape: CardShape) -> usize {
        let slot = self.card_logical_width(shape) + self.gap;
        let room = (self.width - self.inset).max(slot);
        // A small positive count.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let across = (room / slot).ceil() as usize;
        across
    }

    /// The width to request for a grid poster.
    pub(crate) fn grid_width(self) -> u32 {
        self.request(self.grid_poster_height * self.poster_aspect)
    }

    /// The width to request for a backdrop, which spans the window.
    pub(crate) fn backdrop_width(self) -> u32 {
        self.request(self.width)
    }
}
