//! The navigation stack: Back, stale results, and errors (session expiry).

#[path = "common/probe.rs"]
mod probe;

use couchfin_core::{ApiError, LibraryAction, ScreenState, Target};
use probe::Harness;

fn movie(harness: &Harness, name: &str) -> Target {
    Target::Movie {
        id: harness.probe.fake.item_named(name).expect("item").id,
    }
}

fn depth(harness: &Harness) -> usize {
    harness.state().screens.len()
}

#[tokio::test]
async fn back_pops_one_level_from_each_screen() {
    let harness = Harness::started().await;
    assert!(harness.state().at_root());

    let movies = harness.row_index("Libraries");
    harness
        .send(LibraryAction::OpenCard {
            row: movies,
            index: 0,
        })
        .await;
    harness.send(LibraryAction::OpenGridItem(0)).await;
    assert_eq!(depth(&harness), 3);
    assert!(matches!(harness.state().current(), ScreenState::Movie(_)));

    harness.send(LibraryAction::Back).await;
    assert!(matches!(harness.state().current(), ScreenState::Library(_)));
    harness.send(LibraryAction::Back).await;
    assert!(matches!(harness.state().current(), ScreenState::Home(_)));
    assert!(harness.state().at_root());
}

#[tokio::test]
async fn back_from_a_series_returns_to_where_it_was_opened() {
    let harness = Harness::started().await;
    harness
        .send(LibraryAction::OpenCard { row: 1, index: 0 })
        .await;
    assert!(matches!(harness.state().current(), ScreenState::Series(_)));
    harness.send(LibraryAction::Back).await;
    assert!(matches!(harness.state().current(), ScreenState::Home(_)));
}

#[tokio::test]
async fn back_on_home_does_nothing() {
    let harness = Harness::started().await;
    harness.send(LibraryAction::Back).await;
    assert_eq!(depth(&harness), 1);
    assert!(matches!(harness.state().current(), ScreenState::Home(_)));
}

#[tokio::test]
async fn a_result_for_a_screen_left_behind_is_dropped() {
    let harness = Harness::started().await;
    harness.probe.close("item");
    let open = harness.spawn(LibraryAction::Open(movie(&harness, "Arrival")));
    harness.wait_until(|s| s.screens.len() == 2).await;
    harness.send(LibraryAction::Back).await;
    let published = harness.seen.lock().unwrap().len();

    harness.probe.open("item");
    open.await.expect("task");
    assert_eq!(depth(&harness), 1);
    assert_eq!(
        harness.seen.lock().unwrap().len(),
        published,
        "nothing published"
    );
}

#[tokio::test]
async fn a_screen_opened_again_ignores_the_first_visits_answer() {
    let harness = Harness::started().await;
    harness.probe.close("item");
    let first = harness.spawn(LibraryAction::Open(movie(&harness, "Arrival")));
    harness.wait_until(|s| s.screens.len() == 2).await;
    harness.send(LibraryAction::Back).await;
    let second = harness.spawn(LibraryAction::Open(movie(&harness, "Dune")));
    harness.wait_until(|s| s.screens.len() == 2).await;
    harness.probe.open("item");
    first.await.expect("task");
    second.await.expect("task");
    let state = harness.state();
    let ScreenState::Movie(movie) = state.current() else {
        panic!("{:?}", state.current());
    };
    assert_eq!(
        movie.header.as_ref().map(|h| h.title.as_str()),
        Some("Dune")
    );
}

#[tokio::test]
async fn a_rejected_token_is_surfaced_for_the_app() {
    let harness = Harness::new(probe::catalog(), 50).await;
    harness.probe.fake.revoke_tokens();
    harness.flow.start().await;
    let state = harness.state();
    assert!(state.session_expired);
    assert!(
        harness
            .seen
            .lock()
            .unwrap()
            .last()
            .expect("published")
            .session_expired
    );
    // A rejected token is not shown as an ordinary error.
    assert_eq!(state.home().error, None);
}

#[tokio::test]
async fn a_token_rejected_on_a_detail_screen_also_expires_the_session() {
    let harness = Harness::started().await;
    assert!(!harness.state().session_expired);
    harness.probe.fake.revoke_tokens();
    harness
        .send(LibraryAction::Open(movie(&harness, "Dune")))
        .await;
    assert!(harness.state().session_expired);
}

#[tokio::test]
async fn once_expired_actions_are_ignored() {
    let harness = Harness::started().await;
    harness.probe.fake.revoke_tokens();
    harness.send(LibraryAction::Refresh).await;
    assert!(harness.state().session_expired);
    let calls = harness.probe.calls("item");
    harness
        .send(LibraryAction::Open(movie(&harness, "Dune")))
        .await;
    assert_eq!(harness.probe.calls("item"), calls);
    assert_eq!(depth(&harness), 1);
}

#[tokio::test]
async fn other_errors_are_typed_state_and_leave_the_session_alone() {
    let harness = Harness::new(probe::catalog(), 50).await;
    harness.probe.fake.fail_with(ApiError::Timeout);
    harness.flow.start().await;
    let state = harness.state();
    assert_eq!(state.home().error, Some(ApiError::Timeout));
    assert!(!state.session_expired);
    assert!(!state.home().loading);

    harness.probe.fake.clear_failure();
    harness.send(LibraryAction::Refresh).await;
    assert_eq!(harness.state().home().error, None);
    assert_eq!(harness.state().home().rows.len(), 5);
}

#[tokio::test]
async fn refresh_on_a_screen_loads_it_again_in_place() {
    let harness = Harness::started().await;
    harness
        .send(LibraryAction::Open(movie(&harness, "Dune")))
        .await;
    harness.send(LibraryAction::Refresh).await;
    assert_eq!(depth(&harness), 2, "still one level deep");
    assert_eq!(harness.probe.calls("item"), 2);

    let id = harness
        .probe
        .fake
        .library_named("Movies")
        .expect("library")
        .id;
    harness
        .send(LibraryAction::Open(Target::Library {
            id,
            name: "Movies".into(),
        }))
        .await;
    harness.send(LibraryAction::Refresh).await;
    assert_eq!(depth(&harness), 3);
    let state = harness.state();
    let ScreenState::Library(grid) = state.current() else {
        panic!("{:?}", state.current());
    };
    assert_eq!(grid.total, Some(2));
}

#[tokio::test]
async fn the_flow_can_be_driven_from_spawned_tasks() {
    // The futures are Send, so the app can spawn them on a multi-threaded runtime.
    let harness = Harness::started().await;
    let flow = std::sync::Arc::clone(&harness.flow);
    tokio::spawn(async move { flow.dispatch(LibraryAction::Refresh).await })
        .await
        .expect("task");
}
