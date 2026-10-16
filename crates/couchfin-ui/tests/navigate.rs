//! `navigate` gives a controller the keyboard's path through every screen:
//! each `Intent` does exactly what its key does, so focus logic exists once
//! (decision 010). A new screen joins the parity table below, and any
//! intent that behaves differently from its key there fails.

mod common;

use common::flow::{bound, enter_address, failed, home, log_in, restoring};
use common::{all_elements, press, type_text};
use couchfin_core::{ApiError, LoginScreen, Request, RestoreStatus};
use couchfin_ui::{Intent, MainWindow, navigate};
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
/// element's label, value and selection, in tree order.
fn looks(window: &MainWindow) -> Vec<String> {
    all_elements(window)
        .iter()
        .map(|element| {
            format!(
                "{:?} {:?} {:?}",
                element.accessible_label(),
                element.accessible_value(),
                element.accessible_item_selected(),
            )
        })
        .filter(|seen| seen != "None None None")
        .collect()
}

/// Runs `steps` on `screen` and records the commands sent and how the
/// window looks after each step. Typing "x" at the end shows where focus
/// and the text cursor ended up; a final Activate shows what is focused.
fn run(screen: &LoginScreen, steps: &[Intent], step: fn(&MainWindow, Intent)) -> Vec<String> {
    let (window, commands) = bound(screen);
    let mut seen = Vec::new();
    for &intent in steps.iter().chain(&[Intent::Activate]) {
        step(&window, intent);
        seen.push(format!("{intent:?}: {:?}", commands.taken()));
        seen.extend(looks(&window));
        if intent != Intent::Activate {
            continue;
        }
        type_text(&window, "x");
        seen.extend(looks(&window));
    }
    seen
}

fn by_key(window: &MainWindow, intent: Intent) {
    press(window, key(intent));
}

#[test]
fn every_intent_does_what_its_key_does_on_every_screen() {
    for (name, screen) in screens() {
        for first in INTENTS {
            for second in INTENTS {
                let steps = [first, second];
                assert_eq!(
                    run(&screen, &steps, navigate),
                    run(&screen, &steps, by_key),
                    "{name}: {steps:?} by navigate differs from the keys"
                );
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
    assert_eq!(commands_after(&home(None), &[Down, Activate]), ["LogOut"]);
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
