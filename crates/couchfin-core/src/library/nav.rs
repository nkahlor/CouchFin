//! Navigation: opening, going back, focus and refresh.

use couchfin_api::{ApiError, JellyfinApi};

use super::state::{GridState, MovieState, ScreenState, SeriesState, Target};
use super::{Entry, LibraryFlow};

impl<A: JellyfinApi> LibraryFlow<A> {
    pub(super) async fn open(&self, target: Target) {
        let screen = match &target {
            Target::Library { id, name } => {
                ScreenState::Library(GridState::new(id.clone(), name.clone()))
            }
            Target::Movie { id } => ScreenState::Movie(MovieState {
                id: id.clone(),
                header: None,
                loading: true,
                error: None,
            }),
            Target::Series { id, .. } => ScreenState::Series(SeriesState {
                id: id.clone(),
                header: None,
                seasons: Vec::new(),
                selected_season: 0,
                episodes: Vec::new(),
                loading: true,
                error: None,
            }),
        };
        let Some(serial) = self.push(screen) else {
            return;
        };
        match target {
            Target::Library { .. } => self.load_pages(serial, 0, 0).await,
            Target::Movie { id } => self.load_movie(serial, id).await,
            Target::Series { id, season } => self.load_series(serial, id, season).await,
        }
    }

    pub(super) fn back(&self) {
        let mut inner = self.lock();
        if inner.stack.len() > 1 {
            inner.stack.pop();
            self.publish(&inner);
        }
    }

    pub(super) fn focus(&self, row: usize, index: usize) {
        let mut inner = self.lock();
        let Some(Entry {
            screen: ScreenState::Home(home),
            ..
        }) = inner.stack.last_mut()
        else {
            return;
        };
        let Some(row) = home.rows.get_mut(row) else {
            return;
        };
        let index = index.min(row.cards.len().saturating_sub(1));
        if std::mem::replace(&mut row.focused, index) != index {
            self.publish(&inner);
        }
    }

    pub(super) async fn refresh(&self) {
        let top = {
            let mut inner = self.lock();
            if inner.expired {
                return;
            }
            let serial = inner.next_serial;
            let Some(entry) = inner.stack.last_mut() else {
                return;
            };
            match &mut entry.screen {
                ScreenState::Home(home) => {
                    // Rows stay until their new data arrives.
                    home.loading = true;
                    home.error = None;
                    entry.serial = serial;
                    inner.next_serial += 1;
                    self.publish(&inner);
                    Refresh::Home(serial)
                }
                other => {
                    let target = other.target();
                    inner.stack.pop();
                    Refresh::Reopen(target)
                }
            }
        };
        match top {
            Refresh::Home(serial) => self.load_home(serial).await,
            Refresh::Reopen(Some(target)) => self.open(target).await,
            Refresh::Reopen(None) => {}
        }
    }
}

enum Refresh {
    Home(u64),
    Reopen(Option<Target>),
}

impl ScreenState {
    /// A request for this screen failed: it is no longer loading, and shows
    /// `error` if there is one.
    pub(super) fn failed(&mut self, error: Option<ApiError>) {
        match self {
            // Other rows may still be on their way: Home stops loading when
            // all of them have finished.
            Self::Home(home) => home.error = home.error.take().or(error),
            Self::Library(grid) => grid.error = error,
            Self::Movie(movie) => {
                movie.loading = false;
                movie.error = error;
            }
            Self::Series(series) => {
                series.loading = false;
                series.error = error;
            }
        }
    }

    /// Where this screen was opened from, to open it again.
    fn target(&self) -> Option<Target> {
        match self {
            Self::Home(_) => None,
            Self::Library(grid) => Some(Target::Library {
                id: grid.library.clone(),
                name: grid.title.clone(),
            }),
            Self::Movie(movie) => Some(Target::Movie {
                id: movie.id.clone(),
            }),
            Self::Series(series) => Some(Target::Series {
                id: series.id.clone(),
                season: series
                    .seasons
                    .get(series.selected_season)
                    .and_then(|s| s.number),
            }),
        }
    }
}
