//! The real HTTP client, built on reqwest with rustls.

use std::time::Duration;

use reqwest::header::AUTHORIZATION;
use reqwest::{Response, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::api::{JellyfinApi, Session};
use crate::auth_header::authorization;
use crate::dto::{AuthenticationResult, PublicSystemInfo, User};
use crate::error::ApiError;
use crate::ids::DeviceId;
use crate::secret::{AccessToken, Password};
use crate::server_url::ServerUrl;

/// Building the HTTP client failed, which only happens if TLS can't be set
/// up on this system.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("couldn't set up the HTTP client: {0}")]
pub struct BuildError(String);

/// The Jellyfin HTTP client.
///
/// It holds no login state: each call names its server, or takes a
/// [`Session`] once logged in, so one client serves every server and user.
/// Every request carries the `Authorization: MediaBrowser ...` header with
/// this install's [`DeviceId`]. Cloning is cheap and shares the connection
/// pool.
///
/// # Examples
///
/// ```
/// use couchfin_api::{DeviceId, JellyfinApi, JellyfinClient, ServerUrl};
/// use wiremock::matchers::path;
/// use wiremock::{Mock, MockServer, ResponseTemplate};
///
/// # tokio::runtime::Runtime::new()?.block_on(async {
/// let server = MockServer::start().await;
/// Mock::given(path("/System/Info/Public"))
///     .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
///         "ServerName": "Living Room",
///         "Version": "10.11.2",
///         "Id": "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f",
///     })))
///     .mount(&server)
///     .await;
///
/// let client = JellyfinClient::new(DeviceId::generate())?;
/// let url: ServerUrl = server.uri().parse()?;
/// let info = client.public_system_info(&url).await?;
/// assert_eq!(info.server_name, "Living Room");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # })?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct JellyfinClient {
    http: reqwest::Client,
    device_id: DeviceId,
}

impl JellyfinClient {
    /// How long a request may take, from connecting to reading the whole
    /// reply, before it fails with [`ApiError::Timeout`]. Long enough for a
    /// slow server over Wi-Fi, short enough that a wrong address doesn't
    /// leave the login screen hanging.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

    /// Creates a client with [`JellyfinClient::DEFAULT_TIMEOUT`].
    ///
    /// # Errors
    ///
    /// [`BuildError`] if TLS can't be set up.
    pub fn new(device_id: DeviceId) -> Result<Self, BuildError> {
        Self::with_timeout(device_id, Self::DEFAULT_TIMEOUT)
    }

    /// Creates a client with a custom request timeout.
    ///
    /// # Errors
    ///
    /// [`BuildError`] if TLS can't be set up.
    pub fn with_timeout(device_id: DeviceId, timeout: Duration) -> Result<Self, BuildError> {
        // reqwest uses the process-wide rustls provider. Installing ring only
        // fails if a provider is already installed, which is fine to use.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| BuildError(e.to_string()))?;
        Ok(Self { http, device_id })
    }

    /// The device ID this client sends.
    #[must_use]
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    async fn get(
        &self,
        server: &ServerUrl,
        path: &str,
        token: Option<&AccessToken>,
    ) -> Result<Response, ApiError> {
        self.http
            .get(server.endpoint(path))
            .header(AUTHORIZATION, authorization(&self.device_id, token))
            .send()
            .await
            .map_err(|e| ApiError::transport(&e))
    }

    async fn post<B: Serialize + Sync>(
        &self,
        server: &ServerUrl,
        path: &str,
        body: &B,
    ) -> Result<Response, ApiError> {
        self.http
            .post(server.endpoint(path))
            .header(AUTHORIZATION, authorization(&self.device_id, None))
            .json(body)
            .send()
            .await
            .map_err(|e| ApiError::transport(&e))
    }
}

/// Turns an unsuccessful status into an error. `on_unauthorized` is what a
/// 401 means for this endpoint. Every endpoint used here exists on every
/// Jellyfin server, so a 404 means the address points at something else.
fn check_status(response: &Response, on_unauthorized: ApiError) -> Result<(), ApiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    Err(match status {
        StatusCode::UNAUTHORIZED => on_unauthorized,
        StatusCode::NOT_FOUND => ApiError::NotJellyfin {
            detail: format!("{} not found", response.url().path()),
        },
        other => ApiError::Status {
            status: other.as_u16(),
        },
    })
}

/// Reads a JSON body. A body that isn't the JSON Jellyfin sends means this
/// isn't (a compatible) Jellyfin.
async fn read_json<T: DeserializeOwned>(response: Response) -> Result<T, ApiError> {
    let bytes = response
        .bytes()
        .await
        .map_err(|e| ApiError::transport(&e))?;
    serde_json::from_slice(&bytes).map_err(|e| ApiError::NotJellyfin {
        detail: e.to_string(),
    })
}

/// The login request body. Jellyfin calls the password `Pw`.
#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct AuthenticateByName<'a> {
    username: &'a str,
    pw: &'a str,
}

impl JellyfinApi for JellyfinClient {
    async fn public_system_info(&self, server: &ServerUrl) -> Result<PublicSystemInfo, ApiError> {
        let response = self.get(server, "System/Info/Public", None).await?;
        check_status(&response, ApiError::Status { status: 401 })?;
        read_json(response).await
    }

    async fn authenticate_by_name(
        &self,
        server: &ServerUrl,
        username: &str,
        password: &Password,
    ) -> Result<AuthenticationResult, ApiError> {
        let body = AuthenticateByName {
            username,
            pw: password.expose_secret(),
        };
        let response = self.post(server, "Users/AuthenticateByName", &body).await?;
        check_status(&response, ApiError::InvalidCredentials)?;
        read_json(response).await
    }

    async fn current_user(&self, session: &Session) -> Result<User, ApiError> {
        let response = self
            .get(session.server(), "Users/Me", Some(session.token()))
            .await?;
        check_status(&response, ApiError::TokenRejected)?;
        read_json(response).await
    }
}
