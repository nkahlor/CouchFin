//! Slint screens and components, with controller focus handling.
//!
//! Holds no business logic: it renders state from `couchfin-core` and sends
//! actions back. The markup lives in `ui/*.slint` and is compiled to Rust by
//! `build.rs`.
//!
//! [`MainWindow`] is the one window Couchfin shows. Its [`Screen`] property
//! picks the screen, and each screen has a Slint global holding what it
//! renders and the callbacks it sends: [`AddressScreen`], [`LoginScreen`] and
//! [`HomeScreen`]. Reach a global through
//! [`ComponentHandle::global`](slint::ComponentHandle::global). The UI owns
//! every word the user reads; callers pass data, or a message the UI worded,
//! never an error's `Display` text.
//!
//! Every screen works from the keyboard alone, with the same keys Phase 2's
//! controller input will send: Tab or Down moves to the next control,
//! Shift+Tab or Up to the previous one, Enter (or Space on a button) presses
//! it, and Escape asks to go back. Backspace deletes in a text field and
//! means back everywhere else. Showing a screen focuses its first control.

mod generated {
    // Slint's generated code has no doc comments and uses `unwrap()`. The
    // public re-exports below carry the documentation instead.
    #![allow(missing_docs, clippy::unwrap_used)]
    slint::include_modules!();
}

/// The Phase 0 placeholder screen: "Hello" in the middle of a dark window.
///
/// Pressing Back (Escape or Backspace on a keyboard, until controller input
/// arrives in Phase 2) invokes the `quit-requested` callback, which the app
/// wires to quitting. The component only asks; it never quits by itself.
///
/// It is a Slint component, so use [`slint::ComponentHandle`] to show it and
/// reach its [`slint::Window`], and `on_quit_requested` to handle Back.
///
/// # Examples
///
/// ```
/// use std::cell::Cell;
/// use std::rc::Rc;
///
/// use couchfin_ui::HelloWindow;
/// use slint::ComponentHandle;
/// use slint::platform::{Key, WindowEvent};
///
/// # i_slint_backend_testing::init_no_event_loop();
/// let window = HelloWindow::new()?;
/// let quit = Rc::new(Cell::new(false));
/// let flag = Rc::clone(&quit);
/// window.on_quit_requested(move || flag.set(true));
/// window.show()?;
///
/// window.window().dispatch_event(WindowEvent::KeyPressed {
///     text: Key::Escape.into(),
/// });
/// assert!(quit.get());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub use generated::HelloWindow;

/// The one window Couchfin shows: a dark, full-screen backdrop with the
/// current [`Screen`] on it.
///
/// Set the screen with `set_screen`. Only that screen exists; switching
/// destroys the previous one, along with anything typed into it, such as a
/// password, and focuses the new screen's first control.
///
/// # Examples
///
/// ```
/// use std::cell::RefCell;
/// use std::rc::Rc;
///
/// use couchfin_ui::{AddressScreen, MainWindow, Screen};
/// use slint::ComponentHandle;
/// use slint::platform::{Key, WindowEvent};
///
/// # i_slint_backend_testing::init_no_event_loop();
/// let window = MainWindow::new()?;
/// window.set_screen(Screen::Address);
/// let submitted = Rc::new(RefCell::new(String::new()));
/// let sink = Rc::clone(&submitted);
/// window
///     .global::<AddressScreen>()
///     .on_submit(move |address| *sink.borrow_mut() = address.into());
/// window.show()?;
///
/// // The address field has focus; type into it and press Enter.
/// for key in ['j', 'f', Key::Return.into()] {
///     let text = slint::SharedString::from(key);
///     window.window().dispatch_event(WindowEvent::KeyPressed { text });
/// }
/// assert_eq!(*submitted.borrow(), "jf");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub use generated::MainWindow;

/// Which screen [`MainWindow`] shows.
///
/// - `Address`: type the server's address ([`AddressScreen`]).
/// - `Login`: sign in to the server the address check found ([`LoginScreen`]).
/// - `Home`: signed in ([`HomeScreen`]).
pub use generated::Screen;

/// The server address screen's data and callbacks.
///
/// Focus order: the "Server address" field, then the Connect button.
///
/// - `address` (in-out): the field's text. Typing updates it; set it to
///   prefill a previous address.
/// - `busy` (in): while true, shows "Checking the server…", and the field
///   and Connect ignore input.
/// - `error` (in): a message shown under the form; empty shows none.
/// - `submit(address)`: Enter in the field or Connect, with the address as
///   typed. Not sent while `busy`.
/// - `back()`: Escape. This is the first screen, so the app quits.
///
/// # Examples
///
/// ```
/// use couchfin_ui::{AddressScreen, MainWindow};
/// use slint::ComponentHandle;
///
/// # i_slint_backend_testing::init_no_event_loop();
/// let window = MainWindow::new()?;
/// let screen = window.global::<AddressScreen>();
/// screen.set_address("http://jellyfin.lan:8096".into());
/// screen.set_busy(true);
/// screen.on_back(|| { /* quit */ });
/// # Ok::<(), slint::PlatformError>(())
/// ```
pub use generated::AddressScreen;

/// The sign-in screen's data and callbacks.
///
/// Focus order: "Username", "Password", Sign in, Change server. Enter in the
/// username field moves to the password; Enter in the password field signs
/// in.
///
/// - `server-name` (in): shown as "Sign in to {name}".
/// - `server-address` (in): the canonical address from
///   `ServerCheck::server`, shown as "Connected to {address}" before the
///   password is typed. The address check follows redirects to any host, so
///   this is how the user sees where the password will go.
/// - `username` (in-out): the username field's text; set it to prefill.
/// - `busy` (in): while true, shows "Signing in…", and the fields and Sign in
///   ignore input.
/// - `error` (in): a message shown under the form; empty shows none.
/// - `submit(username, password)`: Enter in the password field or Sign in.
///   Not sent while `busy`.
/// - `back()`: Escape or Change server, to type another address.
///
/// The password is masked on screen, hidden from the accessibility tree, and
/// held by no property: it exists only in the field and as `submit`'s second
/// argument. Wrap it in `couchfin_api::Password` straight away and never log
/// it.
///
/// # Examples
///
/// ```
/// use couchfin_ui::{LoginScreen, MainWindow, Screen};
/// use slint::ComponentHandle;
///
/// # i_slint_backend_testing::init_no_event_loop();
/// let window = MainWindow::new()?;
/// window.set_screen(Screen::Login);
/// let screen = window.global::<LoginScreen>();
/// screen.set_server_name("Living Room".into());
/// screen.set_server_address("https://jellyfin.example/".into());
/// screen.on_submit(|username, password| {
///     // Hand both to core; the password goes straight into a `Password`.
///     # let _ = (username, password);
/// });
/// # Ok::<(), slint::PlatformError>(())
/// ```
pub use generated::LoginScreen;

/// The Phase 1 home screen's data and callbacks: which server, which version,
/// and who is signed in.
///
/// - `server-name` (in): shown as the title.
/// - `server-version` (in): shown as "Jellyfin {version}".
/// - `username` (in): shown as "Signed in as {username}".
/// - `back()`: Escape or Backspace.
///
/// # Examples
///
/// ```
/// use couchfin_ui::{HomeScreen, MainWindow, Screen};
/// use slint::ComponentHandle;
///
/// # i_slint_backend_testing::init_no_event_loop();
/// let window = MainWindow::new()?;
/// window.set_screen(Screen::Home);
/// let screen = window.global::<HomeScreen>();
/// screen.set_server_name("Living Room".into());
/// screen.set_server_version("10.11.2".into());
/// screen.set_username("alice".into());
/// # Ok::<(), slint::PlatformError>(())
/// ```
pub use generated::HomeScreen;
