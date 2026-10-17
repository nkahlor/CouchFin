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
    for card in &cards {
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

    // Cards peek in from the edges on purpose, so their texts may cross the
    // window's edge (a long title is caught by the spill check above).
    for (name, r) in &texts {
        if !r.inside(screen) && !name.starts_with("Card::") {
            found.push(format!("text outside the window: {name} {}", r.describe()));
        }
    }
    for button in ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::Button)
        .find_all()
    {
        if !rect(&button).inside(screen) {
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
#[ignore = "fixed by #51: long titles are not elided and overlap their neighbours"]
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
