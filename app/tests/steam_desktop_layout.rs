//! With Steam running (and Couchfin not launched from it), Steam Input's
//! Desktop Layout types a key for every controller press as well. Each
//! scenario here scripts that through `common::steam`, against the real
//! wiring (`app::connect_gamepads_with`, fake pads, real `navigate`, keys
//! dispatched to the window) and checks how far the Home carousel moved.
//! One test, since Slint's testing event loop starts once per process.
//! Decision 013.

#[allow(dead_code)] // shared helpers; this binary uses only some
mod common;

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use common::steam::{Script, Steam};
use common::{WINDOW, close_window, press, selected, ui};
use couchfin_core::{Home, LoginScreen, PublicSystemInfo, User};
use couchfin_gamepad::fake::FakePads;
use couchfin_gamepad::{Button, Gamepads, MapperConfig};
use couchfin_ui::{GuardConfig, MainWindow, render};
use slint::ComponentHandle;
use slint::platform::Key;

fn home() -> LoginScreen {
    LoginScreen::Home(Home {
        server: "https://jellyfin.example/".parse().expect("valid URL"),
        info: PublicSystemInfo {
            server_name: "Living Room".into(),
            version: "10.11.2".into(),
            id: "f9c1b4e2a7d34c0b8e6f1a2b3c4d5e6f"
                .parse()
                .expect("valid ID"),
        },
        user: User {
            id: "4be3b9b3e1a54d7c9b6f2a1c0d9e8f7a"
                .parse()
                .expect("valid ID"),
            name: "alice".into(),
        },
        token_not_saved: None,
    })
}

/// Which card Enter selects now, by its number in the row (1 is the first).
/// Enter is a real key press well away from anything else.
fn card_now() -> u32 {
    press(Key::Return);
    let label = selected().expect("Enter selected a card");
    label
        .strip_prefix("Selected: Title ")
        .and_then(|number| number.parse().ok())
        .unwrap_or_else(|| panic!("unexpected selection {label:?}"))
}

/// Back to the first card, using the keyboard alone (which stops at the
/// ends of the row).
fn rewind() {
    for _ in 0..10 {
        press(Key::LeftArrow);
    }
    assert_eq!(
        card_now(),
        1,
        "the keyboard alone rewinds to the first card"
    );
}

fn checks() {
    let pads = FakePads::new();
    let source = pads.source();
    let mapper_clock = pads.clock();
    let mut steam = Steam::new(&pads);
    let guard_clock = steam.clock();
    let gamepads: Gamepads = ui(move || {
        let window = MainWindow::new().expect("window is created");
        render(&window, &home());
        window.show().expect("window is shown");
        let started = app::connect_gamepads_with(
            &window,
            |deliver| {
                Gamepads::start(
                    MapperConfig::default(),
                    mapper_clock,
                    move || Ok(source),
                    deliver,
                )
            },
            GuardConfig::default(),
            guard_clock,
        )
        .expect("fake pads start");
        WINDOW.with(|slot| *slot.borrow_mut() = Some(window));
        started
    });
    common::wait_for("Living Room");

    let right = Button::DPadRight;
    let right_key = Key::RightArrow;

    // Single presses move exactly once, whether Steam's key comes first or
    // after the pad event.
    for offset in [-5, 2, 15] {
        rewind();
        steam.play(&Script::default().steam_press(0, right, right_key, offset));
        assert_eq!(card_now(), 2, "key {offset} ms from the pad press");
    }

    // The same, one press after another in a row.
    rewind();
    let mut script = Script::default();
    for (index, offset) in [-5, 2, 15].into_iter().enumerate() {
        let at = i64::try_from(index).expect("small") * 300;
        script = script.steam_press(at, right, right_key, offset);
    }
    steam.play(&script);
    assert_eq!(card_now(), 4, "three presses, three moves");

    // A hold gives exactly the gamepad's repeat: the press, then repeats at
    // 400, 460, 520, 580 and 640 ms for a release at 650 ms. Steam's key-down and the
    // OS repeat (500 ms, then every 33 ms) add nothing.
    for offset in [-5, 3] {
        rewind();
        steam.play(&Script::default().steam_hold(0, right, right_key, offset, 650));
        assert_eq!(
            card_now(),
            7,
            "hold with the key {offset} ms from the press"
        );
    }

    // The controller alone is unchanged: presses and its own repeat.
    rewind();
    steam.play(
        &Script::default()
            .pad_press(0, right)
            .pad_hold(300, right, 530),
    );
    assert_eq!(card_now(), 6, "a press, then a hold of four more moves");

    // The keyboard alone is unchanged, including the OS's own repeat: the
    // press, then repeats at 500, 533, 566 and 599 ms for a release at 600 ms.
    rewind();
    steam.play(&Script::default().key_hold(0, right_key, 600));
    assert_eq!(card_now(), 6, "a held key moves on its own repeat");
    rewind();
    steam.play(&Script::default().key_press(0, right_key));
    assert_eq!(card_now(), 2, "a single key press moves once");

    // A real keyboard press of another intent at the same moment still
    // counts: Right from Steam and the pad is one move, the keyboard's Left
    // is another, so the net is no move.
    rewind();
    steam.play(&Script::default().steam_press(0, right, right_key, 2));
    steam.play(&Script::default().steam_press(0, right, right_key, 2));
    assert_eq!(card_now(), 3, "two ordinary presses");
    steam.play(
        &Script::default()
            .steam_press(0, right, right_key, 2)
            .key_press(1, Key::LeftArrow),
    );
    assert_eq!(card_now(), 3, "Right once and Left once");

    gamepads.stop();
    close_window();
}

#[test]
fn steam_desktop_layout_moves_the_carousel_once() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let driver = std::thread::spawn(|| {
        let outcome = catch_unwind(AssertUnwindSafe(checks));
        let _ = slint::quit_event_loop();
        outcome
    });
    slint::run_event_loop_until_quit().expect("the event loop runs");
    if let Err(panic) = driver.join().expect("the driver thread finishes") {
        resume_unwind(panic);
    }
}
