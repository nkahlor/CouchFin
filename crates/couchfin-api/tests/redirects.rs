//! Redirects. A reverse proxy that answers `http://` with `301` to
//! `https://` must never turn a login into "wrong password":
//!
//! - `check_server` follows redirects (a GET with no token) and returns the
//!   address the server really answers on;
//! - login and token calls never follow one, and say where it pointed;
//! - nothing follows a redirect from `https` down to `http`.

mod common;

use common::tls::{self, TlsServer};
use common::{
    authentication_result_json, client, device_id, public_info_json, server_url, session, user_json,
};
use couchfin_api::{ApiError, JellyfinApi, JellyfinClient, Password, ServerUrl};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ENDPOINTS: [&str; 3] = [
    "/System/Info/Public",
    "/Users/AuthenticateByName",
    "/Users/Me",
];

/// `from` redirects every endpoint to the same path under `to`, the way a
/// proxy forcing HTTPS does.
async fn redirect_all(from: &MockServer, to: &str, status: u16) {
    for endpoint in ENDPOINTS {
        Mock::given(path(endpoint))
            .respond_with(
                ResponseTemplate::new(status).insert_header("Location", format!("{to}{endpoint}")),
            )
            .mount(from)
            .await;
    }
}

/// A Jellyfin mock that must not receive any request.
async fn untouched_jellyfin() -> MockServer {
    let jellyfin = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&jellyfin)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&jellyfin)
        .await;
    jellyfin
}

fn trusting(server: &TlsServer) -> JellyfinClient {
    JellyfinClient::builder(device_id())
        .add_root_certificates_pem(server.ca_pem.as_bytes())
        .expect("CA certificate parses")
        .build()
        .expect("client builds")
}

fn password() -> Password {
    Password::new("hunter2")
}

#[tokio::test]
async fn check_server_follows_a_redirect_and_returns_the_canonical_address() {
    let proxy = MockServer::start().await;
    let jellyfin = MockServer::start().await;
    let canonical = format!("{}/jellyfin", jellyfin.uri());
    redirect_all(&proxy, &canonical, 301).await;
    Mock::given(path("/jellyfin/System/Info/Public"))
        .respond_with(ResponseTemplate::new(200).set_body_json(public_info_json()))
        .expect(1)
        .mount(&jellyfin)
        .await;

    let check = client().check_server(&server_url(&proxy)).await.unwrap();

    assert_eq!(check.server.as_str(), format!("{canonical}/"));
    assert_eq!(check.info.server_name, "Living Room");
}

#[tokio::test]
async fn check_server_follows_a_relative_redirect_on_the_same_host() {
    let server = MockServer::start().await;
    Mock::given(path("/System/Info/Public"))
        .respond_with(
            ResponseTemplate::new(308).insert_header("Location", "/jellyfin/System/Info/Public"),
        )
        .mount(&server)
        .await;
    Mock::given(path("/jellyfin/System/Info/Public"))
        .respond_with(ResponseTemplate::new(200).set_body_json(public_info_json()))
        .mount(&server)
        .await;

    let check = client().check_server(&server_url(&server)).await.unwrap();

    assert_eq!(check.server.as_str(), format!("{}/jellyfin/", server.uri()));
}

#[tokio::test]
async fn the_canonical_address_then_logs_in() {
    let proxy = MockServer::start().await;
    let jellyfin = MockServer::start().await;
    redirect_all(&proxy, &jellyfin.uri(), 301).await;
    Mock::given(path("/System/Info/Public"))
        .respond_with(ResponseTemplate::new(200).set_body_json(public_info_json()))
        .mount(&jellyfin)
        .await;
    Mock::given(method("POST"))
        .and(path("/Users/AuthenticateByName"))
        .and(common::authorization(None))
        .respond_with(ResponseTemplate::new(200).set_body_json(authentication_result_json()))
        .expect(1)
        .mount(&jellyfin)
        .await;
    let client = client();

    let check = client.check_server(&server_url(&proxy)).await.unwrap();
    let login = client
        .authenticate_by_name(&check.server, "alice", &password())
        .await
        .unwrap();

    assert_eq!(login.user.name, "alice");
}

#[tokio::test]
async fn a_redirected_login_is_not_followed_and_names_the_new_address() {
    let proxy = MockServer::start().await;
    let jellyfin = untouched_jellyfin().await;
    redirect_all(&proxy, &jellyfin.uri(), 301).await;

    let error = client()
        .authenticate_by_name(&server_url(&proxy), "alice", &password())
        .await
        .unwrap_err();

    assert_eq!(
        error,
        ApiError::Redirected {
            server: Some(server_url(&jellyfin))
        }
    );
    assert_ne!(error.to_string(), "");
}

#[tokio::test]
async fn a_redirected_token_call_is_not_followed() {
    let proxy = MockServer::start().await;
    let jellyfin = untouched_jellyfin().await;
    redirect_all(&proxy, &jellyfin.uri(), 302).await;

    let error = client()
        .current_user(&session(&proxy, common::TOKEN))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        ApiError::Redirected {
            server: Some(server_url(&jellyfin))
        }
    );
}

#[tokio::test]
async fn a_redirect_to_some_other_page_has_no_address_to_suggest() {
    let proxy = MockServer::start().await;
    Mock::given(path("/Users/AuthenticateByName"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "/login?next=%2F"))
        .mount(&proxy)
        .await;

    let error = client()
        .authenticate_by_name(&server_url(&proxy), "alice", &password())
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::Redirected { server: None });
    assert_ne!(error.to_string(), "");
}

#[tokio::test]
async fn check_server_redirected_away_from_the_api_is_not_jellyfin() {
    let proxy = MockServer::start().await;
    let other = MockServer::start().await;
    Mock::given(path("/System/Info/Public"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("Location", format!("{}/elsewhere", other.uri())),
        )
        .mount(&proxy)
        .await;
    Mock::given(path("/elsewhere"))
        .respond_with(ResponseTemplate::new(200).set_body_json(public_info_json()))
        .mount(&other)
        .await;

    let error = client()
        .check_server(&server_url(&proxy))
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::NotJellyfin { .. }), "{error:?}");
}

#[tokio::test]
async fn a_redirect_loop_is_not_jellyfin() {
    let server = MockServer::start().await;
    redirect_all(&server, &server.uri(), 307).await;

    let error = client()
        .check_server(&server_url(&server))
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::NotJellyfin { .. }), "{error:?}");
}

#[tokio::test]
async fn a_redirect_without_a_location_is_not_jellyfin() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(301))
        .mount(&server)
        .await;

    let error = client()
        .check_server(&server_url(&server))
        .await
        .unwrap_err();

    assert!(matches!(error, ApiError::NotJellyfin { .. }), "{error:?}");
}

#[tokio::test]
async fn check_server_works_over_https() {
    let server = tls::start(|_, _| tls::ok_json(&public_info_json()));

    let check = trusting(&server).check_server(&server.url).await.unwrap();

    assert_eq!(check.server, server.url);
}

#[tokio::test]
async fn an_untrusted_certificate_is_unreachable() {
    let server = tls::start(|_, _| tls::ok_json(&public_info_json()));

    let error = client().check_server(&server.url).await.unwrap_err();

    assert!(matches!(error, ApiError::Unreachable { .. }), "{error:?}");
}

#[tokio::test]
async fn check_server_refuses_a_redirect_from_https_to_http() {
    let jellyfin = untouched_jellyfin().await;
    let location = format!("{}/System/Info/Public", jellyfin.uri());
    let server = tls::start(move |_, _| tls::redirect(301, &location));

    let error = trusting(&server)
        .check_server(&server.url)
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::InsecureRedirect);
    assert_ne!(error.to_string(), "");
}

#[tokio::test]
async fn a_login_redirected_from_https_to_http_is_insecure() {
    let jellyfin = untouched_jellyfin().await;
    let base = jellyfin.uri();
    let server = tls::start(move |_, path| tls::redirect(308, &format!("{base}{path}")));

    let error = trusting(&server)
        .authenticate_by_name(&server.url, "alice", &password())
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::InsecureRedirect);
}

#[tokio::test]
async fn a_login_over_https_with_a_trusted_certificate_succeeds() {
    let server = tls::start(|method, path| match (method, path) {
        ("POST", "/Users/AuthenticateByName") => tls::ok_json(&authentication_result_json()),
        _ => tls::ok_json(&user_json()),
    });
    let client = trusting(&server);

    let login = client
        .authenticate_by_name(&server.url, "alice", &password())
        .await
        .unwrap();

    assert_eq!(login.user.name, "alice");
}

#[test]
fn a_certificate_that_is_not_pem_is_refused() {
    let error = JellyfinClient::builder(device_id())
        .add_root_certificates_pem(b"not a certificate")
        .unwrap_err();
    assert_ne!(error.to_string(), "");
}

#[test]
fn a_redirect_error_can_name_a_server() {
    let server: ServerUrl = "https://jellyfin.example.com".parse().unwrap();
    let error = ApiError::Redirected {
        server: Some(server),
    };
    assert!(
        error.to_string().contains("https://jellyfin.example.com/"),
        "{error}"
    );
}
