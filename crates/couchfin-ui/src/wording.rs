//! What the user reads when something goes wrong.
//!
//! Errors' `Display` text is for the log: it can carry an error chain, a URL
//! or a status code. These functions pick short, plain wording instead, for
//! someone on a sofa. Each one matches every variant with no wildcard, so a
//! new error variant fails the build until it has wording here.
//!
//! # Examples
//!
//! ```
//! use couchfin_core::{ApiError, LoginError};
//! use couchfin_ui::wording;
//!
//! let error = LoginError::Api(ApiError::InvalidCredentials);
//! assert_eq!(wording::login_error(&error), "Wrong username or password.");
//! ```

use couchfin_core::{ApiError, LoginError, ServerUrlError, TokenStoreError};

/// The message for a failed address check or sign-in.
#[must_use]
pub fn login_error(error: &LoginError) -> String {
    match error {
        LoginError::InvalidAddress(error) => invalid_address(error).to_owned(),
        LoginError::Api(error) => api_error(error),
    }
}

/// The message for a failed request to the server, including a stored
/// session that couldn't be checked at startup.
#[must_use]
pub fn api_error(error: &ApiError) -> String {
    match error {
        ApiError::Unreachable { .. } => {
            "Couldn't reach the server. Check the address, and that the server is on.".into()
        }
        ApiError::Timeout => {
            "The server took too long to answer. Check that it's running, then try again.".into()
        }
        ApiError::NotJellyfin { .. } => {
            "That address isn't a Jellyfin server. Check it and try again.".into()
        }
        ApiError::InvalidCredentials => "Wrong username or password.".into(),
        ApiError::TokenRejected => "You've been signed out. Please sign in again.".into(),
        // Core has already checked the new address and shows its sign-in
        // form, with the new address under the title.
        ApiError::Redirected {
            server: Some(server),
        } => format!(
            "The server has moved to {server}. Check that's your server before you sign in."
        ),
        ApiError::Redirected { server: None } => {
            "The server sent Couchfin somewhere unexpected. Check the address and try again.".into()
        }
        ApiError::InsecureRedirect => {
            "The server tried to switch to an unencrypted connection, so Couchfin stopped.".into()
        }
        ApiError::ItemNotFound => "That's no longer in your library.".into(),
        ApiError::Status { .. } => "The server had a problem. Try again in a moment.".into(),
    }
}

/// Why a typed address was refused before anything was sent.
fn invalid_address(error: &ServerUrlError) -> &'static str {
    match error {
        ServerUrlError::Empty => "Type your server's address.",
        ServerUrlError::Invalid(_) => {
            "That doesn't look like a server address. Check it and try again."
        }
        ServerUrlError::UnsupportedScheme(_) => {
            "Use an address that starts with http:// or https://."
        }
        ServerUrlError::HasCredentials => {
            "Leave your username and password out of the address. You'll sign in next."
        }
    }
}

/// Home's note while the first rows are on their way.
pub const LOADING: &str = "Loading your library…";

/// Home's note when the server has nothing to show.
pub const EMPTY_LIBRARY: &str =
    "There's nothing here yet. Add movies or shows to a library on your Jellyfin server.";

/// The message for a failed request while browsing: a row on Home, a page
/// of the grid, or a detail page. Unlike [`api_error`] it doesn't talk about
/// addresses or sign-in, which the user isn't looking at. Every variant is
/// listed, so a new one fails the build until it has wording here.
///
/// # Examples
///
/// ```
/// use couchfin_core::ApiError;
/// use couchfin_ui::wording;
///
/// assert_eq!(
///     wording::browse_error(&ApiError::ItemNotFound),
///     "That's no longer in your library."
/// );
/// ```
#[must_use]
pub fn browse_error(error: &ApiError) -> String {
    match error {
        ApiError::Unreachable { .. } => "Couldn't reach the server. Is it on?".into(),
        ApiError::Timeout => "The server took too long to answer. Try again in a moment.".into(),
        ApiError::NotJellyfin { .. } | ApiError::Redirected { .. } => {
            "The server didn't answer the way Jellyfin does. Check that it's still running.".into()
        }
        ApiError::InsecureRedirect => {
            "The server tried to switch to an unencrypted connection, so Couchfin stopped.".into()
        }
        // The app returns to sign-in when the token is rejected, so this
        // shows for a moment at most.
        ApiError::TokenRejected | ApiError::InvalidCredentials => {
            "You've been signed out. Please sign in again.".into()
        }
        ApiError::ItemNotFound => "That's no longer in your library.".into(),
        ApiError::Status { .. } => "The server had a problem. Try again in a moment.".into(),
    }
}

/// The note on Home when this sign-in couldn't be stored, so the next start
/// will ask again. Every cause reads the same: the user can't fix a keyring
/// from the sofa, and the log has the detail.
#[must_use]
pub fn token_not_saved(error: &TokenStoreError) -> String {
    match error {
        TokenStoreError::Unavailable { .. }
        | TokenStoreError::Io { .. }
        | TokenStoreError::Invalid { .. } => {
            "Couchfin couldn't save your sign-in, so you'll need to sign in again next time.".into()
        }
    }
}
