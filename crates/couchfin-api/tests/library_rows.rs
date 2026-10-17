//! The Home rows and the series calls: `resume`, `next_up`, `latest`,
//! `seasons` and `episodes`.

mod common;

use common::library::{self, LIBRARY_ID, SEASON_ID, SERIES_ID, id, query_result};
use common::{TOKEN, authorization, client, session};
use couchfin_api::{ApiError, ItemKind, JellyfinApi};
use serde_json::json;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn resume_asks_for_the_users_unfinished_video() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/UserItems/Resume"))
        .and(query_param_is_missing("userId"))
        .and(query_param("limit", "12"))
        .and(query_param("mediaTypes", "Video"))
        .and(authorization(Some(TOKEN)))
        .respond_with(ResponseTemplate::new(200).set_body_json(query_result(
            &[library::movie_json(), library::episode_json()],
            2,
            0,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let items = client().resume(&session(&server, TOKEN), 12).await.unwrap();

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].kind, ItemKind::Movie);
    assert_eq!(items[1].kind, ItemKind::Episode);
}

#[tokio::test]
async fn next_up_asks_for_the_next_episode_of_each_series() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Shows/NextUp"))
        .and(query_param_is_missing("userId"))
        .and(query_param("limit", "20"))
        .and(authorization(Some(TOKEN)))
        .respond_with(ResponseTemplate::new(200).set_body_json(query_result(
            &[library::episode_json()],
            1,
            0,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let items = client()
        .next_up(&session(&server, TOKEN), 20)
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].series_name.as_deref(), Some("Severance"));
}

#[tokio::test]
async fn latest_reads_a_bare_array_for_one_library() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Items/Latest"))
        .and(query_param_is_missing("userId"))
        .and(query_param("parentId", LIBRARY_ID))
        .and(query_param("limit", "16"))
        .and(authorization(Some(TOKEN)))
        .respond_with(
            // Unlike the other lists, this one has no Items wrapper.
            ResponseTemplate::new(200)
                .set_body_json(json!([library::movie_json(), library::series_json()])),
        )
        .expect(1)
        .mount(&server)
        .await;

    let items = client()
        .latest(&session(&server, TOKEN), &id(LIBRARY_ID), 16)
        .await
        .unwrap();

    assert_eq!(items.len(), 2);
    assert_eq!(items[1].kind, ItemKind::Series);
}

#[tokio::test]
async fn seasons_lists_a_seriess_seasons() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/Shows/{SERIES_ID}/Seasons")))
        .and(query_param_is_missing("userId"))
        .and(authorization(Some(TOKEN)))
        .respond_with(ResponseTemplate::new(200).set_body_json(query_result(
            &[library::season_json()],
            1,
            0,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let seasons = client()
        .seasons(&session(&server, TOKEN), &id(SERIES_ID))
        .await
        .unwrap();

    assert_eq!(seasons.len(), 1);
    assert_eq!(seasons[0].kind, ItemKind::Season);
    assert_eq!(seasons[0].season_number, Some(2));
}

#[tokio::test]
async fn episodes_asks_for_one_season_with_overviews() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/Shows/{SERIES_ID}/Episodes")))
        .and(query_param_is_missing("userId"))
        .and(query_param("seasonId", SEASON_ID))
        .and(query_param("fields", "Overview"))
        .and(query_param_is_missing("season"))
        .and(authorization(Some(TOKEN)))
        .respond_with(ResponseTemplate::new(200).set_body_json(query_result(
            &[library::episode_json()],
            1,
            0,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let episodes = client()
        .episodes(&session(&server, TOKEN), &id(SERIES_ID), &id(SEASON_ID))
        .await
        .unwrap();

    assert_eq!(episodes[0].episode_number, Some(1));
    assert_eq!(
        episodes[0].overview.as_deref(),
        Some("Mark returns to work.")
    );
}

#[tokio::test]
async fn an_unknown_series_is_item_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({"title": "Not Found"})))
        .mount(&server)
        .await;
    let api = client();
    let session = session(&server, TOKEN);

    let seasons = api.seasons(&session, &id(SERIES_ID)).await;
    let episodes = api.episodes(&session, &id(SERIES_ID), &id(SEASON_ID)).await;

    assert_eq!(seasons.unwrap_err(), ApiError::ItemNotFound);
    assert_eq!(episodes.unwrap_err(), ApiError::ItemNotFound);
}

#[tokio::test]
async fn every_row_maps_401_to_token_rejected() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let api = client();
    let session = session(&server, TOKEN);
    let parent = id(LIBRARY_ID);

    let errors = [
        api.resume(&session, 1).await.unwrap_err(),
        api.next_up(&session, 1).await.unwrap_err(),
        api.latest(&session, &parent, 1).await.unwrap_err(),
        api.seasons(&session, &parent).await.unwrap_err(),
        api.episodes(&session, &parent, &parent).await.unwrap_err(),
    ];

    assert!(
        errors.iter().all(|e| *e == ApiError::TokenRejected),
        "{errors:?}"
    );
}

#[tokio::test]
async fn a_server_error_is_a_status_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let error = client()
        .resume(&session(&server, TOKEN), 5)
        .await
        .unwrap_err();

    assert_eq!(error, ApiError::Status { status: 503 });
}
