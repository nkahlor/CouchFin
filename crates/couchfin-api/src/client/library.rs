//! The library endpoints, as helper methods the `JellyfinApi` impl
//! delegates to.

use reqwest::Response;
use serde::de::DeserializeOwned;

use super::JellyfinClient;
use crate::api::Session;
use crate::error::ApiError;
use crate::http::{check_item_status, check_status, read_json};
use crate::ids::ItemId;
use crate::item::Item;
use crate::library::{Library, Page};
use crate::library_query::{self as params, Params};
use crate::query::ItemsQuery;

const VIEWS: &str = "UserViews";
const ITEMS: &str = "Items";
const RESUME: &str = "UserItems/Resume";
const NEXT_UP: &str = "Shows/NextUp";
const LATEST: &str = "Items/Latest";

// None of these send `userId`: without it Jellyfin uses the user the token
// belongs to, user data included (decision 006).
impl JellyfinClient {
    /// A logged-in GET of a library endpoint. `item_scoped` says a 404 means
    /// the item doesn't exist (see [`check_item_status`]).
    pub(super) async fn get_library(
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

    pub(super) async fn list_libraries(&self, session: &Session) -> Result<Vec<Library>, ApiError> {
        let page: Page<Library> = self
            .get_json(session, VIEWS, &params::none(), false)
            .await?;
        Ok(page.items)
    }

    pub(super) async fn list_items(
        &self,
        session: &Session,
        query: &ItemsQuery,
    ) -> Result<Page<Item>, ApiError> {
        self.get_json(session, ITEMS, &params::items(query), false)
            .await
    }

    pub(super) async fn fetch_item(
        &self,
        session: &Session,
        id: &ItemId,
    ) -> Result<Item, ApiError> {
        let endpoint = format!("{ITEMS}/{id}");
        self.get_json(session, &endpoint, &params::none(), true)
            .await
    }

    pub(super) async fn list_resume(
        &self,
        session: &Session,
        limit: u32,
    ) -> Result<Vec<Item>, ApiError> {
        let page: Page<Item> = self
            .get_json(session, RESUME, &params::resume(limit), false)
            .await?;
        Ok(page.items)
    }

    pub(super) async fn list_next_up(
        &self,
        session: &Session,
        limit: u32,
    ) -> Result<Vec<Item>, ApiError> {
        let page: Page<Item> = self
            .get_json(session, NEXT_UP, &params::limit(limit), false)
            .await?;
        Ok(page.items)
    }

    pub(super) async fn list_latest(
        &self,
        session: &Session,
        parent: &ItemId,
        limit: u32,
    ) -> Result<Vec<Item>, ApiError> {
        // The one list that is a bare array, not an object with `Items`.
        self.get_json(session, LATEST, &params::latest(parent, limit), false)
            .await
    }

    pub(super) async fn list_seasons(
        &self,
        session: &Session,
        series: &ItemId,
    ) -> Result<Vec<Item>, ApiError> {
        let endpoint = format!("Shows/{series}/Seasons");
        let page: Page<Item> = self
            .get_json(session, &endpoint, &params::none(), true)
            .await?;
        Ok(page.items)
    }

    pub(super) async fn list_episodes(
        &self,
        session: &Session,
        series: &ItemId,
        season: &ItemId,
    ) -> Result<Vec<Item>, ApiError> {
        let endpoint = format!("Shows/{series}/Episodes");
        let page: Page<Item> = self
            .get_json(session, &endpoint, &params::episodes(season), true)
            .await?;
        Ok(page.items)
    }
}
