//! The library grid: total count, paging and deduplicated page loads.

#[path = "common/probe.rs"]
mod probe;

use couchfin_core::{GridState, LibraryAction, ScreenState, Target};
use probe::Harness;

async fn big_library(page_size: usize) -> Harness {
    let fake = probe::catalog().with_generated_library("Big", 2_000);
    let harness = Harness::new(fake, page_size).await;
    harness.flow.start().await;
    let id = harness.probe.fake.library_named("Big").expect("library").id;
    harness
        .send(LibraryAction::Open(Target::Library {
            id,
            name: "Big".into(),
        }))
        .await;
    harness
}

fn grid(harness: &Harness) -> GridState {
    match harness.state().current() {
        ScreenState::Library(grid) => grid.clone(),
        other => panic!("not a grid: {other:?}"),
    }
}

fn loaded(grid: &GridState) -> usize {
    grid.items.iter().flatten().count()
}

#[tokio::test]
async fn opening_a_library_loads_the_total_and_the_first_page() {
    let harness = big_library(50).await;
    let grid = grid(&harness);
    assert_eq!(grid.title, "Big");
    assert_eq!(grid.total, Some(2_000));
    assert_eq!(grid.items.len(), 2_000);
    assert_eq!(loaded(&grid), 50);
    assert_eq!(grid.item(0).map(|c| c.title.as_str()), Some("Big 00000"));
    assert_eq!(grid.item(49).map(|c| c.title.as_str()), Some("Big 00049"));
    assert!(grid.item(50).is_none(), "not loaded yet");
    assert!(grid.item(2_000).is_none(), "past the end");
    assert_eq!(harness.probe.calls("items"), 1);
}

#[tokio::test]
async fn need_items_loads_only_the_pages_covering_the_range() {
    let harness = big_library(50).await;
    harness
        .send(LibraryAction::NeedItems {
            first: 1_000,
            last: 1_060,
        })
        .await;
    let grid = grid(&harness);
    assert_eq!(loaded(&grid), 150, "first page plus pages 20 and 21");
    assert_eq!(
        grid.item(1_000).map(|c| c.title.as_str()),
        Some("Big 01000")
    );
    assert_eq!(
        grid.item(1_099).map(|c| c.title.as_str()),
        Some("Big 01099")
    );
    assert!(grid.item(1_100).is_none());
    assert!(grid.item(500).is_none());
    assert_eq!(harness.probe.calls("items"), 3);
}

#[tokio::test]
async fn loaded_pages_are_kept_and_not_asked_for_again() {
    let harness = big_library(50).await;
    let need = |first, last| LibraryAction::NeedItems { first, last };
    harness.send(need(1_000, 1_060)).await;
    harness.send(need(1_000, 1_060)).await;
    harness.send(need(0, 10)).await;
    assert_eq!(harness.probe.calls("items"), 3);
    // Overlapping: only page 22 is new.
    harness.send(need(1_050, 1_120)).await;
    assert_eq!(harness.probe.calls("items"), 4);
    assert_eq!(loaded(&grid(&harness)), 200);
}

#[tokio::test]
async fn pages_already_in_flight_are_not_requested_twice() {
    let harness = big_library(50).await;
    harness.probe.close("items");
    let need = LibraryAction::NeedItems {
        first: 300,
        last: 340,
    };
    let first = harness.spawn(need.clone());
    let second = harness.spawn(need);
    tokio::task::yield_now().await;
    tokio::task::yield_now().await;
    harness.probe.open("items");
    first.await.expect("task");
    second.await.expect("task");
    assert_eq!(harness.probe.calls("items"), 2, "page 0, then page 6 once");
    assert_eq!(
        grid(&harness).item(300).map(|c| c.title.as_str()),
        Some("Big 00300")
    );
}

#[tokio::test]
async fn a_range_past_the_end_stops_at_the_last_page() {
    let harness = big_library(50).await;
    harness
        .send(LibraryAction::NeedItems {
            first: 1_990,
            last: 9_999,
        })
        .await;
    assert_eq!(harness.probe.calls("items"), 2);
    assert_eq!(
        grid(&harness).item(1_999).map(|c| c.title.as_str()),
        Some("Big 01999")
    );
}

#[tokio::test]
async fn page_size_is_configurable() {
    let harness = big_library(100).await;
    assert_eq!(loaded(&grid(&harness)), 100);
}

#[tokio::test]
async fn a_failed_page_can_be_asked_for_again() {
    let harness = big_library(50).await;
    harness
        .probe
        .fake
        .fail_with(couchfin_core::ApiError::Timeout);
    harness
        .send(LibraryAction::NeedItems {
            first: 500,
            last: 510,
        })
        .await;
    assert_eq!(grid(&harness).error, Some(couchfin_core::ApiError::Timeout));
    harness.probe.fake.clear_failure();
    harness
        .send(LibraryAction::NeedItems {
            first: 500,
            last: 510,
        })
        .await;
    assert!(grid(&harness).item(500).is_some());
}

#[tokio::test]
async fn opening_a_grid_item_goes_to_its_detail() {
    let harness = big_library(50).await;
    harness.send(LibraryAction::OpenGridItem(3)).await;
    let state = harness.state();
    let ScreenState::Movie(movie) = state.current() else {
        panic!("{:?}", state.current());
    };
    assert_eq!(
        movie.header.as_ref().map(|h| h.title.as_str()),
        Some("Big 00003")
    );
}

#[tokio::test]
async fn a_grid_mixes_movies_and_series_sorted_by_title() {
    let harness = Harness::started().await;
    let id = harness
        .probe
        .fake
        .library_named("Shows")
        .expect("library")
        .id;
    harness
        .send(LibraryAction::Open(Target::Library {
            id,
            name: "Shows".into(),
        }))
        .await;
    let grid = grid(&harness);
    assert_eq!(grid.total, Some(1), "seasons and episodes are not listed");
    assert_eq!(grid.item(0).map(|c| c.title.as_str()), Some("Severance"));
}
