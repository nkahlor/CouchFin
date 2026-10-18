//! Downloading images.

use super::JellyfinClient;
use crate::api::Session;
use crate::error::ApiError;
use crate::http::read_image;
use crate::image::{Image, ImageRequest};

const IMAGE_QUALITY: u8 = 96;

impl JellyfinClient {
    pub(super) async fn fetch_image(
        &self,
        session: &Session,
        request: &ImageRequest,
    ) -> Result<Image, ApiError> {
        let endpoint = format!("Items/{}/Images/{}", request.item, request.kind.as_str());
        // jellyfin-web asks for 96; the server's default is 90.
        let mut query = vec![("quality", IMAGE_QUALITY.to_string())];
        match request.fill_height {
            Some(height) => {
                query.push(("fillWidth", request.max_width.to_string()));
                query.push(("fillHeight", height.to_string()));
            }
            None => query.push(("maxWidth", request.max_width.to_string())),
        }
        if let Some(tag) = &request.tag {
            query.push(("tag", tag.as_str().to_owned()));
        }
        read_image(self.get_library(session, &endpoint, &query, true).await?).await
    }
}
