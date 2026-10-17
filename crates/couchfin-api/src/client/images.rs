//! Downloading images.

use super::JellyfinClient;
use crate::api::Session;
use crate::error::ApiError;
use crate::http::read_image;
use crate::image::{Image, ImageRequest};

impl JellyfinClient {
    pub(super) async fn fetch_image(
        &self,
        session: &Session,
        request: &ImageRequest,
    ) -> Result<Image, ApiError> {
        let endpoint = format!("Items/{}/Images/{}", request.item, request.kind.as_str());
        let mut query = vec![("maxWidth", request.max_width.to_string())];
        if let Some(tag) = &request.tag {
            query.push(("tag", tag.as_str().to_owned()));
        }
        read_image(self.get_library(session, &endpoint, &query, true).await?).await
    }
}
