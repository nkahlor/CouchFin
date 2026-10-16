//! Home's row of placeholder title cards on Slint's testing backend: ten
//! cards, the first focused on arrival, Left and Right walk the row and stop
//! at its ends, the row scrolls to keep the focused card on screen, Down and
//! Up move between the row and Sign out, and Enter selects a card.

mod common;

use std::time::Duration;

use common::{Calls, labelled, press, shown, shows, the};
use couchfin_ui::{HomeView, MainWindow, Screen};
use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementQuery, mock_elapsed_time};
use slint::platform::Key;
use slint::{ComponentHandle, LogicalSize};

/// The longest the row may take to scroll a card into view.
const SCROLL: Duration = Duration::from_millis(150);
const NOTICE: &str =
    "Couchfin couldn't save your sign-in, so you'll need to sign in again next time.";

struct Home {
    window: MainWindow,
    activated: Calls<i32>,
    sign_outs: Calls<()>,
}

/// Home in a window of the given size, with its callbacks recorded.
fn home_at(width: f32, height: f32) -> Home {
    let window = shown(Screen::Home);
    window.window().set_size(LogicalSize::new(width, height));
    let view = window.global::<HomeView>();
    view.set_server_name("Living Room".into());
    view.set_server_version("10.11.2".into());
    view.set_username("alice".into());
    let activated = Calls::new();
    let recorder = activated.clone();
    view.on_card_activated(move |index| recorder.record(index));
    let sign_outs = Calls::new();
    let recorder = sign_outs.clone();
    view.on_sign_out(move || recorder.record(()));
    Home {
        window,
        activated,
        sign_outs,
    }
}

fn home() -> Home {
    home_at(1920.0, 1080.0)
}

fn cards(window: &MainWindow) -> Vec<ElementHandle> {
    ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::ListItem)
        .find_all()
}

/// The title of the row's current card, if exactly one, once the row has
/// scrolled to it. The testing backend only finds elements on screen.
fn current(window: &MainWindow) -> Option<String> {
    mock_elapsed_time(SCROLL);
    let on: Vec<_> = cards(window)
        .into_iter()
        .filter(|card| card.accessible_item_selected() == Some(true))
        .collect();
    match on.as_slice() {
        [card] => card.accessible_label().map(|label| label.to_string()),
        _ => None,
    }
}

fn presses(window: &MainWindow, key: Key, times: usize) {
    for _ in 0..times {
        press(window, key);
    }
}

/// Whether the element lies inside the window, edges included.
fn fully_on_screen(window: &MainWindow, element: &ElementHandle) -> bool {
    let size = window
        .window()
        .size()
        .to_logical(window.window().scale_factor());
    let at = element.absolute_position();
    let own = element.size();
    at.x >= 0.0 && at.y >= 0.0 && at.x + own.width <= size.width && at.y + own.height <= size.height
}

/// Whether the card lies across the row's visible width, edges included.
fn fully_in_row(window: &MainWindow, card: &ElementHandle) -> bool {
    let row = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::List)
        .find_first()
        .expect("Home has a row");
    let (left, width) = (row.absolute_position().x, row.size().width);
    let at = card.absolute_position().x;
    at >= left && at + card.size().width <= left + width
}

#[test]
fn shows_ten_placeholder_cards_with_their_titles() {
    let home = home();
    let mut titles = vec![current(&home.window)];
    for _ in 1..10 {
        press(&home.window, Key::RightArrow);
        titles.push(current(&home.window));
    }
    let expected: Vec<_> = (1..=10).map(|n| Some(format!("Title {n}"))).collect();
    assert_eq!(titles, expected);
    for card in cards(&home.window) {
        assert_eq!(card.accessible_item_count(), Some(10));
    }
}

#[test]
fn the_first_card_has_focus_on_arrival() {
    let home = home();
    assert_eq!(current(&home.window).as_deref(), Some("Title 1"));
    press(&home.window, Key::Return);
    assert_eq!(home.activated.taken(), [0]);
    assert!(
        home.sign_outs.taken().is_empty(),
        "Enter on arrival never signs out"
    );
}

#[test]
fn left_and_right_walk_the_row_and_stop_at_its_ends() {
    let home = home();
    press(&home.window, Key::LeftArrow);
    assert_eq!(current(&home.window).as_deref(), Some("Title 1"));
    presses(&home.window, Key::RightArrow, 2);
    assert_eq!(current(&home.window).as_deref(), Some("Title 3"));
    press(&home.window, Key::LeftArrow);
    assert_eq!(current(&home.window).as_deref(), Some("Title 2"));
    presses(&home.window, Key::RightArrow, 12);
    assert_eq!(current(&home.window).as_deref(), Some("Title 10"));
    press(&home.window, Key::Return);
    assert_eq!(home.activated.taken(), [9]);
}

#[test]
fn enter_selects_the_focused_card_and_says_so() {
    let home = home();
    assert!(labelled(&home.window, "Selected: Title 3").is_empty());
    presses(&home.window, Key::RightArrow, 2);
    press(&home.window, Key::Return);
    assert_eq!(home.activated.taken(), [2]);
    assert!(shows(&home.window, "Selected: Title 3"));
}

#[test]
fn a_card_can_be_selected_by_its_default_action() {
    let home = home();
    the(&home.window, "Title 4").invoke_accessible_default_action();
    assert_eq!(home.activated.taken(), [3]);
    assert_eq!(current(&home.window).as_deref(), Some("Title 4"));
}

#[test]
fn the_row_scrolls_to_keep_the_focused_card_fully_on_screen() {
    for (width, height) in [(1920.0, 1080.0), (1280.0, 720.0)] {
        let home = home_at(width, height);
        // The testing backend only finds elements at least partly on screen.
        assert!(
            labelled(&home.window, "Title 10").is_empty(),
            "{width}x{height}: the row is wider than the screen"
        );
        for n in 1..=10 {
            mock_elapsed_time(SCROLL);
            let card = the(&home.window, &format!("Title {n}"));
            assert!(
                fully_in_row(&home.window, &card),
                "{width}x{height}: Title {n} is focused but not fully on screen"
            );
            press(&home.window, Key::RightArrow);
        }
        for n in (1..=10).rev() {
            mock_elapsed_time(SCROLL);
            let card = the(&home.window, &format!("Title {n}"));
            assert!(
                fully_in_row(&home.window, &card),
                "{width}x{height}: Title {n}"
            );
            press(&home.window, Key::LeftArrow);
        }
    }
}

#[test]
fn the_focused_card_stays_centered_as_titles_scroll() {
    for (width, height) in [(1920.0, 1080.0), (1280.0, 720.0)] {
        let home = home_at(width, height);
        let row = ElementQuery::from_root(&home.window)
            .match_accessible_role(AccessibleRole::List)
            .find_first()
            .expect("Home has a row");
        let center = row.absolute_position().x + row.size().width / 2.0;

        for n in 1..=10 {
            mock_elapsed_time(SCROLL);
            let card = the(&home.window, &format!("Title {n}"));
            let card_center = card.absolute_position().x + card.size().width / 2.0;
            assert!(
                (card_center - center).abs() <= 1.0,
                "{width}x{height}: Title {n} is not centered"
            );
            press(&home.window, Key::RightArrow);
        }
    }
}

#[test]
fn nothing_is_cut_off_on_a_720p_screen() {
    let home = home_at(1280.0, 720.0);
    home.window.global::<HomeView>().set_notice(NOTICE.into());
    for label in [
        "Living Room",
        "Jellyfin 10.11.2",
        "Signed in as alice",
        "Title 1",
        "Sign out",
        NOTICE,
    ] {
        let element = the(&home.window, label);
        assert!(
            fully_on_screen(&home.window, &element),
            "{label:?} is cut off"
        );
    }
}

#[test]
fn down_moves_to_sign_out_and_up_returns_to_the_same_card() {
    let home = home();
    presses(&home.window, Key::RightArrow, 3);
    press(&home.window, Key::DownArrow);
    assert_eq!(
        current(&home.window).as_deref(),
        Some("Title 4"),
        "the row remembers"
    );
    press(&home.window, Key::Return);
    assert_eq!(home.sign_outs.taken().len(), 1);
    assert_eq!(home.activated.taken().len(), 0);

    // Left and Right on Sign out leave the row alone.
    press(&home.window, Key::RightArrow);
    press(&home.window, Key::UpArrow);
    press(&home.window, Key::Return);
    assert_eq!(home.activated.taken(), [3]);
    assert_eq!(home.sign_outs.taken().len(), 0);
}

#[test]
fn tab_and_shift_tab_move_between_the_row_and_sign_out() {
    let home = home();
    press(&home.window, Key::Tab);
    press(&home.window, Key::Return);
    assert_eq!(home.sign_outs.taken().len(), 1);
    common::shift_tab(&home.window);
    press(&home.window, Key::Return);
    assert_eq!(home.activated.taken(), [0]);
}
