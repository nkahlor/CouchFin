//! Home's vertical layout on Slint's testing backend (decision 015): the
//! details strip names the focused card, cards carry no title of their own
//! except when they have no picture, row titles sit above their cards, and
//! Sign out and the notices never lie over the rows.

#[path = "../examples/screenshots/art.rs"]
#[allow(dead_code)]
mod art;
mod common;
#[path = "../examples/screenshots/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use couchfin_ui::{CardData, CardShape, HomeView, ImageState, MainWindow, RowData};
use fixtures::Pictures;
use i_slint_backend_testing::{ElementHandle, ElementQuery};
use slint::platform::Key;
use slint::{ComponentHandle, ModelRc, VecModel};

use common::home::{SCREENS, all_cards, home_at, presses, settle};
use common::press;

/// A rectangle in window coordinates: left, top, right, bottom.
#[derive(Clone, Copy, Debug)]
struct Rect(f32, f32, f32, f32);

fn rect(element: &ElementHandle) -> Rect {
    let (at, size) = (element.absolute_position(), element.size());
    Rect(at.x, at.y, at.x + size.width, at.y + size.height)
}

impl Rect {
    fn overlap(self, other: Rect) -> f32 {
        let width = (self.2.min(other.2) - self.0.max(other.0)).max(0.0);
        let height = (self.3.min(other.3) - self.1.max(other.1)).max(0.0);
        width * height
    }

    fn height(self) -> f32 {
        self.3 - self.1
    }
}

fn find(window: &MainWindow, id: &str) -> Vec<ElementHandle> {
    ElementQuery::from_root(window).match_id(id).find_all()
}

fn one(window: &MainWindow, id: &str) -> ElementHandle {
    let mut found = find(window, id);
    assert_eq!(found.len(), 1, "expected one {id}");
    found.remove(0)
}

fn said(window: &MainWindow, id: &str) -> String {
    one(window, id)
        .accessible_label()
        .map(|label| label.to_string())
        .unwrap_or_default()
}

fn card(title: &str, meta: &str) -> CardData {
    CardData {
        title: title.into(),
        meta: meta.into(),
        ..CardData::default()
    }
}

fn rows(rows: Vec<RowData>) -> ModelRc<RowData> {
    ModelRc::new(VecModel::from(rows))
}

fn row(title: &str, shape: CardShape, cards: Vec<CardData>) -> RowData {
    RowData {
        title: title.into(),
        shape,
        cards: ModelRc::new(VecModel::from(cards)),
        current: 0,
    }
}

fn two_rows(window: &MainWindow) {
    window.global::<HomeView>().set_rows(rows(vec![
        row(
            "Movies",
            CardShape::Poster,
            vec![
                card("Arrival", "2016 · 1 h 56 min"),
                card("Dune", "2021 · 2 h 35 min"),
            ],
        ),
        row(
            "Shows",
            CardShape::Thumb,
            vec![card("Severance", "S2 · E4")],
        ),
    ]));
    settle();
}

#[test]
fn the_strip_shows_the_focused_title_and_metadata_and_follows_focus() {
    let home = home_at(1920.0, 1080.0);
    two_rows(&home.window);
    assert_eq!(said(&home.window, "DetailsStrip::heading"), "Arrival");
    assert_eq!(
        said(&home.window, "DetailsStrip::line"),
        "2016 · 1 h 56 min"
    );
    press(&home.window, Key::RightArrow);
    assert_eq!(said(&home.window, "DetailsStrip::heading"), "Dune");
    assert_eq!(
        said(&home.window, "DetailsStrip::line"),
        "2021 · 2 h 35 min"
    );
    press(&home.window, Key::DownArrow);
    assert_eq!(said(&home.window, "DetailsStrip::heading"), "Severance");
    assert_eq!(said(&home.window, "DetailsStrip::line"), "S2 · E4");
    // Up brings the row's remembered card back.
    press(&home.window, Key::UpArrow);
    assert_eq!(said(&home.window, "DetailsStrip::heading"), "Dune");
}

#[test]
fn the_strip_is_empty_on_sign_out_and_never_over_the_rows() {
    let home = home_at(1920.0, 1080.0);
    two_rows(&home.window);
    presses(&home.window, Key::DownArrow, 2);
    assert_eq!(said(&home.window, "DetailsStrip::heading"), "");
    let viewport = rect(&one(&home.window, "HomePage::viewport"));
    let strip = rect(&one(&home.window, "DetailsStrip::heading"));
    assert!(strip.3 <= viewport.1 + 1.0, "{strip:?} above {viewport:?}");
}

#[test]
fn unfocused_cards_with_pictures_carry_no_text() {
    let home = home_at(1920.0, 1080.0);
    two_rows(&home.window);
    for card in all_cards(&home.window) {
        let texts = card.query_descendants().match_type_name("Text").find_all();
        assert!(
            texts.is_empty(),
            "{:?} draws a text of its own",
            card.accessible_label()
        );
    }
}

#[test]
fn a_card_without_a_picture_shows_its_title_inside_at_most_three_lines() {
    let home = home_at(1920.0, 1080.0);
    let long = "Dr. Strangelove or: How I Learned to Stop Worrying and Love the Bomb";
    let mut missing = card(long, "1964");
    missing.image_state = ImageState::Missing;
    let mut loading = card("Loading", "");
    loading.image_state = ImageState::Loading;
    let mut ready = card("Ready", "");
    ready.image_state = ImageState::Ready;
    home.window.global::<HomeView>().set_rows(rows(vec![row(
        "Movies",
        CardShape::Poster,
        vec![missing, loading, ready],
    )]));
    settle();
    let cards = all_cards(&home.window);
    let missing_card = cards
        .iter()
        .find(|c| c.accessible_label().as_deref() == Some(long))
        .expect("the card");
    let inside = missing_card
        .query_descendants()
        .match_type_name("Text")
        .find_all();
    assert_eq!(inside.len(), 1, "its title is drawn inside");
    let (own, text) = (rect(missing_card), rect(&inside[0]));
    assert!(
        text.0 >= own.0 - 1.0 && text.2 <= own.2 + 1.0 && text.1 >= own.1 && text.3 <= own.3,
        "{text:?} inside {own:?}"
    );
    // The title is read on the plain card, not over a blurred pattern.
    assert!(
        missing_card
            .query_descendants()
            .match_type_name("Image")
            .find_all()
            .is_empty(),
        "a card with no picture draws no placeholder or image"
    );
    // Loading shows the placeholder and ready the picture, neither a text.
    for name in ["Loading", "Ready"] {
        let other = cards
            .iter()
            .find(|c| c.accessible_label().as_deref() == Some(name))
            .expect("the card");
        assert!(
            other
                .query_descendants()
                .match_type_name("Text")
                .find_all()
                .is_empty(),
            "{name} draws no title inside"
        );
    }
}

#[test]
fn row_titles_sit_above_their_cards_at_the_anchor() {
    for (width, height) in SCREENS {
        let home = home_at(width, height);
        home.window
            .global::<HomeView>()
            .set_rows(rows(fixtures::home_rows(Pictures::Varied)));
        settle();
        for steps in [0, 1, 1, 1, 1] {
            presses(&home.window, Key::DownArrow, steps);
            presses(&home.window, Key::RightArrow, 2);
            settle();
            let posters = find(&home.window, "Card::poster");

            for heading in find(&home.window, "CarouselRow::heading") {
                let title = rect(&heading);
                assert!(
                    title.0 >= 159.0 && title.0 <= 161.0,
                    "aligned with the anchor slot: {title:?}"
                );
                for poster in &posters {
                    assert!(
                        title.overlap(rect(poster)) < 1.0,
                        "a row title is covered by a card at {width}x{height}: {title:?} {:?}",
                        rect(poster)
                    );
                }
            }
        }
    }
}

/// The part of each row that lies in the rows' window, in rows.
fn rows_visible(window: &MainWindow) -> f32 {
    let viewport = rect(&one(window, "HomePage::viewport"));
    ElementQuery::from_root(window)
        .match_type_name("CarouselRow")
        .find_all()
        .iter()
        .map(rect)
        .map(|row| row.overlap(viewport) / ((row.2 - row.0) * row.height()))
        .sum()
}

#[test]
fn about_two_rows_show_with_a_peek_of_the_next_at_both_sizes() {
    for (width, height) in SCREENS {
        let home = home_at(width, height);
        home.window
            .global::<HomeView>()
            .set_rows(rows(fixtures::home_rows(Pictures::Varied)));
        settle();
        // Posters are the tallest rows: look at the first poster row.
        presses(&home.window, Key::DownArrow, 2);
        settle();
        let shown = rows_visible(&home.window);
        assert!(
            (1.8..=2.4).contains(&shown),
            "{shown} rows show at {width}x{height}, wanted about two and a peek"
        );
    }
}

#[test]
fn posters_are_big_at_1080p_and_the_focused_one_fits_its_picture_request() {
    let home = home_at(1920.0, 1080.0);
    two_rows(&home.window);
    let widths: Vec<f32> = find(&home.window, "Card::poster")
        .iter()
        .map(rect)
        .filter(|r| ((r.2 - r.0) / r.height() - 2.0 / 3.0).abs() < 0.01)
        .map(|r| r.2 - r.0)
        .collect();
    let rest = widths.iter().copied().fold(f32::MAX, f32::min);
    assert!(
        (230.0..=260.0).contains(&rest),
        "a poster is {rest}px wide, wanted 230-260: {widths:?}"
    );
    // The requested box is fixed per shape; the focused card is the largest
    // thing it has to cover, so nothing is ever upscaled.
    let focused = widths.iter().copied().fold(0.0, f32::max);
    let asked = couchfin_core::ImageSize::of(couchfin_core::ImageShape::Poster, 1.0);
    assert!(
        focused <= f32::from(u16::try_from(asked.width).unwrap_or(u16::MAX)),
        "focused poster {focused}px wide, picture {}px",
        asked.width
    );
}

#[test]
fn sign_out_is_after_the_last_row_and_covers_nothing() {
    for (width, height) in SCREENS {
        let home = home_at(width, height);
        home.window
            .global::<HomeView>()
            .set_rows(rows(fixtures::home_rows(Pictures::Varied)));
        settle();
        let rows_in_page = fixtures::home_rows(Pictures::Varied).len();
        presses(&home.window, Key::DownArrow, rows_in_page);
        settle();
        let button = ElementQuery::from_root(&home.window)
            .match_accessible_role(i_slint_backend_testing::AccessibleRole::Button)
            .find_all();
        assert_eq!(button.len(), 1);
        let button = rect(&button[0]);
        for row in ElementQuery::from_root(&home.window)
            .match_type_name("CarouselRow")
            .find_all()
        {
            assert!(
                button.overlap(rect(&row)) < 1.0,
                "Sign out lies over a row at {width}x{height}"
            );
        }
        assert!(button.3 <= height + 1.0, "on screen: {button:?}");
    }
}

#[test]
fn notices_take_the_full_width_on_one_line_below_the_rows() {
    let home = home_at(1280.0, 720.0);
    let view = home.window.global::<HomeView>();
    view.set_notice("Couchfin couldn't save your sign-in, so you'll need to sign in again.".into());
    view.set_error("Couldn't load Recently Added in Anime. The server had a problem.".into());
    settle();
    let viewport = rect(&one(&home.window, "HomePage::viewport"));
    for label in [
        "Couchfin couldn't save your sign-in, so you'll need to sign in again.",
        "Couldn't load Recently Added in Anime. The server had a problem.",
    ] {
        let notice = ElementHandle::find_by_accessible_label(&home.window, label)
            .next()
            .expect("the notice");
        let at = rect(&notice);
        assert!(at.1 >= viewport.3 - 1.0, "{label:?} below the rows");
        assert!(at.height() < 60.0, "{label:?} is one line");
        assert!(at.0 < 60.0 && at.2 > 1200.0, "{label:?} spans the width");
    }
}
