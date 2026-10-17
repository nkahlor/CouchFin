//! Home bound to couchfin-core's library state: its rows, the actions the
//! cards send back, and which pictures are fetched for them (decision 015).

mod common;

use common::home::presses;
use common::library::{Shell, World, picture};
use common::press;
use couchfin_core::{ApiError, ImageLoad, LibraryState, ScreenState};
use couchfin_ui::{HomeView, ImageState, Screen};
use slint::platform::Key;
use slint::{ComponentHandle, Model};

fn titles(shell: &Shell) -> Vec<String> {
    shell
        .window
        .global::<HomeView>()
        .get_rows()
        .iter()
        .map(|row| row.title.to_string())
        .collect()
}

fn cards_of(shell: &Shell, row: usize) -> Vec<couchfin_ui::CardData> {
    shell
        .window
        .global::<HomeView>()
        .get_rows()
        .row_data(row)
        .expect("a row")
        .cards
        .iter()
        .collect()
}

fn current_of(shell: &Shell, row: usize) -> i32 {
    shell
        .window
        .global::<HomeView>()
        .get_rows()
        .row_data(row)
        .expect("a row")
        .current
}

/// Home with `rows` copies of its Recently Added row, each with its own
/// pictures, so a test can tell which row a load belongs to.
fn many_rows(rows: usize) -> LibraryState {
    let mut state = World::new(40).state();
    let ScreenState::Home(home) = &mut state.screens[0] else {
        panic!("Home is first");
    };
    let template = home
        .rows
        .iter()
        .find(|row| row.title.starts_with("Recently Added"))
        .expect("a Recently Added row")
        .clone();
    home.rows = (0..rows)
        .map(|r| {
            let mut row = template.clone();
            row.title = format!("Row {r}");
            row.order = u32::try_from(r).expect("small");
            row.kind =
                couchfin_core::RowKind::RecentlyAdded(format!("lib-{r}").parse().expect("id"));
            for (n, card) in row.cards.iter_mut().enumerate() {
                if let Some(image) = &mut card.image {
                    image.item = format!("r{r}c{n}").parse().expect("id");
                }
            }
            row
        })
        .collect();
    state
}

fn id_of(row: usize, card: usize) -> String {
    format!("r{row}c{card}")
}

#[test]
fn the_rows_come_from_core_in_order_with_their_cards() {
    let shell = Shell::new();
    shell.show(World::new(40).state());
    assert_eq!(shell.window.get_screen(), Screen::Home);
    assert_eq!(
        titles(&shell),
        ["Continue Watching", "Recently Added in Big", "Libraries"]
    );
    let resumed = cards_of(&shell, 0);
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].title, "Big 00001");
    assert!(resumed[0].progress > 0.0, "a resume row shows progress");
    assert!(cards_of(&shell, 1).len() > 5);
    assert_eq!(cards_of(&shell, 2)[0].title, "Big");
}

#[test]
fn cards_carry_the_line_the_details_strip_shows() {
    let shell = Shell::new();
    shell.show(World::new(40).state());
    // A movie: year and runtime, formatted here, not in core.
    assert_eq!(cards_of(&shell, 0)[0].meta, "2000 · 1 h 40 min");
    // A library tile: just what it is.
    assert_eq!(cards_of(&shell, 2)[0].meta, "Library");
}

#[test]
fn a_card_without_a_picture_says_so_and_one_still_loading_does_not() {
    let shell = Shell::new();
    let mut state = World::new(40).state();
    let ScreenState::Home(home) = &mut state.screens[0] else {
        panic!("Home is first");
    };
    home.rows[1].cards[2].image = None;
    let first = home.rows[1].cards[0]
        .image
        .as_ref()
        .expect("a picture")
        .item
        .to_string();
    shell.show(state);
    let cards = cards_of(&shell, 1);
    assert_eq!(
        cards[2].image_state,
        ImageState::Missing,
        "no image ref: nothing will come"
    );
    assert_eq!(
        cards[0].image_state,
        ImageState::Loading,
        "a picture still loading is not missing"
    );

    // The server answers that it has none: the title moves inside.
    shell.loader.finish(&first, ImageLoad::NoImage);
    shell.feed.pump();
    let cards = cards_of(&shell, 1);
    assert_eq!(
        cards[0].image_state,
        ImageState::Missing,
        "NoImage means missing"
    );
    assert_eq!(cards[1].image_state, ImageState::Loading);
}

#[test]
fn the_screen_is_left_alone_until_the_library_is_shown() {
    let shell = Shell::new();
    shell.window.set_screen(Screen::Login);
    shell.feed.state(World::new(5).state());
    shell.feed.pump();
    assert_eq!(shell.window.get_screen(), Screen::Login);
    shell.feed.show();
    shell.feed.pump();
    assert_eq!(shell.window.get_screen(), Screen::Home);
    shell.feed.reset();
    shell.window.set_screen(Screen::Login);
    shell.feed.pump();
    assert_eq!(shell.window.get_screen(), Screen::Login);
    assert!(titles(&shell).is_empty(), "reset empties Home");
}

#[test]
fn enter_on_a_card_opens_it_and_moving_reports_the_focus() {
    let shell = Shell::new();
    shell.show(World::new(40).state());
    press(&shell.window, Key::DownArrow);
    press(&shell.window, Key::RightArrow);
    press(&shell.window, Key::Return);
    assert_eq!(
        shell.commands.taken(),
        [
            "FocusChanged { row: 1, index: 0 }",
            "FocusChanged { row: 1, index: 1 }",
            "OpenCard { row: 1, index: 1 }",
        ]
    );
}

#[test]
fn back_on_home_quits() {
    let shell = Shell::new();
    shell.show(World::new(5).state());
    press(&shell.window, Key::Escape);
    assert_eq!(shell.commands.taken(), ["Quit"]);
}

#[test]
fn rows_arriving_above_keep_the_focused_row_and_each_rows_card() {
    let shell = Shell::new();
    let full = World::new(40).state();
    let mut partial = full.clone();
    let ScreenState::Home(home) = &mut partial.screens[0] else {
        panic!("Home is first");
    };
    home.rows.drain(..1);
    home.loading = true;
    shell.show(partial);
    presses(&shell.window, Key::RightArrow, 2);
    assert_eq!(current_of(&shell, 0), 2);

    shell.update(full);
    assert_eq!(titles(&shell).len(), 3);
    assert_eq!(current_of(&shell, 1), 2, "Recently Added kept its card");
    let view = shell.window.global::<HomeView>();
    assert_eq!(view.get_focused_row(), 1, "focus stayed on Recently Added");
    assert_eq!(view.get_posters_above(), 0);
    assert_eq!(
        view.get_thumbs_above(),
        1,
        "Continue Watching is above it now"
    );
}

#[test]
fn a_new_row_starts_on_the_card_core_remembers() {
    let shell = Shell::new();
    let mut state = World::new(40).state();
    let ScreenState::Home(home) = &mut state.screens[0] else {
        panic!("Home is first");
    };
    home.rows[1].focused = 4;
    shell.show(state);
    assert_eq!(current_of(&shell, 1), 4);
}

#[test]
fn home_says_when_it_is_loading_or_why_it_failed() {
    let shell = Shell::new();
    let mut state = World::new(5).state();
    let ScreenState::Home(home) = &mut state.screens[0] else {
        panic!("Home is first");
    };
    home.rows.clear();
    home.loading = true;
    shell.show(state.clone());
    let view = shell.window.global::<HomeView>();
    assert_eq!(view.get_status(), "Loading your library…");

    let ScreenState::Home(home) = &mut state.screens[0] else {
        panic!("Home is first");
    };
    home.loading = false;
    home.error = Some(ApiError::Timeout);
    shell.update(state);
    assert_eq!(
        view.get_status(),
        "The server took too long to answer. Try again in a moment."
    );
    assert_eq!(view.get_error(), "");

    shell.update(World::new(5).state());
    assert_eq!(view.get_status(), "");
}

#[test]
fn a_failed_row_is_reported_beside_the_rows_that_loaded() {
    let shell = Shell::new();
    let mut state = World::new(5).state();
    let ScreenState::Home(home) = &mut state.screens[0] else {
        panic!("Home is first");
    };
    home.error = Some(ApiError::Status { status: 500 });
    shell.show(state);
    let view = shell.window.global::<HomeView>();
    assert_eq!(
        view.get_error(),
        "The server had a problem. Try again in a moment."
    );
    assert_eq!(view.get_status(), "");
}

#[test]
fn only_cards_near_the_focus_get_pictures() {
    let shell = Shell::new();
    shell.show(many_rows(6));
    let started = shell.loader.started();
    let rows_asked: std::collections::BTreeSet<char> = started
        .iter()
        .filter_map(|(item, ..)| item.chars().nth(1))
        .collect();
    // The three rows on screen under the details strip and one more below
    // the fold; nothing further down.
    assert_eq!(rows_asked, ['0', '1', '2', '3'].into());
    let first_row: Vec<_> = started
        .iter()
        .filter(|(item, ..)| item.starts_with("r0c"))
        .collect();
    assert!(first_row.len() >= 4, "the cards on screen: {first_row:?}");
    assert!(
        first_row.len() < 20,
        "not the whole row: {}",
        first_row.len()
    );
    assert!(
        started.iter().any(|(item, ..)| *item == id_of(0, 0)),
        "the focused card is first"
    );
    // Sized for the screen: a poster at most about 280 points wide.
    assert!(
        started
            .iter()
            .all(|(_, width, _)| *width <= 320 && *width >= 32)
    );
}

#[test]
fn moving_down_cancels_what_scrolled_away_and_starts_the_rows_coming_in() {
    let shell = Shell::new();
    shell.show(many_rows(8));
    assert!(
        shell
            .loader
            .pending()
            .iter()
            .any(|item| item.starts_with("r0c"))
    );
    presses(&shell.window, Key::DownArrow, 5);
    let pending = shell.loader.pending();
    assert!(
        pending.iter().all(|item| !item.starts_with("r0c")),
        "row 0 is far behind now: {pending:?}"
    );
    assert!(
        shell
            .loader
            .cancelled()
            .iter()
            .any(|item| item.starts_with("r0c")),
        "its loads were cancelled"
    );
    assert!(
        pending
            .iter()
            .any(|item| item.starts_with("r6c") || item.starts_with("r7c"))
    );
}

#[test]
fn a_finished_load_fills_the_card_and_no_image_keeps_the_placeholder() {
    let shell = Shell::new();
    shell.show(many_rows(3));
    shell.loader.finish(&id_of(0, 0), picture());
    shell.loader.finish(&id_of(0, 1), ImageLoad::NoImage);
    shell.feed.pump();
    let cards = cards_of(&shell, 0);
    assert_eq!(cards[0].image.size().width, 2);
    assert_eq!(cards[0].image.size().height, 3);
    assert_eq!(
        cards[1].image.size().width,
        0,
        "NoImage keeps the placeholder"
    );
    // And it isn't asked for again when the user moves.
    let before = shell.loader.started().len();
    press(&shell.window, Key::RightArrow);
    let again = shell
        .loader
        .started()
        .iter()
        .skip(before)
        .filter(|(item, ..)| *item == id_of(0, 1))
        .count();
    assert_eq!(again, 0);
}

#[test]
fn a_result_for_a_slot_that_moved_on_is_dropped() {
    let shell = Shell::new();
    shell.show(many_rows(8));
    let stale = shell
        .loader
        .pending()
        .into_iter()
        .find(|item| item.starts_with("r0c"))
        .expect("a row 0 load");
    presses(&shell.window, Key::DownArrow, 5);
    // The load was cancelled; if its answer still arrives it changes nothing.
    assert!(shell.loader.cancelled().contains(&stale));
    shell.loader.finish_cancelled(&stale, picture());
    shell.feed.pump();
    assert!(
        cards_of(&shell, 0)
            .iter()
            .all(|card| card.image.size().width == 0)
    );
}
