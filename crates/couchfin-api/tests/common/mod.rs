//! Helpers shared by the wiremock tests. Each test binary uses a different
//! subset, so unused ones are allowed.
#![allow(dead_code)]

use std::time::Duration;

use couchfin_api::{AccessToken, DeviceId, JellyfinClient, ServerUrl, Session};
use serde_json::{Value, json};
use wiremock::{Match, MockServer, Request};

pub const DEVICE_ID: &str = "3f2b9c1e-8d4a-4b6f-9e2d-7a1c5b8e0f42";
pub const TOKEN: &str = "0123456789abcdef0123456789abcdef";
pub const USER_ID: &str = "38a5a5bbdc3549ccb0b07fc7e43c1d43";
pub const SERVER_ID: &str = "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f";

pub fn device_id() -> DeviceId {
    DEVICE_ID.parse().expect("DEVICE_ID is valid")
}

pub fn client() -> JellyfinClient {
    JellyfinClient::new(device_id()).expect("client builds")
}

pub fn client_with_timeout(timeout: Duration) -> JellyfinClient {
    JellyfinClient::with_timeout(device_id(), timeout).expect("client builds")
}

pub fn server_url(server: &MockServer) -> ServerUrl {
    server.uri().parse().expect("wiremock URI is valid")
}

pub fn session(server: &MockServer, token: &str) -> Session {
    Session::new(
        server_url(server),
        AccessToken::new(token).expect("test token is valid"),
    )
}

/// The exact `Authorization` header Jellyfin should receive. Jellyfin reads
/// `Device` as the name it shows under Dashboard > Devices.
pub fn expected_authorization(token: Option<&str>) -> String {
    let header = format!(
        "MediaBrowser Client=\"Couchfin\", Device=\"Couchfin\", DeviceId=\"{DEVICE_ID}\", Version=\"{}\"",
        env!("CARGO_PKG_VERSION")
    );
    match token {
        Some(token) => format!("{header}, Token=\"{token}\""),
        None => header,
    }
}

/// Matches the whole `Authorization` header. wiremock's own `header` matcher
/// splits values on commas, which would let a header with missing or extra
/// parts through.
pub struct Authorization(String);

impl Match for Authorization {
    fn matches(&self, request: &Request) -> bool {
        let values: Vec<_> = request.headers.get_all("authorization").iter().collect();
        values.len() == 1 && values[0].to_str().is_ok_and(|v| v == self.0)
    }
}

pub fn authorization(token: Option<&str>) -> Authorization {
    Authorization(expected_authorization(token))
}

/// A trimmed copy of what Jellyfin 10.11 returns from `/System/Info/Public`.
pub fn public_info_json() -> Value {
    json!({
        "LocalAddress": "http://192.168.1.20:8096",
        "ServerName": "Living Room",
        "Version": "10.11.2",
        "ProductName": "Jellyfin Server",
        "OperatingSystem": "",
        "Id": SERVER_ID,
        "StartupWizardCompleted": true
    })
}

/// A trimmed copy of Jellyfin's `UserDto`.
pub fn user_json() -> Value {
    json!({
        "Name": "alice",
        "ServerId": SERVER_ID,
        "Id": USER_ID,
        "HasPassword": true,
        "HasConfiguredPassword": true,
        "EnableAutoLogin": false,
        "Policy": { "IsAdministrator": false }
    })
}

/// A trimmed copy of Jellyfin's `AuthenticationResult`.
pub fn authentication_result_json() -> Value {
    json!({
        "User": user_json(),
        "SessionInfo": { "Id": "abc", "DeviceName": "Couchfin" },
        "AccessToken": TOKEN,
        "ServerId": SERVER_ID
    })
}

/// A local URL nothing listens on: bind a port, then free it.
pub fn closed_port_url() -> ServerUrl {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("can bind a local port");
    let port = listener
        .local_addr()
        .expect("bound socket has an address")
        .port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
        .parse()
        .expect("local URL is valid")
}
