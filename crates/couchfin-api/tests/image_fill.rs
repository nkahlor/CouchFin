//! The size parameters of an image request (decision 006): a box to fill, and
//! the quality jellyfin-web asks for.

mod common;

use common::library::{MOVIE_ID, id};
use common::{TOKEN, client, session};
use couchfin_api::{ImageKind, ImageRequest, JellyfinApi};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xD9];

#[tokio::test]
async fn a_box_to_fill_asks_for_fill_width_height_and_quality() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/Items/{MOVIE_ID}/Images/Primary")))
        .and(query_param("fillWidth", "280"))
        .and(query_param("fillHeight", "420"))
        .and(query_param("quality", "96"))
        .and(query_param_is_missing("maxWidth"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(JPEG, "image/jpeg"))
        .expect(1)
        .mount(&server)
        .await;
    let request = ImageRequest::new(id(MOVIE_ID), ImageKind::Primary, 280).with_fill_height(420);

    client()
        .image(&session(&server, TOKEN), &request)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_request_without_a_height_still_asks_for_max_width_and_quality() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(query_param("quality", "96"))
        .and(query_param("maxWidth", "400"))
        .and(query_param_is_missing("fillWidth"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(JPEG, "image/jpeg"))
        .expect(1)
        .mount(&server)
        .await;
    let request = ImageRequest::new(id(MOVIE_ID), ImageKind::Primary, 400);

    client()
        .image(&session(&server, TOKEN), &request)
        .await
        .unwrap();
}
