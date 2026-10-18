//! Home rows: order, hidden empty rows, progress, focus and concurrent loading.

#[path = "common/probe.rs"]
mod probe;

use std::time::Duration;

use couchfin_core::{CardShape, LibraryAction, RowKind, ScreenState, Target};
use probe::Harness;

fn titles(harness: &Harness) -> Vec<String> {
    harness
        .state()
        .home()
        .rows
        .iter()
        .map(|row| row.title.clone())
        .collect()
}

#[tokio::test]
async fn rows_come_in_decision_014_order_and_empty_ones_are_dropped() {
    let harness = Harness::started().await;
    assert_eq!(
        titles(&harness),
        [
            "Continue Watching",
            "Next Up",
            "Recently Added in Movies",
            "Recently Added in Shows",
            "Libraries"
        ]
    );
    // "Empty" has no recent items, so no row; it is still a tile.
    let state = harness.state();
    let libraries = &state.home().rows[4];
    assert_eq!(libraries.kind, RowKind::Libraries);
    assert_eq!(libraries.cards.len(), 3);
    assert!(!state.home().loading);
}

#[tokio::test]
async fn a_server_with_nothing_to_resume_has_no_continue_watching() {
    let fake = probe::server();
    let fake = couchfin_core::fake::FakeJellyfin::new(fake).with_user("alice", "hunter2");
    let harness = Harness::new(fake, 50).await;
    harness.flow.start().await;
    assert_eq!(harness.state().home().rows.len(), 0);
}

#[tokio::test]
async fn shapes_follow_the_row() {
    let harness = Harness::started().await;
    let shapes: Vec<_> = harness
        .state()
        .home()
        .rows
        .iter()
        .map(|r| r.shape)
        .collect();
    let (poster, thumb) = (CardShape::Poster, CardShape::Thumb);
    assert_eq!(shapes, [thumb, thumb, poster, poster, thumb]);
}

#[tokio::test]
async fn continue_watching_has_progress_and_episode_subtitles() {
    let harness = Harness::started().await;
    let state = harness.state();
    let row = &state.home().rows[0];
    let [movie, episode] = row.cards.as_slice() else {
        panic!("two cards expected: {row:#?}");
    };
    assert_eq!(movie.title, "Arrival");
    assert_eq!(movie.subtitle, None);
    assert_eq!(movie.progress, Some(0.5));
    assert_eq!(episode.title, "Severance");
    assert_eq!(episode.subtitle.as_deref(), Some("S2 · E3"));
    let progress = episode.progress.expect("a third watched");
    assert!((progress - 1.0 / 3.0).abs() < 0.001, "{progress}");
    assert!(movie.image.is_some());
}

#[tokio::test]
async fn cards_carry_the_year_and_runtime_for_the_details_strip() {
    let harness = Harness::started().await;
    let state = harness.state();
    let movie = &state.home().rows[0].cards[0];
    assert_eq!(movie.title, "Arrival");
    assert_eq!(movie.year, Some(2000));
    assert_eq!(movie.runtime, Some(Duration::from_mins(100)));
    // A library tile has neither.
    let tile = &state.home().rows[4].cards[0];
    assert_eq!((tile.year, tile.runtime), (None, None));
}

#[tokio::test]
async fn next_up_has_subtitles_but_no_progress() {
    let harness = Harness::started().await;
    let state = harness.state();
    let card = &state.home().rows[1].cards[0];
    assert_eq!(card.subtitle.as_deref(), Some("S1 · E2"));
    assert_eq!(card.progress, None);
}

#[tokio::test]
async fn recently_added_lists_the_libraries_posters() {
    let harness = Harness::started().await;
    let state = harness.state();
    let names: Vec<_> = state.home().rows[2]
        .cards
        .iter()
        .map(|c| c.title.as_str())
        .collect();
    assert_eq!(names, ["Dune", "Arrival"], "newest first");
    assert!(matches!(
        state.home().rows[2].kind,
        RowKind::RecentlyAdded(_)
    ));
}

#[tokio::test]
async fn each_row_keeps_its_focus_across_back() {
    let harness = Harness::started().await;
    harness
        .send(LibraryAction::FocusChanged { row: 0, index: 1 })
        .await;
    harness
        .send(LibraryAction::FocusChanged { row: 2, index: 1 })
        .await;
    let libraries = harness.row_index("Libraries");
    harness
        .send(LibraryAction::OpenCard {
            row: libraries,
            index: 0,
        })
        .await;
    assert!(matches!(harness.state().current(), ScreenState::Library(_)));
    harness.send(LibraryAction::Back).await;

    let state = harness.state();
    let focus: Vec<_> = state.home().rows.iter().map(|r| r.focused).collect();
    assert_eq!(focus, [1, 0, 1, 0, 0]);
}

#[tokio::test]
async fn focus_is_clamped_to_the_row() {
    let harness = Harness::started().await;
    harness
        .send(LibraryAction::FocusChanged { row: 0, index: 99 })
        .await;
    assert_eq!(harness.state().home().rows[0].focused, 1);
    harness
        .send(LibraryAction::FocusChanged { row: 99, index: 0 })
        .await;
}

#[tokio::test]
async fn rows_are_published_as_they_arrive() {
    let harness = Harness::new(probe::catalog(), 50).await;
    harness.probe.close("next_up");
    let start = {
        let flow = std::sync::Arc::clone(&harness.flow);
        tokio::spawn(async move { flow.start().await })
    };
    // Next Up is still held, yet the other rows are already there.
    harness
        .wait_until(|s| s.home().rows.iter().any(|r| r.kind == RowKind::Libraries))
        .await;
    let partial = harness.state();
    assert!(partial.home().loading);
    assert!(
        partial
            .home()
            .rows
            .iter()
            .all(|r| r.kind != RowKind::NextUp)
    );
    assert!(
        partial
            .home()
            .rows
            .iter()
            .any(|r| r.kind == RowKind::ContinueWatching)
    );

    harness.probe.open("next_up");
    start.await.expect("start finishes");
    let done = harness.state();
    assert!(!done.home().loading);
    assert_eq!(done.home().rows.len(), 5);
    assert_eq!(
        done.home().rows[1].kind,
        RowKind::NextUp,
        "inserted in order"
    );

    // Every published state is a snapshot: the first one has fewer rows.
    let seen = harness.seen.lock().unwrap();
    let counts: Vec<_> = seen.iter().map(|s| s.home().rows.len()).collect();
    assert!(counts.windows(2).all(|w| w[0] <= w[1]), "{counts:?}");
    assert!(counts.contains(&1) || counts.contains(&2), "{counts:?}");
}

#[tokio::test]
async fn refresh_reloads_rows_and_keeps_focus() {
    let harness = Harness::started().await;
    harness
        .send(LibraryAction::FocusChanged { row: 0, index: 1 })
        .await;
    let fake = harness.probe.fake.clone();
    let _ = fake.with_resume("Dune", Duration::from_mins(10));
    harness.send(LibraryAction::Refresh).await;
    let state = harness.state();
    assert_eq!(state.home().rows[0].cards.len(), 3);
    assert_eq!(state.home().rows[0].focused, 1);
    assert_eq!(harness.probe.calls("resume"), 2);
}

#[tokio::test]
async fn opening_cards_targets_the_right_screen() {
    let harness = Harness::started().await;
    let state = harness.state();
    assert!(matches!(
        state.home().rows[0].cards[0].target,
        Target::Movie { .. }
    ));
    assert!(matches!(
        state.home().rows[0].cards[1].target,
        Target::Series {
            season: Some(2),
            ..
        }
    ));
    assert!(matches!(
        state.home().rows[4].cards[0].target,
        Target::Library { .. }
    ));
}
