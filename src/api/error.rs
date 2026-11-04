#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("could not reach the server: {0}")]
    Unreachable(#[source] reqwest::Error),

    #[error("the server took too long to answer")]
    Timeout(#[source] reqwest::Error),

    #[error("request failed: {0}")]
    Request(#[source] reqwest::Error),

    #[error("invalid server URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
}

impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            Self::Timeout(error)
        } else if error.is_connect() {
            Self::Unreachable(error)
        } else {
            Self::Request(error)
        }
    }
}
