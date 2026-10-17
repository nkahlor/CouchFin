//! `image` (`GET /Items/{id}/Images/{type}`): the bytes and content type
//! couchfin-core's image cache stores.

mod common;

use common::library::{MOVIE_ID, PRIMARY_TAG, id};
use common::{TOKEN, authorization, client, session};
use couchfin_api::{ApiError, ImageKind, ImageRequest, ImageTag, JellyfinApi};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0, 1,
];

fn tag() -> ImageTag {
    PRIMARY_TAG.parse().expect("tag is valid")
}

#[tokio::test]
async fn returns_the_bytes_and_content_type() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/Items/{MOVIE_ID}/Images/Primary")))
        .and(query_param("maxWidth", "400"))
        .and(query_param("tag", PRIMARY_TAG))
        .and(authorization(Some(TOKEN)))
        .respond_with(ResponseTemplate::new(200).set_body_raw(JPEG, "image/jpeg"))
        .expect(1)
        .mount(&server)
        .await;
    let request = ImageRequest::new(id(MOVIE_ID), ImageKind::Primary, 400).with_tag(tag());

    let image = client()
        .image(&session(&server, TOKEN), &request)
        .await
        .unwrap();

    assert_eq!(image.bytes, JPEG);
    assert_eq!(image.content_type, "image/jpeg");
}

#[tokio::test]
async fn each_kind_has_its_own_path_and_the_tag_is_optional() {
    let server = MockServer::start().await;
    for kind in ["Backdrop", "Thumb"] {
        Mock::given(method("GET"))
            .and(path(format!("/Items/{MOVIE_ID}/Images/{kind}")))
            .and(query_param("maxWidth", "1920"))
            .and(query_param_is_missing("tag"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(b"x".to_vec(), "image/webp"))
            .expect(1)
            .mount(&server)
            .await;
    }
    let api = client();
    let session = session(&server, TOKEN);

    for kind in [ImageKind::Backdrop, ImageKind::Thumb] {
        let request = ImageRequest::new(id(MOVIE_ID), kind, 1920);
        let image = api.image(&session, &request).await.unwrap();
        assert_eq!(image.content_type, "image/webp");
    }
}

#[tokio::test]
async fn a_missing_image_is_item_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let request = ImageRequest::new(id(MOVIE_ID), ImageKind::Thumb, 300);

    let error = client()
        .image(&session(&server, TOKEN), &request)
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::ItemNotFound);
}

#[tokio::test]
async fn a_rejected_token_is_token_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let request = ImageRequest::new(id(MOVIE_ID), ImageKind::Primary, 300);

    let error = client()
        .image(&session(&server, TOKEN), &request)
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::TokenRejected);
}

#[tokio::test]
async fn a_reply_without_a_content_type_is_not_jellyfin() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(JPEG))
        .mount(&server)
        .await;
    let request = ImageRequest::new(id(MOVIE_ID), ImageKind::Primary, 300);

    let error = client()
        .image(&session(&server, TOKEN), &request)
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::NotJellyfin { .. }), "{error:?}");
}
