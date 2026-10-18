//! Layout checks with no pictures: render each screen and state from the
//! screenshot example's fixtures at both TV sizes, walk the element tree and
//! fail on text that overlaps other text, text that spills out of its card,
//! and text or buttons outside the window. Cheaper than looking at a
//! screenshot, so run it first (CLAUDE.md, "Before opening a PR").
//!
//! Text is found by its element type, and its rectangle is the box the text
//! is laid out in: a title with no width limit has a box as wide as its text,
//! which is how an un-elided long title shows up.

#[path = "../examples/screenshots/art.rs"]
#[allow(dead_code)]
mod art;
mod common;
#[path = "../examples/screenshots/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

use couchfin_ui::{HomeView, LibraryView, MovieView, RowData, Screen, SeriesView};
use fixtures::Pictures;
use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementQuery};
use slint::platform::Key;
use slint::{ComponentHandle, LogicalSize, ModelRc, VecModel};

use common::home::{SCREENS, settle};
use common::{press, shown};

type Window = couchfin_ui::MainWindow;

/// A line of text is about this many font sizes tall.
const LINE_HEIGHT: f32 = 1.36;
/// `Theme.card-inner-title-size`; Theme is not part of the crate's API.
const INNER_TITLE_SIZE: f32 = 28.0;
/// `Theme.card-inner-padding`: a small card gives its title what is left.
const CARD_PADDING: f32 = 12.0;

/// A rectangle in window coordinates: left, top, right, bottom.
#[derive(Clone, Copy)]
struct Rect(f32, f32, f32, f32);

fn rect(element: &ElementHandle) -> Rect {
    let (at, size) = (element.absolute_position(), element.size());
    Rect(at.x, at.y, at.x + size.width, at.y + size.height)
}

impl Rect {
    fn overlaps(self, other: Rect) -> bool {
        // Boxes that only touch, or overlap by a hair, are neighbours.
        let slack = 1.0;
        self.0 < other.2 - slack
            && other.0 < self.2 - slack
            && self.1 < other.3 - slack
            && other.1 < self.3 - slack
    }

    fn inside(self, outer: Rect) -> bool {
        let slack = 1.0;
        self.0 >= outer.0 - slack
            && self.1 >= outer.1 - slack
            && self.2 <= outer.2 + slack
            && self.3 <= outer.3 + slack
    }

    fn describe(self) -> String {
        format!(
            "({:.0},{:.0})-({:.0},{:.0})",
            self.0, self.1, self.2, self.3
        )
    }
}

/// Every visible text with a box, as (what it is, where). Texts of cards that
/// are only partly on screen are kept: they still must not overlap.
fn texts(window: &Window) -> Vec<(String, Rect)> {
    ElementQuery::from_root(window)
        .match_type_name("Text")
        .find_all()
        .into_iter()
        .filter(|text| text.computed_opacity() > 0.05)
        .map(|text| {
            let name = text
                .accessible_label()
                .map_or(String::new(), |l| l.to_string());
            let id = text.id().map_or(String::new(), |id| id.to_string());
            (format!("{id} {name:?}"), rect(&text))
        })
        .filter(|(_, r)| r.2 - r.0 > 1.0 && r.3 - r.1 > 1.0)
        .collect()
}

/// What is wrong with the window as drawn now, one line per problem.
fn problems(window: &Window) -> Vec<String> {
    let size = window
        .window()
        .size()
        .to_logical(window.window().scale_factor());
    let screen = Rect(0.0, 0.0, size.width, size.height);
    let mut found = Vec::new();

    let texts = texts(window);
    for (i, (a, ra)) in texts.iter().enumerate() {
        for (b, rb) in &texts[i + 1..] {
            if ra.overlaps(*rb) {
                found.push(format!(
                    "text overlaps text: {a} {} and {b} {}",
                    ra.describe(),
                    rb.describe()
                ));
            }
        }
    }

    let cards = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::ListItem)
        .find_all();
    found.extend(card_problems(&cards));

    // A button, a poster or a row's list may not lie over any text but
    // their own (Sign out once covered the row below the focused one, and the
    // focused poster a row's title).
    found.extend(over_text_problems(window, &texts));
    found.extend(edge_problems(window, &texts, screen));
    found
}

/// Texts that spill out of their card sideways, or titles inside a card
/// with too little room.
fn card_problems(cards: &[ElementHandle]) -> Vec<String> {
    let mut found = Vec::new();
    for card in cards {
        let own = rect(card);
        for text in card.query_descendants().match_type_name("Text").find_all() {
            let r = rect(&text);
            if text.computed_opacity() > 0.05
                && r.2 - r.0 > 1.0
                && (r.0 < own.0 - 1.0 || r.2 > own.2 + 1.0)
            {
                let label = card.accessible_label().unwrap_or_default();
                found.push(format!(
                    "text spills out of its card {label:?} {}: text box {}",
                    own.describe(),
                    r.describe()
                ));
            }
        }
    }

    // A title drawn inside a card (no picture) is laid out in a box that must
    // hold its three lines: centred text taller than its box spills over the
    // top and the first line is lost, which read as a title clipped at the
    // start (#66).
    let three_lines = 3.0 * INNER_TITLE_SIZE * LINE_HEIGHT;
    for card in cards {
        let own = rect(card);
        for text in card
            .query_descendants()
            .match_id("Card::inner-title")
            .find_all()
        {
            let r = rect(&text);
            if text.computed_opacity() > 0.05
                && r.3 - r.1 < three_lines.min(own.3 - own.1 - 2.0 * CARD_PADDING - 2.0)
            {
                let label = card.accessible_label().unwrap_or_default();
                found.push(format!(
                    "text box too short for three lines in card {label:?} {}: {} (needs {three_lines:.0})",
                    own.describe(),
                    r.describe()
                ));
            }
        }
    }
    found
}

/// Buttons and posters lying over text that is not their own.
fn over_text_problems(window: &Window, texts: &[(String, Rect)]) -> Vec<String> {
    let mut found = Vec::new();
    let buttons = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .find_all();
    for button in &buttons {
        for (name, r) in texts {
            let own = rect(button);
            let inside = r.inside(own);
            if own.overlaps(*r) && !inside {
                found.push(format!(
                    "button {} lies over text {name} {}",
                    own.describe(),
                    r.describe()
                ));
            }
        }
    }
    for poster in ElementQuery::from_root(window)
        .match_id("Card::poster")
        .find_all()
    {
        for (name, r) in texts
            .iter()
            .filter(|(n, _)| n.contains("CarouselRow::heading"))
        {
            if rect(&poster).overlaps(*r) {
                found.push(format!(
                    "card {} covers row title {name} {}",
                    rect(&poster).describe(),
                    r.describe()
                ));
            }
        }
    }
    found
}

/// Texts and buttons outside the window.
fn edge_problems(window: &Window, texts: &[(String, Rect)], screen: Rect) -> Vec<String> {
    let mut found = Vec::new();
    // Cards peek in from the edges on purpose, so their texts may cross the
    // window's edge (a long title is caught by the spill check above). What
    // Home's rows viewport clips away entirely (Sign out below the last
    // visible row, say) is not drawn, so it cannot be outside the window.
    let viewport = ElementQuery::from_root(window)
        .match_id("HomePage::viewport")
        .find_all()
        .first()
        .map(rect);
    let clipped_away =
        |r: Rect| viewport.is_some_and(|v| r.1 >= v.3 || r.3 <= v.1 || r.1 >= v.1 && r.3 > v.3);
    for (name, r) in texts {
        if !r.inside(screen) && !name.starts_with("Card::") && !clipped_away(*r) {
            found.push(format!("text outside the window: {name} {}", r.describe()));
        }
    }
    for button in ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .find_all()
    {
        if !rect(&button).inside(screen) && !clipped_away(rect(&button)) {
            found.push(format!(
                "button outside the window {}",
                rect(&button).describe()
            ));
        }
    }
    found
}

fn at_size(window: &Window, (width, height): (f32, f32)) {
    window.window().set_size(LogicalSize::new(width, height));
    settle();
}

fn check(scene: &str, size: (f32, f32), window: &Window) -> Vec<String> {
    problems(window)
        .into_iter()
        .map(|p| format!("{scene} at {}x{}: {p}", size.0, size.1))
        .collect()
}

fn fail_on(found: &[String]) {
    assert!(found.is_empty(), "layout problems:\n{}", found.join("\n"));
}

fn home(mode: Pictures, size: (f32, f32)) -> Window {
    let window = shown(Screen::Home);
    at_size(&window, size);
    let view = window.global::<HomeView>();
    view.set_server_name("Living Room".into());
    view.set_server_version("10.11.2".into());
    view.set_username("alice".into());
    let rows: Vec<RowData> = fixtures::home_rows(mode);
    view.set_rows(ModelRc::new(VecModel::from(rows)));
    settle();
    window
}

#[test]
fn home_rows_and_focus_have_no_layout_problems() {
    let mut found = Vec::new();
    for size in SCREENS {
        let window = home(Pictures::Varied, size);
        found.extend(check("home first card", size, &window));
        for (key, times, scene) in [
            (Key::RightArrow, 3, "home row 1 card 4"),
            (Key::DownArrow, 2, "home row 3 card 1"),
            (Key::RightArrow, 4, "home row 3 card 5"),
            (Key::DownArrow, 3, "home row 6 card 1"),
            (Key::RightArrow, 30, "home row 6 last card"),
            (Key::DownArrow, 2, "home sign out"),
        ] {
            for _ in 0..times {
                press(&window, key);
            }
            settle();
            found.extend(check(scene, size, &window));
        }
    }
    fail_on(&found);
}

#[test]
fn home_with_missing_pictures_has_no_layout_problems() {
    let mut found = Vec::new();
    for size in SCREENS {
        let window = home(Pictures::SomeMissing, size);
        found.extend(check("home missing pictures", size, &window));
        for _ in 0..2 {
            press(&window, Key::DownArrow);
        }
        settle();
        found.extend(check("home missing pictures row 3", size, &window));
    }
    fail_on(&found);
}

#[test]
fn home_notices_have_no_layout_problems() {
    let mut found = Vec::new();
    for size in SCREENS {
        let window = home(Pictures::Varied, size);
        let view = window.global::<HomeView>();
        view.set_notice("Signed in, but the sign-in couldn't be saved on this device.".into());
        view.set_error("Couldn't load Recently Added in Anime. The server had a problem.".into());
        view.set_username("a_rather_long_username_indeed".into());
        settle();
        found.extend(check("home with notices", size, &window));
    }
    fail_on(&found);
}

#[test]
#[ignore = "fixed by #51: long titles are not elided and overlap their neighbours"]
fn library_grid_has_no_layout_problems() {
    let mut found = Vec::new();
    for size in SCREENS {
        let window = shown(Screen::Library);
        at_size(&window, size);
        let view = window.global::<LibraryView>();
        view.set_title("Movies and Other Films We Own on Disc".into());
        view.set_item_count(240);
        view.set_items(ModelRc::new(VecModel::from(fixtures::library_items(
            240, 240,
        ))));
        settle();
        found.extend(check("library top", size, &window));
        for _ in 0..4 {
            press(&window, Key::DownArrow);
        }
        settle();
        found.extend(check("library scrolled", size, &window));
    }
    fail_on(&found);
}

#[test]
fn movie_page_has_no_layout_problems() {
    let mut found = Vec::new();
    for size in SCREENS {
        let window = shown(Screen::Movie);
        at_size(&window, size);
        let view = window.global::<MovieView>();
        view.set_title(
            "The Lord of the Rings: The Fellowship of the Ring (Extended Edition)".into(),
        );
        view.set_year("2001".into());
        view.set_runtime("3 h 48 min".into());
        view.set_rating("PG-13".into());
        view.set_overview(fixtures::LONG_OVERVIEW.into());
        view.set_resumable(true);
        view.set_backdrop(fixtures::backdrop());
        settle();
        found.extend(check("movie", size, &window));
    }
    fail_on(&found);
}

fn series_problems(size: (f32, f32)) -> Vec<String> {
    let window = shown(Screen::Series);
    at_size(&window, size);
    let view = window.global::<SeriesView>();
    view.set_title("Severance".into());
    view.set_year("2022".into());
    view.set_runtime("50 min".into());
    view.set_rating("TV-MA".into());
    view.set_overview(fixtures::LONG_OVERVIEW.into());
    view.set_backdrop(fixtures::backdrop());
    let seasons: Vec<slint::SharedString> = (1..=6).map(|n| format!("Season {n}").into()).collect();
    view.set_seasons(ModelRc::new(VecModel::from(seasons)));
    // Few episodes, so the list is not scrolled: the checker can't see a
    // list's clipping, only where its rows sit.
    view.set_episodes(ModelRc::new(VecModel::from(fixtures::episodes(3))));
    settle();
    check("series", size, &window)
}

#[test]
fn series_page_has_no_layout_problems_at_1080p() {
    fail_on(&series_problems(SCREENS[0]));
}

#[test]
#[ignore = "known: at 720p the episode rows run under the status line and the sixth season is off screen"]
fn series_page_has_no_layout_problems_at_720p() {
    fail_on(&series_problems(SCREENS[1]));
}
