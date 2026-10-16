//! Reading redirects. reqwest is built with redirects off, so each call
//! decides for itself: `check_server` follows them by hand, and the login
//! and token calls turn them into errors.

use reqwest::header::LOCATION;
use reqwest::{Response, Url};

use crate::error::ApiError;

/// How many redirects `check_server` follows before giving up, as many as
/// reqwest's default policy.
pub(crate) const MAX_REDIRECTS: usize = 10;

/// Where `response` redirects to, or `None` if it isn't a redirect.
///
/// # Errors
///
/// [`ApiError::InsecureRedirect`] for a redirect from `https` to anything
/// else, since following it would send later requests unencrypted, and
/// [`ApiError::NotJellyfin`] for a redirect without a usable `Location`.
pub(crate) fn target(response: &Response) -> Result<Option<Url>, ApiError> {
    let status = response.status();
    // 300 (multiple choices) and 304 (not modified) aren't redirects to follow.
    if !matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
        return Ok(None);
    }
    let not_jellyfin = |why: &str| ApiError::NotJellyfin {
        detail: format!("{} redirect {why}", status.as_u16()),
    };
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| not_jellyfin("without a Location"))?;
    // Location may be relative to the request URL.
    let target = response
        .url()
        .join(location)
        .map_err(|_| not_jellyfin("to an invalid URL"))?;
    if response.url().scheme() == "https" && target.scheme() != "https" {
        return Err(ApiError::InsecureRedirect);
    }
    if !matches!(target.scheme(), "http" | "https") {
        return Err(not_jellyfin("to a URL that isn't http or https"));
    }
    Ok(Some(target))
}
