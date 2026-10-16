//! Wires couchfin-core's login flow to the window: the flow runs on tokio,
//! the window on Slint's event loop on the main thread.
//!
//! `src/main.rs` builds the real pieces (the HTTP client, the settings file,
//! the keyring) and calls [`connect`]. `tests/` calls it with the same
//! client against wiremock.

use std::sync::Arc;

use couchfin_core::{JellyfinApi, LoginFlow, SettingsStore, TokenStore};
use couchfin_ui::{Command, MainWindow, bind, render};
use slint::ComponentHandle;
use tokio::runtime::Handle;

/// Connects `flow` to `window` and starts it.
///
/// - Every screen the flow publishes is cloned and rendered on the UI thread
///   with `slint::invoke_from_event_loop`. The flow calls its observer under
///   its lock, so the observer only hands the screen over.
/// - Each action from the window is spawned on `runtime`, so the UI thread
///   never waits on the network.
/// - Back where there is nothing to go back to quits the event loop.
///
/// Call it on the UI thread, before running the event loop. The returned
/// flow is also kept alive by the window's callbacks.
pub fn connect<A, S, T>(
    window: &MainWindow,
    flow: LoginFlow<A, S, T>,
    runtime: &Handle,
) -> Arc<LoginFlow<A, S, T>>
where
    A: JellyfinApi + 'static,
    S: SettingsStore + 'static,
    T: TokenStore + 'static,
{
    let weak = window.as_weak();
    let flow = Arc::new(flow.on_change(move |screen| {
        let screen = screen.clone();
        let window = weak.clone();
        let shown = slint::invoke_from_event_loop(move || {
            if let Some(window) = window.upgrade() {
                render(&window, &screen);
            }
        });
        if let Err(error) = shown {
            // Only when the event loop has stopped: the app is quitting.
            tracing::debug!(%error, "dropped a screen update");
        }
    }));
    render(window, &flow.screen());

    bind(window, {
        let flow = Arc::clone(&flow);
        let runtime = runtime.clone();
        move |command| match command {
            Command::Login(action) => {
                let flow = Arc::clone(&flow);
                runtime.spawn(async move { flow.dispatch(action).await });
            }
            Command::Quit => {
                // Fails only when the event loop has already stopped.
                let _ = slint::quit_event_loop();
            }
        }
    });

    runtime.spawn({
        let flow = Arc::clone(&flow);
        async move { flow.start().await }
    });
    flow
}
