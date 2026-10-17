//! Slint screens and components, with controller focus handling.
//!
//! Holds no business logic: it renders couchfin-core's state and sends
//! actions back. The markup lives in `ui/*.slint` and is compiled to Rust by
//! `build.rs`.
//!
//! - [`MainWindow`] is the one window Couchfin shows. Its [`Screen`] picks
//!   the screen, and each screen's data and callbacks sit in a Slint global:
//!   [`ConnectView`], [`AddressView`], [`LoginView`], [`HomeView`],
//!   [`LibraryView`], [`MovieView`] and [`SeriesView`].
//! - [`render`] shows a core [`LoginScreen`](couchfin_core::LoginScreen) in
//!   the window, and [`bind`] turns its callbacks into [`Command`]s: a
//!   [`LoginAction`](couchfin_core::LoginAction) for the flow, or quit.
//! - [`wording`] is every message the user reads about an error. An error's
//!   `Display` text is for the log only.
//!
//! - [`navigate`] feeds a controller's [`Intent`] (D-pad, A, B) into the
//!   window as its key, so a controller and the keyboard share one focus
//!   path. [`Navigator`] wraps it and drops the duplicate key Steam's
//!   Desktop Layout types for the same press (decision 013).
//!
//! Every screen works from the keyboard alone: Tab or Down moves to the next
//! control, Shift+Tab or Up to the previous one, Enter (or Space on a
//! button) presses it, and Escape asks to go back. Backspace deletes in a
//! text field and means back everywhere else. Showing a screen focuses its
//! first control. The design is in `docs/decisions/008-login-screens.md`,
//! and controller input in `010-gamepad-input.md`.
//!
//! # Examples
//!
//! ```
//! use std::cell::RefCell;
//! use std::rc::Rc;
//!
//! use couchfin_core::{EnterAddress, LoginAction, LoginScreen, Request};
//! use couchfin_ui::{Command, MainWindow, bind, render};
//! use slint::ComponentHandle;
//! use slint::platform::{Key, WindowEvent};
//!
//! # i_slint_backend_testing::init_no_event_loop();
//! let window = MainWindow::new()?;
//! let sent = Rc::new(RefCell::new(Vec::new()));
//! let sink = Rc::clone(&sent);
//! bind(&window, move |command| sink.borrow_mut().push(command));
//! render(
//!     &window,
//!     &LoginScreen::EnterAddress(EnterAddress {
//!         address: "jellyfin.lan".into(),
//!         request: Request::Idle,
//!     }),
//! );
//! window.show()?;
//!
//! // The address field has focus: press Enter to submit what it holds.
//! let text: slint::SharedString = Key::Return.into();
//! window.window().dispatch_event(WindowEvent::KeyPressed { text });
//! assert!(matches!(
//!     sent.borrow().as_slice(),
//!     [Command::Login(LoginAction::SubmitAddress(address))] if address == "jellyfin.lan"
//! ));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod bind;
mod guard;
mod navigate;
mod navigator;
pub mod wording;

mod generated {
    // Slint's generated code has no doc comments and uses `unwrap()`. The
    // public re-exports below carry the documentation instead.
    #![allow(missing_docs, clippy::unwrap_used)]
    slint::include_modules!();
}

pub use bind::{Command, bind, render};
pub use guard::{Clock, Dropped, DuplicateGuard, GuardConfig, SystemClock, Verdict};
pub use navigate::{Intent, navigate};
pub use navigator::{INPUT_LOG, Navigator, log_stamp};

/// The one window Couchfin shows: a dark backdrop with the current
/// [`Screen`] on it. The app makes it fullscreen.
///
/// - `screen` (in, default `Connecting`): only that screen exists. Switching
///   destroys the previous one, along with anything typed into it, such as a
///   password, and focuses the new screen's first control.
/// - `back-quits` (in, default true): what Back means on the current screen,
///   quit or change server. [`render`] sets it and [`bind`] reads it.
///
/// Use [`render`] and [`bind`] rather than setting these and the views by
/// hand; see the crate example.
pub use generated::MainWindow;

/// Which screen [`MainWindow`] shows, and the view it reads.
///
/// - `Connecting`: starting up or checking a stored session ([`ConnectView`]).
/// - `ConnectFailed`: that check failed; Retry or Change server
///   ([`ConnectView`]).
/// - `Address`: type the server's address ([`AddressView`]).
/// - `Login`: sign in to the server the address check found ([`LoginView`]).
/// - `Home`: signed in; rows of cards ([`HomeView`]).
/// - `Library`: a library's poster grid ([`LibraryView`]).
/// - `Movie`: a movie's detail page ([`MovieView`]).
/// - `Series`: a series' detail page ([`SeriesView`]).
pub use generated::Screen;

/// The startup screens' data and callbacks.
///
/// - `server` (in): the stored server, shown as "Connecting to {server}…",
///   or "Starting…" while empty; on failure, "Couldn't connect to {server}".
/// - `error` (in): why connecting failed.
/// - `retry()`, `change-server()`: the Retry and Change server buttons, in
///   that focus order.
/// - `back()`: Escape (or Backspace on a button).
pub use generated::ConnectView;

/// The server address screen's data and callbacks.
///
/// Focus order: the "Server address" field, then Connect.
///
/// - `address` (in-out): the field's text. Typing updates it; set it to
///   prefill.
/// - `busy` (in): shows "Checking the server…"; the field and Connect ignore
///   input.
/// - `error` (in): a message under the form; empty shows none.
/// - `submit(address)`: Enter in the field or Connect, with the address as
///   typed. Not sent while `busy`.
/// - `back()`: Escape.
pub use generated::AddressView;

/// The sign-in screen's data and callbacks.
///
/// Focus order: "Username", "Password", Sign in, Change server. Enter in the
/// username field moves to the password; Enter in the password field signs
/// in.
///
/// - `server-name` (in): shown as "Sign in to {name}".
/// - `server-address` (in): the canonical address core checked, shown as
///   "Connected to {address}" before the password is typed. The address
///   check follows redirects to any host, so this is how the user sees where
///   the password will go.
/// - `username` (in-out): the username field's text; set it to prefill.
/// - `busy` (in): shows "Signing in…"; the fields and Sign in ignore input.
/// - `error` (in): a message under the form; empty shows none.
/// - `password-resets` (in): each change empties the password field and
///   focuses it.
/// - `submit(username, password)`: Enter in the password field or Sign in.
///   Not sent while `busy`.
/// - `back()`: Escape or Change server.
///
/// The password is masked on screen, hidden from the accessibility tree, and
/// held by no property: it exists only in the field and as `submit`'s second
/// argument, which [`bind`] puts straight into a `Password`.
pub use generated::LoginView;

/// The home screen's data and callbacks.
///
/// Under the header is a vertical stack of carousel rows: Continue Watching
/// and Next Up (16:9 thumbnails), Recently Added in Movies and in Shows
/// (2:3 posters) and Libraries (thumbnails), placeholders until Stage B of
/// #18 fills them from couchfin-core. The first card of the first row has
/// focus on arrival, so an Enter held from signing in can't press Sign out.
///
/// Left and Right step through the focused row and stop at its ends: the
/// current card stays in one slot near the left and the row slides under
/// it, with the card before it peeking in and the cards passed dimmed. Up
/// and Down (Shift+Tab and Tab) move between rows; each row keeps its own
/// card, and the page scrolls so the focused row always sits at the same
/// height. Down from the bottom row reaches Sign out and Up returns to the
/// card you left. A row with no cards is not shown and is stepped over.
///
/// - `server-name` (in): the title.
/// - `server-version` (in): shown as "Jellyfin {version}".
/// - `username` (in): shown as "Signed in as {username}".
/// - `notice` (in): a quiet note, such as a sign-in that couldn't be saved;
///   empty shows none.
/// - `rows` (in-out): the [`RowData`] to show, top to bottom. The screen
///   writes each row's `current` as the user moves along it.
/// - `card-activated(row, index)`: Enter on a card, with its row and its
///   place in the row (0 is the first of each). The page also shows
///   "Selected: {title}". [`bind`] only logs it, since there is no detail
///   page yet.
/// - `sign-out()`: the Sign out button.
/// - `back()`: Escape or Backspace.
pub use generated::HomeView;

/// A library's grid of 2:3 posters, built for thousands of items: only the
/// rows on screen, and the cards in them, exist (decision 015).
///
/// One focus target, on the first item on arrival. Left and Right step
/// through the items, Up and Down move a row, and the grid scrolls to keep
/// the focused card in view.
///
/// - `title` (in): the library's name, shown above the grid.
/// - `item-count` (in): how many items the library has in all.
/// - `items` (in): the [`CardData`] by place, 0 to `item-count` - 1. The grid
///   reads only places it draws, so a lazily loading model works; a place
///   without data shows an empty card.
/// - `items-needed(first, last)`: the places now on screen (`last` is
///   exclusive), whenever that changes, so core can page them in.
/// - `item-activated(index)`: Enter on a card, with its place in the library.
/// - `back()`: Escape or Backspace.
pub use generated::LibraryView;

/// A movie's detail page: backdrop, title, year, runtime, rating, overview
/// and a Play or Resume button that has focus. Playback arrives in Phase 4,
/// so pressing it also shows "Playback arrives soon".
///
/// - `title`, `year`, `runtime`, `rating`, `overview` (in): shown as given.
/// - `backdrop` (in): the picture; empty shows the placeholder.
/// - `resumable` (in): the button says Resume rather than Play.
/// - `play()`: Enter on the button.
/// - `back()`: Escape or Backspace.
pub use generated::MovieView;

/// A series' detail page: the movie page's header, a row of seasons and the
/// selected season's episodes.
///
/// Focus starts on the seasons: Left and Right choose one (firing
/// `season-selected`), Down or Enter enters the episodes. There Up and Down
/// step through them (the list scrolls) and Up from the first returns to the
/// seasons.
///
/// - `title`, `year`, `runtime`, `rating`, `overview`, `backdrop` (in): as
///   on [`MovieView`].
/// - `seasons` (in): the seasons' names.
/// - `selected-season` (in-out): which one is selected.
/// - `episodes` (in): the selected season's [`EpisodeData`].
/// - `season-selected(index)`: a different season was chosen, so core loads
///   its episodes.
/// - `episode-activated(index)`: Enter on an episode; the page also shows
///   "Playback arrives soon".
/// - `back()`: Escape or Backspace.
pub use generated::SeriesView;

/// One card of a row or the library grid: `title`, a thumbnail's `subtitle`
/// (such as "S2 · E4"), `progress` from 0 to 1 (0 shows no bar), and the
/// `image` and `blurred-image`, which are empty for the placeholder look.
pub use generated::CardData;

/// The shape of a card's picture: `Poster` is 2:3 and `Thumb` is 16:9.
pub use generated::CardShape;

/// One row on Home: its `title`, the `shape` of its cards, the `cards`, and
/// `current`, the row's focused card.
pub use generated::RowData;

/// One episode on the series page: its `number`, `title`, `runtime` as
/// shown, and whether it is `watched`.
pub use generated::EpisodeData;
