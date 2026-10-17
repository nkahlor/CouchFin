//! Trimmed copies of what Jellyfin 10.11 returns for library calls, with the
//! many fields Couchfin ignores left in on purpose.
#![allow(dead_code)]

use couchfin_api::{ItemId, UserId};
use serde_json::{Value, json};

use super::{SERVER_ID, USER_ID};

pub const MOVIE_ID: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90";
pub const SERIES_ID: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";
pub const SEASON_ID: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";
pub const EPISODE_ID: &str = "e9e9e9e9e9e9e9e9e9e9e9e9e9e9e9e9";
pub const LIBRARY_ID: &str = "7e64e319657a9516ec78490da03edccb";
pub const PRIMARY_TAG: &str = "b1c2d3e4f5061728394a5b6c7d8e9f01";

pub fn user() -> UserId {
    USER_ID.parse().expect("USER_ID is valid")
}

pub fn id(value: &str) -> ItemId {
    value.parse().expect("test IDs are valid")
}

pub fn movie_json() -> Value {
    json!({
        "Name": "Arrival",
        "ServerId": SERVER_ID,
        "Id": MOVIE_ID,
        "HasSubtitles": true,
        "Container": "mkv",
        "PremiereDate": "2016-11-10T00:00:00.0000000Z",
        "CriticRating": 94,
        "OfficialRating": "PG-13",
        "Overview": "A linguist is recruited to talk to aliens.",
        "Taglines": [],
        "Genres": ["Science Fiction"],
        "CommunityRating": 7.9,
        "RunTimeTicks": 69_600_000_000_u64,
        "ProductionYear": 2016,
        "IsFolder": false,
        "Type": "Movie",
        "UserData": {
            "PlaybackPositionTicks": 18_000_000_000_u64,
            "PlayCount": 0,
            "IsFavorite": false,
            "Played": false,
            "PlayedPercentage": 25.86,
            "Key": "movie-key"
        },
        "PrimaryImageAspectRatio": 0.666_666_666_666_666_6,
        "ImageTags": { "Primary": PRIMARY_TAG, "Thumb": "11111111111111111111111111111111" },
        "BackdropImageTags": ["22222222222222222222222222222222"],
        "MediaType": "Video",
        "SomethingFromTheFuture": { "nested": [1, 2, 3] }
    })
}

pub fn series_json() -> Value {
    json!({
        "Name": "Severance",
        "ServerId": SERVER_ID,
        "Id": SERIES_ID,
        "OfficialRating": "TV-MA",
        "ProductionYear": 2022,
        "IsFolder": true,
        "Type": "Series",
        "UserData": { "UnplayedItemCount": 9, "PlaybackPositionTicks": 0, "PlayCount": 0,
            "IsFavorite": false, "Played": false, "Key": "series-key" },
        "ImageTags": { "Primary": PRIMARY_TAG },
        "BackdropImageTags": []
    })
}

pub fn season_json() -> Value {
    json!({
        "Name": "Season 2",
        "Id": SEASON_ID,
        "Type": "Season",
        "IndexNumber": 2,
        "SeriesName": "Severance",
        "SeriesId": SERIES_ID,
        "IsFolder": true,
        "ImageTags": {}
    })
}

pub fn episode_json() -> Value {
    json!({
        "Name": "Hello, Ms. Cobel",
        "Id": EPISODE_ID,
        "Type": "Episode",
        "IndexNumber": 1,
        "ParentIndexNumber": 2,
        "SeriesName": "Severance",
        "SeriesId": SERIES_ID,
        "SeasonId": SEASON_ID,
        "RunTimeTicks": 30_000_000_000_u64,
        "Overview": "Mark returns to work.",
        "UserData": { "PlaybackPositionTicks": 0, "PlayCount": 1, "IsFavorite": false,
            "Played": true, "Key": "episode-key" },
        "ImageTags": { "Primary": PRIMARY_TAG }
    })
}

/// A kind this client has no variant for.
pub fn box_set_json() -> Value {
    json!({ "Name": "Nolan Collection", "Id": "cccccccccccccccccccccccccccccccc",
        "Type": "BoxSet", "IsFolder": true })
}

pub fn view_json() -> Value {
    json!({ "Name": "Movies", "ServerId": SERVER_ID, "Id": LIBRARY_ID,
        "CollectionType": "movies", "Type": "CollectionFolder", "IsFolder": true,
        "ImageTags": { "Primary": PRIMARY_TAG } })
}

pub fn query_result(items: &[Value], total: usize, start: usize) -> Value {
    json!({ "Items": items, "TotalRecordCount": total, "StartIndex": start })
}
