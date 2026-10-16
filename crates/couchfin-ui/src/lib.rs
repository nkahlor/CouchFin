//! Slint screens and components, with controller focus handling.
//!
//! Holds no business logic: it renders couchfin-core's state and sends
//! actions back. The markup lives in `ui/*.slint` and is compiled to Rust by
//! `build.rs`.
//!
//! - [`MainWindow`] is the one window Couchfin shows. Its [`Screen`] picks
//!   the screen, and each screen's data and callbacks sit in a Slint global:
//!   [`ConnectView`], [`AddressView`], [`LoginView`] and [`HomeView`].
//! - [`render`] shows a core [`LoginScreen`](couchfin_core::LoginScreen) in
//!   the window, and [`bind`] turns its callbacks into [`Command`]s: a
//!   [`LoginAction`](couchfin_core::LoginAction) for the flow, or quit.
//! - [`wording`] is every message the user reads about an error. An error's
//!   `Display` text is for the log only.
//!
//! Every screen works from the keyboard alone, with the same keys Phase 2's
//! controller input will send: Tab or Down moves to the next control,
//! Shift+Tab or Up to the previous one, Enter (or Space on a button) presses
//! it, and Escape asks to go back. Backspace deletes in a text field and
//! means back everywhere else. Showing a screen focuses its first control.
//! The design is in `docs/decisions/008-login-screens.md`.
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
pub mod wording;

mod generated {
    // Slint's generated code has no doc comments and uses `unwrap()`. The
    // public re-exports below carry the documentation instead.
    #![allow(missing_docs, clippy::unwrap_used)]
    slint::include_modules!();
}

pub use bind::{Command, bind, render};

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
/// - `Home`: signed in ([`HomeView`]).
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

/// The Phase 1 home screen's data and callbacks.
///
/// The page holds focus on arrival; Down or Tab moves to Sign out.
///
/// - `server-name` (in): the title.
/// - `server-version` (in): shown as "Jellyfin {version}".
/// - `username` (in): shown as "Signed in as {username}".
/// - `notice` (in): a quiet note, such as a sign-in that couldn't be saved;
///   empty shows none.
/// - `sign-out()`: the Sign out button.
/// - `back()`: Escape or Backspace.
pub use generated::HomeView;
