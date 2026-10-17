//! Home as a vertical stack of carousel rows, on Slint's testing backend:
//! the rows come from `HomeView.rows`, drawn exactly as given, Up and
//! Down move between rows, each row remembers its own card, and the page
//! scrolls so the focused row sits at one height (decision 015).

mod common;

use common::home::{
    SCREENS, all_cards, current, current_card, fully_on_screen, home, home_at, part, presses,
    row_cards, settle,
};
use common::{labelled, press, the};
use couchfin_ui::{CardData, CardShape, HomeView, RowData};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, VecModel};

const ROWS: [&str; 5] = [
    "Continue Watching",
    "Next Up",
    "Recently Added in Movies",
    "Recently Added in Shows",
    "Libraries",
];

fn card(title: &str) -> CardData {
    CardData {
        title: title.into(),
        ..Default::default()
    }
}

fn row(title: &str, shape: CardShape, cards: &[&str]) -> RowData {
    RowData {
        title: title.into(),
        shape,
        cards: ModelRc::new(VecModel::from(
            cards.iter().map(|title| card(title)).collect::<Vec<_>>(),
        )),
        current: 0,
    }
}

#[test]
fn the_placeholder_rows_come_in_order() {
    let home = home();
    // Each Down scrolls the next row in; its title and cards show.
    for (n, title) in ROWS.iter().enumerate() {
        settle();
        let shown = row_cards(&home.window, title);
        assert!(!shown.is_empty(), "row {n} {title:?} is shown with cards");
        press(&home.window, Key::DownArrow);
    }
}

#[test]
fn rows_come_from_the_model_and_are_drawn_exactly_as_given() {
    // Core drops empty rows, so the page does no stepping over them: a row
    // it is given is a row it shows and Down lands on.
    let home = home();
    let rows = vec![
        row("First", CardShape::Poster, &["A1", "A2"]),
        row("Third", CardShape::Thumb, &["C1", "C2"]),
        row("Fourth", CardShape::Poster, &["D1"]),
    ];
    home.window
        .global::<HomeView>()
        .set_rows(ModelRc::new(VecModel::from(rows)));
    assert_eq!(current(&home.window).as_deref(), Some("A1"));
    press(&home.window, Key::DownArrow);
    assert_eq!(current(&home.window).as_deref(), Some("C1"));
    press(&home.window, Key::Return);
    assert_eq!(home.activated.taken(), [(1, 0)]);
    press(&home.window, Key::DownArrow);
    assert_eq!(current(&home.window).as_deref(), Some("D1"));
    press(&home.window, Key::UpArrow);
    press(&home.window, Key::UpArrow);
    assert_eq!(current(&home.window).as_deref(), Some("A1"));
}

#[test]
fn an_empty_row_is_not_special_cased_by_the_page() {
    // Not something core sends; this pins that the page has no second rule.
    let home = home();
    let rows = vec![
        row("First", CardShape::Poster, &["A1"]),
        row("Nothing here", CardShape::Poster, &[]),
        row("Third", CardShape::Poster, &["C1"]),
    ];
    home.window
        .global::<HomeView>()
        .set_rows(ModelRc::new(VecModel::from(rows)));
    press(&home.window, Key::DownArrow);
    assert!(
        !labelled(&home.window, "Nothing here").is_empty(),
        "it is drawn"
    );
    press(&home.window, Key::Return);
    assert!(
        home.activated.taken().is_empty(),
        "there is no card to open"
    );
    press(&home.window, Key::DownArrow);
    assert_eq!(current(&home.window).as_deref(), Some("C1"));
}

#[test]
fn each_row_remembers_its_own_card() {
    let home = home();
    presses(&home.window, Key::RightArrow, 3);
    press(&home.window, Key::DownArrow);
    presses(&home.window, Key::RightArrow, 2);
    press(&home.window, Key::DownArrow);
    press(&home.window, Key::RightArrow);
    assert_eq!(current(&home.window).as_deref(), Some("Movie 2"));

    press(&home.window, Key::UpArrow);
    assert_eq!(current(&home.window).as_deref(), Some("Episode 3"));
    press(&home.window, Key::UpArrow);
    assert_eq!(current(&home.window).as_deref(), Some("Title 4"));
    press(&home.window, Key::DownArrow);
    press(&home.window, Key::DownArrow);
    assert_eq!(current(&home.window).as_deref(), Some("Movie 2"));
    press(&home.window, Key::Return);
    assert_eq!(home.activated.taken(), [(2, 1)]);
}

#[test]
fn the_focused_row_stays_at_one_height_at_both_screen_sizes() {
    for (width, height) in SCREENS {
        let home = home_at(width, height);
        let anchor = current_card(&home.window)
            .expect("a current card")
            .absolute_position()
            .y;
        for (n, title) in ROWS.iter().enumerate() {
            let card = current_card(&home.window).expect("a current card");
            let y = card.absolute_position().y;
            assert!(
                (y - anchor).abs() < 0.5,
                "{width}x{height}: {title:?} is at {y}, not the anchor {anchor}"
            );
            assert!(
                fully_on_screen(&home.window, &card),
                "{width}x{height}: {title:?} is cut off"
            );
            if n < ROWS.len() - 1 {
                press(&home.window, Key::DownArrow);
            }
        }
        // And back up, to a row scrolled out of sight and in again.
        presses(&home.window, Key::UpArrow, ROWS.len() - 1);
        let y = current_card(&home.window)
            .expect("a current card")
            .absolute_position()
            .y;
        assert!((y - anchor).abs() < 0.5, "{width}x{height}: back at {y}");
    }
}

#[test]
fn the_next_row_peeks_in_below_the_focused_one_on_a_tv() {
    let home = home();
    assert!(
        !row_cards(&home.window, ROWS[1]).is_empty(),
        "Next Up shows under Continue Watching at 1080p"
    );
}

#[test]
fn thumbnails_are_16_by_9_with_progress_and_a_subtitle_and_posters_are_2_by_3() {
    let home = home();
    let thumb = &row_cards(&home.window, ROWS[0])[0];
    let size = part(thumb, "poster").size();
    assert!((size.width / size.height - 16.0 / 9.0).abs() < 0.1);
    assert!(
        thumb
            .query_descendants()
            .match_id("Card::progress-bar")
            .find_first()
            .is_some()
    );
    assert_eq!(thumb.accessible_description().as_deref(), Some("S2 · E4"));

    presses(&home.window, Key::DownArrow, 2);
    settle();
    let poster = &row_cards(&home.window, ROWS[2])[0];
    let size = part(poster, "poster").size();
    assert!((size.width / size.height - 2.0 / 3.0).abs() < 0.1);
    assert!(
        poster
            .query_descendants()
            .match_id("Card::progress-bar")
            .find_first()
            .is_none()
    );
}

#[test]
fn a_cards_progress_bar_is_as_long_as_its_progress() {
    let home = home();
    let card = &row_cards(&home.window, ROWS[0])[0];
    let bar = card
        .query_descendants()
        .match_id("Card::progress-fill")
        .find_first()
        .expect("a progress fill");
    let poster = part(card, "poster").size().width;
    assert!(
        (bar.size().width / poster - 0.2).abs() < 0.02,
        "Title 1 is 20% watched"
    );
}

#[test]
fn cards_without_pictures_show_the_placeholder() {
    let home = home();
    assert!(!all_cards(&home.window).is_empty());
    for card in all_cards(&home.window) {
        assert!(part(&card, "poster").size().width > 0.0);
    }
    assert!(the(&home.window, "Title 1").accessible_label().is_some());
}
