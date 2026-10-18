//! Wires couchfin-core's login and library flows to the window: the flows
//! run on tokio, the window on Slint's event loop on the main thread.
//!
//! `src/main.rs` builds the real pieces (the HTTP client, the settings file,
//! the keyring) and calls [`connect`]. `tests/` calls it with the same
//! client against wiremock.

mod gamepad;
mod library;

use std::sync::{Arc, Weak};

use couchfin_core::{
    CacheDir, JellyfinApi, LoginAction, LoginFlow, LoginScreen, SettingsStore, TokenStore,
};
use couchfin_ui::{Command, MainWindow, bind, render};
use library::Library;
use slint::ComponentHandle;
use tokio::runtime::Handle;

pub use gamepad::{connect_gamepads, connect_gamepads_with};

/// Connects `flow` to `window` and starts it, with the image cache in the
/// system cache directory. See [`connect_with`].
pub fn connect<A, S, T>(
    window: &MainWindow,
    flow: LoginFlow<A, S, T>,
    api: A,
    runtime: &Handle,
) -> Arc<LoginFlow<A, S, T>>
where
    A: JellyfinApi + Clone + 'static,
    S: SettingsStore + 'static,
    T: TokenStore + 'static,
{
    connect_with(window, flow, api, CacheDir::system(), runtime)
}

/// Connects `flow` to `window` and starts it.
///
/// - Every screen the flow publishes is cloned and rendered on the UI thread
///   with `slint::invoke_from_event_loop`. The flow calls its observer under
///   its lock, so the observer only hands the screen over.
/// - When the flow reaches Home, a `LibraryFlow` and an `ImageCache` are
///   built from `api` (a clone of the client the login flow uses, so one
///   connection pool) and `cache_dir`, and the browsing screens show them.
///   Leaving Home, or the server rejecting the token, drops them; the latter
///   sends `LoginAction::SessionExpired`. With no `cache_dir`, cards keep
///   their placeholders.
/// - Each action from the window is spawned on `runtime`, so the UI thread
///   never waits on the network.
/// - Back where there is nothing to go back to quits the event loop.
///
/// Call it on the UI thread, before running the event loop. The returned
/// flow is also kept alive by the window's callbacks.
pub fn connect_with<A, S, T>(
    window: &MainWindow,
    flow: LoginFlow<A, S, T>,
    api: A,
    cache_dir: Option<CacheDir>,
    runtime: &Handle,
) -> Arc<LoginFlow<A, S, T>>
where
    A: JellyfinApi + Clone + 'static,
    S: SettingsStore + 'static,
    T: TokenStore + 'static,
{
    let library = Library::new(window, api, cache_dir, runtime);
    let weak = window.as_weak();
    let flow = Arc::new_cyclic(|me: &Weak<LoginFlow<A, S, T>>| {
        let me = me.clone();
        let library = Arc::clone(&library);
        flow.on_change(move |screen| {
            let screen = screen.clone();
            let (window, me, library) = (weak.clone(), me.clone(), Arc::clone(&library));
            let shown = slint::invoke_from_event_loop(move || {
                let Some(window) = window.upgrade() else {
                    return;
                };
                render(&window, &screen);
                match (&screen, me.upgrade()) {
                    (LoginScreen::Home(_), Some(login)) => {
                        if let Some(session) = login.session() {
                            library.start(session);
                        }
                    }
                    _ => library.stop(),
                }
            });
            if let Err(error) = shown {
                // Only when the event loop has stopped: the app is quitting.
                tracing::debug!(%error, "dropped a screen update");
            }
        })
    });
    library.on_expired({
        let (login, runtime) = (Arc::downgrade(&flow), runtime.clone());
        move || {
            if let Some(login) = login.upgrade() {
                runtime.spawn(async move { login.dispatch(LoginAction::SessionExpired).await });
            }
        }
    });
    render(window, &flow.screen());

    bind(window, {
        let flow = Arc::clone(&flow);
        let runtime = runtime.clone();
        move |command| match command {
            Command::Login(action) => {
                let flow = Arc::clone(&flow);
                runtime.spawn(async move { flow.dispatch(action).await });
            }
            // The browsing screens send theirs through `Library`.
            Command::Library(_) => {}
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
