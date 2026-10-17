//! The library grid bound to couchfin-core's paged `GridState`: a lazy
//! model, the pages it asks for, and pictures only for what is on screen.

mod common;

use std::collections::BTreeSet;

use common::library::{Shell, World, picture};
use common::press;
use couchfin_core::{ApiError, LibraryAction, ScreenState, Variant};
use couchfin_ui::{LibraryView, Screen};
use slint::platform::Key;
use slint::{ComponentHandle, Model};

/// A shown grid of a 2,000-movie library with only its first page loaded.
fn grid() -> (Shell, World) {
    let shell = Shell::new();
    let world = World::new(2_000);
    world.open_big();
    shell.show(world.state());
    (shell, world)
}

/// The picture IDs of the loaded places of the grid on top.
fn ids(world: &World) -> Vec<Option<String>> {
    let state = world.state();
    let ScreenState::Library(grid) = state.current() else {
        panic!("the grid is on top");
    };
    grid.items
        .iter()
        .map(|slot| {
            slot.as_ref()
                .and_then(|card| card.image.as_ref())
                .map(|image| image.item.as_str().to_owned())
        })
        .collect()
}

/// The ID of the picture of this place.
fn id(world: &World, place: usize) -> String {
    ids(world)[place]
        .clone()
        .expect("a loaded place with a picture")
}

/// The places whose pictures are wanted now.
fn places(shell: &Shell, world: &World) -> BTreeSet<usize> {
    let ids = ids(world);
    shell
        .loader
        .started()
        .iter()
        .filter(|(.., wanted)| *wanted)
        .filter_map(|(item, ..)| ids.iter().position(|id| id.as_deref() == Some(item)))
        .collect()
}

#[test]
fn the_grid_shows_the_total_and_reads_items_lazily() {
    let (shell, _world) = grid();
    assert_eq!(shell.window.get_screen(), Screen::Library);
    let view = shell.window.global::<LibraryView>();
    assert_eq!(view.get_title(), "Big");
    assert_eq!(view.get_item_count(), 2_000);
    let items = view.get_items();
    assert_eq!(items.row_count(), 2_000);
    assert_eq!(items.row_data(0).expect("item 0").title, "Big 00000");
    assert_eq!(items.row_data(49).expect("item 49").title, "Big 00049");
    // Not loaded yet: an empty placeholder card, not a missing one.
    let waiting = items.row_data(50).expect("item 50 has a placeholder");
    assert_eq!(waiting.title, "");
    assert_eq!(waiting.image.size().width, 0);
    assert!(items.row_data(2_000).is_none(), "past the end");
}

#[test]
fn what_the_grid_shows_is_asked_of_core_with_a_margin_and_only_once() {
    let (shell, _world) = grid();
    let view = shell.window.global::<LibraryView>();
    shell.commands.taken();
    view.invoke_items_needed(0, 24);
    view.invoke_items_needed(0, 24);
    assert_eq!(
        shell.commands.taken(),
        ["NeedItems { first: 0, last: 35 }"],
        "visible 0 to 23 and half a screen more, once"
    );
    view.invoke_items_needed(100, 124);
    assert_eq!(
        shell.commands.taken(),
        ["NeedItems { first: 88, last: 135 }"]
    );
    view.invoke_items_needed(5, 5);
    assert!(
        shell.commands.taken().is_empty(),
        "an empty range asks nothing"
    );
}

#[test]
fn activating_and_going_back_send_their_actions() {
    let (shell, _world) = grid();
    shell.commands.taken();
    press(&shell.window, Key::RightArrow);
    press(&shell.window, Key::Return);
    press(&shell.window, Key::Escape);
    let sent = shell.commands.taken();
    assert!(sent.contains(&"OpenGridItem(1)".to_owned()), "{sent:?}");
    assert_eq!(sent.last().map(String::as_str), Some("Back"));
}

#[test]
fn pictures_load_for_the_visible_places_and_a_margin_nearest_first() {
    let (shell, world) = grid();
    let view = shell.window.global::<LibraryView>();
    view.invoke_items_needed(0, 24);
    assert_eq!(
        places(&shell, &world),
        (0..36).collect(),
        "24 visible, 12 more"
    );
    let started = shell.loader.started();
    let widths: BTreeSet<u32> = started.iter().map(|(_, _, width, _)| *width).collect();
    assert_eq!(widths.len(), 1, "one size for the grid: {widths:?}");
    assert!(
        started
            .iter()
            .all(|(_, variant, ..)| *variant == Variant::Sharp)
    );
}

#[test]
fn scrolling_far_cancels_loads_and_lets_go_of_pictures_left_behind() {
    let (shell, world) = grid();
    let view = shell.window.global::<LibraryView>();
    view.invoke_items_needed(0, 24);
    shell
        .loader
        .finish(&id(&world, 0), Variant::Sharp, picture());
    shell.feed.pump();
    assert_eq!(
        view.get_items()
            .row_data(0)
            .expect("item")
            .image
            .size()
            .width,
        2
    );

    let first_id = id(&world, 1);
    // A long scroll: core pages in the new place, the old pictures go.
    world.send(LibraryAction::NeedItems {
        first: 1_000,
        last: 1_040,
    });
    view.invoke_items_needed(1_000, 1_024);
    shell.update(world.state());
    assert_eq!(
        places(&shell, &world)
            .into_iter()
            .filter(|p| *p < 1_000)
            .count(),
        0,
        "nothing near the start is wanted any more"
    );
    assert!(
        shell
            .loader
            .cancelled()
            .iter()
            .any(|(item, _)| *item == first_id),
        "an unfinished load far behind was cancelled"
    );
    assert_eq!(
        view.get_items()
            .row_data(0)
            .expect("item")
            .image
            .size()
            .width,
        0,
        "the picture left far behind was let go"
    );
    let wanted = places(&shell, &world);
    assert!(wanted.contains(&1_000) && wanted.contains(&1_023));
}

#[test]
fn a_picture_arrives_on_the_right_card_and_a_nearby_one_stays() {
    let (shell, world) = grid();
    let view = shell.window.global::<LibraryView>();
    view.invoke_items_needed(0, 24);
    shell
        .loader
        .finish(&id(&world, 3), Variant::Sharp, picture());
    shell.feed.pump();
    let items = view.get_items();
    assert_eq!(items.row_data(3).expect("item").image.size().width, 2);
    assert_eq!(items.row_data(4).expect("item").image.size().width, 0);

    // A short scroll keeps what is just behind.
    view.invoke_items_needed(12, 36);
    shell.update(world.state());
    assert_eq!(items.row_data(3).expect("item").image.size().width, 2);
}

#[test]
fn a_failed_page_is_reported_and_leaves_the_grid_standing() {
    let (shell, world) = grid();
    let mut state = world.state();
    let ScreenState::Library(grid) = state.screens.last_mut().expect("a screen") else {
        panic!("the grid is on top");
    };
    grid.error = Some(ApiError::Timeout);
    shell.update(state);
    let view = shell.window.global::<LibraryView>();
    assert_eq!(
        view.get_error(),
        "The server took too long to answer. Try again in a moment."
    );
    assert_eq!(view.get_item_count(), 2_000);
}

#[test]
fn leaving_the_grid_drops_its_loads_and_a_second_visit_starts_afresh() {
    let (shell, world) = grid();
    let view = shell.window.global::<LibraryView>();
    view.invoke_items_needed(0, 24);
    shell
        .loader
        .finish(&id(&world, 0), Variant::Sharp, picture());
    shell.feed.pump();

    // A place Home's rows don't show.
    let far = id(&world, 30);
    assert!(shell.loader.pending().iter().any(|(item, _)| *item == far));
    world.send(LibraryAction::Back);
    shell.update(world.state());
    assert_eq!(shell.window.get_screen(), Screen::Home);
    assert!(
        shell.loader.pending().iter().all(|(item, _)| *item != far),
        "no grid load stays wanted"
    );

    world.open_big();
    shell.update(world.state());
    assert_eq!(shell.window.get_screen(), Screen::Library);
    let view = shell.window.global::<LibraryView>();
    view.invoke_items_needed(0, 24);
    assert_eq!(
        view.get_items()
            .row_data(0)
            .expect("item")
            .image
            .size()
            .width,
        0,
        "the earlier picture isn't carried over"
    );
    assert!(places(&shell, &world).contains(&0), "and is fetched again");
}
