//! Redirects and certificates over HTTPS, against the small rustls server in
//! `common/tls.rs` (wiremock only speaks plain HTTP). A redirect from `https`
//! down to `http` is never followed, on any call.

mod common;

use common::tls::{self, trusting};
use common::{
    authentication_result_json, client, device_id, password, public_info_json, untouched_jellyfin,
    user_json,
};
use couchfin_api::{ApiError, JellyfinApi, JellyfinClient};

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
