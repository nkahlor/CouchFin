//! Movie and series detail screens.

#[path = "common/probe.rs"]
mod probe;

use std::time::Duration;

use couchfin_core::{LibraryAction, ScreenState, SeriesState, Target};
use probe::Harness;

fn movie_target(harness: &Harness, name: &str) -> Target {
    Target::Movie {
        id: harness.probe.fake.item_named(name).expect("item").id,
    }
}

fn series_state(harness: &Harness) -> SeriesState {
    match harness.state().current() {
        ScreenState::Series(series) => series.clone(),
        other => panic!("not a series: {other:?}"),
    }
}

async fn open_series(harness: &Harness, season: Option<u32>) {
    let id = harness
        .probe
        .fake
        .item_named("Severance")
        .expect("series")
        .id;
    harness
        .send(LibraryAction::Open(Target::Series { id, season }))
        .await;
}

fn names(series: &SeriesState) -> Vec<&str> {
    series.episodes.iter().map(|e| e.title.as_str()).collect()
}

#[tokio::test]
async fn a_movie_shows_its_header_and_that_it_can_resume() {
    let harness = Harness::started().await;
    harness
        .send(LibraryAction::Open(movie_target(&harness, "Arrival")))
        .await;
    let state = harness.state();
    let ScreenState::Movie(movie) = state.current() else {
        panic!("{:?}", state.current());
    };
    let header = movie.header.as_ref().expect("loaded");
    assert_eq!(header.title, "Arrival");
    assert_eq!(header.year, Some(2000));
    assert_eq!(header.runtime, Some(Duration::from_mins(100)));
    assert_eq!(header.rating.as_deref(), Some("PG"));
    assert_eq!(header.overview.as_deref(), Some("About Arrival."));
    assert_eq!(header.resume_from, Some(Duration::from_mins(50)));
    assert!(header.backdrop.is_some() && header.poster.is_some());
    assert!(!movie.loading);
}

#[tokio::test]
async fn a_movie_nobody_started_is_not_resumable() {
    let harness = Harness::started().await;
    harness
        .send(LibraryAction::Open(movie_target(&harness, "Dune")))
        .await;
    let state = harness.state();
    let ScreenState::Movie(movie) = state.current() else {
        panic!("{:?}", state.current());
    };
    assert_eq!(movie.header.as_ref().map(|h| h.resume_from), Some(None));
}

#[tokio::test]
async fn a_series_shows_header_seasons_and_the_first_seasons_episodes() {
    let harness = Harness::started().await;
    open_series(&harness, None).await;
    let series = series_state(&harness);
    assert_eq!(
        series.header.as_ref().map(|h| h.title.as_str()),
        Some("Severance")
    );
    let seasons: Vec<_> = series.seasons.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(seasons, ["Severance S1", "Severance S2"]);
    assert_eq!(series.seasons[1].number, Some(2));
    assert_eq!(series.selected_season, 0);
    assert_eq!(
        names(&series),
        ["Severance S1E1", "Severance S1E2", "Severance S1E3"]
    );
    let episode = &series.episodes[1];
    assert_eq!(episode.number, Some(2));
    assert_eq!(episode.runtime, Some(Duration::from_mins(45)));
    assert!(!episode.watched);
    assert!(!series.loading);
}

#[tokio::test]
async fn selecting_a_season_reloads_the_episodes() {
    let harness = Harness::started().await;
    open_series(&harness, None).await;
    harness.send(LibraryAction::SelectSeason(1)).await;
    let series = series_state(&harness);
    assert_eq!(series.selected_season, 1);
    assert_eq!(
        names(&series),
        ["Severance S2E1", "Severance S2E2", "Severance S2E3"]
    );
    assert_eq!(harness.probe.calls("episodes"), 2);

    // Out of range: ignored.
    harness.send(LibraryAction::SelectSeason(9)).await;
    assert_eq!(series_state(&harness).selected_season, 1);
}

#[tokio::test]
async fn an_episode_card_opens_its_series_on_the_right_season() {
    let harness = Harness::started().await;
    harness
        .send(LibraryAction::OpenCard { row: 0, index: 1 })
        .await;
    let series = series_state(&harness);
    assert_eq!(series.selected_season, 1);
    assert_eq!(series.episodes.len(), 3);
}

#[tokio::test]
async fn a_late_season_answer_does_not_replace_the_selected_one() {
    let harness = Harness::started().await;
    open_series(&harness, None).await;
    harness.probe.close("episodes");
    let slow = harness.spawn(LibraryAction::SelectSeason(1));
    tokio::task::yield_now().await;
    let back = harness.spawn(LibraryAction::SelectSeason(0));
    tokio::task::yield_now().await;
    harness.probe.open("episodes");
    slow.await.expect("task");
    back.await.expect("task");
    let series = series_state(&harness);
    assert_eq!(series.selected_season, 0);
    assert_eq!(series.episodes[0].title, "Severance S1E1");
}

#[tokio::test]
async fn a_missing_item_is_a_typed_error_on_its_screen() {
    let harness = Harness::started().await;
    let id = "nope".parse().expect("valid id");
    harness
        .send(LibraryAction::Open(Target::Movie { id }))
        .await;
    let state = harness.state();
    let ScreenState::Movie(movie) = state.current() else {
        panic!("{:?}", state.current());
    };
    assert_eq!(movie.error, Some(couchfin_core::ApiError::ItemNotFound));
    assert!(!movie.loading);
}
