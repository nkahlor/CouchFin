//! The fake's `JellyfinApi` impl: each method computes its answer in the
//! parent module and returns it in a ready future.

use std::future::{Future, ready};

use super::FakeJellyfin;
use crate::api::{JellyfinApi, Session};
use crate::dto::{AuthenticationResult, ServerCheck, User};
use crate::error::ApiError;
use crate::ids::ItemId;
use crate::image::{Image, ImageRequest};
use crate::item::Item;
use crate::library::{Library, Page};
use crate::query::ItemsQuery;
use crate::secret::Password;
use crate::server_url::ServerUrl;

impl JellyfinApi for FakeJellyfin {
    fn check_server(
        &self,
        address: &ServerUrl,
    ) -> impl Future<Output = Result<ServerCheck, ApiError>> + Send {
        ready(self.check(address))
    }

    fn authenticate_by_name(
        &self,
        server: &ServerUrl,
        username: &str,
        password: &Password,
    ) -> impl Future<Output = Result<AuthenticationResult, ApiError>> + Send {
        ready(self.log_in(server, username, password))
    }

    fn current_user(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<User, ApiError>> + Send {
        ready(self.user(session))
    }

    fn libraries(
        &self,
        session: &Session,
    ) -> impl Future<Output = Result<Vec<Library>, ApiError>> + Send {
        self.read(session, |c| Ok(c.libraries()))
    }

    fn items(
        &self,
        session: &Session,
        query: &ItemsQuery,
    ) -> impl Future<Output = Result<Page<Item>, ApiError>> + Send {
        self.read(session, |c| Ok(c.page(query)))
    }

    fn item(
        &self,
        session: &Session,
        id: &ItemId,
    ) -> impl Future<Output = Result<Item, ApiError>> + Send {
        self.read(session, |c| c.item(id))
    }

    fn resume(
        &self,
        session: &Session,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.read(session, |c| Ok(c.resume(limit)))
    }

    fn next_up(
        &self,
        session: &Session,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.read(session, |c| Ok(c.next_up(limit)))
    }

    fn latest(
        &self,
        session: &Session,
        parent: &ItemId,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.read(session, |c| Ok(c.latest(parent, limit)))
    }

    fn seasons(
        &self,
        session: &Session,
        series: &ItemId,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.read(session, |c| c.seasons(series))
    }

    fn episodes(
        &self,
        session: &Session,
        series: &ItemId,
        season: &ItemId,
    ) -> impl Future<Output = Result<Vec<Item>, ApiError>> + Send {
        self.read(session, |c| c.episodes(series, season))
    }

    fn image(
        &self,
        session: &Session,
        request: &ImageRequest,
    ) -> impl Future<Output = Result<Image, ApiError>> + Send {
        self.read(session, |c| c.image(request))
    }
}
