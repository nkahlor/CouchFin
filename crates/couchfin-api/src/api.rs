//! The `JellyfinApi` trait that couchfin-core depends on, so it can run
//! against [`JellyfinClient`](crate::JellyfinClient) in the app and
//! [`FakeJellyfin`](crate::fake::FakeJellyfin) in tests.

use std::future::Future;

use crate::dto::{AuthenticationResult, PublicSystemInfo, User};
use crate::error::ApiError;
use crate::secret::{AccessToken, Password};
use crate::server_url::ServerUrl;

/// A logged-in connection: the server and the token it issued.
///
/// This is what couchfin-core persists between runs, so that after a restart
/// it can call [`JellyfinApi::current_user`] and skip the login screen.
/// `Debug` shows the server but redacts the token.
///
/// # Examples
///
/// ```
/// use couchfin_api::{AccessToken, Session};
///
/// let session = Session::new(
///     "jellyfin.lan:8096".parse()?,
///     AccessToken::new("0123456789abcdef0123456789abcdef")?,
/// );
/// assert_eq!(session.server().as_str(), "http://jellyfin.lan:8096/");
/// assert!(!format!("{session:?}").contains("0123456789abcdef"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    server: ServerUrl,
    token: AccessToken,
}

impl Session {
    /// Pairs a server with a token it issued, either from a fresh login
    /// ([`AuthenticationResult::access_token`]) or from storage.
    #[must_use]
    pub fn new(server: ServerUrl, token: AccessToken) -> Self {
        Self { server, token }
    }

    /// The server this session belongs to.
    #[must_use]
    pub fn server(&self) -> &ServerUrl {
        &self.server
    }

    /// The access token.
    #[must_use]
    pub fn token(&self) -> &AccessToken {
        &self.token
    }
}

/// The Jellyfin endpoints Couchfin uses, so callers can be tested against a
/// fake instead of a server.
///
/// The methods return `impl Future + Send` rather than being `async fn`, so
/// the futures can run on tokio's multi-threaded runtime. That makes the
/// trait unusable as `dyn JellyfinApi`; take it as a generic parameter
/// instead (`struct App<A: JellyfinApi>`). Implementations are cheap to
/// clone and share state between clones.
///
/// # Examples
///
/// Code written against the trait runs the same on the fake as on the real
/// client:
///
/// ```
/// use couchfin_api::fake::FakeJellyfin;
/// use couchfin_api::{ApiError, JellyfinApi, Password, ServerUrl, Session};
///
/// async fn log_in<A: JellyfinApi>(
///     api: &A,
///     server: &ServerUrl,
///     password: &Password,
/// ) -> Result<String, ApiError> {
///     let info = api.public_system_info(server).await?;
///     let login = api.authenticate_by_name(server, "alice", password).await?;
///     let session = Session::new(server.clone(), login.access_token);
///     let me = api.current_user(&session).await?;
///     Ok(format!("{} on {}", me.name, info.server_name))
/// }
///
/// let server: ServerUrl = "http://jellyfin.lan:8096".parse()?;
/// let fake = FakeJellyfin::new(server.clone())
///     .with_server_info("Living Room", "10.11.2")
///     .with_user("alice", "hunter2");
///
/// let runtime = tokio::runtime::Builder::new_current_thread().build()?;
/// let greeting = runtime.block_on(log_in(&fake, &server, &Password::new("hunter2")))?;
/// assert_eq!(greeting, "alice on Living Room");
///
/// let wrong = runtime.block_on(log_in(&fake, &server, &Password::new("nope")));
/// assert_eq!(wrong, Err(ApiError::InvalidCredentials));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait JellyfinApi: Send + Sync {
    /// `GET /System/Info/Public`: checks that `server` is a Jellyfin server
    /// and returns its name, version and ID. Needs no login.
    ///
    /// # Errors
    ///
    /// [`ApiError::Unreachable`] or [`ApiError::Timeout`] if the server can't
    /// be reached, [`ApiError::NotJellyfin`] if the reply isn't Jellyfin's
    /// (including a 404), [`ApiError::Status`] for other HTTP errors.
    fn public_system_info(
        &self,
        server: &ServerUrl,
    ) -> impl Future<Output = Result<PublicSystemInfo, ApiError>> + Send;

    /// `POST /Users/AuthenticateByName`: logs in and returns the user and a
    /// new access token. The server registers this device (by its
    /// [`DeviceId`](crate::DeviceId) and the name
    /// [`DEVICE_NAME`](crate::DEVICE_NAME)) under Dashboard > Devices.
    ///
    /// # Errors
    ///
    /// [`ApiError::InvalidCredentials`] for a wrong username or password,
    /// otherwise as for [`JellyfinApi::public_system_info`].
    fn authenticate_by_name(
        &self,
        server: &ServerUrl,
        username: &str,
        password: &Password,
    ) -> impl Future<Output = Result<AuthenticationResult, ApiError>> + Send;

    /// `GET /Users/Me`: the user a session belongs to. A success means the
    /// stored token is still valid.
    ///
    /// # Errors
    ///
    /// [`ApiError::TokenRejected`] if the server no longer accepts the
    /// token, so the user must log in again; otherwise as for
    /// [`JellyfinApi::public_system_info`].
    fn current_user(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<User, ApiError>> + Send;
}
