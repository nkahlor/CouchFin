//! Movie and series detail pages.

use couchfin_api::{ItemId, JellyfinApi};
use futures_util::future::join;

use super::LibraryFlow;
use super::cards;
use super::state::{ScreenState, Season};

impl<A: JellyfinApi> LibraryFlow<A> {
    pub(super) async fn load_movie(&self, serial: u64, id: ItemId) {
        match self.api.item(&self.session, &id).await {
            Ok(item) => {
                self.update(serial, |screen| {
                    if let ScreenState::Movie(movie) = screen {
                        movie.header = Some(cards::header(&item));
                        movie.loading = false;
                    }
                });
            }
            Err(error) => self.fail(serial, error),
        }
    }

    /// Loads the header and the seasons together, then the episodes of the
    /// season for `wanted` (a season number) or the first.
    pub(super) async fn load_series(&self, serial: u64, id: ItemId, wanted: Option<u32>) {
        let header = async {
            match self.api.item(&self.session, &id).await {
                Ok(item) => {
                    self.update(serial, |screen| {
                        if let ScreenState::Series(series) = screen {
                            series.header = Some(cards::header(&item));
                        }
                    });
                }
                Err(error) => self.fail(serial, error),
            }
        };
        let seasons = async {
            match self.api.seasons(&self.session, &id).await {
                Ok(items) => {
                    let seasons: Vec<Season> = items.iter().map(cards::season).collect();
                    let selected = wanted
                        .and_then(|n| seasons.iter().position(|s| s.number == Some(n)))
                        .unwrap_or(0);
                    let first = seasons.get(selected).map(|s| s.id.clone());
                    self.update(serial, |screen| {
                        if let ScreenState::Series(series) = screen {
                            series.seasons = seasons;
                            series.selected_season = selected;
                            series.loading = first.is_some();
                        }
                    });
                    first.map(|season| (season, selected))
                }
                Err(error) => {
                    self.fail(serial, error);
                    None
                }
            }
        };
        if let ((), Some((season, index))) = join(header, seasons).await {
            self.load_episodes(serial, &id, &season, index).await;
        }
    }

    pub(super) async fn select_season(&self, index: usize) {
        let picked = {
            let mut inner = self.lock();
            if inner.expired {
                return;
            }
            let Some(entry) = inner.stack.last_mut() else {
                return;
            };
            let ScreenState::Series(series) = &mut entry.screen else {
                return;
            };
            let Some(season) = series.seasons.get(index) else {
                return;
            };
            let season = season.id.clone();
            if series.selected_season == index && !series.episodes.is_empty() {
                return;
            }
            series.selected_season = index;
            // Don't show the previous season's episodes under the new tab.
            series.episodes.clear();
            series.loading = true;
            let picked = (entry.serial, series.id.clone(), season);
            self.publish(&inner);
            picked
        };
        self.load_episodes(picked.0, &picked.1, &picked.2, index)
            .await;
    }

    /// Loads one season's episodes; the answer counts only while that season
    /// is still the selected one.
    async fn load_episodes(&self, serial: u64, series: &ItemId, season: &ItemId, index: usize) {
        match self.api.episodes(&self.session, series, season).await {
            Ok(items) => {
                self.update(serial, |screen| {
                    if let ScreenState::Series(state) = screen
                        && state.selected_season == index
                    {
                        state.episodes = items.iter().map(cards::episode).collect();
                        state.loading = false;
                    }
                });
            }
            Err(error) => self.fail(serial, error),
        }
    }
}
