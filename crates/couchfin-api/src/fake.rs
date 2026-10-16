//! An in-memory Jellyfin server for testing code that uses [`JellyfinApi`]
//! without HTTP. Always compiled (no cargo feature): it is small, has no
//! dependencies of its own, and couchfin-core, couchfin-ui and the app's
//! tests can all use it without feature plumbing.

use std::future::{Future, ready};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::api::{JellyfinApi, Session};
use crate::dto::{AuthenticationResult, PublicSystemInfo, ServerCheck, User};
use crate::error::ApiError;
use crate::ids::{InvalidValueError, UserId};
use crate::secret::{AccessToken, Password};
use crate::server_url::ServerUrl;

/// A fake Jellyfin server at one address, with users, issued tokens and
/// scripted failures.
///
/// - Requests to any other [`ServerUrl`] fail with
///   [`ApiError::Unreachable`], except addresses added with
///   [`FakeJellyfin::with_redirect_from`]: there `check_server` succeeds and
///   returns the fake's own address as canonical, and the other calls fail
///   with [`ApiError::Redirected`], as the real client does.
/// - Logging in with a known user name (exact match) and password issues a
///   new token; anything else is [`ApiError::InvalidCredentials`].
/// - [`JellyfinApi::current_user`] accepts only issued tokens;
///   [`FakeJellyfin::revoke_tokens`] makes them [`ApiError::TokenRejected`].
/// - [`FakeJellyfin::fail_with`] makes every call return an error, for
///   testing timeouts and server errors.
///
/// Clones share the same state, so a test can keep a handle to script the
/// server while the code under test owns another.
///
/// # Examples
///
/// ```
/// use couchfin_api::fake::FakeJellyfin;
/// use couchfin_api::{ApiError, JellyfinApi, ServerUrl};
///
/// let server: ServerUrl = "http://jellyfin.lan:8096".parse()?;
/// let fake = FakeJellyfin::new(server.clone());
/// let runtime = tokio::runtime::Builder::new_current_thread().build()?;
///
/// assert!(runtime.block_on(fake.check_server(&server)).is_ok());
///
/// fake.fail_with(ApiError::Timeout);
/// let error = runtime.block_on(fake.check_server(&server));
/// assert_eq!(error, Err(ApiError::Timeout));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct FakeJellyfin {
    server: ServerUrl,
    state: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    info: PublicSystemInfo,
    users: Vec<FakeUser>,
    tokens: Vec<(AccessToken, UserId)>,
    issued: usize,
    failure: Option<ApiError>,
    redirects_from: Vec<ServerUrl>,
}

#[derive(Debug)]
struct FakeUser {
    user: User,
    password: Password,
}

impl FakeJellyfin {
    /// A server at `server` called "Fake Jellyfin", version 10.11.0, with no
    /// users.
    #[must_use]
    pub fn new(server: ServerUrl) -> Self {
        let info = PublicSystemInfo {
            server_name: "Fake Jellyfin".to_owned(),
            version: "10.11.0".to_owned(),
            id: fake_id("server", 0),
        };
        let state = State {
            info,
            users: Vec::new(),
            tokens: Vec::new(),
            issued: 0,
            failure: None,
            redirects_from: Vec::new(),
        };
        Self {
            server,
            state: Arc::new(Mutex::new(state)),
        }
    }

    /// Sets the name and version that public system info reports.
    #[must_use]
    pub fn with_server_info(self, server_name: &str, version: &str) -> Self {
        {
            let mut state = self.lock();
            server_name.clone_into(&mut state.info.server_name);
            version.clone_into(&mut state.info.version);
        }
        self
    }

    /// Adds a user who can log in with `password`.
    #[must_use]
    pub fn with_user(self, name: &str, password: &str) -> Self {
        {
            let mut state = self.lock();
            let id = fake_id("user", state.users.len());
            state.users.push(FakeUser {
                user: User {
                    id,
                    name: name.to_owned(),
                },
                password: Password::new(password),
            });
        }
        self
    }

    /// Makes `old` redirect to this fake's address, like a proxy that sends
    /// `http://` to `https://`.
    #[must_use]
    pub fn with_redirect_from(self, old: ServerUrl) -> Self {
        self.lock().redirects_from.push(old);
        self
    }

    /// The address this fake answers on.
    #[must_use]
    pub fn server(&self) -> &ServerUrl {
        &self.server
    }

    /// Makes every call fail with `error` until [`FakeJellyfin::clear_failure`].
    pub fn fail_with(&self, error: ApiError) {
        self.lock().failure = Some(error);
    }

    /// Stops failing calls.
    pub fn clear_failure(&self) {
        self.lock().failure = None;
    }

    /// Forgets every issued token, as if they were revoked or expired.
    pub fn revoke_tokens(&self) {
        self.lock().tokens.clear();
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock can only come from a failing test;
        // the state is still usable for reporting it.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Applies the scripted failure and the address check that every call
    /// starts with.
    fn answer(&self, server: &ServerUrl) -> Result<MutexGuard<'_, State>, ApiError> {
        let state = self.lock();
        if let Some(error) = &state.failure {
            return Err(error.clone());
        }
        if *server == self.server {
            Ok(state)
        } else if state.redirects_from.contains(server) {
            Err(ApiError::Redirected {
                server: Some(self.server.clone()),
            })
        } else {
            Err(ApiError::Unreachable {
                detail: format!("the fake server only answers on {}", self.server),
            })
        }
    }
}

/// A readable ID such as `fake-user-0`.
fn fake_id<T: TryFrom<String, Error = InvalidValueError>>(kind: &str, n: usize) -> T {
    T::try_from(format!("fake-{kind}-{n}")).expect("letters, digits and '-' are always a valid ID")
}

// The fake has nothing to wait for, so each trait method computes its answer
// here, when it is called, and returns it in a ready future.
impl FakeJellyfin {
    fn check(&self, address: &ServerUrl) -> Result<ServerCheck, ApiError> {
        let state = match self.answer(address) {
            // check_server follows the redirect to the canonical address.
            Err(ApiError::Redirected { .. }) => self.answer(&self.server)?,
            other => other?,
        };
        Ok(ServerCheck {
            server: self.server.clone(),
            info: state.info.clone(),
        })
    }

    fn log_in(
        &self,
        server: &ServerUrl,
        username: &str,
        password: &Password,
    ) -> Result<AuthenticationResult, ApiError> {
        let mut state = self.answer(server)?;
        let user = state
            .users
            .iter()
            .find(|u| u.user.name == username && u.password == *password)
            .map(|u| u.user.clone())
            .ok_or(ApiError::InvalidCredentials)?;
        state.issued += 1;
        let access_token: AccessToken = fake_id("token", state.issued);
        state.tokens.push((access_token.clone(), user.id.clone()));
        Ok(AuthenticationResult {
            user,
            access_token,
            server_id: state.info.id.clone(),
        })
    }

    fn user(&self, session: &Session) -> Result<User, ApiError> {
        let state = self.answer(session.server())?;
        let user_id = state
            .tokens
            .iter()
            .find(|(token, _)| token == session.token())
            .map(|(_, id)| id)
            .ok_or(ApiError::TokenRejected)?;
        state
            .users
            .iter()
            .map(|u| &u.user)
            .find(|u| u.id == *user_id)
            .cloned()
            .ok_or(ApiError::TokenRejected)
    }
}

impl JellyfinApi for FakeJellyfin {
    fn check_server(
        &self,
        address: &ServerUrl,
    ) -> impl Future<Output = Result<ServerCheck, ApiError>> + Send {
        ready(self.check(address))
    }

    fn authenticate_by_name(
        &self,
        server: &ServerUrl,
        username: &str,
        password: &Password,
    ) -> impl Future<Output = Result<AuthenticationResult, ApiError>> + Send {
        ready(self.log_in(server, username, password))
    }

    fn current_user(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<User, ApiError>> + Send {
        ready(self.user(session))
    }
}
