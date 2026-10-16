//! Home on the testing backend, sized like a TV, with its callbacks recorded,
//! and ways to find the carousel's cards and their parts.

use std::time::Duration;

use couchfin_ui::{HomeView, MainWindow, Screen};
use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementQuery, mock_elapsed_time};
use slint::platform::Key;
use slint::{ComponentHandle, LogicalSize};

use super::{Calls, press, shown};

/// The longest any carousel animation (slide, scale, dim) may take.
pub const SETTLE: Duration = Duration::from_millis(150);

/// The two screens Couchfin is checked on.
pub const SCREENS: [(f32, f32); 2] = [(1920.0, 1080.0), (1280.0, 720.0)];

pub struct Home {
    pub window: MainWindow,
    pub activated: Calls<i32>,
    pub sign_outs: Calls<()>,
}

/// Home in a window of the given size, with its callbacks recorded.
pub fn home_at(width: f32, height: f32) -> Home {
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

pub fn home() -> Home {
    home_at(1920.0, 1080.0)
}

/// Lets every carousel animation finish.
pub fn settle() {
    mock_elapsed_time(SETTLE);
}

/// The cards at least partly on screen, left to right. The testing backend
/// only finds elements on screen.
pub fn cards(window: &MainWindow) -> Vec<ElementHandle> {
    let mut cards = ElementQuery::from_root(window)
        .match_accessible_role(AccessibleRole::ListItem)
        .find_all();
    cards.sort_by(|a, b| a.absolute_position().x.total_cmp(&b.absolute_position().x));
    cards
}

/// The row's current card, once the row has settled.
pub fn current_card(window: &MainWindow) -> Option<ElementHandle> {
    settle();
    let mut on: Vec<_> = cards(window)
        .into_iter()
        .filter(|card| card.accessible_item_selected() == Some(true))
        .collect();
    (on.len() == 1).then(|| on.remove(0))
}

/// The title of the row's current card, if exactly one.
pub fn current(window: &MainWindow) -> Option<String> {
    current_card(window)
        .and_then(|card| card.accessible_label())
        .map(|label| label.to_string())
}

/// A named part of a card (`poster`, `ring`, `shade`), from `ui/widgets/card.slint`.
pub fn part(card: &ElementHandle, name: &str) -> ElementHandle {
    card.query_descendants()
        .match_id(format!("Card::{name}"))
        .find_first()
        .unwrap_or_else(|| panic!("a card has a {name}"))
}

pub fn presses(window: &MainWindow, key: Key, times: usize) {
    for _ in 0..times {
        press(window, key);
    }
}

/// Whether the element lies inside the window, edges included.
pub fn fully_on_screen(window: &MainWindow, element: &ElementHandle) -> bool {
    let size = window
        .window()
        .size()
        .to_logical(window.window().scale_factor());
    let at = element.absolute_position();
    let own = element.size();
    at.x >= 0.0 && at.y >= 0.0 && at.x + own.width <= size.width && at.y + own.height <= size.height
}
