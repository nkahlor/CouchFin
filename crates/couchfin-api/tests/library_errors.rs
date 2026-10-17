//! How `libraries`, `items` and `item` fail: a missing item, a rejected
//! token, a reply that isn't Jellyfin's and a redirect.

mod common;

use common::library::{MOVIE_ID, id};
use common::{TOKEN, client, session};
use couchfin_api::{ApiError, ItemsQuery, JellyfinApi};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn a_missing_item_is_item_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let error = client()
        .item(&session(&server, TOKEN), &id(MOVIE_ID))
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::ItemNotFound);
}

#[tokio::test]
async fn every_call_maps_401_to_token_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let api = client();
    let session = session(&server, TOKEN);

    assert_eq!(
        api.libraries(&session).await.unwrap_err(),
        ApiError::TokenRejected
    );
    assert_eq!(
        api.items(&session, &ItemsQuery::default())
            .await
            .unwrap_err(),
        ApiError::TokenRejected
    );
    assert_eq!(
        api.item(&session, &id(MOVIE_ID)).await.unwrap_err(),
        ApiError::TokenRejected
    );
}

#[tokio::test]
async fn a_reply_that_is_not_an_item_list_is_not_jellyfin() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html></html>"))
        .mount(&server)
        .await;

    let error = client()
        .items(&session(&server, TOKEN), &ItemsQuery::default())
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::NotJellyfin { .. }), "{error:?}");
}

#[tokio::test]
async fn a_redirect_is_never_followed_with_the_token() {
    let server = MockServer::start().await;
    let elsewhere = common::untouched_jellyfin().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("Location", format!("{}/UserViews", elsewhere.uri())),
        )
        .mount(&server)
        .await;

    let error = client()
        .libraries(&session(&server, TOKEN))
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::Redirected { .. }), "{error:?}");
}
