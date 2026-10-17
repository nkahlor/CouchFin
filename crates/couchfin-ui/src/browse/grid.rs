//! The library grid's lazy model, and which pictures the visible part of
//! the grid needs.

use std::cell::RefCell;
use std::collections::HashMap;

use couchfin_core::{Card, GridState, Variant};
use slint::{Image, Model, ModelNotify, ModelTracker};

use super::geometry::Geometry;
use super::loader::{Slot, Wish};
use crate::generated::CardData;

/// How many items past the visible ones to fetch pictures for, as a share of
/// the visible count: half a screen each side.
const AHEAD_DIVISOR: usize = 2;
/// Pictures kept past that, as a multiple of the margin, so scrolling back
/// a little finds them again but a long scroll doesn't pile up memory.
const KEEP_MARGINS: usize = 3;

/// `LibraryView.items`: one `CardData` per place in the library, made on
/// demand from core's grid. Slint reads only the places it draws.
pub(crate) struct GridModel {
    items: RefCell<Vec<Option<Card>>>,
    images: RefCell<HashMap<usize, Image>>,
    notify: ModelNotify,
}

impl GridModel {
    pub(crate) fn new() -> Self {
        Self {
            items: RefCell::new(Vec::new()),
            images: RefCell::new(HashMap::new()),
            notify: ModelNotify::default(),
        }
    }

    /// Takes core's slots, telling Slint only about the places that changed.
    pub(crate) fn update(&self, grid: &GridState) {
        let resized = self.items.borrow().len() != grid.items.len();
        let changed: Vec<usize> = {
            let old = self.items.borrow();
            grid.items
                .iter()
                .enumerate()
                .filter(|(place, item)| old.get(*place) != Some(item))
                .map(|(place, _)| place)
                .collect()
        };
        self.items.borrow_mut().clone_from(&grid.items);
        if resized {
            self.notify.reset();
        } else {
            for place in changed {
                self.notify.row_changed(place);
            }
        }
    }

    /// The picture for a place, or the placeholder for `None`.
    pub(crate) fn set_image(&self, place: usize, image: Option<Image>) {
        let changed = match image {
            Some(image) => {
                self.images.borrow_mut().insert(place, image);
                true
            }
            None => self.images.borrow_mut().remove(&place).is_some(),
        };
        if changed && place < self.items.borrow().len() {
            self.notify.row_changed(place);
        }
    }

    /// The pictures worth fetching when `first` to `last` (exclusive) are on
    /// screen, visible places first and then the margin, ahead of the
    /// scroll before behind it.
    pub(crate) fn wanted(&self, visible: (usize, usize), geometry: &Geometry) -> Vec<(Slot, Wish)> {
        let items = self.items.borrow();
        let (first, last) = visible;
        let margin = margin(visible);
        let order = (first..last)
            .chain(last..last + margin)
            .chain((first.saturating_sub(margin)..first).rev());
        let width = geometry.grid_width();
        order
            .filter_map(|place| {
                let image = items.get(place)?.as_ref()?.image.clone()?;
                Some((
                    Slot::Grid(place),
                    Wish {
                        image,
                        width,
                        variant: Variant::Sharp,
                    },
                ))
            })
            .collect()
    }
}

/// The places beyond the visible range that are worth preparing.
pub(crate) fn margin((first, last): (usize, usize)) -> usize {
    (last.saturating_sub(first) / AHEAD_DIVISOR).max(1)
}

/// Whether a loaded picture at `place` stays while `visible` is on screen.
pub(crate) fn keeps(place: usize, visible: (usize, usize)) -> bool {
    let keep = margin(visible) * KEEP_MARGINS;
    place + keep >= visible.0 && place < visible.1 + keep
}

impl Model for GridModel {
    type Data = CardData;

    fn row_count(&self) -> usize {
        self.items.borrow().len()
    }

    fn row_data(&self, place: usize) -> Option<CardData> {
        let items = self.items.borrow();
        let slot = items.get(place)?;
        let Some(card) = slot else {
            // Not loaded yet: the empty placeholder card.
            return Some(CardData::default());
        };
        Some(CardData {
            title: card.title.as_str().into(),
            subtitle: card.subtitle.clone().unwrap_or_default().into(),
            progress: card.progress.unwrap_or(0.0),
            image: self
                .images
                .borrow()
                .get(&place)
                .cloned()
                .unwrap_or_default(),
            blurred_image: Image::default(),
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }
}
