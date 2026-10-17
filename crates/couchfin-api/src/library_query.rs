//! The paths and query strings of the library endpoints, kept apart from
//! sending them so the client stays small.

use crate::ids::{ItemId, UserId};
use crate::item::ItemKind;
use crate::query::ItemsQuery;

/// Query parameters, in the order they are sent.
pub(crate) type Params = Vec<(&'static str, String)>;

fn for_user(user: &UserId) -> Params {
    vec![("userId", user.as_str().to_owned())]
}

pub(crate) fn libraries(user: &UserId) -> Params {
    for_user(user)
}

pub(crate) fn items(user: &UserId, query: &ItemsQuery) -> Params {
    let mut params = for_user(user);
    params.push(("sortBy", "SortName".to_owned()));
    params.push(("recursive", query.recursive.to_string()));
    params.push(("startIndex", query.start_index.to_string()));
    if let Some(parent) = &query.parent {
        params.push(("parentId", parent.as_str().to_owned()));
    }
    if !query.kinds.is_empty() {
        let kinds: Vec<_> = query.kinds.iter().map(ItemKind::as_str).collect();
        params.push(("includeItemTypes", kinds.join(",")));
    }
    if let Some(limit) = query.limit {
        params.push(("limit", limit.to_string()));
    }
    params
}

pub(crate) fn item(user: &UserId) -> Params {
    for_user(user)
}

pub(crate) fn resume(user: &UserId, limit: u32) -> Params {
    let mut params = for_user(user);
    params.push(("limit", limit.to_string()));
    // Resumable audio books and music aren't shown.
    params.push(("mediaTypes", "Video".to_owned()));
    params
}

pub(crate) fn next_up(user: &UserId, limit: u32) -> Params {
    let mut params = for_user(user);
    params.push(("limit", limit.to_string()));
    params
}

pub(crate) fn latest(user: &UserId, parent: &ItemId, limit: u32) -> Params {
    let mut params = for_user(user);
    params.push(("parentId", parent.as_str().to_owned()));
    params.push(("limit", limit.to_string()));
    params
}

pub(crate) fn seasons(user: &UserId) -> Params {
    for_user(user)
}

pub(crate) fn episodes(user: &UserId, season: &ItemId) -> Params {
    let mut params = for_user(user);
    params.push(("seasonId", season.as_str().to_owned()));
    // Episode lists show the synopsis, which Jellyfin only sends on request.
    params.push(("fields", "Overview".to_owned()));
    params
}
