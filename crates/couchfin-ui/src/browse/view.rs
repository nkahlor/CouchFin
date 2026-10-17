//! The browsing screens' state: the models they read, the pictures loaded
//! for them, and what the latest [`LibraryState`] says is on top.

use std::rc::Rc;
use std::sync::Arc;

use couchfin_core::{
    CardShape, HomeState, ImageLoad, ImageRef, LibraryState, ScreenState, Variant,
};
use slint::{ComponentHandle, Image, ModelRc};

use super::detail::Detail;
use super::feed::Feed;
use super::geometry::Geometry;
use super::grid::{self, GridModel};
use super::home::HomeModels;
use super::loader::{ImageLoader, Loads, Slot, Wish, to_image};
use crate::generated::{HomeView, LibraryView, MainWindow, MovieView, Screen, SeriesView};
use crate::wording;

pub(super) struct Browser {
    loads: Loads,
    home: HomeModels,
    grid: Option<(String, Rc<GridModel>)>,
    detail: Detail,
    /// The series on the page, so a different one starts with empty lists.
    series: Option<String>,
    backdrop: Option<ImageRef>,
    latest: Option<LibraryState>,
    /// The sign-in flow is on Home, so the library owns the window's screen.
    shown: bool,
    /// The grid places on screen (the last end is exclusive).
    visible: (usize, usize),
    /// The range last asked of core, so a repeat isn't sent again.
    asked: Option<(usize, usize)>,
}

impl Browser {
    pub(super) fn new(loader: Arc<dyn ImageLoader>, feed: Feed) -> Self {
        Self {
            loads: Loads::new(loader, feed),
            home: HomeModels::new(),
            grid: None,
            detail: Detail::new(),
            series: None,
            backdrop: None,
            latest: None,
            shown: false,
            visible: (0, 0),
            asked: None,
        }
    }

    pub(super) fn install(&self, window: &MainWindow) {
        let home = window.global::<HomeView>();
        home.set_rows(self.home.model());
        home.set_status(wording::LOADING.into());
        self.detail.install(window);
    }

    pub(super) fn picked_season(&mut self, index: usize) {
        self.detail.pick(index);
    }

    /// The user moved on Home: the pictures wanted may have changed.
    pub(super) fn refresh(&mut self, window: &MainWindow) {
        self.reconcile(window);
    }

    /// The grid shows places `first` to `last` (exclusive). Returns the
    /// range core should load (the margin included), if it isn't the one
    /// already asked for.
    pub(super) fn visible(
        &mut self,
        window: &MainWindow,
        first: usize,
        last: usize,
    ) -> Option<(usize, usize)> {
        self.visible = (first, last);
        self.reconcile(window);
        if last <= first {
            return None;
        }
        let margin = grid::margin(self.visible);
        let range = (first.saturating_sub(margin), last + margin - 1);
        (self.asked.replace(range) != Some(range)).then_some(range)
    }

    pub(super) fn show(&mut self, window: &MainWindow) {
        self.shown = true;
        let screen = self
            .latest
            .as_ref()
            .map_or(Screen::Home, super::state_screen);
        window.set_screen(screen);
    }

    pub(super) fn reset(&mut self, window: &MainWindow) {
        self.shown = false;
        self.loads.clear();
        self.home.clear();
        self.grid = None;
        self.detail.forget();
        self.series = None;
        self.backdrop = None;
        self.latest = None;
        self.visible = (0, 0);
        self.asked = None;
        let home = window.global::<HomeView>();
        home.set_focused_row(0);
        home.set_posters_above(0);
        home.set_thumbs_above(0);
        home.set_status(wording::LOADING.into());
        home.set_error("".into());
        window.global::<MovieView>().set_backdrop(Image::default());
        window.global::<SeriesView>().set_backdrop(Image::default());
    }

    /// Takes a new state: fills the models for every screen that has data,
    /// switches to the one on top, and starts or cancels picture loads.
    pub(super) fn apply(&mut self, window: &MainWindow, state: LibraryState) {
        self.sync_home(window, state.home());
        let on_top = state.current();
        if !matches!(on_top, ScreenState::Library(_)) {
            self.drop_grid();
        }
        let backdrop = match on_top {
            ScreenState::Home(_) => None,
            ScreenState::Library(grid) => {
                self.sync_grid(window, grid);
                None
            }
            ScreenState::Movie(movie) => Detail::movie(window, movie),
            ScreenState::Series(series) => {
                if self.series.as_deref() != Some(series.id.as_str()) {
                    self.detail.forget();
                    self.series = Some(series.id.as_str().to_owned());
                }
                self.detail.series(window, series)
            }
        };
        if backdrop != self.backdrop {
            window.global::<MovieView>().set_backdrop(Image::default());
            window.global::<SeriesView>().set_backdrop(Image::default());
            self.backdrop = backdrop;
        }
        if self.shown {
            window.set_screen(super::state_screen(&state));
        }
        self.latest = Some(state);
        self.reconcile(window);
    }

    fn sync_home(&mut self, window: &MainWindow, home: &HomeState) {
        let view = window.global::<HomeView>();
        let before = usize::try_from(view.get_focused_row())
            .ok()
            .and_then(|row| self.home.key_at(row))
            .map(str::to_owned);
        self.home.sync(home);
        // The focused row stays the same row when others arrive above it.
        let focused = before
            .and_then(|key| {
                home.rows
                    .iter()
                    .position(|row| super::loader::row_key(&row.kind) == key)
            })
            .unwrap_or(0);
        let above = &home.rows[..focused.min(home.rows.len())];
        let count = |shape| i32::try_from(above.iter().filter(|r| r.shape == shape).count());
        view.set_focused_row(i32::try_from(focused).unwrap_or(0));
        view.set_posters_above(count(CardShape::Poster).unwrap_or(0));
        view.set_thumbs_above(count(CardShape::Thumb).unwrap_or(0));

        let described = home.error.as_ref().map(|error| {
            tracing::warn!(%error, "Home couldn't load a row");
            wording::browse_error(error)
        });
        let (status, error) = if home.rows.is_empty() {
            let status = described.unwrap_or_else(|| {
                if home.loading {
                    wording::LOADING.to_owned()
                } else {
                    wording::EMPTY_LIBRARY.to_owned()
                }
            });
            (status, String::new())
        } else {
            (String::new(), described.unwrap_or_default())
        };
        view.set_status(status.into());
        view.set_error(error.into());
    }

    /// Forgets the grid and its pictures.
    fn drop_grid(&mut self) {
        if self.grid.take().is_some() {
            self.loads.forget(|slot| matches!(slot, Slot::Grid(_)));
        }
        self.visible = (0, 0);
        self.asked = None;
    }

    fn sync_grid(&mut self, window: &MainWindow, grid: &couchfin_core::GridState) {
        let view = window.global::<LibraryView>();
        let model = match &self.grid {
            Some((library, model)) if library == grid.library.as_str() => Rc::clone(model),
            _ => {
                self.drop_grid();
                let model = Rc::new(GridModel::new());
                view.set_items(ModelRc::from(Rc::clone(&model)));
                self.grid = Some((grid.library.as_str().to_owned(), Rc::clone(&model)));
                model
            }
        };
        model.update(grid);
        view.set_title(grid.title.as_str().into());
        view.set_item_count(i32::try_from(grid.total.unwrap_or(0)).unwrap_or(i32::MAX));
        let error = grid.error.as_ref().map(|error| {
            tracing::warn!(%error, "the library grid couldn't load a page");
            wording::browse_error(error)
        });
        view.set_error(error.unwrap_or_default().into());
    }

    /// Starts the loads the top screen wants and cancels the rest.
    fn reconcile(&mut self, window: &MainWindow) {
        let Some(state) = &self.latest else {
            return;
        };
        let geometry = Geometry::of(window);
        let wanted: Vec<(Slot, Wish)> = match state.current() {
            ScreenState::Home(home) => {
                let focused =
                    usize::try_from(window.global::<HomeView>().get_focused_row()).unwrap_or(0);
                self.home.wanted(home, focused, &geometry)
            }
            ScreenState::Library(_) => self
                .grid
                .as_ref()
                .map(|(_, model)| model.wanted(self.visible, &geometry))
                .unwrap_or_default(),
            ScreenState::Movie(_) | ScreenState::Series(_) => self
                .backdrop
                .iter()
                .map(|image| {
                    (
                        Slot::Backdrop,
                        Wish {
                            image: image.clone(),
                            width: geometry.backdrop_width(),
                            variant: Variant::Sharp,
                        },
                    )
                })
                .collect(),
        };
        let (home, visible) = (&self.home, self.visible);
        let cleared = self.loads.reconcile(wanted, |slot| match slot {
            Slot::Card(key, ..) => home.has(key),
            Slot::Grid(place) => grid::keeps(*place, visible),
            Slot::Backdrop => false,
        });
        for slot in cleared {
            self.put(window, &slot, None);
        }
    }

    /// A load finished: shows its picture if the slot still wants it.
    pub(super) fn image(
        &mut self,
        window: &MainWindow,
        slot: &Slot,
        ticket: u64,
        loaded: &ImageLoad,
    ) {
        if !self.loads.finish(slot, ticket) {
            return;
        }
        // No picture keeps the placeholder.
        if let ImageLoad::Ready(pixels) = loaded {
            self.put(window, slot, Some(to_image(pixels)));
        }
    }

    /// Shows `image` in `slot`, or the placeholder for `None`.
    fn put(&self, window: &MainWindow, slot: &Slot, image: Option<Image>) {
        match slot {
            Slot::Card(key, index, variant) => self.home.set_image(key, *index, *variant, image),
            Slot::Grid(place) => {
                if let Some((_, model)) = &self.grid {
                    model.set_image(*place, image);
                }
            }
            Slot::Backdrop => {
                let image = image.unwrap_or_default();
                window.global::<MovieView>().set_backdrop(image.clone());
                window.global::<SeriesView>().set_backdrop(image);
            }
        }
    }
}
