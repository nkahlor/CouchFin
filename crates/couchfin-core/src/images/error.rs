//! What can go wrong loading an image.

use couchfin_api::ApiError;

/// Why an image could not be loaded.
///
/// A missing image is not an error: [`ImageLoad::NoImage`](super::ImageLoad)
/// reports it, so the UI can show a placeholder. A damaged cache file is not
/// one either: it is deleted and fetched again.
///
/// # Examples
///
/// ```
/// use couchfin_core::{ApiError, ImageError};
///
/// let error = ImageError::from(ApiError::InvalidCredentials);
/// assert_eq!(error, ImageError::Api(ApiError::InvalidCredentials));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    /// The server did not deliver the image.
    #[error("could not fetch the image: {0}")]
    Api(#[from] ApiError),
    /// The bytes are not a JPEG, PNG or WebP image.
    #[error("could not decode the image: {detail}")]
    Decode {
        /// What the decoder said.
        detail: String,
    },
    /// The cache directory could not be read or changed.
    #[error("image cache file error: {detail}")]
    Io {
        /// What the operating system said.
        detail: String,
    },
    /// A background decoding task died before it finished.
    #[error("image cache failure: {detail}")]
    Cache {
        /// What happened.
        detail: String,
    },
}
