//! The `JellyfinApi` trait that couchfin-core depends on, so it can run
//! against [`JellyfinClient`](crate::JellyfinClient) in the app and
//! [`FakeJellyfin`](crate::fake::FakeJellyfin) in tests.

use std::future::Future;

use crate::dto::{AuthenticationResult, ServerCheck, User};
use crate::error::ApiError;
use crate::ids::ItemId;
use crate::image::{Image, ImageRequest};
use crate::item::Item;
use crate::library::{Library, Page};
use crate::query::ItemsQuery;
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
///     typed: &ServerUrl,
///     password: &Password,
/// ) -> Result<String, ApiError> {
///     // Check first, then use the canonical address for everything else.
///     let check = api.check_server(typed).await?;
///     let login = api.authenticate_by_name(&check.server, "alice", password).await?;
///     let session = Session::new(check.server.clone(), login.access_token);
///     let me = api.current_user(&session).await?;
///     Ok(format!("{} on {} at {}", me.name, check.info.server_name, check.server))
/// }
///
/// // The server moved behind an HTTPS proxy; the old address redirects.
/// let server: ServerUrl = "https://jellyfin.lan".parse()?;
/// let typed: ServerUrl = "jellyfin.lan".parse()?;
/// let fake = FakeJellyfin::new(server)
///     .with_redirect_from(typed.clone())
///     .with_server_info("Living Room", "10.11.2")
///     .with_user("alice", "hunter2");
///
/// let runtime = tokio::runtime::Builder::new_current_thread().build()?;
/// let greeting = runtime.block_on(log_in(&fake, &typed, &Password::new("hunter2")))?;
/// assert_eq!(greeting, "alice on Living Room at https://jellyfin.lan/");
///
/// let wrong = runtime.block_on(log_in(&fake, &typed, &Password::new("nope")));
/// assert_eq!(wrong, Err(ApiError::InvalidCredentials));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub trait JellyfinApi: Send + Sync {
    /// `GET /System/Info/Public`: checks that `address` is a Jellyfin server
    /// and returns its name, version and ID, plus its canonical address.
    /// Needs no login.
    ///
    /// This is the only call that follows redirects (up to 10), because it
    /// sends no password or token. The canonical address is where the
    /// redirects end, minus `System/Info/Public`, so it keeps a reverse-proxy
    /// base path. Call this first and use [`ServerCheck::server`] from then
    /// on; the other calls refuse to follow redirects.
    ///
    /// # Errors
    ///
    /// [`ApiError::Unreachable`] or [`ApiError::Timeout`] if the server can't
    /// be reached; [`ApiError::NotJellyfin`] if the reply isn't Jellyfin's
    /// (including a 404, a redirect loop, or a redirect away from the API);
    /// [`ApiError::InsecureRedirect`] for a redirect from `https` to `http`;
    /// [`ApiError::Status`] for other HTTP errors.
    fn check_server(
        &self,
        address: &ServerUrl,
    ) -> impl Future<Output = Result<ServerCheck, ApiError>> + Send;

    /// `POST /Users/AuthenticateByName`: logs in and returns the user and a
    /// new access token. The server registers this device (by its
    /// [`DeviceId`](crate::DeviceId) and the name
    /// [`DEVICE_NAME`](crate::DEVICE_NAME)) under Dashboard > Devices.
    ///
    /// `server` should come from [`JellyfinApi::check_server`].
    ///
    /// # Errors
    ///
    /// [`ApiError::InvalidCredentials`] for a wrong username or password;
    /// [`ApiError::Redirected`] if the server answers with a redirect, which
    /// is never followed; otherwise as for [`JellyfinApi::check_server`].
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
    /// token, so the user must log in again; [`ApiError::Redirected`] if the
    /// server answers with a redirect (run [`JellyfinApi::check_server`]
    /// again: the address has changed); otherwise as for
    /// [`JellyfinApi::check_server`].
    fn current_user(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<User, ApiError>> + Send;

    /// `GET /UserViews`: the user's libraries, in the server's order.
    ///
    /// Every library call below acts for the user the session's token
    /// belongs to (no `userId` is sent: Jellyfin falls back to the
    /// authenticated user, user data included). It fails with
    /// [`ApiError::TokenRejected`] on a 401 and [`ApiError::Redirected`] on a
    /// redirect, like [`JellyfinApi::current_user`]. Replies that aren't Jellyfin's are
    /// [`ApiError::NotJellyfin`].
    ///
    /// # Errors
    ///
    /// As described above, plus the connection errors of
    /// [`JellyfinApi::check_server`].
    fn libraries(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<Vec<Library>, ApiError>> + Send;

    /// `GET /Items`: one page of items sorted by name (`sortBy=SortName`),
    /// with the size of the whole list. This is what the library grid pages
    /// through.
    ///
    /// # Errors
    ///
    /// As for [`JellyfinApi::libraries`].
    ///
    /// # Examples
    ///
    /// ```
    /// use couchfin_api::fake::FakeJellyfin;
    /// use couchfin_api::{ItemsQuery, JellyfinApi, Password, Session};
    ///
    /// # tokio::runtime::Builder::new_current_thread().build()?.block_on(async {
    /// let server = "http://jellyfin.lan:8096".parse()?;
    /// let api = FakeJellyfin::new(server).with_user("alice", "hunter2").with_generated_library("Movies", 2_000);
    /// let login = api.authenticate_by_name(api.server(), "alice", &Password::new("hunter2")).await?;
    /// let session = Session::new(api.server().clone(), login.access_token);
    /// let library = api.library_named("Movies").ok_or("no such library")?;
    ///
    /// let query = ItemsQuery::default().in_parent(library.id).recursive().page(100, 50);
    /// let page = api.items(&session, &query).await?;
    /// assert_eq!((page.items.len(), page.total, page.start_index), (50, 2_000, 100));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// # })?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    fn items(
        &self,
        session: &Session,
        query: &ItemsQuery,
    ) -> impl Future<Output = Result<Page<Item>, ApiError>> + Send;

    /// `GET /Items/{id}`: one item's details, including its overview.
    ///
    /// # Errors
    ///
    /// [`ApiError::ItemNotFound`] if there is no such item; otherwise as for
    /// [`JellyfinApi::libraries`].
    fn item(
        &self,
        session: &Session,
        id: &ItemId,
    ) -> impl Future<Output = Result<Item, ApiError>> + Send;

    /// `GET /UserItems/Resume`: the user's unfinished videos, most recently
    /// watched first (Continue Watching). At most `limit` items.
    ///
    /// # Errors
    ///
    /// As for [`JellyfinApi::libraries`].
    fn resume(
        &self,
        session: &Session,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send;

    /// `GET /Shows/NextUp`: the next unwatched episode of each series the
    /// user is following. At most `limit` items.
    ///
    /// # Errors
    ///
    /// As for [`JellyfinApi::libraries`].
    fn next_up(
        &self,
        session: &Session,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send;

    /// `GET /Items/Latest`: the most recently added movies or series in the
    /// library `parent` (Recently Added), newest first. At most `limit`
    /// items.
    ///
    /// # Errors
    ///
    /// As for [`JellyfinApi::libraries`].
    fn latest(
        &self,
        session: &Session,
        parent: &ItemId,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send;

    /// `GET /Shows/{id}/Seasons`: a series' seasons in order.
    ///
    /// # Errors
    ///
    /// [`ApiError::ItemNotFound`] if there is no such series; otherwise as
    /// for [`JellyfinApi::libraries`].
    fn seasons(
        &self,
        session: &Session,
        series: &ItemId,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send;

    /// `GET /Shows/{id}/Episodes?seasonId=`: the episodes of one season in
    /// order, with their overviews.
    ///
    /// # Errors
    ///
    /// [`ApiError::ItemNotFound`] if there is no such series; otherwise as
    /// for [`JellyfinApi::libraries`].
    fn episodes(
        &self,
        session: &Session,
        series: &ItemId,
        season: &ItemId,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send;

    /// `GET /Items/{id}/Images/{type}`: an item's image scaled to at most
    /// `max_width`, as the bytes and `Content-Type` the server sent.
    ///
    /// # Errors
    ///
    /// [`ApiError::ItemNotFound`] if the item has no such image;
    /// [`ApiError::NotJellyfin`] if the reply has no `Content-Type`;
    /// otherwise as for [`JellyfinApi::libraries`].
    fn image(
        &self,
        session: &Session,
        request: &ImageRequest,
    ) -> impl Future<Output = Result<Image, ApiError>> + Send;
}
