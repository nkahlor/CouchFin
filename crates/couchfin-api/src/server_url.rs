//! The server's base address, validated and normalised once so every request
//! can join a relative endpoint path onto it.

use std::fmt;
use std::str::FromStr;

use reqwest::Url;

/// Why a typed-in server address was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ServerUrlError {
    /// Nothing was entered.
    #[error("enter the server's address")]
    Empty,
    /// The text isn't a URL at all.
    #[error("not a valid address: {0}")]
    Invalid(String),
    /// Only `http` and `https` are supported.
    #[error("unsupported scheme \"{0}\"; use http:// or https://")]
    UnsupportedScheme(String),
    /// The address has `user:password@` in it. Couchfin refuses it, so a
    /// password can never sit in a stored or logged URL.
    #[error("the address must not contain a username or password")]
    HasCredentials,
}

/// A Jellyfin server's base URL, such as `http://192.168.1.20:8096/` or
/// `https://example.com/jellyfin/`.
///
/// Parsing is forgiving about what people type or paste, and always produces
/// the same normal form:
///
/// - surrounding whitespace is trimmed;
/// - with no scheme, `http://` is assumed (Jellyfin's default);
/// - the scheme and host are lower-cased, and a default port is dropped;
/// - a query, a fragment, and a pasted web-client path (`/web/...`) are
///   dropped, keeping any reverse-proxy base path before it;
/// - the path always ends in `/`.
///
/// # Examples
///
/// ```
/// use couchfin_api::ServerUrl;
///
/// let url: ServerUrl = " Jellyfin.lan:8096/web/index.html#!/home.html ".parse()?;
/// assert_eq!(url.as_str(), "http://jellyfin.lan:8096/");
///
/// let proxied: ServerUrl = "https://example.com/jellyfin".parse()?;
/// assert_eq!(proxied.as_str(), "https://example.com/jellyfin/");
/// # Ok::<(), couchfin_api::ServerUrlError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServerUrl(Url);

impl ServerUrl {
    /// The normalised URL, for storing it or showing it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// The full URL of an API endpoint such as `System/Info/Public`.
    pub(crate) fn endpoint(&self, path: &str) -> Url {
        let mut url = self.0.clone();
        // The base path always ends in '/', so appending keeps it.
        url.set_path(&format!("{}{path}", self.0.path()));
        url
    }
}

impl FromStr for ServerUrl {
    type Err = ServerUrlError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let input = input.trim();
        if input.is_empty() {
            return Err(ServerUrlError::Empty);
        }
        let with_scheme = if input.contains("://") {
            input.to_owned()
        } else {
            format!("http://{input}")
        };
        let url = Url::parse(&with_scheme).map_err(|e| ServerUrlError::Invalid(e.to_string()))?;
        Self::from_url(url)
    }
}

impl ServerUrl {
    /// Checks and normalises an already parsed URL.
    fn from_url(mut url: Url) -> Result<Self, ServerUrlError> {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(ServerUrlError::UnsupportedScheme(url.scheme().to_owned()));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ServerUrlError::HasCredentials);
        }
        url.set_query(None);
        url.set_fragment(None);
        url.set_path(&base_path(url.path()));
        Ok(Self(url))
    }

    /// The server base of an endpoint URL: `https://host/jellyfin/` from
    /// `https://host/jellyfin/System/Info/Public` and `System/Info/Public`.
    /// `None` if the URL isn't that endpoint. Proxies may change the case of
    /// the path, and Jellyfin ignores it, so the match does too.
    pub(crate) fn from_endpoint(url: &Url, endpoint: &str) -> Option<Self> {
        let path = url.path();
        let (base, tail) = path.split_at_checked(path.len().checked_sub(endpoint.len())?)?;
        if !base.ends_with('/') || !tail.eq_ignore_ascii_case(endpoint) {
            return None;
        }
        let mut base_url = url.clone();
        base_url.set_path(base);
        Self::from_url(base_url).ok()
    }
}

/// Cuts a pasted web-client path (`/jellyfin/web/index.html`) back to the
/// server's base (`/jellyfin/`), and makes sure the result ends in `/`.
fn base_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let keep = segments
        .iter()
        .rposition(|s| *s == "web")
        .unwrap_or(segments.len());
    let mut base = String::from("/");
    for segment in &segments[..keep] {
        base.push_str(segment);
        base.push('/');
    }
    base
}

impl fmt::Display for ServerUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
