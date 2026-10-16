//! Request failures, split by what the user can do about them.

/// Why a request to the server failed.
///
/// Each variant needs a different message or a different next step, so the
/// UI can match on it: re-type the address, re-type the password, or go back
/// to the login screen. The messages from `Display` are short and plain;
/// none of them contain a token or password.
///
/// It is `Clone` and `PartialEq` so fakes can return scripted errors and
/// tests can compare them.
///
/// # Examples
///
/// ```
/// use couchfin_api::ApiError;
///
/// fn advice(error: &ApiError) -> &'static str {
///     match error {
///         ApiError::Unreachable { .. } | ApiError::Timeout => "Check the address and that the server is on",
///         ApiError::NotJellyfin { .. } => "That address isn't a Jellyfin server",
///         ApiError::InvalidCredentials => "Wrong username or password",
///         ApiError::TokenRejected => "Please log in again",
///         ApiError::Status { .. } => "The server had a problem; try again",
///     }
/// }
/// assert_eq!(advice(&ApiError::InvalidCredentials), "Wrong username or password");
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
    /// endpoint, a web page, or JSON without the fields Couchfin needs.
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
    /// Any other unsuccessful HTTP status, such as 500 or 503.
    #[error("the server answered with HTTP status {status}")]
    Status {
        /// The HTTP status code.
        status: u16,
    },
}

impl ApiError {
    /// Maps a failure to send a request or read its reply. Kept out of the
    /// public API (no `From` impl) so callers never depend on reqwest.
    pub(crate) fn transport(error: &reqwest::Error) -> Self {
        if error.is_timeout() {
            return Self::Timeout;
        }
        // reqwest's own message names only the URL; the cause ("Connection
        // refused", "dns error", a certificate problem) is in the source
        // chain. URLs never carry credentials (see `ServerUrl`), and tokens
        // and passwords travel in headers and bodies, so this is safe to log.
        let mut detail = error.to_string();
        let mut source = std::error::Error::source(error);
        while let Some(cause) = source {
            detail.push_str(": ");
            detail.push_str(&cause.to_string());
            source = cause.source();
        }
        Self::Unreachable { detail }
    }
}
