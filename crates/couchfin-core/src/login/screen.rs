//! The login flow's state, as plain data the UI renders.

use couchfin_api::{ApiError, Password, PublicSystemInfo, ServerUrl, ServerUrlError, User};

use crate::tokens::TokenStoreError;

/// What the login flow is showing. The UI renders one screen per variant and
/// sends [`LoginAction`]s back.
///
/// Every field is public, so UI tests can build any screen directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginScreen {
    /// Before [`LoginFlow::start`](crate::LoginFlow::start) has looked at
    /// the stored settings. Show nothing, or a splash.
    Starting,
    /// Checking a stored session at startup, or showing why that failed.
    Restoring(Restoring),
    /// Asking for the server's address.
    EnterAddress(EnterAddress),
    /// Asking for a username and password on a checked server.
    LogIn(LogIn),
    /// Logged in.
    Home(Home),
}

/// [`LoginScreen::Restoring`]: a stored session is being checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restoring {
    /// The stored server.
    pub server: ServerUrl,
    /// Whether the check is running or failed.
    pub status: RestoreStatus,
}

/// How the check of a stored session is going.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreStatus {
    /// Asking the server. Show "Connecting to …".
    Checking,
    /// The server couldn't be reached or answered with an error. The token
    /// is kept: offer [`LoginAction::Retry`] and [`LoginAction::ChangeServer`].
    Failed(ApiError),
}

/// [`LoginScreen::EnterAddress`]: the server address form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnterAddress {
    /// The text to show in the address field: what was last submitted, the
    /// stored server when changing server, or empty on the first run.
    pub address: String,
    /// The state of the last submit.
    pub request: Request,
}

/// [`LoginScreen::LogIn`]: the login form for a checked server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogIn {
    /// The server's canonical address, for "Connected to …".
    pub server: ServerUrl,
    /// The server's name and version.
    pub info: PublicSystemInfo,
    /// The username to fill in: the last one used, or empty.
    pub username: String,
    /// The state of the last login attempt. It is also `Failed` when the
    /// flow arrived here because a stored token was rejected
    /// ([`ApiError::TokenRejected`]) or the server moved
    /// ([`ApiError::Redirected`]), so the form can say why it is asking.
    /// The flow never holds the password after a request, so the UI should
    /// clear its password field when this becomes `Failed`.
    pub request: Request,
}

/// [`LoginScreen::Home`]: logged in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    /// The server's canonical address.
    pub server: ServerUrl,
    /// The server's name and version, from `/System/Info/Public`.
    pub info: PublicSystemInfo,
    /// Who is logged in, from `/Users/Me`.
    pub user: User,
    /// Set when this login's token couldn't be stored (neither in the
    /// keyring nor in the token file), so the next start will ask to log in
    /// again. The UI may warn about it. Always `None` after a restored
    /// session.
    pub token_not_saved: Option<TokenStoreError>,
}

/// The state of a form's request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// Nothing sent yet, or the form was reset.
    Idle,
    /// Waiting for the server. The form should show progress and ignore a
    /// second submit (the flow ignores it too).
    InFlight,
    /// The last request failed. The UI picks the wording by matching on the
    /// error.
    Failed(LoginError),
}

/// Why a login-flow request failed. Match on it to choose what to tell the
/// user; `Display` is for logs only.
///
/// # Examples
///
/// ```
/// use couchfin_core::{ApiError, LoginError, ServerUrlError};
///
/// fn message(error: &LoginError) -> &'static str {
///     match error {
///         LoginError::InvalidAddress(ServerUrlError::Empty) => "Enter the server's address",
///         LoginError::InvalidAddress(_) => "That isn't a valid address",
///         LoginError::Api(ApiError::Unreachable { .. } | ApiError::Timeout) => {
///             "Couldn't reach the server"
///         }
///         LoginError::Api(ApiError::InvalidCredentials) => "Wrong username or password",
///         LoginError::Api(ApiError::TokenRejected) => "Please log in again",
///         LoginError::Api(_) => "Something went wrong",
///     }
/// }
/// assert_eq!(
///     message(&LoginError::Api(ApiError::InvalidCredentials)),
///     "Wrong username or password"
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoginError {
    /// The typed address isn't a usable server URL; nothing was sent.
    #[error(transparent)]
    InvalidAddress(#[from] ServerUrlError),
    /// The server couldn't be reached or refused the request.
    #[error(transparent)]
    Api(#[from] ApiError),
}

/// What the UI asks the login flow to do. An action that doesn't fit the
/// current screen (such as `LogOut` on the address form, or a second submit
/// while one is in flight) is ignored.
#[derive(Debug, Clone)]
pub enum LoginAction {
    /// Check this address. On [`LoginScreen::EnterAddress`].
    SubmitAddress(String),
    /// Log in on the checked server. On [`LoginScreen::LogIn`].
    SubmitLogin {
        /// The username.
        username: String,
        /// The password; never logged.
        password: Password,
    },
    /// Check the stored session again. On a failed [`LoginScreen::Restoring`].
    Retry,
    /// Go to the address form, abandoning any request in flight. On
    /// [`LoginScreen::Restoring`], [`LoginScreen::LogIn`], and
    /// [`LoginScreen::EnterAddress`] while its check is in flight.
    ChangeServer,
    /// Forget the token and show the login form. On [`LoginScreen::Home`].
    LogOut,
    /// The server rejected the token mid-browse (the app saw
    /// `LibraryState::session_expired`). Forgets the token and shows the
    /// login form with [`ApiError::TokenRejected`], like an expired token at
    /// startup, so the UI can say why. On [`LoginScreen::Home`].
    SessionExpired,
}
