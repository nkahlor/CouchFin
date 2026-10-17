//! The real HTTP client, built on reqwest with rustls.

use std::time::Duration;

use reqwest::header::AUTHORIZATION;
use reqwest::{RequestBuilder, Response, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::api::{JellyfinApi, Session};
use crate::auth_header::authorization;
use crate::builder::{BuildError, JellyfinClientBuilder};
use crate::dto::{AuthenticationResult, ServerCheck, User};
use crate::error::ApiError;
use crate::http::{check_item_status, check_status, read_image, read_json};
use crate::ids::{DeviceId, ItemId, UserId};
use crate::image::{Image, ImageRequest};
use crate::item::Item;
use crate::library::{Library, Page};
use crate::library_query::{self as params, Params};
use crate::query::ItemsQuery;
use crate::redirect::{self, MAX_REDIRECTS};
use crate::secret::{AccessToken, Password};
use crate::server_url::ServerUrl;

const PUBLIC_INFO: &str = "System/Info/Public";
const AUTHENTICATE: &str = "Users/AuthenticateByName";
const ME: &str = "Users/Me";
const VIEWS: &str = "UserViews";
const ITEMS: &str = "Items";
const RESUME: &str = "UserItems/Resume";
const NEXT_UP: &str = "Shows/NextUp";
const LATEST: &str = "Items/Latest";

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

    /// A logged-in GET of a library endpoint. `item_scoped` says a 404 means
    /// the item doesn't exist (see [`check_item_status`]).
    async fn get_library(
        &self,
        session: &Session,
        endpoint: &str,
        params: &Params,
        item_scoped: bool,
    ) -> Result<Response, ApiError> {
        let mut url = session.server().endpoint(endpoint);
        if !params.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(params.iter().map(|(k, v)| (*k, v.as_str())));
        }
        let response = self
            .send_once(self.http.get(url), endpoint, Some(session.token()))
            .await?;
        if item_scoped {
            check_item_status(&response)?;
        } else {
            check_status(&response, ApiError::TokenRejected)?;
        }
        Ok(response)
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        session: &Session,
        endpoint: &str,
        params: &Params,
        item_scoped: bool,
    ) -> Result<T, ApiError> {
        let response = self
            .get_library(session, endpoint, params, item_scoped)
            .await?;
        read_json(response).await
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

    async fn libraries(&self, session: &Session, user: &UserId) -> Result<Vec<Library>, ApiError> {
        let page: Page<Library> = self
            .get_json(session, VIEWS, &params::libraries(user), false)
            .await?;
        Ok(page.items)
    }

    async fn items(
        &self,
        session: &Session,
        user: &UserId,
        query: &ItemsQuery,
    ) -> Result<Page<Item>, ApiError> {
        self.get_json(session, ITEMS, &params::items(user, query), false)
            .await
    }

    async fn item(&self, session: &Session, user: &UserId, id: &ItemId) -> Result<Item, ApiError> {
        let endpoint = format!("{ITEMS}/{id}");
        self.get_json(session, &endpoint, &params::item(user), true)
            .await
    }

    async fn resume(
        &self,
        session: &Session,
        user: &UserId,
        limit: u32,
    ) -> Result<Vec<Item>, ApiError> {
        let page: Page<Item> = self
            .get_json(session, RESUME, &params::resume(user, limit), false)
            .await?;
        Ok(page.items)
    }

    async fn next_up(
        &self,
        session: &Session,
        user: &UserId,
        limit: u32,
    ) -> Result<Vec<Item>, ApiError> {
        let page: Page<Item> = self
            .get_json(session, NEXT_UP, &params::next_up(user, limit), false)
            .await?;
        Ok(page.items)
    }

    async fn latest(
        &self,
        session: &Session,
        user: &UserId,
        parent: &ItemId,
        limit: u32,
    ) -> Result<Vec<Item>, ApiError> {
        // The one list that is a bare array, not an object with `Items`.
        self.get_json(session, LATEST, &params::latest(user, parent, limit), false)
            .await
    }

    async fn seasons(
        &self,
        session: &Session,
        user: &UserId,
        series: &ItemId,
    ) -> Result<Vec<Item>, ApiError> {
        let endpoint = format!("Shows/{series}/Seasons");
        let page: Page<Item> = self
            .get_json(session, &endpoint, &params::seasons(user), true)
            .await?;
        Ok(page.items)
    }

    async fn episodes(
        &self,
        session: &Session,
        user: &UserId,
        series: &ItemId,
        season: &ItemId,
    ) -> Result<Vec<Item>, ApiError> {
        let endpoint = format!("Shows/{series}/Episodes");
        let page: Page<Item> = self
            .get_json(session, &endpoint, &params::episodes(user, season), true)
            .await?;
        Ok(page.items)
    }

    async fn image(&self, session: &Session, request: &ImageRequest) -> Result<Image, ApiError> {
        let endpoint = format!("Items/{}/Images/{}", request.item, request.kind.as_str());
        let mut query = vec![("maxWidth", request.max_width.to_string())];
        if let Some(tag) = &request.tag {
            query.push(("tag", tag.as_str().to_owned()));
        }
        read_image(self.get_library(session, &endpoint, &query, true).await?).await
    }
}
