//! Home's row of placeholder title cards on Slint's testing backend: ten
//! cards, the first focused on arrival, Left and Right walk the row and stop
//! at its ends, Down and Up move between the row and Sign out, and Enter
//! selects a card. How the carousel slides and looks is in
//! `home_carousel.rs`.

mod common;

use common::home::{cards, current, fully_on_screen, home, home_at, presses};
use common::{labelled, press, shows, the};
use couchfin_ui::HomeView;
use slint::ComponentHandle;
use slint::platform::Key;

const NOTICE: &str =
    "Couchfin couldn't save your sign-in, so you'll need to sign in again next time.";

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
