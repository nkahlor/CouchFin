//! The library grid on Slint's testing backend: 2:3 posters under a title,
//! Up, Down, Left and Right within the grid, Enter and Back as callbacks,
//! and, with 2,000 items, only about a screenful of cards ever drawn
//! (decision 015).

mod common;

use common::browse::library_at;
use common::home::{SCREENS, all_cards, current, current_card, fully_on_screen, part, presses};
use common::{press, shows};
use slint::platform::Key;

const BIG: usize = 2000;

#[test]
fn shows_the_title_and_a_grid_of_posters_with_the_first_focused() {
    let library = library_at(BIG, 1920.0, 1080.0);
    assert!(shows(&library.window, "Movies"));
    assert_eq!(current(&library.window).as_deref(), Some("Item 1"));
    let card = current_card(&library.window).expect("a current card");
    let size = part(&card, "poster").size();
    // The focused card is grown by 10%; the ratio is still 2:3.
    assert!((size.width / size.height - 2.0 / 3.0).abs() < 0.02);
    assert!(part(&card, "ring").computed_opacity() > 0.99);
    assert_eq!(card.accessible_item_count(), Some(BIG));
}

#[test]
fn the_arrows_move_within_the_grid_and_stop_at_its_edges() {
    let library = library_at(BIG, 1920.0, 1080.0);
    press(&library.window, Key::LeftArrow);
    press(&library.window, Key::UpArrow);
    assert_eq!(current(&library.window).as_deref(), Some("Item 1"));
    press(&library.window, Key::RightArrow);
    assert_eq!(current(&library.window).as_deref(), Some("Item 2"));
    press(&library.window, Key::DownArrow);
    // 1920 px fits eight posters across, so Down is eight places on.
    assert_eq!(current(&library.window).as_deref(), Some("Item 10"));
    press(&library.window, Key::UpArrow);
    press(&library.window, Key::LeftArrow);
    assert_eq!(current(&library.window).as_deref(), Some("Item 1"));
    press(&library.window, Key::Return);
    assert_eq!(library.activated.taken(), [0]);
}

#[test]
fn the_row_below_a_short_last_row_ends_on_the_last_item() {
    let library = library_at(10, 1920.0, 1080.0);
    press(&library.window, Key::RightArrow);
    press(&library.window, Key::RightArrow);
    presses(&library.window, Key::DownArrow, 3);
    assert_eq!(current(&library.window).as_deref(), Some("Item 10"));
    press(&library.window, Key::Return);
    assert_eq!(library.activated.taken(), [9]);
    presses(&library.window, Key::RightArrow, 3);
    assert_eq!(current(&library.window).as_deref(), Some("Item 10"));
}

#[test]
fn escape_and_backspace_go_back() {
    let library = library_at(BIG, 1920.0, 1080.0);
    press(&library.window, Key::Escape);
    press(&library.window, Key::Backspace);
    assert_eq!(library.backs.taken().len(), 2);
}

#[test]
fn scrolling_keeps_the_focused_card_on_screen_at_both_sizes() {
    for (width, height) in SCREENS {
        let library = library_at(BIG, width, height);
        for step in 0..40 {
            press(&library.window, Key::DownArrow);
            let card = current_card(&library.window).expect("a current card");
            assert!(
                fully_on_screen(&library.window, &card),
                "{width}x{height}: card {} is off screen after {step} Downs",
                card.accessible_label().unwrap_or_default()
            );
        }
        presses(&library.window, Key::UpArrow, 40);
        assert_eq!(current(&library.window).as_deref(), Some("Item 1"));
    }
}

#[test]
fn a_2000_item_library_draws_only_about_a_screenful_of_cards() {
    for (width, height) in SCREENS {
        let library = library_at(BIG, width, height);
        presses(&library.window, Key::DownArrow, 30);
        common::home::settle();
        let drawn = all_cards(&library.window).len();
        assert!(
            (4..=60).contains(&drawn),
            "{width}x{height}: {drawn} card elements for {BIG} items"
        );
        // Nor were the other items even asked for.
        let read = library.read.borrow().iter().filter(|read| **read).count();
        assert!(read < 120, "{width}x{height}: {read} of {BIG} items read");
    }
}

#[test]
fn the_grid_says_which_places_are_on_screen() {
    let library = library_at(BIG, 1920.0, 1080.0);
    let (first, last) = *library.needed.taken().last().expect("a first request");
    assert_eq!(first, 0);
    assert!((8..=60).contains(&last), "the first screen ends at {last}");
    presses(&library.window, Key::DownArrow, 20);
    common::home::settle();
    let (first, last) = *library
        .needed
        .taken()
        .last()
        .expect("a request after scrolling");
    assert!(first > 0 && last > first && last <= i32::try_from(BIG).unwrap());
    assert!(last - first < 60);
}

#[test]
fn places_not_loaded_yet_show_an_empty_card() {
    let library = library_at(0, 1920.0, 1080.0);
    assert!(all_cards(&library.window).is_empty());
    press(&library.window, Key::Return);
    assert_eq!(library.activated.taken().len(), 0);
}
