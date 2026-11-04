use std::time::Duration;

use reqwest::{
    Client, Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};

use crate::api::models::AuthResponse;
use crate::api::{ApiError, secret::Secret};

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct AuthRequest<'a> {
    username: &'a str,
    pw: &'a str,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct PublicInfoResponse {
    pub local_address: String,
    pub server_name: String,
    pub version: String,
    pub product_name: String,
    pub operating_system: String,
    pub id: String,
    pub startup_wizard_completed: bool,
}

pub struct JellyfinClient {
    base_url: Url,
    client: Client,
}

impl JellyfinClient {
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
    const CLIENT_NAME: &str = "CouchFin";

    const DEVICE_NAME: &str = "CouchFin";
    const DEVICE_ID: &str = "couchfin-dev";

    pub fn new(base_url: &str) -> Result<Self, ApiError> {
        let mut base_url = Url::parse(base_url)?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, Self::auth_header(None));

        let client = Client::builder()
            .timeout(Self::DEFAULT_TIMEOUT)
            .default_headers(headers)
            .build()?;
        Ok(Self { base_url, client })
    }

    fn auth_header(token: Option<&str>) -> HeaderValue {
        let mut value = format!(
            r#"MediaBrowser Client="{}", Device="{}", DeviceId="{}", Version="{}""#,
            Self::CLIENT_NAME,
            Self::DEVICE_NAME,
            Self::DEVICE_ID,
            env!("CARGO_PKG_VERSION"),
        );
        if let Some(token) = token {
            value.push_str(&format!(r#", Token="{token}""#));
        }
        let mut header = HeaderValue::from_str(&value).expect("auth header is valid ASCII");
        header.set_sensitive(true);
        header
    }

    fn endpoint(&self, path: &str) -> Result<Url, ApiError> {
        Ok(self.base_url.join(path)?)
    }

    pub async fn get_public_info(&self) -> Result<PublicInfoResponse, ApiError> {
        let url = self.endpoint("System/Info/Public")?;

        Ok(self
            .client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<PublicInfoResponse>()
            .await?)
    }

    pub async fn login_by_name(
        &self,
        username: &str,
        password: Secret,
    ) -> Result<AuthResponse, ApiError> {
        let url = self.endpoint("Users/AuthenticateByName")?;
        let credentials = AuthRequest {
            username,
            pw: password.expose(),
        };

        Ok(self
            .client
            .post(url)
            .json::<AuthRequest>(&credentials)
            .send()
            .await?
            .error_for_status()?
            .json::<AuthResponse>()
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, header_exists, header_regex, method, path},
    };

    mod get_public_info {
        use super::*;

        #[tokio::test]
        async fn fetches_and_deserializes_response() {
            let mock_server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/System/Info/Public"))
                .respond_with(ResponseTemplate::new(200).set_body_raw(
                    r#"{
                    "LocalAddress": "127.0.0.1",
                    "ServerName": "Test Jellyfin",
                    "Version": "10.9.6",
                    "ProductName": "Jellyfin Server",
                    "OperatingSystem": "Linux",
                    "Id": "abc-123",
                    "StartupWizardCompleted": true
                }"#,
                    "application/json",
                ))
                .mount(&mock_server)
                .await;

            let client = JellyfinClient::new(&mock_server.uri()).unwrap();
            let info = client.get_public_info().await.unwrap();

            assert_eq!(info.local_address, "127.0.0.1");
            assert_eq!(info.server_name, "Test Jellyfin");
            assert_eq!(info.version, "10.9.6");
            assert_eq!(info.product_name, "Jellyfin Server");
            assert_eq!(info.operating_system, "Linux");
            assert_eq!(info.id, "abc-123");
            assert!(info.startup_wizard_completed);
        }

        #[tokio::test]
        async fn maps_http_error_to_api_error() {
            let mock_server = MockServer::start().await;

            Mock::given(method("GET"))
                .and(path("/System/Info/Public"))
                .respond_with(ResponseTemplate::new(503))
                .mount(&mock_server)
                .await;

            let client = JellyfinClient::new(&mock_server.uri()).unwrap();

            let err = client.get_public_info().await.unwrap_err();
            assert!(matches!(err, ApiError::Request(_)));
        }
    }

    mod login_by_name {
        use super::*;
        use serde_json::json;

        const AUTH_RESPONSE: &str = include_str!("fixtures/authenticate_by_name.json");

        async fn mock_login(server: &MockServer, response: ResponseTemplate) {
            Mock::given(method("POST"))
                .and(path("/Users/AuthenticateByName"))
                .respond_with(response)
                .mount(server)
                .await;
        }

        fn status_of(err: &ApiError) -> Option<u16> {
            match err {
                ApiError::Request(e) => e.status().map(|s| s.as_u16()),
                _ => None,
            }
        }

        #[tokio::test]
        async fn posts_credentials_and_deserializes_response() {
            let mock_server = MockServer::start().await;

            Mock::given(method("POST"))
                .and(path("/Users/AuthenticateByName"))
                .and(header_exists("Authorization"))
                .and(body_json(json!({ "Username": "alice", "Pw": "hunter2" })))
                .respond_with(
                    ResponseTemplate::new(200).set_body_raw(AUTH_RESPONSE, "application/json"),
                )
                .expect(1)
                .mount(&mock_server)
                .await;

            let client = JellyfinClient::new(&mock_server.uri()).unwrap();
            let auth = client
                .login_by_name("alice", Secret::new("hunter2"))
                .await
                .unwrap();

            assert_eq!(auth.access_token, "4f2a9c1e8b7d6a5f4e3d2c1b0a998877");
            assert_eq!(auth.server_id, "45ce9e8ebf6343688c18ede5cbc61080");
            assert_eq!(auth.user.id, "a1b2c3d4e5f60718293a4b5c6d7e8f90");
            assert_eq!(auth.user.name.as_deref(), Some("alice"));
            assert_eq!(auth.session_info.user_id, auth.user.id);
            assert_eq!(auth.session_info.device_id.as_deref(), Some("couchfin-dev"));

            let policy = auth.user.policy.expect("policy present");
            assert!(!policy.is_administrator);
            let config = auth.user.configuration.expect("configuration present");
            assert_eq!(config.audio_language_preference, None);
        }

        #[tokio::test]
        async fn serializes_special_characters_in_credentials() {
            let mock_server = MockServer::start().await;

            Mock::given(method("POST"))
                .and(path("/Users/AuthenticateByName"))
                .and(body_json(
                    json!({ "Username": "zoë", "Pw": r#"p"a\ss {word}"# }),
                ))
                .respond_with(
                    ResponseTemplate::new(200).set_body_raw(AUTH_RESPONSE, "application/json"),
                )
                .expect(1)
                .mount(&mock_server)
                .await;

            let client = JellyfinClient::new(&mock_server.uri()).unwrap();
            client
                .login_by_name("zoë", Secret::new(r#"p"a\ss {word}"#))
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn works_when_server_is_under_a_subpath() {
            let mock_server = MockServer::start().await;

            Mock::given(method("POST"))
                .and(path("/jellyfin/Users/AuthenticateByName"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_raw(AUTH_RESPONSE, "application/json"),
                )
                .expect(1)
                .mount(&mock_server)
                .await;

            let client = JellyfinClient::new(&format!("{}/jellyfin", mock_server.uri())).unwrap();
            client
                .login_by_name("alice", Secret::new("hunter2"))
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn wrong_credentials_return_401_error() {
            let mock_server = MockServer::start().await;
            mock_login(&mock_server, ResponseTemplate::new(401)).await;

            let client = JellyfinClient::new(&mock_server.uri()).unwrap();
            let err = client
                .login_by_name("alice", Secret::new("wrong"))
                .await
                .unwrap_err();

            assert!(matches!(err, ApiError::Request(_)), "got {err:?}");
            assert_eq!(status_of(&err), Some(401));
        }

        #[tokio::test]
        async fn bad_request_returns_400_error() {
            let mock_server = MockServer::start().await;
            mock_login(
                &mock_server,
                ResponseTemplate::new(400).set_body_string("Error processing request."),
            )
            .await;

            let client = JellyfinClient::new(&mock_server.uri()).unwrap();
            let err = client.login_by_name("", Secret::new("")).await.unwrap_err();

            assert_eq!(status_of(&err), Some(400));
        }

        #[tokio::test]
        async fn server_error_returns_500_error() {
            let mock_server = MockServer::start().await;
            mock_login(&mock_server, ResponseTemplate::new(500)).await;

            let client = JellyfinClient::new(&mock_server.uri()).unwrap();
            let err = client
                .login_by_name("alice", Secret::new("hunter2"))
                .await
                .unwrap_err();

            assert_eq!(status_of(&err), Some(500));
        }

        #[tokio::test]
        async fn malformed_json_is_a_decode_error() {
            let mock_server = MockServer::start().await;
            mock_login(
                &mock_server,
                ResponseTemplate::new(200).set_body_raw("{ not json", "application/json"),
            )
            .await;

            let client = JellyfinClient::new(&mock_server.uri()).unwrap();
            let err = client
                .login_by_name("alice", Secret::new("hunter2"))
                .await
                .unwrap_err();

            let ApiError::Request(e) = &err else {
                panic!("expected Request error, got {err:?}")
            };
            assert!(e.is_decode(), "expected decode error, got {e:?}");
        }

        #[tokio::test]
        async fn response_missing_access_token_is_a_decode_error() {
            let mock_server = MockServer::start().await;
            let mut body: serde_json::Value = serde_json::from_str(AUTH_RESPONSE).unwrap();
            body.as_object_mut().unwrap().remove("AccessToken");
            mock_login(&mock_server, ResponseTemplate::new(200).set_body_json(body)).await;

            let client = JellyfinClient::new(&mock_server.uri()).unwrap();
            let err = client
                .login_by_name("alice", Secret::new("hunter2"))
                .await
                .unwrap_err();

            let ApiError::Request(e) = &err else {
                panic!("expected Request error, got {err:?}")
            };
            assert!(e.is_decode(), "expected decode error, got {e:?}");
        }

        #[tokio::test]
        async fn unreachable_server_is_unreachable_error() {
            let port = std::net::TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port();

            let client = JellyfinClient::new(&format!("http://127.0.0.1:{port}")).unwrap();
            let err = client
                .login_by_name("alice", Secret::new("hunter2"))
                .await
                .unwrap_err();

            assert!(matches!(err, ApiError::Unreachable(_)), "got {err:?}");
        }
    }

    #[tokio::test]
    async fn sends_client_identification_header() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/System/Info/Public"))
            .and(header_regex(
                "Authorization",
                r#"^MediaBrowser Client="CouchFin", Device="[^"]+", DeviceId="[^"]+", Version="[^"]+"$"#,
            ))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let client = JellyfinClient::new(&mock_server.uri()).unwrap();
        let err = client.get_public_info().await.unwrap_err();

        let ApiError::Request(e) = err else {
            panic!("unexpected error: {err:?}")
        };
        assert_eq!(e.status().map(|s| s.as_u16()), Some(503));
    }
}
