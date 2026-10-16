//! A controller drives the real window: scripted pads press Right then
//! Select on Home, `app::connect_gamepads` turns them into `Intent`s on the
//! event-loop thread, and the real `navigate` moves the carousel and
//! activates the card. One test, since Slint's testing event loop starts
//! once per process.

#[allow(dead_code)] // shared helpers; this binary uses only some
mod common;

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use common::{WINDOW, close_window, ui, wait_for};
use couchfin_core::{Home, LoginScreen, PublicSystemInfo, User};
use couchfin_gamepad::fake::FakePads;
use couchfin_gamepad::{Button, Gamepads, MapperConfig, PadId};
use couchfin_ui::{MainWindow, render};
use slint::ComponentHandle;

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

fn checks() {
    let pads = FakePads::new();
    let source = pads.source();
    let clock = pads.clock();
    let gamepads: Gamepads = ui(move || {
        let window = MainWindow::new().expect("window is created");
        render(&window, &home());
        window.show().expect("window is shown");
        let started = app::connect_gamepads(&window, |deliver| {
            Gamepads::start(MapperConfig::default(), clock, move || Ok(source), deliver)
        })
        .expect("fake pads start");
        WINDOW.with(|slot| *slot.borrow_mut() = Some(window));
        started
    });
    wait_for("Living Room");

    // Right slides to the second card; A (South) selects it.
    pads.press(PadId(0), Button::DPadRight);
    pads.release(PadId(0), Button::DPadRight);
    pads.press(PadId(0), Button::South);
    pads.release(PadId(0), Button::South);
    pads.wait_until_idle();
    wait_for("Selected: Title 2");

    gamepads.stop();
    close_window();
}

#[test]
fn a_controller_moves_focus_and_selects_on_home() {
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
