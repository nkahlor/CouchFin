//! Request failures, split by what the user can do about them.

use crate::server_url::ServerUrl;

/// Why a request to the server failed.
///
/// Each variant calls for a different next step, so callers match on it:
/// re-type the address, re-type the password, or go back to the login screen.
///
/// `Display` is **for logs and diagnostics, not for the UI**. Some messages
/// carry technical detail (an error chain, a URL, a status code), and none
/// contain a token or password. The UI picks its own wording by matching on
/// the variant, as below.
///
/// It is `Clone` and `PartialEq` so fakes can return scripted errors and
/// tests can compare them.
///
/// # Examples
///
/// How a caller maps each variant to a next step. The strings stand in for
/// whatever wording the UI chooses:
///
/// ```
/// use couchfin_api::ApiError;
///
/// fn advice(error: &ApiError) -> String {
///     match error {
///         ApiError::Unreachable { .. } | ApiError::Timeout => {
///             "Check the address and that the server is on".into()
///         }
///         ApiError::NotJellyfin { .. } => "That address isn't a Jellyfin server".into(),
///         ApiError::InvalidCredentials => "Wrong username or password".into(),
///         ApiError::TokenRejected => "Please log in again".into(),
///         ApiError::Redirected { server: Some(server) } => {
///             format!("The server moved to {server}; use that address")
///         }
///         ApiError::Redirected { server: None } => "Check the server address".into(),
///         ApiError::InsecureRedirect => "The server tried to drop encryption".into(),
///         ApiError::Status { .. } => "The server had a problem; try again".into(),
///     }
/// }
/// assert_eq!(advice(&ApiError::InvalidCredentials), "Wrong username or password");
/// let moved = ApiError::Redirected { server: Some("https://jellyfin.lan".parse()?) };
/// assert_eq!(advice(&moved), "The server moved to https://jellyfin.lan/; use that address");
/// # Ok::<(), couchfin_api::ServerUrlError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// No connection: DNS failed, the connection was refused or reset, or TLS
    /// failed (for example an untrusted certificate).
    #[error("couldn't reach the server: {detail}")]
    Unreachable {
        /// The underlying error chain, for the log.
        detail: String,
    },
    /// The server didn't answer within the client's timeout.
    #[error("the server took too long to answer")]
    Timeout,
    /// Something answered, but not the way Jellyfin does: a missing
    /// endpoint, a web page, JSON without the fields Couchfin needs, or a
    /// reply cut off before its end.
    #[error("this doesn't look like a Jellyfin server: {detail}")]
    NotJellyfin {
        /// What was wrong with the reply, for the log.
        detail: String,
    },
    /// The login was refused (HTTP 401 from `AuthenticateByName`): wrong
    /// username or password.
    #[error("wrong username or password")]
    InvalidCredentials,
    /// The access token was refused (HTTP 401 on a logged-in request): it was
    /// revoked or has expired, so the user has to log in again.
    #[error("the server no longer accepts this login")]
    TokenRejected,
    /// A login or logged-in request was answered with a redirect, which
    /// Couchfin never follows: it would resend the password or token to an
    /// address the user didn't choose, and HTTP turns a redirected POST into
    /// a GET. It usually means the address changed (for example a proxy now
    /// forces `https://`); run [`JellyfinApi::check_server`] again, or use
    /// `server` if it is known.
    ///
    /// [`JellyfinApi::check_server`]: crate::JellyfinApi::check_server
    #[error("{}", redirected_message(server.as_ref()))]
    Redirected {
        /// The server address the redirect points to, when its target is the
        /// same endpoint under another base address.
        server: Option<ServerUrl>,
    },
    /// An `https://` server redirected to plain `http://`. Couchfin refuses,
    /// because the token would then travel unencrypted.
    #[error("the server tried to switch from https:// to unencrypted http://")]
    InsecureRedirect,
    /// Any other unsuccessful HTTP status, such as 500 or 503.
    #[error("the server answered with HTTP status {status}")]
    Status {
        /// The HTTP status code.
        status: u16,
    },
}

fn redirected_message(server: Option<&ServerUrl>) -> String {
    match server {
        Some(server) => format!("the server has moved to {server}; use that address"),
        None => "the server redirected the request to a page that isn't Jellyfin".to_owned(),
    }
}

// The mappings from reqwest errors stay crate-private (no `From` impl), so
// callers never depend on reqwest.
impl ApiError {
    /// Maps a failure to send a request and get the reply's headers.
    pub(crate) fn transport(error: &reqwest::Error) -> Self {
        if error.is_timeout() {
            return Self::Timeout;
        }
        Self::Unreachable {
            detail: error_chain(error),
        }
    }

    /// Maps a failure while reading the reply's body. The server was reached
    /// and started answering, so this isn't `Unreachable`.
    pub(crate) fn body(error: &reqwest::Error) -> Self {
        if error.is_timeout() {
            return Self::Timeout;
        }
        Self::NotJellyfin {
            detail: format!("the reply was cut off: {}", error_chain(error)),
        }
    }
}

/// reqwest's own message names only the URL; the cause ("Connection
/// refused", "dns error", a certificate problem) is in the source chain.
/// URLs never carry credentials (see `ServerUrl`), and tokens and passwords
/// travel in headers and bodies, so this is safe to log.
fn error_chain(error: &reqwest::Error) -> String {
    let mut detail = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        detail.push_str(": ");
        detail.push_str(&cause.to_string());
        source = cause.source();
    }
    detail
}
