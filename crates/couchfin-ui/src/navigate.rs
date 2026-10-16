//! Navigation from outside the window: a controller's D-pad and buttons
//! arrive as [`Intent`]s, and [`navigate`] feeds each one in as its key.
//!
//! Sending the key, rather than calling into the screens, keeps one path
//! for both kinds of input: `ui/widgets/keys.slint` decides what every key
//! means, and each control handles it, so focus logic exists once
//! (`docs/decisions/010-gamepad-input.md` and `011-home-row.md`).

use slint::ComponentHandle;
use slint::SharedString;
use slint::platform::{Key, WindowEvent};

use crate::generated::MainWindow;

/// A navigation step, from the keyboard or a controller.
///
/// Each one stands for a key, and does whatever that key does where focus
/// is. In a text field, Left and Right move the cursor, as the arrow keys
/// do; Up and Down still move between controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intent {
    /// The up arrow: the control above, or the previous one in a form.
    Up,
    /// The down arrow: the control below, or the next one in a form.
    Down,
    /// The left arrow: the previous card in a row, or the cursor in a field.
    Left,
    /// The right arrow: the next card in a row, or the cursor in a field.
    Right,
    /// Enter: press the focused control.
    Activate,
    /// Escape: go back, or quit where there is nothing to go back to.
    Back,
}

impl Intent {
    fn key(self) -> Key {
        match self {
            Intent::Up => Key::UpArrow,
            Intent::Down => Key::DownArrow,
            Intent::Left => Key::LeftArrow,
            Intent::Right => Key::RightArrow,
            Intent::Activate => Key::Return,
            Intent::Back => Key::Escape,
        }
    }
}

/// Feeds `intent` to whatever has focus in `window`, exactly as the matching
/// key would (arrow keys, Enter, Escape). Call it on the Slint event-loop thread.
///
/// It presses and releases the key at once, so a held D-pad repeats by
/// calling this again, as a keyboard's auto-repeat sends more presses.
///
/// # Examples
///
/// ```
/// use std::cell::RefCell;
/// use std::rc::Rc;
///
/// use couchfin_core::LoginScreen;
/// use couchfin_ui::{Command, Intent, MainWindow, bind, navigate, render};
/// use slint::ComponentHandle;
///
/// # i_slint_backend_testing::init_no_event_loop();
/// let window = MainWindow::new()?;
/// let quit = Rc::new(RefCell::new(false));
/// let seen = Rc::clone(&quit);
/// bind(&window, move |command| {
///     *seen.borrow_mut() = matches!(command, Command::Quit);
/// });
/// render(&window, &LoginScreen::Starting);
/// window.show()?;
///
/// // The B button: Back while starting up quits, as Escape does.
/// navigate(&window, Intent::Back);
/// assert!(*quit.borrow());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn navigate(window: &MainWindow, intent: Intent) {
    let text: SharedString = intent.key().into();
    let window = window.window();
    window.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    window.dispatch_event(WindowEvent::KeyReleased { text });
}
