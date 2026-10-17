//! Reading replies: status codes to errors, bodies to JSON or bytes.

use reqwest::header::CONTENT_TYPE;
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;

use crate::error::ApiError;
use crate::image::Image;

/// Turns an unsuccessful status into an error. `on_unauthorized` is what a
/// 401 means for this endpoint. Every endpoint used here exists on every
/// Jellyfin server, so a 404 means the address points at something else.
pub(crate) fn check_status(response: &Response, on_unauthorized: ApiError) -> Result<(), ApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    Err(match status {
        StatusCode::UNAUTHORIZED => on_unauthorized,
        StatusCode::NOT_FOUND => ApiError::NotJellyfin {
            detail: format!("{} not found", response.url().path()),
        },
        other => ApiError::Status {
            status: other.as_u16(),
        },
    })
}

/// Like [`check_status`] for a logged-in request about one item, where a 404
/// means the item (or image) doesn't exist rather than a wrong address.
pub(crate) fn check_item_status(response: &Response) -> Result<(), ApiError> {
    if response.status() == StatusCode::NOT_FOUND {
        return Err(ApiError::ItemNotFound);
    }
    check_status(response, ApiError::TokenRejected)
}

/// Reads a JSON body. A body that is cut off or isn't the JSON Jellyfin sends
/// means this isn't (a compatible) Jellyfin; a timeout while reading stays a
/// timeout.
pub(crate) async fn read_json<T: DeserializeOwned>(response: Response) -> Result<T, ApiError> {
    let bytes = response.bytes().await.map_err(|e| ApiError::body(&e))?;
    serde_json::from_slice(&bytes).map_err(|e| ApiError::NotJellyfin {
        detail: e.to_string(),
    })
}

/// Reads an image body and its `Content-Type`, which the image cache needs
/// to decode it.
pub(crate) async fn read_image(response: Response) -> Result<Image, ApiError> {
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .ok_or_else(|| ApiError::NotJellyfin {
            detail: "the image reply has no Content-Type".to_owned(),
        })?;
    let bytes = response.bytes().await.map_err(|e| ApiError::body(&e))?;
    Ok(Image {
        bytes: bytes.to_vec(),
        content_type,
    })
}
