//! Identifier newtypes. Each one only holds characters that are safe inside a
//! quoted `Authorization` header value and a URL path, so no ID can break the
//! header or a request path.

use std::fmt;
use std::str::FromStr;

use serde::Deserialize;

/// The longest identifier or token accepted. Jellyfin's own are 32 or 36
/// characters; this leaves room without letting a hostile server send
/// megabytes.
const MAX_LEN: usize = 256;

/// A string that isn't a valid identifier or token: empty, longer than 256
/// characters, or containing anything other than ASCII letters, digits, `.`,
/// `_`, `~` and `-`.
///
/// # Examples
///
/// ```
/// use couchfin_api::DeviceId;
///
/// let error = "has space".parse::<DeviceId>().unwrap_err();
/// assert!(error.to_string().starts_with("Device ID must be"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{what} must be 1 to {MAX_LEN} characters of ASCII letters, digits, '.', '_', '~' or '-'")]
pub struct InvalidValueError {
    what: &'static str,
}

/// Checks the URL "unreserved" characters (RFC 3986), which Jellyfin's header
/// parser and URL paths both pass through unchanged.
pub(crate) fn validate(value: &str, what: &'static str) -> Result<(), InvalidValueError> {
    let safe = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '~' | '-');
    if !value.is_empty() && value.len() <= MAX_LEN && value.chars().all(safe) {
        Ok(())
    } else {
        Err(InvalidValueError { what })
    }
}

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident, $what:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
        #[serde(try_from = "String")]
        pub struct $name(String);

        impl $name {
            /// The identifier as a string, for storing it or putting it in a URL.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = InvalidValueError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                validate(value, $what)?;
                Ok(Self(value.to_owned()))
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidValueError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                validate(&value, $what)?;
                Ok(Self(value))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id_type!(
    /// Identifies this Couchfin install to the server, which lists it under
    /// Dashboard > Devices.
    ///
    /// It must stay the same across restarts, or every launch shows up as a
    /// new device. Create one with [`DeviceId::generate`] on first run, store
    /// [`DeviceId::as_str`], and parse it back on later runs; storing it is
    /// the caller's job (couchfin-core).
    ///
    /// # Examples
    ///
    /// ```
    /// use couchfin_api::DeviceId;
    ///
    /// let id = DeviceId::generate();
    /// let stored = id.as_str().to_owned();
    /// let restored: DeviceId = stored.parse()?;
    /// assert_eq!(restored, id);
    /// # Ok::<(), couchfin_api::InvalidValueError>(())
    /// ```
    DeviceId,
    "Device ID"
);

id_type!(
    /// A Jellyfin user's ID, as the server sends it (32 hex digits).
    ///
    /// # Examples
    ///
    /// ```
    /// use couchfin_api::UserId;
    ///
    /// let id: UserId = "38a5a5bbdc3549ccb0b07fc7e43c1d43".parse()?;
    /// assert_eq!(id.as_str(), "38a5a5bbdc3549ccb0b07fc7e43c1d43");
    /// # Ok::<(), couchfin_api::InvalidValueError>(())
    /// ```
    UserId,
    "User ID"
);

id_type!(
    /// A Jellyfin server's ID, from its public system info. It stays the same
    /// when the server's address changes.
    ///
    /// # Examples
    ///
    /// ```
    /// use couchfin_api::ServerId;
    ///
    /// let id: ServerId = "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f".parse()?;
    /// assert_eq!(id.to_string(), "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f");
    /// # Ok::<(), couchfin_api::InvalidValueError>(())
    /// ```
    ServerId,
    "Server ID"
);

impl DeviceId {
    /// Creates a new random device ID (a UUID v4), for the first run of an
    /// install.
    #[must_use]
    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
