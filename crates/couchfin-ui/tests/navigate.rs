//! `navigate` gives a controller the keyboard's path through every screen:
//! each `Intent` does exactly what its key does, so focus logic exists once
//! (decision 010). A new screen joins the parity table below, and any
//! intent that behaves differently from its key there fails.

mod common;

use std::time::Duration;

use common::browse::{library_calls, movie, series};
use common::flow::{bound, enter_address, failed, home, log_in, restoring};
use common::{Calls, all_elements, press, type_text};
use couchfin_core::{ApiError, LoginScreen, Request, RestoreStatus};
use couchfin_ui::{Intent, MainWindow, navigate};
use i_slint_backend_testing::mock_elapsed_time;
use slint::platform::Key;

const INTENTS: [Intent; 6] = [
    Intent::Up,
    Intent::Down,
    Intent::Left,
    Intent::Right,
    Intent::Activate,
    Intent::Back,
];

/// The key each intent stands for: the contract `navigate` keeps.
fn key(intent: Intent) -> Key {
    match intent {
        Intent::Up => Key::UpArrow,
        Intent::Down => Key::DownArrow,
        Intent::Left => Key::LeftArrow,
        Intent::Right => Key::RightArrow,
        Intent::Activate => Key::Return,
        Intent::Back => Key::Escape,
    }
}

/// Every screen the window can show, in each state that changes what the
/// keys do.
fn screens() -> Vec<(&'static str, LoginScreen)> {
    vec![
        ("starting", LoginScreen::Starting),
        ("restoring", restoring(RestoreStatus::Checking)),
        (
            "restore failed",
            restoring(RestoreStatus::Failed(ApiError::Timeout)),
        ),
        ("address", enter_address("jellyfin.lan", Request::Idle)),
        (
            "address checking",
            enter_address("jellyfin.lan", Request::InFlight),
        ),
        (
            "address failed",
            enter_address("x", failed(ApiError::Timeout)),
        ),
        ("login", log_in(Request::Idle)),
        ("login busy", log_in(Request::InFlight)),
        ("home", home(None)),
    ]
}

/// What a person (or a screen reader) can tell about the window: every
/// on-screen element's label, value and selection, and where it is, how big
/// and how opaque, in tree order. Geometry and opacity catch a carousel that
/// slides, scales or dims differently.
fn looks(window: &MainWindow) -> Vec<String> {
    all_elements(window)
        .iter()
        .map(|element| {
            let at = element.absolute_position();
            let size = element.size();
            format!(
                "{:?} {:?} {:?} {:.1},{:.1} {:.1}x{:.1} {:.3}",
                element.accessible_label(),
                element.accessible_value(),
                element.accessible_item_selected(),
                at.x,
                at.y,
                size.width,
                size.height,
                element.computed_opacity(),
            )
        })
        .collect()
}

/// Runs `steps` on `screen` in two windows side by side, one through
/// `navigate` and one through the keys, and checks that after each step both
/// sent the same commands and look the same. Both are checked at the same
/// moment, part-way through any animation, as a held D-pad sees them.
/// Typing "x" at the end shows where focus and the text cursor ended up; a
/// final Activate shows what is focused.
fn assert_same(name: &str, screen: &LoginScreen, steps: &[Intent]) {
    assert_same_by(name, &|| bound(screen), steps);
}

/// [`assert_same`] for any screen, made by `make` with its calls recorded as
/// text.
fn assert_same_by(name: &str, make: &dyn Fn() -> (MainWindow, Calls<String>), steps: &[Intent]) {
    let (by_intent, intent_commands) = make();
    let (by_key, key_commands) = make();
    let seen = |window: &MainWindow, commands: &common::Calls<String>, label: &str| {
        let mut seen = vec![format!("{label}: {:?}", commands.taken())];
        seen.extend(looks(window));
        seen
    };
    for (n, &intent) in steps.iter().chain(&[Intent::Activate]).enumerate() {
        navigate(&by_intent, intent);
        press(&by_key, key(intent));
        mock_elapsed_time(Duration::from_millis(60));
        let label = format!("step {n}, {intent:?}");
        assert_eq!(
            seen(&by_intent, &intent_commands, &label),
            seen(&by_key, &key_commands, &label),
            "{name}: {steps:?} by navigate differs from the keys at {label}"
        );
    }
    type_text(&by_intent, "x");
    type_text(&by_key, "x");
    assert_eq!(
        seen(&by_intent, &intent_commands, "typed"),
        seen(&by_key, &key_commands, "typed"),
        "{name}: {steps:?} then typing differs"
    );
}

#[test]
fn every_intent_does_what_its_key_does_on_every_screen() {
    for (name, screen) in screens() {
        for first in INTENTS {
            for second in INTENTS {
                assert_same(name, &screen, &[first, second]);
            }
        }
    }
}

/// The browsing screens that aren't a `LoginScreen`: the library grid and
/// the detail pages, each at the start and part-way along.
type Make = Box<dyn Fn() -> (MainWindow, Calls<String>)>;

fn browse_screens() -> Vec<(&'static str, Make)> {
    vec![
        ("library grid", Box::new(|| library_calls(2000))),
        ("library grid, empty", Box::new(|| library_calls(0))),
        ("movie", Box::new(movie)),
        ("series", Box::new(|| series(12))),
        ("series without episodes", Box::new(|| series(0))),
    ]
}

#[test]
fn every_intent_does_what_its_key_does_on_the_browsing_screens() {
    for (name, make) in browse_screens() {
        for first in INTENTS {
            for second in INTENTS {
                assert_same_by(name, &*make, &[first, second]);
            }
        }
    }
}

#[test]
fn every_intent_does_what_its_key_does_deep_in_the_browsing_screens() {
    use Intent::{Down, Right};
    for (name, make) in browse_screens() {
        for moved in [vec![Down, Right, Down], vec![Down; 11]] {
            for first in INTENTS {
                for second in INTENTS {
                    let mut steps = moved.clone();
                    steps.extend([first, second]);
                    assert_same_by(name, &*make, &steps);
                }
            }
        }
    }
}

#[test]
fn every_intent_does_what_its_key_along_home_down_its_rows() {
    // Each row in turn, then Sign out and past it, with a move along each.
    for downs in [1, 4, 5] {
        for first in INTENTS {
            for second in INTENTS {
                let mut steps = vec![Intent::Right, Intent::Right];
                steps.extend(vec![Intent::Down; downs]);
                steps.extend([first, second]);
                assert_same(&format!("home after {downs} downs"), &home(None), &steps);
            }
        }
    }
}

#[test]
fn every_intent_does_what_its_key_does_along_the_home_carousel() {
    // From the middle and the end of the row, every pair of intents.
    for moved in [4, 9] {
        for first in INTENTS {
            for second in INTENTS {
                let mut steps = vec![Intent::Right; moved];
                steps.extend([first, second]);
                assert_same(&format!("home after {moved} cards"), &home(None), &steps);
            }
        }
    }
}

fn commands_after(screen: &LoginScreen, steps: &[Intent]) -> Vec<String> {
    let (window, commands) = bound(screen);
    for &intent in steps {
        navigate(&window, intent);
    }
    commands.taken()
}

#[test]
fn back_quits_or_changes_server_as_escape_does() {
    use Intent::Back;
    assert_eq!(commands_after(&LoginScreen::Starting, &[Back]), ["Quit"]);
    assert_eq!(
        commands_after(&restoring(RestoreStatus::Checking), &[Back]),
        ["ChangeServer"]
    );
    assert_eq!(
        commands_after(&log_in(Request::Idle), &[Back]),
        ["ChangeServer"]
    );
    assert_eq!(commands_after(&home(None), &[Back]), ["Quit"]);
}

#[test]
fn activate_presses_the_focused_control() {
    use Intent::{Activate, Down};
    let failed_restore = restoring(RestoreStatus::Failed(ApiError::Timeout));
    assert_eq!(commands_after(&failed_restore, &[Activate]), ["Retry"]);
    assert_eq!(
        commands_after(&failed_restore, &[Down, Activate]),
        ["ChangeServer"]
    );
    assert_eq!(
        commands_after(&enter_address("jellyfin.lan", Request::Idle), &[Activate]),
        ["SubmitAddress(jellyfin.lan)"]
    );
}

#[test]
fn up_and_down_move_between_controls() {
    use Intent::{Activate, Down, Up};
    assert_eq!(
        commands_after(&log_in(Request::Idle), &[Down, Down, Down, Up, Activate]),
        ["SubmitLogin(alice, )"]
    );
    assert_eq!(
        commands_after(&home(None), &[Down, Down, Down, Down, Down, Activate]),
        ["LogOut"]
    );
}

#[test]
fn left_and_right_move_the_cursor_in_a_text_field() {
    let (window, _) = bound(&enter_address("jellyfin.lan", Request::Idle));
    navigate(&window, Intent::Left);
    navigate(&window, Intent::Left);
    navigate(&window, Intent::Left);
    type_text(&window, "-");
    navigate(&window, Intent::Right);
    type_text(&window, "+");
    let field = common::the(&window, "Server address");
    assert_eq!(field.accessible_value().as_deref(), Some("jellyfin.-l+an"));
}
