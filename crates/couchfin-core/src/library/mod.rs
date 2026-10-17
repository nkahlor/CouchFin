//! Library state: the navigation stack, Home rows, the paged grid and the
//! detail pages, as a state the UI renders and actions it sends back.

mod cards;
mod detail;
mod grid;
mod home;
mod nav;
mod state;

pub use state::{
    Card, CardShape, Episode, GridState, Header, HomeState, ImageRef, LibraryState, MovieState,
    Row, RowKind, ScreenState, Season, SeriesState, Target,
};

use std::sync::{Mutex, MutexGuard, PoisonError};

use couchfin_api::{ApiError, JellyfinApi, Session};

type Observer = Box<dyn Fn(&LibraryState) + Send + Sync>;

/// Items per request when nothing else is configured.
const DEFAULT_PAGE_SIZE: usize = 50;

/// What the UI asks the library flow to do. An action that doesn't fit the
/// top screen (such as `SelectSeason` on Home) is ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryAction {
    /// Open the card at `row` and `index` of Home (the visible rows, as in
    /// [`HomeState::rows`]).
    OpenCard {
        /// Index into the rows.
        row: usize,
        /// Index into the row's cards.
        index: usize,
    },
    /// Open the item at this position of the library grid.
    OpenGridItem(usize),
    /// Open a screen directly.
    Open(Target),
    /// Show another season of the series on top (index into its seasons).
    SelectSeason(usize),
    /// Pop one screen. On Home it does nothing: that is the UI's business
    /// (it quits), and [`LibraryState::at_root`] tells it when.
    Back,
    /// The user moved the focus to a card of a Home row.
    FocusChanged {
        /// Index into the rows.
        row: usize,
        /// Index into the row's cards.
        index: usize,
    },
    /// The grid is about to show items `first` to `last` (inclusive).
    NeedItems {
        /// First position.
        first: usize,
        /// Last position.
        last: usize,
    },
    /// Load the top screen again, keeping what it shows until the new data
    /// arrives.
    Refresh,
}

/// Runs the library screens for a logged-in [`Session`]: the navigation
/// stack, Home's rows, the paged library grid and the detail pages.
///
/// It follows [`LoginFlow`](crate::LoginFlow)'s pattern. The UI reads
/// [`LibraryFlow::state`] (or is told of every change through
/// [`LibraryFlow::on_change`]) and sends [`LibraryAction`]s with
/// [`LibraryFlow::dispatch`]. The methods take `&self` and return `Send`
/// futures, so the app shares one flow in an `Arc` and spawns each call. A
/// result for a screen that has since been popped is dropped.
///
/// Home loads its rows concurrently and publishes each as it arrives; empty
/// rows are never shown. Rows that follow decision 014 are Continue Watching,
/// Next Up, Recently Added in each movie or TV library, and Libraries.
///
/// # Session expiry
///
/// When any request fails with `ApiError::TokenRejected`,
/// [`LibraryState::session_expired`] becomes true, is published, and the
/// flow ignores further navigation. The app watches for it in its
/// `on_change` observer, drops this flow, and sends
/// [`LoginAction::LogOut`](crate::LoginAction::LogOut) to its
/// [`LoginFlow`](crate::LoginFlow), which forgets the token and shows the
/// login form. No other error ends the session; they are typed `error`
/// fields on the screen that failed.
///
/// # Examples
///
/// ```
/// use couchfin_core::fake::FakeJellyfin;
/// use couchfin_core::{JellyfinApi, LibraryAction, LibraryFlow, Password, ScreenState};
/// use couchfin_api::{LibraryKind, Session};
///
/// let server = "http://jellyfin.lan:8096".parse()?;
/// let fake = FakeJellyfin::new(server)
///     .with_user("alice", "hunter2")
///     .with_library("Movies", LibraryKind::Movies)
///     .with_movie("Movies", "Arrival");
///
/// let runtime = tokio::runtime::Builder::new_current_thread().build()?;
/// runtime.block_on(async {
///     let login = fake
///         .authenticate_by_name(fake.server(), "alice", &Password::new("hunter2"))
///         .await?;
///     let session = Session::new(fake.server().clone(), login.access_token);
///
///     let flow = LibraryFlow::new(fake, session);
///     flow.start().await;
///     let state = flow.state();
///     let titles: Vec<_> = state.home().rows.iter().map(|row| row.title.as_str()).collect();
///     assert_eq!(titles, ["Recently Added in Movies", "Libraries"]);
///
///     // Open the library tile, then its first movie.
///     flow.dispatch(LibraryAction::OpenCard { row: 1, index: 0 }).await;
///     flow.dispatch(LibraryAction::OpenGridItem(0)).await;
///     assert!(matches!(flow.state().current(), ScreenState::Movie(_)));
///
///     flow.dispatch(LibraryAction::Back).await;
///     flow.dispatch(LibraryAction::Back).await;
///     assert!(flow.state().at_root());
///     Ok::<(), Box<dyn std::error::Error>>(())
/// })?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct LibraryFlow<A> {
    api: A,
    session: Session,
    page_size: usize,
    state: Mutex<Inner>,
    observer: Option<Observer>,
}

struct Inner {
    stack: Vec<Entry>,
    /// Every pushed screen gets the next number, so a request can tell
    /// whether the screen it was for is still there.
    next_serial: u64,
    started: bool,
    expired: bool,
}

struct Entry {
    serial: u64,
    screen: ScreenState,
}

impl<A: JellyfinApi> LibraryFlow<A> {
    /// A flow on an empty Home, still loading until [`LibraryFlow::start`].
    #[must_use]
    pub fn new(api: A, session: Session) -> Self {
        let home = HomeState {
            loading: true,
            ..HomeState::default()
        };
        Self {
            api,
            session,
            page_size: DEFAULT_PAGE_SIZE,
            state: Mutex::new(Inner {
                stack: vec![Entry {
                    serial: 0,
                    screen: ScreenState::Home(home),
                }],
                next_serial: 1,
                started: false,
                expired: false,
            }),
            observer: None,
        }
    }

    /// Sets how many grid items each request loads (at least 1; the default
    /// is 50).
    #[must_use]
    pub fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = page_size.max(1);
        self
    }

    /// Calls `observer` with every new state, in order. It runs while the
    /// flow is locked, so it must be quick and must not call back into the
    /// flow: clone what the UI needs and hand it to the UI thread.
    #[must_use]
    pub fn on_change(mut self, observer: impl Fn(&LibraryState) + Send + Sync + 'static) -> Self {
        self.observer = Some(Box::new(observer));
        self
    }

    /// The current state.
    #[must_use]
    pub fn state(&self) -> LibraryState {
        snapshot(&self.lock())
    }

    /// Loads Home. Returns when every row has arrived or failed. Does
    /// nothing after the first call.
    pub async fn start(&self) {
        {
            let mut inner = self.lock();
            if std::mem::replace(&mut inner.started, true) {
                return;
            }
        }
        self.load_home(0).await;
    }

    /// Handles one action from the UI. Returns when any request the action
    /// started has finished.
    pub async fn dispatch(&self, action: LibraryAction) {
        match action {
            LibraryAction::OpenCard { row, index } => {
                let target = self.with_top(|screen| match screen {
                    ScreenState::Home(home) => home
                        .rows
                        .get(row)?
                        .cards
                        .get(index)
                        .map(|card| card.target.clone()),
                    _ => None,
                });
                if let Some(Some(target)) = target {
                    self.open(target).await;
                }
            }
            LibraryAction::OpenGridItem(index) => {
                let target = self.with_top(|screen| match screen {
                    ScreenState::Library(grid) => grid.item(index).map(|card| card.target.clone()),
                    _ => None,
                });
                if let Some(Some(target)) = target {
                    self.open(target).await;
                }
            }
            LibraryAction::Open(target) => self.open(target).await,
            LibraryAction::SelectSeason(index) => self.select_season(index).await,
            LibraryAction::Back => self.back(),
            LibraryAction::FocusChanged { row, index } => self.focus(row, index),
            LibraryAction::NeedItems { first, last } => self.need_items(first, last).await,
            LibraryAction::Refresh => self.refresh().await,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // Each change is applied whole, so the state is consistent even after
        // a panic elsewhere.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn publish(&self, inner: &Inner) {
        if let Some(observer) = &self.observer {
            observer(&snapshot(inner));
        }
    }

    /// Reads the top screen.
    fn with_top<R>(&self, read: impl FnOnce(&ScreenState) -> R) -> Option<R> {
        self.lock().stack.last().map(|entry| read(&entry.screen))
    }

    /// Pushes a screen and returns its serial; none once the session expired.
    fn push(&self, screen: ScreenState) -> Option<u64> {
        let mut inner = self.lock();
        if inner.expired {
            return None;
        }
        let serial = inner.next_serial;
        inner.next_serial += 1;
        inner.stack.push(Entry { serial, screen });
        self.publish(&inner);
        Some(serial)
    }

    /// Changes the screen with this serial, wherever it is in the stack, and
    /// publishes. A screen that was popped is gone, and its result with it.
    fn update<R>(&self, serial: u64, change: impl FnOnce(&mut ScreenState) -> R) -> Option<R> {
        let mut inner = self.lock();
        let entry = inner
            .stack
            .iter_mut()
            .find(|entry| entry.serial == serial)?;
        let result = change(&mut entry.screen);
        self.publish(&inner);
        Some(result)
    }

    /// Records a failed request: a rejected token expires the session,
    /// anything else becomes the screen's typed error.
    fn fail(&self, serial: u64, error: ApiError) {
        let mut inner = self.lock();
        let Some(entry) = inner.stack.iter_mut().find(|entry| entry.serial == serial) else {
            return;
        };
        let expired = error == ApiError::TokenRejected;
        entry.screen.failed((!expired).then_some(error));
        inner.expired |= expired;
        self.publish(&inner);
    }
}

fn snapshot(inner: &Inner) -> LibraryState {
    LibraryState {
        screens: inner.stack.iter().map(|e| e.screen.clone()).collect(),
        session_expired: inner.expired,
    }
}
