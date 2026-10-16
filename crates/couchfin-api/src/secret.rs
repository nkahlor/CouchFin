//! Secrets: the access token and the login password. Neither implements
//! `Display`, `Serialize`, or a `Debug` that shows the value, so they can't
//! end up in a log line by accident. Reading the value takes an explicit
//! `expose_secret()` call.

use std::fmt;

use serde::Deserialize;

use crate::ids::{InvalidValueError, validate};

/// The access token Jellyfin issues at login. It goes in the `Token` part of
/// the `Authorization` header on every request after that.
///
/// `Debug` prints `AccessToken(<redacted>)`. Store it with
/// [`AccessToken::expose_secret`] (in the system keyring, couchfin-core's
/// job) and restore it with [`AccessToken::new`].
///
/// # Examples
///
/// ```
/// use couchfin_api::AccessToken;
///
/// let token = AccessToken::new("0123456789abcdef0123456789abcdef")?;
/// assert_eq!(format!("{token:?}"), "AccessToken(<redacted>)");
/// assert_eq!(token.expose_secret(), "0123456789abcdef0123456789abcdef");
/// # Ok::<(), couchfin_api::InvalidValueError>(())
/// ```
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct AccessToken(String);

impl AccessToken {
    /// Wraps a stored token.
    ///
    /// # Errors
    ///
    /// [`InvalidValueError`] if the token is empty, longer than 256
    /// characters, or has characters other than ASCII letters, digits, `.`,
    /// `_`, `~` and `-`. Jellyfin's tokens are 32 hex digits.
    pub fn new(token: impl Into<String>) -> Result<Self, InvalidValueError> {
        Self::try_from(token.into())
    }

    /// The token itself. Only for sending it to the server or storing it
    /// securely; never log it.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AccessToken {
    type Error = InvalidValueError;

    fn try_from(token: String) -> Result<Self, Self::Error> {
        validate(&token, "Access token")?;
        Ok(Self(token))
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccessToken(<redacted>)")
    }
}

/// A password typed on the login screen. Any string is allowed, including an
/// empty one, since Jellyfin users may have no password.
///
/// `Debug` prints `Password(<redacted>)`.
///
/// # Examples
///
/// ```
/// use couchfin_api::Password;
///
/// let password = Password::new("hunter2");
/// assert_eq!(format!("{password:?}"), "Password(<redacted>)");
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Password(String);

impl Password {
    /// Wraps a password.
    #[must_use]
    pub fn new(password: impl Into<String>) -> Self {
        Self(password.into())
    }

    /// The password itself, for the login request body. Never log it.
    #[must_use]
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(<redacted>)")
    }
}
