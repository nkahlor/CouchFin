//! `check_server` (`GET /System/Info/Public`): the "is this a Jellyfin server?"
//! check, and the
//! failure modes the login screen has to tell apart.

mod common;

use std::time::Duration;

use common::{SERVER_ID, authorization, client, client_with_timeout, public_info_json, server_url};
use couchfin_api::{ApiError, JellyfinApi, ServerUrl};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn reads_the_server_name_version_and_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/System/Info/Public"))
        .and(authorization(None))
        .respond_with(ResponseTemplate::new(200).set_body_json(public_info_json()))
        .expect(1)
        .mount(&server)
        .await;

    let check = client().check_server(&server_url(&server)).await.unwrap();

    assert_eq!(check.server, server_url(&server));
    let info = check.info;
    assert_eq!(info.server_name, "Living Room");
    assert_eq!(info.version, "10.11.2");
    assert_eq!(info.id.as_str(), SERVER_ID);
}

#[tokio::test]
async fn keeps_a_reverse_proxy_base_path() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/jellyfin/System/Info/Public"))
        .respond_with(ResponseTemplate::new(200).set_body_json(public_info_json()))
        .expect(1)
        .mount(&server)
        .await;
    let url: ServerUrl = format!("{}/jellyfin", server.uri()).parse().unwrap();

    let check = client().check_server(&url).await.unwrap();

    assert_eq!(check.server, url);
    assert_eq!(check.info.server_name, "Living Room");
}

#[tokio::test]
async fn a_web_page_is_not_a_jellyfin_server() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>router login</html>"))
        .mount(&server)
        .await;

    let error = client()
        .check_server(&server_url(&server))
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::NotJellyfin { .. }), "{error:?}");
}

#[tokio::test]
async fn json_without_the_jellyfin_fields_is_not_a_jellyfin_server() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
        .mount(&server)
        .await;

    let error = client()
        .check_server(&server_url(&server))
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::NotJellyfin { .. }), "{error:?}");
}

#[tokio::test]
async fn a_missing_endpoint_is_not_a_jellyfin_server() {
    let server = MockServer::start().await;

    let error = client()
        .check_server(&server_url(&server))
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::NotJellyfin { .. }), "{error:?}");
}

#[tokio::test]
async fn a_server_error_reports_its_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let error = client()
        .check_server(&server_url(&server))
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::Status { status: 503 });
}

#[tokio::test]
async fn a_closed_port_is_unreachable() {
    let error = client()
        .check_server(&common::closed_port_url())
        .await
        .unwrap_err();

    let ApiError::Unreachable { detail } = &error else {
        panic!("expected Unreachable, got {error:?}");
    };
    assert_ne!(detail, "");
}

#[tokio::test]
async fn a_slow_server_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(public_info_json())
                .set_delay(Duration::from_secs(5)),
        )
        .mount(&server)
        .await;

    let error = client_with_timeout(Duration::from_millis(200))
        .check_server(&server_url(&server))
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::Timeout);
}

#[test]
fn every_error_has_a_message() {
    let errors = [
        ApiError::Unreachable {
            detail: "connection refused".into(),
        },
        ApiError::Timeout,
        ApiError::NotJellyfin {
            detail: "no ServerName".into(),
        },
        ApiError::InvalidCredentials,
        ApiError::TokenRejected,
        ApiError::Status { status: 500 },
        ApiError::Redirected { server: None },
        ApiError::InsecureRedirect,
    ];
    for error in errors {
        assert!(!error.to_string().is_empty(), "{error:?}");
    }
}

/// Headers promising 1000 bytes of body, followed by only a few.
const CUT_OFF_REPLY: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1000\r\n\r\n{\"ServerName\":";

#[tokio::test]
async fn a_reply_cut_off_mid_body_is_not_reported_as_unreachable() {
    let url = common::raw_http::start(CUT_OFF_REPLY, Duration::ZERO);

    let error = client().check_server(&url).await.unwrap_err();

    let ApiError::NotJellyfin { detail } = &error else {
        panic!("expected NotJellyfin, got {error:?}");
    };
    assert!(detail.contains("cut off"), "{detail}");
}

#[tokio::test]
async fn a_body_that_stops_arriving_times_out() {
    let url = common::raw_http::start(CUT_OFF_REPLY, Duration::from_secs(5));

    let error = client_with_timeout(Duration::from_millis(300))
        .check_server(&url)
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::Timeout);
}
