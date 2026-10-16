//! The real HTTP client, built on reqwest with rustls.

use std::time::Duration;

use reqwest::header::AUTHORIZATION;
use reqwest::{RequestBuilder, Response, StatusCode, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::api::{JellyfinApi, Session};
use crate::auth_header::authorization;
use crate::builder::{BuildError, JellyfinClientBuilder};
use crate::dto::{AuthenticationResult, ServerCheck, User};
use crate::error::ApiError;
use crate::ids::DeviceId;
use crate::redirect::{self, MAX_REDIRECTS};
use crate::secret::{AccessToken, Password};
use crate::server_url::ServerUrl;

const PUBLIC_INFO: &str = "System/Info/Public";
const AUTHENTICATE: &str = "Users/AuthenticateByName";
const ME: &str = "Users/Me";

/// The Jellyfin HTTP client.
///
/// It holds no login state: each call names its server, or takes a
/// [`Session`] once logged in, so one client serves every server and user.
/// Every request carries the `Authorization: MediaBrowser ...` header with
/// this install's [`DeviceId`]. Cloning is cheap and shares the connection
/// pool.
///
/// Only [`JellyfinApi::check_server`] follows redirects; see there.
///
/// # Examples
///
/// The address typed here redirects, as an HTTPS-forcing proxy would, and
/// `check_server` returns the address to use from then on:
///
/// ```
/// use couchfin_api::{DeviceId, JellyfinApi, JellyfinClient, ServerUrl};
/// use wiremock::matchers::path;
/// use wiremock::{Mock, MockServer, ResponseTemplate};
///
/// # tokio::runtime::Runtime::new()?.block_on(async {
/// let jellyfin = MockServer::start().await;
/// Mock::given(path("/jellyfin/System/Info/Public"))
///     .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
///         "ServerName": "Living Room",
///         "Version": "10.11.2",
///         "Id": "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f",
///     })))
///     .mount(&jellyfin)
///     .await;
/// let old = MockServer::start().await;
/// Mock::given(path("/System/Info/Public"))
///     .respond_with(ResponseTemplate::new(301).insert_header(
///         "Location",
///         format!("{}/jellyfin/System/Info/Public", jellyfin.uri()),
///     ))
///     .mount(&old)
///     .await;
///
/// let client = JellyfinClient::new(DeviceId::generate())?;
/// let typed: ServerUrl = old.uri().parse()?;
/// let check = client.check_server(&typed).await?;
/// assert_eq!(check.info.server_name, "Living Room");
/// assert_eq!(check.server.as_str(), format!("{}/jellyfin/", jellyfin.uri()));
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

    /// Creates a client with the default settings.
    ///
    /// # Errors
    ///
    /// [`BuildError`] if TLS can't be set up.
    pub fn new(device_id: DeviceId) -> Result<Self, BuildError> {
        Self::builder(device_id).build()
    }

    /// Starts configuring a client, for a custom timeout or extra trusted
    /// certificates.
    #[must_use]
    pub fn builder(device_id: DeviceId) -> JellyfinClientBuilder {
        JellyfinClientBuilder::new(device_id)
    }

    pub(crate) fn from_parts(http: reqwest::Client, device_id: DeviceId) -> Self {
        Self { http, device_id }
    }

    /// The device ID this client sends.
    #[must_use]
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    async fn send(
        &self,
        request: RequestBuilder,
        token: Option<&AccessToken>,
    ) -> Result<Response, ApiError> {
        request
            .header(AUTHORIZATION, authorization(&self.device_id, token))
            .send()
            .await
            .map_err(|e| ApiError::transport(&e))
    }

    /// A login or token call: one request, redirects refused.
    async fn send_once(
        &self,
        request: RequestBuilder,
        endpoint: &str,
        token: Option<&AccessToken>,
    ) -> Result<Response, ApiError> {
        let response = self.send(request, token).await?;
        match redirect::target(&response)? {
            Some(target) => Err(ApiError::Redirected {
                server: ServerUrl::from_endpoint(&target, endpoint),
            }),
            None => Ok(response),
        }
    }

    /// Follows redirects from `url` (GET, no token) to the final reply.
    async fn get_following(&self, mut url: Url) -> Result<Response, ApiError> {
        for _ in 0..=MAX_REDIRECTS {
            let response = self.send(self.http.get(url.clone()), None).await?;
            match redirect::target(&response)? {
                Some(target) => url = target,
                None => return Ok(response),
            }
        }
        Err(ApiError::NotJellyfin {
            detail: format!("more than {MAX_REDIRECTS} redirects"),
        })
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

/// Reads a JSON body. A body that is cut off or isn't the JSON Jellyfin sends
/// means this isn't (a compatible) Jellyfin; a timeout while reading stays a
/// timeout.
async fn read_json<T: DeserializeOwned>(response: Response) -> Result<T, ApiError> {
    let bytes = response.bytes().await.map_err(|e| ApiError::body(&e))?;
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
    async fn check_server(&self, address: &ServerUrl) -> Result<ServerCheck, ApiError> {
        let response = self.get_following(address.endpoint(PUBLIC_INFO)).await?;
        check_status(&response, ApiError::Status { status: 401 })?;
        let server = ServerUrl::from_endpoint(response.url(), PUBLIC_INFO).ok_or_else(|| {
            ApiError::NotJellyfin {
                detail: format!("redirected away from the API, to {}", response.url()),
            }
        })?;
        let info = read_json(response).await?;
        Ok(ServerCheck { server, info })
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
        let request = self.http.post(server.endpoint(AUTHENTICATE)).json(&body);
        let response = self.send_once(request, AUTHENTICATE, None).await?;
        check_status(&response, ApiError::InvalidCredentials)?;
        read_json(response).await
    }

    async fn current_user(&self, session: &Session) -> Result<User, ApiError> {
        let request = self.http.get(session.server().endpoint(ME));
        let response = self.send_once(request, ME, Some(session.token())).await?;
        check_status(&response, ApiError::TokenRejected)?;
        read_json(response).await
    }
}
