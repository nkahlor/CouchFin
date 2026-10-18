//! Home's rows as Slint models, kept in step with core's `HomeState`, and
//! which of their pictures are worth fetching.

use std::rc::Rc;

use couchfin_core::{Card, CardShape as CoreShape, HomeState};
use slint::{Model, ModelRc, VecModel};

use super::format;
use super::geometry::Geometry;
use super::loader::{Picture, Slot, Wish, placeholder_image, row_key};
use crate::generated::{CardData, CardShape, RowData};

/// Rows kept loaded behind the focused one, and, past the last one on
/// screen, ahead of it: one more row is just below the fold, ready for Down.
const ROWS_BEHIND: usize = 1;
const ROWS_BEYOND: usize = 1;
/// Cards loaded behind the current one (the passed card that peeks in, and
/// one more) and past the last one that fits.
const CARDS_BEHIND: usize = 2;
const CARDS_BEYOND: usize = 2;

struct RowSlot {
    key: String,
    cards: Rc<VecModel<CardData>>,
    /// The cards `cards` was last made from.
    source: Vec<Card>,
}

/// The rows on Home as the model `HomeView.rows` reads.
pub(crate) struct HomeModels {
    rows: Rc<VecModel<RowData>>,
    slots: Vec<RowSlot>,
}

fn shape(shape: CoreShape) -> CardShape {
    match shape {
        CoreShape::Poster => CardShape::Poster,
        CoreShape::Thumb => CardShape::Thumb,
    }
}

fn card_data(card: &Card, keep: Option<&CardData>) -> CardData {
    let mut data = CardData {
        title: card.title.as_str().into(),
        subtitle: card.subtitle.clone().unwrap_or_default().into(),
        progress: card.progress.unwrap_or(0.0),
        placeholder: placeholder_image(card),
        meta: format::card_meta(card).into(),
        ..CardData::default()
    };
    match keep {
        // The same picture stays on screen while the text changes.
        Some(kept) => {
            data.image = kept.image.clone();
            data.image_state = kept.image_state;
        }
        None => Picture::Loading.fill(&mut data, card.image.is_some()),
    }
    data
}

impl HomeModels {
    pub(crate) fn new() -> Self {
        Self {
            rows: Rc::new(VecModel::default()),
            slots: Vec::new(),
        }
    }

    pub(crate) fn model(&self) -> ModelRc<RowData> {
        ModelRc::from(Rc::clone(&self.rows))
    }

    pub(crate) fn clear(&mut self) {
        self.rows.set_vec(Vec::new());
        self.slots.clear();
    }

    /// The row with this key exists.
    pub(crate) fn has(&self, key: &str) -> bool {
        self.slots.iter().any(|slot| slot.key == key)
    }

    /// The key of the row at `index`.
    pub(crate) fn key_at(&self, index: usize) -> Option<&str> {
        self.slots.get(index).map(|slot| slot.key.as_str())
    }

    /// Makes the models match `home`. Rows and cards that are already there
    /// are changed in place, so what they show (and each row's current
    /// card) survives rows arriving or a refresh; a new row starts on the
    /// card core remembers.
    pub(crate) fn sync(&mut self, home: &HomeState) {
        let mut at = 0;
        for row in &home.rows {
            let key = row_key(&row.kind);
            let found = self.slots[at.min(self.slots.len())..]
                .iter()
                .position(|slot| slot.key == key);
            if let Some(skip) = found {
                for _ in 0..skip {
                    self.slots.remove(at);
                    self.rows.remove(at);
                }
                self.update(at, row);
            } else {
                let cards = Rc::new(VecModel::from(
                    row.cards
                        .iter()
                        .map(|card| card_data(card, None))
                        .collect::<Vec<_>>(),
                ));
                self.rows.insert(
                    at,
                    RowData {
                        title: row.title.as_str().into(),
                        shape: shape(row.shape),
                        cards: ModelRc::from(Rc::clone(&cards)),
                        current: i32::try_from(row.focused).unwrap_or(0),
                    },
                );
                self.slots.insert(
                    at,
                    RowSlot {
                        key,
                        cards,
                        source: row.cards.clone(),
                    },
                );
            }
            at += 1;
        }
        while self.slots.len() > at {
            self.slots.pop();
            self.rows.remove(at);
        }
    }

    /// Brings row `at` in line with `row`, keeping its current card.
    fn update(&mut self, at: usize, row: &couchfin_core::Row) {
        if let Some(mut data) = self.rows.row_data(at) {
            let (title, wanted_shape) = (row.title.as_str(), shape(row.shape));
            if data.title != title || data.shape != wanted_shape {
                data.title = title.into();
                data.shape = wanted_shape;
                self.rows.set_row_data(at, data);
            }
        }
        let slot = &mut self.slots[at];
        for (index, card) in row.cards.iter().enumerate() {
            match (slot.source.get(index), slot.cards.row_data(index)) {
                (Some(old), Some(_)) if old == card => {}
                (Some(old), Some(data)) => {
                    // The same picture stays on screen while the text changes.
                    let same_picture = old.image == card.image;
                    slot.cards
                        .set_row_data(index, card_data(card, same_picture.then_some(&data)));
                }
                _ => slot.cards.push(card_data(card, None)),
            }
        }
        while slot.cards.row_count() > row.cards.len() {
            slot.cards.remove(slot.cards.row_count() - 1);
        }
        slot.source.clone_from(&row.cards);
    }

    /// The row's current card, as the screen last wrote it.
    pub(crate) fn current(&self, row: usize) -> usize {
        self.rows
            .row_data(row)
            .and_then(|data| usize::try_from(data.current).ok())
            .unwrap_or(0)
    }

    /// Shows `picture` on a card.
    pub(crate) fn set_picture(&self, key: &str, index: usize, picture: &Picture) {
        let Some(slot) = self.slots.iter().find(|slot| slot.key == key) else {
            return;
        };
        let Some(mut data) = slot.cards.row_data(index) else {
            return;
        };
        let has_image = slot
            .source
            .get(index)
            .is_some_and(|card| card.image.is_some());
        picture.fill(&mut data, has_image);
        slot.cards.set_row_data(index, data);
    }

    /// The pictures worth fetching, most important first: the focused row's
    /// cards from the current one outward, then the other rows on screen
    /// from the top down, then a margin of one row, so everything visible
    /// (and the next Down) has its picture requested at once.
    pub(crate) fn wanted(
        &self,
        home: &HomeState,
        focused: usize,
        geometry: &Geometry,
    ) -> Vec<(Slot, Wish)> {
        let first_row = focused.saturating_sub(ROWS_BEHIND);
        let visible = geometry.rows_on_screen(home.rows.iter().skip(focused).map(|row| row.shape));
        let last_row = focused + visible + ROWS_BEYOND - 1;
        let mut order: Vec<usize> = (first_row..=last_row).collect();
        order.sort_by_key(|row| row.abs_diff(focused));
        let mut wanted = Vec::new();
        for r in order {
            let Some(row) = home.rows.get(r) else {
                continue;
            };
            let current = self.current(r).min(row.cards.len().saturating_sub(1));
            let fits = geometry.cards_across(row.shape);
            let from = current.saturating_sub(CARDS_BEHIND);
            let to = (current + fits + CARDS_BEYOND).min(row.cards.len());
            let size = geometry.card_size(row.shape);
            let key = row_key(&row.kind);
            let mut places: Vec<usize> = (from..to).collect();
            places.sort_by_key(|place| (place.abs_diff(current), *place));
            for place in places {
                let Some(image) = row.cards[place].image.clone() else {
                    continue;
                };
                wanted.push((Slot::Card(key.clone(), place), Wish { image, size }));
            }
        }
        wanted
    }
}
