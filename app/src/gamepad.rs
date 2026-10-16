//! Wires game controllers to the window: each [`Input`] from the gamepad
//! thread becomes an [`Intent`] and is delivered on the UI thread.

use couchfin_gamepad::{Gamepads, Input, StartError};
use couchfin_ui::{Intent, MainWindow, navigate};
use slint::ComponentHandle;

/// Starts the gamepads with `start`, which is given the callback to pass as
/// `on_input` to [`Gamepads::start`] or [`Gamepads::start_sdl`], and keeps
/// the window driven by them for as long as the returned value lives.
///
/// - `navigate` must run on the Slint event-loop thread, so each intent is
///   sent there with `slint::invoke_from_event_loop` and a weak window
///   handle; the gamepad thread never touches the window.
/// - If the gamepads can't start (no SDL, say) it logs a warning and
///   returns `None`: the app carries on with the keyboard alone.
///
/// Call it once the window exists. Hold the result until the event loop
/// ends, then drop it so the gamepad thread stops.
pub fn connect_gamepads(
    window: &MainWindow,
    start: impl FnOnce(Box<dyn FnMut(Input) + Send>) -> Result<Gamepads, StartError>,
) -> Option<Gamepads> {
    let weak = window.as_weak();
    let deliver = move |input: Input| {
        let Some(intent) = intent(input) else { return };
        let weak = weak.clone();
        let sent = slint::invoke_from_event_loop(move || {
            if let Some(window) = weak.upgrade() {
                navigate(&window, intent);
            }
        });
        if let Err(error) = sent {
            // Only when the event loop has stopped: the app is quitting.
            tracing::debug!(%error, "dropped a controller press");
        }
    };
    match start(Box::new(deliver)) {
        Ok(gamepads) => Some(gamepads),
        Err(error) => {
            tracing::warn!(%error, "controllers unavailable, using the keyboard only");
            None
        }
    }
}

/// The intent a controller input stands for, or `None` for an input that
/// does nothing yet. No wildcard arm: a new `Input` must be decided here.
fn intent(input: Input) -> Option<Intent> {
    match input {
        Input::Up => Some(Intent::Up),
        Input::Down => Some(Intent::Down),
        Input::Left => Some(Intent::Left),
        Input::Right => Some(Intent::Right),
        Input::Select => Some(Intent::Activate),
        Input::Back => Some(Intent::Back),
        // A later task (a context menu) will use Start.
        Input::Menu => None,
    }
}
