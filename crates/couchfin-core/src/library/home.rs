//! Home: the rows load concurrently, each published as it arrives.

use couchfin_api::{Item, JellyfinApi, LibraryKind};
use futures_util::future::{join_all, join3};

use super::LibraryFlow;
use super::cards;
use super::state::{Card, CardShape, HomeState, Row, RowKind, ScreenState};

/// Items per Home row, so Home stays fast (decision 014).
const ROW_LIMIT: u32 = 16;

/// Where the rows sit; recently-added rows follow in library order.
const CONTINUE_ORDER: u32 = 0;
const NEXT_UP_ORDER: u32 = 1;
const RECENT_ORDER: u32 = 2;
const LIBRARIES_ORDER: u32 = u32::MAX;

impl HomeState {
    /// Shows `cards` as the row `kind`: replaces its cards (keeping the
    /// focus where it was), adds it in place, or removes it when `cards` is
    /// empty.
    fn set_row(&mut self, mut row: Row) {
        let existing = self.rows.iter().position(|r| r.kind == row.kind);
        if row.cards.is_empty() {
            if let Some(at) = existing {
                self.rows.remove(at);
            }
            return;
        }
        if let Some(at) = existing {
            row.focused = self.rows[at].focused.min(row.cards.len() - 1);
            self.rows[at] = row;
        } else {
            let at = self.rows.partition_point(|r| r.order <= row.order);
            self.rows.insert(at, row);
        }
    }
}

fn cards_of(items: &[Item], shape: CardShape, with_progress: bool) -> Vec<Card> {
    items
        .iter()
        .filter_map(|item| cards::card(item, shape, with_progress))
        .collect()
}

impl<A: JellyfinApi> LibraryFlow<A> {
    pub(super) async fn load_home(&self, serial: u64) {
        join3(
            self.continue_watching(serial),
            self.next_up(serial),
            self.library_rows(serial),
        )
        .await;
        self.update(serial, |screen| {
            if let ScreenState::Home(home) = screen {
                home.loading = false;
            }
        });
    }

    fn show_row(
        &self,
        serial: u64,
        kind: RowKind,
        title: String,
        shape: CardShape,
        order: u32,
        cards: Vec<Card>,
    ) {
        self.update(serial, |screen| {
            if let ScreenState::Home(home) = screen {
                home.set_row(Row {
                    kind,
                    title,
                    shape,
                    cards,
                    focused: 0,
                    order,
                });
            }
        });
    }

    async fn continue_watching(&self, serial: u64) {
        match self.api.resume(&self.session, ROW_LIMIT).await {
            Ok(items) => self.show_row(
                serial,
                RowKind::ContinueWatching,
                "Continue Watching".into(),
                CardShape::Thumb,
                CONTINUE_ORDER,
                cards_of(&items, CardShape::Thumb, true),
            ),
            Err(error) => self.fail(serial, error),
        }
    }

    async fn next_up(&self, serial: u64) {
        match self.api.next_up(&self.session, ROW_LIMIT).await {
            Ok(items) => self.show_row(
                serial,
                RowKind::NextUp,
                "Next Up".into(),
                CardShape::Thumb,
                NEXT_UP_ORDER,
                cards_of(&items, CardShape::Thumb, false),
            ),
            Err(error) => self.fail(serial, error),
        }
    }

    /// The library tiles, then each movie or TV library's Recently Added.
    async fn library_rows(&self, serial: u64) {
        let libraries = match self.api.libraries(&self.session).await {
            Ok(libraries) => libraries,
            Err(error) => return self.fail(serial, error),
        };
        // Music and the like can't be browsed yet, so they get no tile.
        let browsable: Vec<_> = libraries
            .iter()
            .filter(|l| !matches!(l.kind, LibraryKind::Other(_)))
            .collect();
        self.show_row(
            serial,
            RowKind::Libraries,
            "Libraries".into(),
            CardShape::Thumb,
            LIBRARIES_ORDER,
            browsable.iter().map(|l| cards::library_card(l)).collect(),
        );
        let recent = browsable
            .into_iter()
            .filter(|l| matches!(l.kind, LibraryKind::Movies | LibraryKind::TvShows))
            .zip(RECENT_ORDER..)
            .map(|(library, order)| async move {
                match self.api.latest(&self.session, &library.id, ROW_LIMIT).await {
                    Ok(items) => self.show_row(
                        serial,
                        RowKind::RecentlyAdded(library.id.clone()),
                        format!("Recently Added in {}", library.name),
                        CardShape::Poster,
                        order,
                        cards_of(&items, CardShape::Poster, false),
                    ),
                    Err(error) => self.fail(serial, error),
                }
            });
        join_all(recent).await;
    }
}
