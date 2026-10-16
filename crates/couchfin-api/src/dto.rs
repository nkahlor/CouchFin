//! The parts of Jellyfin's JSON replies that Couchfin uses. Jellyfin sends
//! `PascalCase` keys and many more fields; unknown fields are ignored, and a
//! missing required one makes the reply count as "not Jellyfin".

use serde::Deserialize;

use crate::ids::{ServerId, UserId};
use crate::secret::AccessToken;
use crate::server_url::ServerUrl;

/// What `GET /System/Info/Public` says about a server. Anyone can read it
/// without logging in, so it is how Couchfin checks an address is a Jellyfin
/// server before asking for a password.
///
/// # Examples
///
/// ```
/// use couchfin_api::{PublicSystemInfo, ServerId};
///
/// let info = PublicSystemInfo {
///     server_name: "Living Room".into(),
///     version: "10.11.2".into(),
///     id: "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f".parse()?,
/// };
/// assert_eq!(format!("{} {}", info.server_name, info.version), "Living Room 10.11.2");
/// # Ok::<(), couchfin_api::InvalidValueError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PublicSystemInfo {
    /// The name the admin gave the server.
    pub server_name: String,
    /// The Jellyfin version, such as `10.11.2`.
    pub version: String,
    /// The server's ID.
    pub id: ServerId,
}

/// A Jellyfin user, from `GET /Users/Me` or the login reply.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct User {
    /// The user's ID.
    pub id: UserId,
    /// The user name, as shown on the home screen.
    pub name: String,
}

/// The reply to a successful `POST /Users/AuthenticateByName`.
///
/// Its `Debug` output redacts the token.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthenticationResult {
    /// Who logged in.
    pub user: User,
    /// The token for every later request. Store it securely.
    pub access_token: AccessToken,
    /// The ID of the server that issued the token.
    pub server_id: ServerId,
}

/// The result of [`JellyfinApi::check_server`]: the server's public info, and
/// the address it really answers on.
///
/// `server` differs from the address the user typed when the server
/// redirects, for example a reverse proxy sending `http://` to `https://`,
/// or adding a base path such as `/jellyfin/`. Use it, not the typed address,
/// for logging in and every request after; it is the address to store.
///
/// [`JellyfinApi::check_server`]: crate::JellyfinApi::check_server
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCheck {
    /// The canonical server address.
    pub server: ServerUrl,
    /// What the server says about itself.
    pub info: PublicSystemInfo,
}
