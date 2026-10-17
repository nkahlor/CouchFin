//! How big things are on screen: which pictures are worth fetching, and at
//! what size.
//!
//! Request sizes do not follow the window. They are fixed per card shape and
//! snapped to a few scale factors by [`ImageSize`], so the same URLs repeat
//! and every cache (the server's, HTTP's, ours) hits.

use couchfin_core::{CardShape, ImageShape, ImageSize};
use slint::ComponentHandle;

use crate::generated::{HomeView, MainWindow, Theme};

/// The sizes the window shows, in logical pixels, and its scale factor.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Geometry {
    scale: f32,
    /// The window's width.
    width: f32,
    /// The window's height.
    height: f32,
    /// The poster height of a Home row (thumbnails follow from it).
    poster_height: f32,
    poster_aspect: f32,
    thumb_aspect: f32,
    thumb_ratio: f32,
    inset: f32,
    gap: f32,
    /// A row's height apart from its pictures: title, room for a grown card,
    /// the card's text and the gap to the next row.
    row_chrome: f32,
    thumb_subtitle: f32,
}

impl Geometry {
    pub(crate) fn of(window: &MainWindow) -> Self {
        let theme = window.global::<Theme>();
        let scale = window.window().scale_factor();
        let size = window.window().size().to_logical(scale);
        let shown = window.global::<HomeView>().get_poster_height();
        Self {
            scale,
            width: size.width,
            height: size.height,
            // Zero until the Home page has been laid out once.
            poster_height: if shown > 0.0 {
                shown
            } else {
                theme.get_poster_max_height()
            },
            poster_aspect: theme.get_poster_aspect(),
            thumb_aspect: theme.get_thumb_aspect(),
            thumb_ratio: theme.get_thumb_height_ratio(),
            inset: theme.get_carousel_inset(),
            gap: theme.get_card_gap(),
            row_chrome: theme.get_row_title_height()
                + theme.get_row_title_gap()
                + 2.0 * theme.get_card_focus_room()
                + theme.get_card_title_gap()
                + theme.get_card_title_height()
                + theme.get_row_gap(),
            thumb_subtitle: theme.get_card_subtitle_height(),
        }
    }

    fn card_logical_width(self, shape: CardShape) -> f32 {
        match shape {
            CardShape::Poster => self.poster_height * self.poster_aspect,
            CardShape::Thumb => self.poster_height * self.thumb_ratio * self.thumb_aspect,
        }
    }

    /// The box to request for a Home card's picture.
    pub(crate) fn card_size(self, shape: CardShape) -> ImageSize {
        ImageSize::of(
            match shape {
                CardShape::Poster => ImageShape::Poster,
                CardShape::Thumb => ImageShape::Thumb,
            },
            self.scale,
        )
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

    /// How many rows, counting from the focused one down through
    /// `shapes_from_focus`, start on screen. A row that only peeks in at the
    /// bottom counts.
    pub(crate) fn rows_on_screen(
        self,
        shapes_from_focus: impl Iterator<Item = CardShape>,
    ) -> usize {
        let mut top = 0.0;
        let mut count = 0;
        for shape in shapes_from_focus {
            if top >= self.height {
                break;
            }
            count += 1;
            top += self.row_chrome
                + match shape {
                    CardShape::Poster => self.poster_height,
                    CardShape::Thumb => self.poster_height * self.thumb_ratio + self.thumb_subtitle,
                };
        }
        count.max(1)
    }

    /// The box to request for a grid poster.
    pub(crate) fn grid_size(self) -> ImageSize {
        ImageSize::of(ImageShape::Poster, self.scale)
    }

    /// The box to request for a backdrop, which spans the window.
    pub(crate) fn backdrop_size(self) -> ImageSize {
        ImageSize::of(ImageShape::Backdrop, self.scale)
    }
}
