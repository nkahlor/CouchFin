//! `libraries`, `items` and `item` (`GET /UserViews`, `/Items`,
//! `/Items/{id}`): the library list, the paged grid and one item's details.

mod common;

use std::time::Duration;

use common::library::{self, MOVIE_ID, PRIMARY_TAG, SERIES_ID, id, query_result};
use common::{TOKEN, authorization, client, session};
use couchfin_api::{ItemId, ItemKind, ItemsQuery, JellyfinApi, LibraryKind};
use serde_json::json;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn libraries_lists_the_users_views() {
    let server = MockServer::start().await;
    let mixed = json!({ "Name": "Home videos", "Id": "dddddddddddddddddddddddddddddddd" });
    Mock::given(method("GET"))
        .and(path("/UserViews"))
        .and(query_param_is_missing("userId"))
        .and(authorization(Some(TOKEN)))
        .respond_with(ResponseTemplate::new(200).set_body_json(query_result(
            &[library::view_json(), mixed],
            2,
            0,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let libraries = client().libraries(&session(&server, TOKEN)).await.unwrap();

    assert_eq!(libraries.len(), 2);
    assert_eq!(libraries[0].id.as_str(), library::LIBRARY_ID);
    assert_eq!(libraries[0].name, "Movies");
    assert_eq!(libraries[0].kind, LibraryKind::Movies);
    let tag = libraries[0].primary_image.as_ref();
    assert_eq!(tag.map(couchfin_api::ImageTag::as_str), Some(PRIMARY_TAG));
    assert_eq!(libraries[1].kind, LibraryKind::Mixed);
}

#[tokio::test]
async fn an_unknown_collection_type_is_kept() {
    let server = MockServer::start().await;
    let music = json!({ "Name": "Music", "Id": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "CollectionType": "music" });
    Mock::given(method("GET"))
        .and(path("/UserViews"))
        .respond_with(ResponseTemplate::new(200).set_body_json(query_result(&[music], 1, 0)))
        .mount(&server)
        .await;

    let libraries = client().libraries(&session(&server, TOKEN)).await.unwrap();

    assert_eq!(libraries[0].kind, LibraryKind::Other("music".to_owned()));
}

#[tokio::test]
async fn items_sends_the_paging_and_sort_query() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Items"))
        .and(query_param_is_missing("userId"))
        .and(query_param("parentId", library::LIBRARY_ID))
        .and(query_param("sortBy", "SortName"))
        .and(query_param("recursive", "true"))
        .and(query_param("includeItemTypes", "Movie,Series"))
        .and(query_param("startIndex", "50"))
        .and(query_param("limit", "25"))
        .and(authorization(Some(TOKEN)))
        .respond_with(ResponseTemplate::new(200).set_body_json(query_result(
            &[library::movie_json(), library::series_json()],
            2_000,
            50,
        )))
        .expect(1)
        .mount(&server)
        .await;
    let query = ItemsQuery::default()
        .in_parent(id(library::LIBRARY_ID))
        .of_kinds([ItemKind::Movie, ItemKind::Series])
        .recursive()
        .page(50, 25);

    let page = client()
        .items(&session(&server, TOKEN), &query)
        .await
        .unwrap();

    assert_eq!(page.total, 2_000);
    assert_eq!(page.start_index, 50);
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].kind, ItemKind::Movie);
    assert_eq!(page.items[1].kind, ItemKind::Series);
}

#[tokio::test]
async fn items_without_filters_sends_only_what_was_asked() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Items"))
        .and(query_param("recursive", "false"))
        .and(query_param("startIndex", "0"))
        .and(query_param_is_missing("parentId"))
        .and(query_param_is_missing("includeItemTypes"))
        .and(query_param_is_missing("limit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(query_result(&[], 0, 0)))
        .expect(1)
        .mount(&server)
        .await;

    let page = client()
        .items(&session(&server, TOKEN), &ItemsQuery::default())
        .await
        .unwrap();

    assert_eq!(page.items, []);
    assert_eq!(page.total, 0);
}

#[tokio::test]
async fn a_page_without_a_total_counts_the_items() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Items"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Items": [library::movie_json()]
        })))
        .mount(&server)
        .await;

    let page = client()
        .items(&session(&server, TOKEN), &ItemsQuery::default())
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.start_index, 0);
}

#[tokio::test]
async fn item_parses_a_realistic_movie() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/Items/{MOVIE_ID}")))
        .and(query_param_is_missing("userId"))
        .and(authorization(Some(TOKEN)))
        .respond_with(ResponseTemplate::new(200).set_body_json(library::movie_json()))
        .expect(1)
        .mount(&server)
        .await;

    let item = client()
        .item(&session(&server, TOKEN), &id(MOVIE_ID))
        .await
        .unwrap();

    assert_eq!(item.id.as_str(), MOVIE_ID);
    assert_eq!(item.name, "Arrival");
    assert_eq!(item.kind, ItemKind::Movie);
    assert_eq!(item.production_year, Some(2016));
    assert_eq!(item.runtime, Some(Duration::from_mins(116)));
    assert_eq!(
        item.overview.as_deref(),
        Some("A linguist is recruited to talk to aliens.")
    );
    assert_eq!(item.official_rating.as_deref(), Some("PG-13"));
    let primary = item.image_tags.primary.as_ref();
    assert_eq!(
        primary.map(couchfin_api::ImageTag::as_str),
        Some(PRIMARY_TAG)
    );
    assert!(item.image_tags.thumb.is_some());
    assert!(item.image_tags.backdrop.is_some());
    let data = item.user_data.expect("user data is present");
    assert!(!data.played);
    assert_eq!(data.position, Duration::from_mins(30));
    assert_eq!(data.played_percentage, Some(25.86));
    assert_eq!(item.series_name, None);
    assert_eq!(item.season_number, None);
}

#[tokio::test]
async fn item_parses_a_series_with_missing_optional_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/Items/{SERIES_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(library::series_json()))
        .mount(&server)
        .await;

    let item = client()
        .item(&session(&server, TOKEN), &id(SERIES_ID))
        .await
        .unwrap();

    assert_eq!(item.kind, ItemKind::Series);
    assert_eq!(item.runtime, None);
    assert_eq!(item.overview, None);
    assert!(item.image_tags.thumb.is_none());
    assert!(item.image_tags.backdrop.is_none());
    let data = item.user_data.expect("user data is present");
    assert_eq!(data.position, Duration::ZERO);
    assert_eq!(data.played_percentage, None);
}

#[tokio::test]
async fn item_parses_season_and_episode_numbers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/Items/{}", library::EPISODE_ID)))
        .respond_with(ResponseTemplate::new(200).set_body_json(library::episode_json()))
        .mount(&server)
        .await;

    let item = client()
        .item(&session(&server, TOKEN), &id(library::EPISODE_ID))
        .await
        .unwrap();

    assert_eq!(item.kind, ItemKind::Episode);
    assert_eq!(item.series_name.as_deref(), Some("Severance"));
    assert_eq!(item.series_id.as_ref().map(ItemId::as_str), Some(SERIES_ID));
    assert_eq!(item.season_number, Some(2));
    assert_eq!(item.episode_number, Some(1));
    assert!(item.user_data.is_some_and(|d| d.played));
}

#[tokio::test]
async fn a_season_reports_its_own_number_as_the_season_number() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/Items/{}", library::SEASON_ID)))
        .respond_with(ResponseTemplate::new(200).set_body_json(library::season_json()))
        .mount(&server)
        .await;

    let item = client()
        .item(&session(&server, TOKEN), &id(library::SEASON_ID))
        .await
        .unwrap();

    assert_eq!(item.kind, ItemKind::Season);
    assert_eq!(item.season_number, Some(2));
    assert_eq!(item.episode_number, None);
}

#[tokio::test]
async fn an_unknown_item_type_is_kept_instead_of_failing() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Items"))
        .respond_with(ResponseTemplate::new(200).set_body_json(query_result(
            &[library::box_set_json(), library::movie_json()],
            2,
            0,
        )))
        .mount(&server)
        .await;

    let page = client()
        .items(&session(&server, TOKEN), &ItemsQuery::default())
        .await
        .unwrap();

    assert_eq!(page.items[0].kind, ItemKind::Other("BoxSet".to_owned()));
    assert_eq!(page.items[1].kind, ItemKind::Movie);
}
