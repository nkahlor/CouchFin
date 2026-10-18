//! The movie and series pages' fields, filled from core's detail states.

use std::rc::Rc;

use couchfin_core::{ApiError, Episode, ImageRef, MovieState, Season, SeriesState};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::format;
use crate::generated::{EpisodeData, MainWindow, MovieView, SeriesView};
use crate::wording;

/// The series page's lists, kept so an unchanged list isn't swapped out
/// (which would restart its scroll) on every state, plus the season the user
/// picked that core hasn't caught up with yet.
pub(crate) struct Detail {
    seasons: Rc<VecModel<SharedString>>,
    episodes: Rc<VecModel<EpisodeData>>,
    last_seasons: Vec<Season>,
    last_episodes: Vec<Episode>,
    picked: Option<usize>,
}

fn error_text(error: Option<&ApiError>) -> SharedString {
    error
        .map(|error| {
            tracing::warn!(%error, "a detail page failed to load");
            wording::browse_error(error)
        })
        .unwrap_or_default()
        .into()
}

impl Detail {
    pub(crate) fn new() -> Self {
        Self {
            seasons: Rc::new(VecModel::default()),
            episodes: Rc::new(VecModel::default()),
            last_seasons: Vec::new(),
            last_episodes: Vec::new(),
            picked: None,
        }
    }

    /// Hands the series page its models.
    pub(crate) fn install(&self, window: &MainWindow) {
        let view = window.global::<SeriesView>();
        view.set_seasons(ModelRc::from(Rc::clone(&self.seasons)));
        view.set_episodes(ModelRc::from(Rc::clone(&self.episodes)));
    }

    /// The user chose a season; core will confirm it with a later state.
    pub(crate) fn pick(&mut self, index: usize) {
        self.picked = Some(index);
    }

    /// A different series is on top: forget the lists and the pick.
    pub(crate) fn forget(&mut self) {
        self.seasons.set_vec(Vec::new());
        self.episodes.set_vec(Vec::new());
        self.last_seasons.clear();
        self.last_episodes.clear();
        self.picked = None;
    }

    /// Fills the movie page; returns its backdrop to fetch.
    pub(crate) fn movie(window: &MainWindow, movie: &MovieState) -> Option<ImageRef> {
        let view = window.global::<MovieView>();
        view.set_error(error_text(movie.error.as_ref()));
        let header = movie.header.as_ref();
        view.set_title(header.map_or("", |h| h.title.as_str()).into());
        view.set_year(format::year(header.and_then(|h| h.year)).into());
        view.set_runtime(format::runtime(header.and_then(|h| h.runtime)).into());
        view.set_rating(
            header
                .and_then(|h| h.rating.clone())
                .unwrap_or_default()
                .into(),
        );
        view.set_overview(
            header
                .and_then(|h| h.overview.clone())
                .unwrap_or_default()
                .into(),
        );
        view.set_resumable(header.is_some_and(|h| h.resume_from.is_some()));
        header?.backdrop.clone()
    }

    /// Fills the series page; returns its backdrop to fetch.
    pub(crate) fn series(&mut self, window: &MainWindow, series: &SeriesState) -> Option<ImageRef> {
        let view = window.global::<SeriesView>();
        view.set_error(error_text(series.error.as_ref()));
        let header = series.header.as_ref();
        view.set_title(header.map_or("", |h| h.title.as_str()).into());
        view.set_year(format::year(header.and_then(|h| h.year)).into());
        view.set_runtime(format::runtime(header.and_then(|h| h.runtime)).into());
        view.set_rating(
            header
                .and_then(|h| h.rating.clone())
                .unwrap_or_default()
                .into(),
        );
        view.set_overview(
            header
                .and_then(|h| h.overview.clone())
                .unwrap_or_default()
                .into(),
        );

        if self.last_seasons != series.seasons {
            self.seasons.set_vec(
                series
                    .seasons
                    .iter()
                    .map(|season| season.name.as_str().into())
                    .collect::<Vec<SharedString>>(),
            );
            self.last_seasons.clone_from(&series.seasons);
        }
        if self.last_episodes != series.episodes {
            self.episodes.set_vec(
                series
                    .episodes
                    .iter()
                    .map(|episode| EpisodeData {
                        number: episode
                            .number
                            .and_then(|n| i32::try_from(n).ok())
                            .unwrap_or(0),
                        title: episode.title.as_str().into(),
                        runtime: format::runtime(episode.runtime).into(),
                        watched: episode.watched,
                    })
                    .collect::<Vec<_>>(),
            );
            self.last_episodes.clone_from(&series.episodes);
        }

        // The page moves the selection at once; core's value is taken only
        // once it agrees, or when the user hasn't chosen.
        if self.picked == Some(series.selected_season) {
            self.picked = None;
        }
        if self.picked.is_none() {
            view.set_selected_season(i32::try_from(series.selected_season).unwrap_or(0));
        }
        header?.backdrop.clone()
    }
}
