//! Held directions repeat after a delay, at a steady rate, until released.
//! Every instant here is exact: the mapper only knows the time it is given.

mod common;

use std::time::Duration;

use common::{OTHER_PAD, PAD, Rig, press, release};
use couchfin_gamepad::{Button, Input, MapperConfig, PadEvent, Repeat};

#[test]
fn a_held_direction_repeats_after_400_ms_then_every_60_ms() {
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &[press(PAD, Button::DPadDown)]), [Input::Down]);
    assert_eq!(rig.deadline_ms(), Some(400));
    assert_eq!(rig.tick(399), []);
    assert_eq!(rig.tick(400), [Input::Down]);
    assert_eq!(rig.deadline_ms(), Some(460));
    assert_eq!(rig.tick(459), []);
    assert_eq!(rig.tick(460), [Input::Down]);
    assert_eq!(rig.tick(520), [Input::Down]);
    assert_eq!(rig.deadline_ms(), Some(580));
}

#[test]
fn nothing_is_due_while_nothing_is_held() {
    let rig = Rig::new();
    assert_eq!(rig.deadline_ms(), None);
}

#[test]
fn releasing_stops_the_repeat() {
    let mut rig = Rig::new();
    rig.feed(0, &[press(PAD, Button::DPadUp)]);
    rig.tick(400);
    assert_eq!(rig.feed(430, &[release(PAD, Button::DPadUp)]), []);
    assert_eq!(rig.deadline_ms(), None);
    assert_eq!(rig.tick(5_000), []);
}

#[test]
fn a_release_at_the_deadline_wins_over_the_repeat() {
    let mut rig = Rig::new();
    rig.feed(0, &[press(PAD, Button::DPadLeft)]);
    assert_eq!(rig.feed(400, &[release(PAD, Button::DPadLeft)]), []);
}

#[test]
fn select_back_and_menu_never_repeat() {
    let mut rig = Rig::new();
    rig.feed(
        0,
        &[
            press(PAD, Button::South),
            press(PAD, Button::East),
            press(PAD, Button::Start),
        ],
    );
    assert_eq!(rig.deadline_ms(), None);
    assert_eq!(rig.tick(5_000), []);
}

#[test]
fn a_late_wake_gives_one_repeat_not_a_burst() {
    let mut rig = Rig::new();
    rig.feed(0, &[press(PAD, Button::DPadRight)]);
    // Woken 300 ms late: five repeats were due, but only one fires.
    assert_eq!(rig.tick(700), [Input::Right]);
    assert_eq!(rig.deadline_ms(), Some(760));
}

#[test]
fn the_delay_and_rate_are_configurable() {
    let repeat =
        Repeat::new(Duration::from_millis(250), Duration::from_millis(100)).expect("valid repeat");
    let mut rig = Rig::with(MapperConfig {
        repeat,
        ..MapperConfig::default()
    });
    rig.feed(0, &[press(PAD, Button::DPadDown)]);
    assert_eq!(rig.tick(249), []);
    assert_eq!(rig.tick(250), [Input::Down]);
    assert_eq!(rig.deadline_ms(), Some(350));
}

#[test]
fn the_latest_direction_pressed_is_the_one_that_repeats() {
    let mut rig = Rig::new();
    rig.feed(0, &[press(PAD, Button::DPadUp)]);
    assert_eq!(
        rig.feed(100, &[press(PAD, Button::DPadRight)]),
        [Input::Right]
    );
    assert_eq!(rig.deadline_ms(), Some(500));
    assert_eq!(rig.tick(500), [Input::Right]);
}

#[test]
fn releasing_an_older_direction_leaves_the_newer_one_repeating() {
    let mut rig = Rig::new();
    rig.feed(0, &[press(PAD, Button::DPadUp)]);
    rig.feed(100, &[press(PAD, Button::DPadRight)]);
    assert_eq!(rig.feed(200, &[release(PAD, Button::DPadUp)]), []);
    assert_eq!(rig.tick(500), [Input::Right]);
}

#[test]
fn releasing_the_repeating_direction_does_not_resume_an_older_one() {
    // Like a keyboard: letting go of the newer key stops repeating, even
    // though the older one is still down.
    let mut rig = Rig::new();
    rig.feed(0, &[press(PAD, Button::DPadUp)]);
    rig.feed(100, &[press(PAD, Button::DPadRight)]);
    assert_eq!(rig.feed(200, &[release(PAD, Button::DPadRight)]), []);
    assert_eq!(rig.deadline_ms(), None);
}

#[test]
fn disconnecting_mid_hold_stops_that_pads_repeat() {
    let mut rig = Rig::new();
    rig.feed(0, &[press(PAD, Button::DPadDown)]);
    assert_eq!(rig.feed(200, &[PadEvent::Removed(PAD)]), []);
    assert_eq!(rig.deadline_ms(), None);
    assert_eq!(rig.tick(5_000), []);
}

#[test]
fn a_pad_that_reconnects_starts_with_nothing_held() {
    let mut rig = Rig::new();
    rig.feed(0, &[press(PAD, Button::DPadDown)]);
    rig.feed(100, &[PadEvent::Added(PAD)]);
    assert_eq!(rig.deadline_ms(), None);
    // The stale release from before the reconnect does nothing.
    assert_eq!(rig.feed(150, &[release(PAD, Button::DPadDown)]), []);
}

#[test]
fn each_pad_repeats_on_its_own() {
    let mut rig = Rig::new();
    rig.feed(0, &[press(PAD, Button::DPadDown)]);
    rig.feed(100, &[press(OTHER_PAD, Button::DPadUp)]);
    assert_eq!(rig.deadline_ms(), Some(400));
    assert_eq!(rig.tick(400), [Input::Down]);
    assert_eq!(rig.deadline_ms(), Some(460));
    assert_eq!(rig.tick(460), [Input::Down]);
    assert_eq!(rig.tick(500), [Input::Up]);
}

#[test]
fn disconnecting_one_pad_leaves_the_others_repeat_running() {
    let mut rig = Rig::new();
    rig.feed(0, &[press(PAD, Button::DPadDown)]);
    rig.feed(0, &[press(OTHER_PAD, Button::DPadUp)]);
    rig.feed(100, &[PadEvent::Removed(PAD)]);
    assert_eq!(rig.tick(400), [Input::Up]);
}
