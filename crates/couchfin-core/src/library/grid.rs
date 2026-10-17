//! The library grid: the total, then pages on demand.

use couchfin_api::{Item, ItemId, ItemKind, ItemsQuery, JellyfinApi, Page};
use futures_util::future::join_all;

use super::LibraryFlow;
use super::cards;
use super::state::{CardShape, GridState, ScreenState};

impl GridState {
    /// Fills the slots of a loaded page, and sizes the grid to the total.
    fn apply(&mut self, page: &Page<Item>) {
        if self.total != Some(page.total) {
            self.total = Some(page.total);
            self.items.resize(page.total, None);
        }
        for (offset, item) in page.items.iter().enumerate() {
            if let Some(slot) = self.items.get_mut(page.start_index + offset) {
                *slot = cards::card(item, CardShape::Poster, false);
            }
        }
        self.error = None;
    }
}

impl<A: JellyfinApi> LibraryFlow<A> {
    /// Loads the pages covering positions `first` to `last` of the grid
    /// with this serial, except those already loaded or in flight.
    pub(super) async fn load_pages(&self, serial: u64, first: usize, last: usize) {
        let Some((library, pages)) = self.claim_pages(serial, first, last) else {
            return;
        };
        join_all(
            pages
                .into_iter()
                .map(|page| self.load_page(serial, &library, page)),
        )
        .await;
    }

    pub(super) async fn need_items(&self, first: usize, last: usize) {
        let serial = {
            let inner = self.lock();
            match inner.stack.last() {
                Some(entry) if matches!(entry.screen, ScreenState::Library(_)) => entry.serial,
                _ => return,
            }
        };
        self.load_pages(serial, first, last).await;
    }

    /// Marks the missing pages of the range as requested, so a second call
    /// for the same range finds nothing to do.
    fn claim_pages(&self, serial: u64, first: usize, last: usize) -> Option<(ItemId, Vec<usize>)> {
        let mut inner = self.lock();
        let entry = inner
            .stack
            .iter_mut()
            .find(|entry| entry.serial == serial)?;
        let ScreenState::Library(grid) = &mut entry.screen else {
            return None;
        };
        let last = match grid.total {
            Some(0) => return None,
            Some(total) => last.min(total - 1),
            None => last,
        };
        let pages: Vec<usize> = (first / self.page_size..=last / self.page_size)
            .filter(|page| grid.requested.insert(*page))
            .collect();
        Some((grid.library.clone(), pages))
    }

    async fn load_page(&self, serial: u64, library: &ItemId, page: usize) {
        let query = ItemsQuery::default()
            .in_parent(library.clone())
            .of_kinds([ItemKind::Movie, ItemKind::Series])
            .recursive()
            .page(page * self.page_size, self.page_size);
        match self.api.items(&self.session, &query).await {
            Ok(loaded) => {
                self.update(serial, |screen| {
                    if let ScreenState::Library(grid) = screen {
                        grid.apply(&loaded);
                    }
                });
            }
            Err(error) => {
                // Forget the claim, so scrolling there again retries.
                self.update(serial, |screen| {
                    if let ScreenState::Library(grid) = screen {
                        grid.requested.remove(&page);
                    }
                });
                self.fail(serial, error);
            }
        }
    }
}
