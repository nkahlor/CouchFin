//! The login flow: from startup to Home, as a state the UI renders and
//! actions it sends back.

mod screen;
mod steps;

pub use screen::{
    EnterAddress, Home, LogIn, LoginAction, LoginError, LoginScreen, Request, RestoreStatus,
    Restoring,
};

use std::sync::{Mutex, MutexGuard, PoisonError};

use couchfin_api::{AccessToken, JellyfinApi, ServerUrl};

use crate::settings::{Settings, SettingsStore};
use crate::tokens::{TokenStore, TokenStoreError};

type Observer = Box<dyn Fn(&LoginScreen) + Send + Sync>;

/// Runs the login flow: restores a stored session at startup, checks the
/// address the user types, logs in, and persists the session.
///
/// The UI reads [`LoginFlow::screen`] (or is told of every change through
/// [`LoginFlow::on_change`]) and sends [`LoginAction`]s with
/// [`LoginFlow::dispatch`]. Its methods take `&self` and return `Send`
/// futures, so the app shares one flow in an `Arc` and spawns each call on
/// its tokio runtime; the UI thread never waits on the network. When a newer
/// action replaces the screen while a request is still running (for example
/// [`LoginAction::ChangeServer`] during a check), the old request's result is
/// dropped.
///
/// The settings and token stores are called from these futures. Their calls
/// block, but briefly (a file, or a D-Bus call with a timeout).
///
/// # Examples
///
/// ```
/// use couchfin_core::fake::{FakeJellyfin, MemorySettings, MemoryTokens};
/// use couchfin_core::{LoginAction, LoginFlow, LoginScreen, Password, Settings};
///
/// let fake = FakeJellyfin::new("http://jellyfin.lan:8096".parse()?)
///     .with_server_info("Living Room", "10.11.2")
///     .with_user("alice", "hunter2");
/// let (settings, tokens) = (MemorySettings::new(), MemoryTokens::new());
///
/// let runtime = tokio::runtime::Builder::new_current_thread().build()?;
/// runtime.block_on(async {
///     let flow = LoginFlow::new(
///         fake.clone(),
///         Settings::load_or_create(&settings),
///         settings.clone(),
///         tokens.clone(),
///     );
///     flow.start().await; // first run: asks for the address
///     flow.dispatch(LoginAction::SubmitAddress("jellyfin.lan:8096".into())).await;
///     flow.dispatch(LoginAction::SubmitLogin {
///         username: "alice".into(),
///         password: Password::new("hunter2"),
///     })
///     .await;
///     assert!(matches!(flow.screen(), LoginScreen::Home(_)));
/// });
///
/// // A restart with the same stores skips the login.
/// runtime.block_on(async {
///     let flow = LoginFlow::new(fake, Settings::load_or_create(&settings), settings, tokens);
///     flow.start().await;
///     let LoginScreen::Home(home) = flow.screen() else { panic!() };
///     assert_eq!(home.info.server_name, "Living Room");
///     assert_eq!(home.user.name, "alice");
/// });
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct LoginFlow<A, S, T> {
    api: A,
    settings_store: S,
    tokens: T,
    state: Mutex<State>,
    observer: Option<Observer>,
}

struct State {
    screen: LoginScreen,
    settings: Settings,
    /// Bumped on every new screen started by an action, so a request can
    /// tell whether its result is still wanted.
    generation: u64,
}

/// Identifies the action a request belongs to; see [`State::generation`].
#[derive(Clone, Copy)]
struct Ticket(u64);

impl<A: JellyfinApi, S: SettingsStore, T: TokenStore> LoginFlow<A, S, T> {
    /// A flow on [`LoginScreen::Starting`]. `settings` are the loaded
    /// settings (from [`Settings::load_or_create`] on `settings_store`); the
    /// flow saves changes to them back to `settings_store`, and the token to
    /// `tokens`.
    #[must_use]
    pub fn new(api: A, settings: Settings, settings_store: S, tokens: T) -> Self {
        Self {
            api,
            settings_store,
            tokens,
            state: Mutex::new(State {
                screen: LoginScreen::Starting,
                settings,
                generation: 0,
            }),
            observer: None,
        }
    }

    /// Calls `observer` with every new screen, in order. It runs while the
    /// flow is locked, so it must be quick and must not call back into the
    /// flow: clone the screen and hand it to the UI thread (with Slint,
    /// `slint::invoke_from_event_loop`).
    #[must_use]
    pub fn on_change(mut self, observer: impl Fn(&LoginScreen) + Send + Sync + 'static) -> Self {
        self.observer = Some(Box::new(observer));
        self
    }

    /// The current screen.
    #[must_use]
    pub fn screen(&self) -> LoginScreen {
        self.lock().screen.clone()
    }

    /// Leaves [`LoginScreen::Starting`]. With a stored server and token it
    /// checks the session and goes to Home if the token still works. With
    /// only a server it checks that server and shows its login form.
    /// Otherwise it asks for an address. Does nothing after the first call.
    pub async fn start(&self) {
        self.restore(|screen| matches!(screen, LoginScreen::Starting))
            .await;
    }

    /// Handles one action from the UI. Actions that don't fit the current
    /// screen are ignored (see [`LoginAction`]). Returns when any request
    /// the action started has finished.
    pub async fn dispatch(&self, action: LoginAction) {
        match action {
            LoginAction::SubmitAddress(address) => self.submit_address(address).await,
            LoginAction::SubmitLogin { username, password } => {
                self.submit_login(username, password).await;
            }
            LoginAction::Retry => self.restore(steps::restore_failed).await,
            LoginAction::ChangeServer => self.change_server(),
            LoginAction::LogOut => self.log_out(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // The state is replaced whole, never left half-updated, so it is
        // still consistent after a panic elsewhere.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn publish(&self, screen: &LoginScreen) {
        if let Some(observer) = &self.observer {
            observer(screen);
        }
    }

    /// Moves to a new screen if `next` allows it from the current one,
    /// returning a ticket for the request that screen starts.
    fn begin<R>(
        &self,
        next: impl FnOnce(&LoginScreen) -> Option<(LoginScreen, R)>,
    ) -> Option<(Ticket, R)> {
        let mut state = self.lock();
        let (screen, extra) = next(&state.screen)?;
        state.generation += 1;
        state.screen = screen;
        self.publish(&state.screen);
        Some((Ticket(state.generation), extra))
    }

    /// Shows a request's result, unless a newer action has moved on.
    fn finish(&self, ticket: Ticket, screen: LoginScreen) {
        let mut state = self.lock();
        if state.generation == ticket.0 {
            state.screen = screen;
            self.publish(&state.screen);
        }
    }

    fn is_current(&self, ticket: Ticket) -> bool {
        self.lock().generation == ticket.0
    }

    fn settings(&self) -> Settings {
        self.lock().settings.clone()
    }

    /// Stores `server` (and `username`) if the request is still current.
    /// Moving to another server forgets the old server's token.
    fn remember(&self, ticket: Ticket, server: &ServerUrl, username: Option<&str>) {
        let (settings, previous) = {
            let mut state = self.lock();
            if state.generation != ticket.0 {
                return;
            }
            let previous = state.settings.server.replace(server.clone());
            if let Some(username) = username {
                state.settings.username = Some(username.to_owned());
            }
            (state.settings.clone(), previous)
        };
        if let Some(previous) = previous.filter(|previous| previous != server) {
            self.forget_token(&previous);
        }
        if let Err(error) = self.settings_store.save(&settings) {
            tracing::warn!(%error, "couldn't save settings");
        }
    }

    /// A token that can't be read counts as none: the user logs in again.
    fn load_token(&self, server: &ServerUrl) -> Option<AccessToken> {
        self.tokens.load(server).unwrap_or_else(|error| {
            tracing::warn!(%error, "couldn't read the stored token");
            None
        })
    }

    /// Stores the token; the error, if any, goes on the Home screen.
    fn save_token(&self, server: &ServerUrl, token: &AccessToken) -> Option<TokenStoreError> {
        let error = self.tokens.save(server, token).err()?;
        tracing::warn!(%error, "couldn't store the token; the next start will ask to log in");
        Some(error)
    }

    fn forget_token(&self, server: &ServerUrl) {
        if let Err(error) = self.tokens.delete(server) {
            tracing::warn!(%error, "couldn't delete the stored token");
        }
    }
}
