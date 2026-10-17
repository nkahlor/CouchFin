//! How `Item` reads the odd JSON a real library produces: bad tags, odd
//! numbers and missing or null fields never fail a whole page.

use std::time::Duration;

use couchfin_api::{Item, ItemKind};
use serde_json::json;

fn parse(value: serde_json::Value) -> Item {
    serde_json::from_value(value).expect("item parses")
}

#[test]
fn null_and_negative_numbers_become_none() {
    let item = parse(json!({
        "Id": "abc", "Name": "Odd", "Type": "Episode",
        "ProductionYear": -1, "RunTimeTicks": -5,
        "IndexNumber": -1, "ParentIndexNumber": null,
        "OfficialRating": null, "Overview": null
    }));

    assert_eq!(item.production_year, None);
    assert_eq!(item.runtime, None);
    assert_eq!(item.season_number, None);
    assert_eq!(item.episode_number, None);
    assert_eq!(item.overview, None);
}

#[test]
fn image_tags_that_are_null_or_unsafe_are_dropped() {
    let item = parse(json!({
        "Id": "abc", "Name": "Odd", "Type": "Movie",
        "ImageTags": { "Primary": null, "Thumb": "has space" },
        "BackdropImageTags": ["../x"]
    }));

    assert_eq!(item.image_tags.primary, None);
    assert_eq!(item.image_tags.thumb, None);
    assert_eq!(item.image_tags.backdrop, None);
}

#[test]
fn a_missing_type_is_an_unknown_kind_and_user_data_defaults() {
    let item = parse(json!({ "Id": "abc", "Name": "Bare", "UserData": { "Key": "k" } }));

    assert_eq!(item.kind, ItemKind::Other(String::new()));
    let data = item.user_data.expect("user data is present");
    assert!(!data.played);
    assert_eq!(data.position, Duration::ZERO);
}

#[test]
fn an_item_without_an_id_is_rejected() {
    let result = serde_json::from_value::<Item>(json!({ "Name": "No id", "Type": "Movie" }));

    assert!(result.is_err());
}

#[test]
fn huge_tick_counts_do_not_overflow() {
    let item = parse(json!({
        "Id": "abc", "Name": "Long", "Type": "Movie", "RunTimeTicks": i64::MAX
    }));

    assert!(item.runtime.is_some());
}
