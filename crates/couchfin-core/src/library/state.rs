//! The plain-data state the library screens render. Every type is `Clone`
//! and compared by value, with public fields, so UI tests can build any
//! screen directly.

use std::time::Duration;

use couchfin_api::{ApiError, ImageKind, ImageRequest, ImageTag, ItemId};

/// What the navigation stack holds, bottom to top. The first screen is always
/// [`ScreenState::Home`].
///
/// `session_expired` is the app's cue to return to sign-in: see
/// [`LibraryFlow`](super::LibraryFlow).
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryState {
    /// The stack, Home first. The last one is on screen.
    pub screens: Vec<ScreenState>,
    /// The server rejected the access token (`ApiError::TokenRejected`). The
    /// flow stops loading; the app should go back to sign-in.
    pub session_expired: bool,
}

impl LibraryState {
    /// The screen on top of the stack.
    ///
    /// # Panics
    ///
    /// Never for a state a flow published; only if `screens` was emptied by
    /// hand.
    #[must_use]
    pub fn current(&self) -> &ScreenState {
        self.screens.last().expect("the stack always holds Home")
    }

    /// The Home screen at the bottom of the stack.
    ///
    /// # Panics
    ///
    /// Never for a state a flow published.
    #[must_use]
    pub fn home(&self) -> &HomeState {
        match self.screens.first() {
            Some(ScreenState::Home(home)) => home,
            _ => panic!("the bottom of the stack is Home"),
        }
    }

    /// Whether Home is on top, where Back does nothing in core and the UI
    /// decides what it means (quit).
    #[must_use]
    pub fn at_root(&self) -> bool {
        self.screens.len() == 1
    }
}

/// One screen of the stack.
#[derive(Debug, Clone, PartialEq)]
pub enum ScreenState {
    /// The rows.
    Home(HomeState),
    /// The poster grid of one library.
    Library(GridState),
    /// A movie's detail page.
    Movie(MovieState),
    /// A series' detail page.
    Series(SeriesState),
}

/// Where a card leads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A library's grid.
    Library {
        /// The library.
        id: ItemId,
        /// Its name, for the grid's title.
        name: String,
    },
    /// A movie's detail page.
    Movie {
        /// The movie.
        id: ItemId,
    },
    /// A series' detail page. An episode leads here too.
    Series {
        /// The series.
        id: ItemId,
        /// The season number to select, when the card was an episode.
        season: Option<u32>,
    },
}

/// Which of a card's images the UI should show, as the ingredients of an
/// [`ImageRequest`] (the image cache decides the size).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    /// The item the image belongs to.
    pub item: ItemId,
    /// Which of its images.
    pub kind: ImageKind,
    /// The image's tag, so a changed image is fetched again.
    pub tag: ImageTag,
}

impl ImageRef {
    /// The request that fetches this image, scaled to at most `max_width`.
    #[must_use]
    pub fn request(&self, max_width: u32) -> ImageRequest {
        ImageRequest::new(self.item.clone(), self.kind, max_width).with_tag(self.tag.clone())
    }
}

/// The shape of a row's cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardShape {
    /// 2:3 poster.
    Poster,
    /// 16:9 thumbnail, with a progress bar when the card has progress.
    Thumb,
}

/// One card of a row or grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    /// Where selecting it goes.
    pub target: Target,
    /// The title; an episode shows its series' title.
    pub title: String,
    /// A second line, such as `S2 · E4` for an episode.
    pub subtitle: Option<String>,
    /// How far through it the user is, 0 to 1; only when started.
    pub progress: Option<f32>,
    /// The picture, if it has one.
    pub image: Option<ImageRef>,
    /// The production year, for the details line; none for a library tile.
    pub year: Option<u32>,
    /// How long it runs, for the details line; none for a library tile.
    pub runtime: Option<Duration>,
}

/// Which row it is; also what identifies a row across reloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKind {
    /// Unfinished videos.
    ContinueWatching,
    /// The next episode of each series in progress.
    NextUp,
    /// The newest items of one library.
    RecentlyAdded(ItemId),
    /// One tile per library.
    Libraries,
}

/// A carousel on Home. Rows with no cards never reach the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Which row.
    pub kind: RowKind,
    /// The title above it.
    pub title: String,
    /// The shape of its cards.
    pub shape: CardShape,
    /// Its cards; never empty.
    pub cards: Vec<Card>,
    /// The focused card, remembered across navigating away and back.
    pub focused: usize,
    /// Position on Home (lower is higher up); the flow keeps rows sorted.
    pub order: u32,
}

/// The Home screen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HomeState {
    /// The non-empty rows loaded so far, top to bottom.
    pub rows: Vec<Row>,
    /// Some rows are still on their way.
    pub loading: bool,
    /// The first failure of a row's request (other than a rejected token).
    pub error: Option<ApiError>,
}

/// A library grid. The UI's lazy model reads [`GridState::total`] and
/// [`GridState::item`], and sends `NeedItems` for what it is about to show.
#[derive(Debug, Clone, PartialEq)]
pub struct GridState {
    /// The library.
    pub library: ItemId,
    /// Its name.
    pub title: String,
    /// How many items it holds; `None` until the first page arrives.
    pub total: Option<usize>,
    /// One slot per item, `None` while not loaded. Empty until `total` is
    /// known.
    pub items: Vec<Option<Card>>,
    /// The last failed page request (other than a rejected token).
    pub error: Option<ApiError>,
    /// Pages asked for and not failed (in flight or loaded).
    pub(super) requested: std::collections::BTreeSet<usize>,
}

impl GridState {
    pub(super) fn new(library: ItemId, title: String) -> Self {
        Self {
            library,
            title,
            total: None,
            items: Vec::new(),
            error: None,
            requested: std::collections::BTreeSet::new(),
        }
    }

    /// The item at `index`, if it is loaded.
    #[must_use]
    pub fn item(&self, index: usize) -> Option<&Card> {
        self.items.get(index)?.as_ref()
    }
}

/// What the detail pages show about an item.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    /// The title.
    pub title: String,
    /// The release year.
    pub year: Option<u32>,
    /// How long it runs.
    pub runtime: Option<Duration>,
    /// The age rating.
    pub rating: Option<String>,
    /// The synopsis.
    pub overview: Option<String>,
    /// Where playback would resume; `None` if not started or finished.
    pub resume_from: Option<Duration>,
    /// The wide background.
    pub backdrop: Option<ImageRef>,
    /// The poster.
    pub poster: Option<ImageRef>,
}

/// A movie's detail page.
#[derive(Debug, Clone, PartialEq)]
pub struct MovieState {
    /// The movie.
    pub id: ItemId,
    /// Loaded details.
    pub header: Option<Header>,
    /// The request is running.
    pub loading: bool,
    /// Why it failed (other than a rejected token).
    pub error: Option<ApiError>,
}

/// One season in the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Season {
    /// The season.
    pub id: ItemId,
    /// Its name, such as "Season 2".
    pub name: String,
    /// Its number, if the server has one.
    pub number: Option<u32>,
}

/// One episode in the list.
#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    /// The episode.
    pub id: ItemId,
    /// Its number in the season.
    pub number: Option<u32>,
    /// Its title.
    pub title: String,
    /// How long it runs.
    pub runtime: Option<Duration>,
    /// The user has finished it.
    pub watched: bool,
}

/// A series' detail page.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesState {
    /// The series.
    pub id: ItemId,
    /// Loaded details.
    pub header: Option<Header>,
    /// The seasons, in order.
    pub seasons: Vec<Season>,
    /// Index into `seasons`.
    pub selected_season: usize,
    /// The selected season's episodes.
    pub episodes: Vec<Episode>,
    /// Something is still loading (header, seasons or episodes).
    pub loading: bool,
    /// The last failure (other than a rejected token).
    pub error: Option<ApiError>,
}
