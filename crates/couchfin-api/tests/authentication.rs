//! `POST /Users/AuthenticateByName` and `GET /Users/Me`, including the exact
//! `Authorization` header Jellyfin uses to list the device.

mod common;

use common::{
    SERVER_ID, TOKEN, USER_ID, authentication_result_json, authorization, client, server_url,
    session, user_json,
};
use couchfin_api::{ApiError, JellyfinApi, Password, Session};
use serde_json::json;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn logs_in_with_the_device_header_and_reads_the_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .and(authorization(None))
        .and(body_json(json!({ "Username": "alice", "Pw": "hunter2" })))
        .respond_with(ResponseTemplate::new(200).set_body_json(authentication_result_json()))
        .expect(1)
        .mount(&server)
        .await;

    let result = client()
        .authenticate_by_name(&server_url(&server), "alice", &Password::new("hunter2"))
        .await
        .unwrap();

    assert_eq!(result.access_token.expose_secret(), TOKEN);
    assert_eq!(result.user.name, "alice");
    assert_eq!(result.user.id.as_str(), USER_ID);
    assert_eq!(result.server_id.as_str(), SERVER_ID);
}

#[tokio::test]
async fn a_wrong_password_is_invalid_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Error processing request."))
        .mount(&server)
        .await;

    let error = client()
        .authenticate_by_name(&server_url(&server), "alice", &Password::new("wrong"))
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::InvalidCredentials);
}

#[tokio::test]
async fn a_login_reply_without_a_token_is_not_jellyfin() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "User": user_json() })))
        .mount(&server)
        .await;

    let error = client()
        .authenticate_by_name(&server_url(&server), "alice", &Password::new("hunter2"))
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::NotJellyfin { .. }), "{error:?}");
}

#[tokio::test]
async fn a_token_with_header_breaking_characters_is_refused() {
    let mut reply = authentication_result_json();
    reply["AccessToken"] = json!("abc\", Token=\"other");
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(reply))
        .mount(&server)
        .await;

    let error = client()
        .authenticate_by_name(&server_url(&server), "alice", &Password::new("hunter2"))
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::NotJellyfin { .. }), "{error:?}");
}

#[tokio::test]
async fn current_user_sends_the_token_and_reads_the_user() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .and(authorization(Some(TOKEN)))
        .respond_with(ResponseTemplate::new(200).set_body_json(user_json()))
        .expect(1)
        .mount(&server)
        .await;

    let user = client()
        .current_user(&session(&server, TOKEN))
        .await
        .unwrap();

    assert_eq!(user.name, "alice");
    assert_eq!(user.id.as_str(), USER_ID);
}

#[tokio::test]
async fn a_rejected_token_tells_the_caller_to_log_in_again() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let error = client()
        .current_user(&session(&server, "revoked"))
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::TokenRejected);
}

#[tokio::test]
async fn the_token_from_login_is_sent_on_the_next_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .and(authorization(None))
        .respond_with(ResponseTemplate::new(200).set_body_json(authentication_result_json()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/Users/Me"))
        .and(authorization(Some(TOKEN)))
        .respond_with(ResponseTemplate::new(200).set_body_json(user_json()))
        .expect(1)
        .mount(&server)
        .await;
    let client = client();
    let url = server_url(&server);

    let result = client
        .authenticate_by_name(&url, "alice", &Password::new("hunter2"))
        .await
        .unwrap();
    let session = Session::new(url, result.access_token);
    let user = client.current_user(&session).await.unwrap();

    assert_eq!(user.id, result.user.id);
}

#[tokio::test]
async fn an_unreachable_server_during_login_is_unreachable() {
    let error = client()
        .authenticate_by_name(&common::closed_port_url(), "alice", &Password::new("x"))
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::Unreachable { .. }), "{error:?}");
}

#[test]
fn the_header_constants_name_the_device_couchfin() {
    assert_eq!(couchfin_api::CLIENT_NAME, "Couchfin");
    assert_eq!(couchfin_api::DEVICE_NAME, "Couchfin");
    assert_eq!(couchfin_api::CLIENT_VERSION, env!("CARGO_PKG_VERSION"));
}

#[test]
fn the_client_keeps_its_device_id() {
    assert_eq!(client().device_id(), &common::device_id());
}
