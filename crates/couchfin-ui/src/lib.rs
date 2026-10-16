//! Slint screens and components, with controller focus handling.
//!
//! Holds no business logic: it renders state from `couchfin-core` and sends
//! actions back. The markup lives in `ui/*.slint` and is compiled to Rust by
//! `build.rs`.

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
