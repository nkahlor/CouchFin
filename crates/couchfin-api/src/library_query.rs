//! The query strings of the library endpoints, kept apart from sending them
//! so the client stays small. None carries `userId`: Jellyfin then uses the
//! user the token belongs to (decision 006).

use crate::ids::ItemId;
use crate::item::ItemKind;
use crate::query::ItemsQuery;

/// Query parameters, in the order they are sent.
pub(crate) type Params = Vec<(&'static str, String)>;

pub(crate) fn none() -> Params {
    Vec::new()
}

pub(crate) fn items(query: &ItemsQuery) -> Params {
    let mut params = vec![
        ("sortBy", "SortName".to_owned()),
        ("recursive", query.recursive.to_string()),
        ("startIndex", query.start_index.to_string()),
    ];
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

pub(crate) fn limit(limit: u32) -> Params {
    vec![("limit", limit.to_string())]
}

pub(crate) fn resume(limit: u32) -> Params {
    // Resumable audio books and music aren't shown.
    vec![
        ("limit", limit.to_string()),
        ("mediaTypes", "Video".to_owned()),
    ]
}

pub(crate) fn latest(parent: &ItemId, limit: u32) -> Params {
    vec![
        ("parentId", parent.as_str().to_owned()),
        ("limit", limit.to_string()),
    ]
}

pub(crate) fn episodes(season: &ItemId) -> Params {
    vec![
        ("seasonId", season.as_str().to_owned()),
        // Episode lists show the synopsis, which Jellyfin only sends on request.
        ("fields", "Overview".to_owned()),
    ]
}
