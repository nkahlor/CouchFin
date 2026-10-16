//! Home's row as a carousel, on Slint's testing backend: the active card
//! stays in one slot near the left while the row slides under it, the card
//! before it peeks in from the left edge, cards already passed are dimmed,
//! and the active card is larger and ringed. Every animation settles within
//! 150 ms, and focus moves at once even while the row is still sliding.

mod common;

use std::time::Duration;

use common::home::{SCREENS, cards, current_card, home, home_at, part, presses, settle};
use common::press;
use couchfin_ui::MainWindow;
use i_slint_backend_testing::{ElementHandle, mock_elapsed_time};
use slint::platform::Key;

fn index(card: &ElementHandle) -> usize {
    card.accessible_item_index()
        .expect("a card knows its place")
}

fn active(window: &MainWindow) -> ElementHandle {
    current_card(window).expect("the row has one current card")
}

fn label(card: &ElementHandle) -> String {
    card.accessible_label().unwrap_or_default().to_string()
}

#[test]
fn the_active_card_stays_in_one_slot_from_the_first_card_to_the_last() {
    for (width, height) in SCREENS {
        let home = home_at(width, height);
        let anchor = active(&home.window).absolute_position().x;
        assert!(
            anchor > 0.0 && anchor < width / 4.0,
            "{width}x{height}: the slot is near the left, at {anchor}"
        );
        for n in 1..=10 {
            let card = active(&home.window);
            assert_eq!(label(&card), format!("Title {n}"));
            let x = card.absolute_position().x;
            assert!(
                (x - anchor).abs() < 0.5,
                "{width}x{height}: Title {n} is at {x}, not the slot at {anchor}"
            );
            press(&home.window, Key::RightArrow);
        }
        presses(&home.window, Key::LeftArrow, 9);
        let x = active(&home.window).absolute_position().x;
        assert!(
            (x - anchor).abs() < 0.5,
            "{width}x{height}: back to the slot"
        );
    }
}

#[test]
fn the_card_before_the_active_one_peeks_in_from_the_left_edge() {
    for (width, height) in SCREENS {
        let home = home_at(width, height);
        let shown = cards(&home.window);
        assert_eq!(
            index(&shown[0]),
            0,
            "{width}x{height}: nothing before Title 1"
        );
        for current in 1..10 {
            press(&home.window, Key::RightArrow);
            settle();
            let shown = cards(&home.window);
            let peek = &shown[0];
            assert_eq!(
                index(peek),
                current - 1,
                "{width}x{height}: only one card peeks"
            );
            let (x, w) = (peek.absolute_position().x, peek.size().width);
            assert!(
                x < 0.0 && x + w > 0.0,
                "{width}x{height}: {} spans {x}..{} and isn't cut by the edge",
                label(peek),
                x + w
            );
        }
    }
}

#[test]
fn the_row_keeps_sliding_to_the_last_card_and_leaves_space_after_it() {
    let home = home();
    presses(&home.window, Key::RightArrow, 9);
    settle();
    let shown = cards(&home.window);
    let last = shown.last().expect("cards are on screen");
    assert_eq!(label(last), "Title 10");
    assert_eq!(
        shown.len(),
        2,
        "Title 9 peeks, Title 10 is active, then space"
    );
}

#[test]
fn cards_already_passed_are_dimmed_and_the_rest_are_not() {
    let home = home();
    presses(&home.window, Key::RightArrow, 4);
    settle();
    let shown = cards(&home.window);
    assert!(
        shown.iter().any(|card| index(card) < 4),
        "a passed card peeks in"
    );
    for card in shown {
        let shade = part(&card, "shade").computed_opacity();
        if index(&card) < 4 {
            assert!(shade > 0.99, "{} isn't dimmed", label(&card));
        } else {
            assert_eq!(shade, 0.0, "{} is dimmed", label(&card));
        }
    }
}

#[test]
fn the_active_card_is_larger_and_ringed_and_the_others_are_not() {
    let home = home();
    press(&home.window, Key::RightArrow);
    settle();
    let shown = cards(&home.window);
    let active_card = active(&home.window);
    let big = part(&active_card, "poster").size();
    assert!(part(&active_card, "ring").computed_opacity() > 0.99);
    for card in shown.iter().filter(|card| index(card) != 1) {
        let size = part(card, "poster").size();
        assert!(
            big.width > size.width * 1.05 && big.height > size.height * 1.05,
            "Title 2 isn't larger than {}",
            label(card)
        );
        assert_eq!(part(card, "ring").computed_opacity(), 0.0);
    }
}

#[test]
fn moving_to_sign_out_drops_the_highlight_and_up_brings_it_back() {
    let home = home();
    press(&home.window, Key::RightArrow);
    press(&home.window, Key::DownArrow);
    settle();
    let shown = cards(&home.window);
    let width = part(&shown[1], "poster").size().width;
    for card in &shown {
        assert_eq!(part(card, "ring").computed_opacity(), 0.0);
        assert!((part(card, "poster").size().width - width).abs() < 0.5);
    }
    press(&home.window, Key::UpArrow);
    assert!(part(&active(&home.window), "ring").computed_opacity() > 0.99);
}

#[test]
fn focus_moves_at_once_while_the_row_is_still_sliding() {
    let home = home();
    let anchor = active(&home.window).absolute_position().x;
    // A held D-pad repeats every 60 ms or so; nothing here waits for the slide.
    for _ in 0..3 {
        press(&home.window, Key::RightArrow);
        mock_elapsed_time(Duration::from_millis(60));
    }
    press(&home.window, Key::Return);
    assert_eq!(home.activated.taken(), [3]);

    // Turning back mid-slide retargets the slide rather than finishing first.
    press(&home.window, Key::LeftArrow);
    let card = active(&home.window);
    assert_eq!(label(&card), "Title 3");
    assert!((card.absolute_position().x - anchor).abs() < 0.5);
}
