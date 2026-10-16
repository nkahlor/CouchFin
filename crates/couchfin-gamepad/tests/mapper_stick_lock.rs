//! A thumb is never exactly on an axis. Once the stick engages a direction
//! it keeps it until the stick comes back to the centre, and a diagonal
//! engages nothing (decision 012).

mod common;

use common::{PAD, Rig, axis, press, release};
use couchfin_gamepad::{Axis, Button, Deadzone, Input, MapperConfig, PadEvent};

fn stick(x: i16, y: i16) -> [PadEvent; 2] {
    [axis(PAD, Axis::LeftX, x), axis(PAD, Axis::LeftY, y)]
}

#[test]
fn right_with_a_sloppy_thumb_stays_right_for_the_whole_hold() {
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &stick(30_000, 2_000)), [Input::Right]);
    // The thumb slides down and relaxes sideways: x is low, y is high.
    for (ms, x, y) in [
        (50, 20_000, 12_000),
        (100, 10_000, 25_000),
        (150, 9_000, 30_000),
    ] {
        assert_eq!(rig.feed(ms, &stick(x, y)), [], "at {ms} ms");
    }
    // Only Right repeats while the lock holds.
    assert_eq!(rig.tick(400), [Input::Right]);
    assert_eq!(rig.tick(460), [Input::Right]);
}

#[test]
fn a_lock_survives_the_stick_passing_through_a_diagonal() {
    let mut rig = Rig::new();
    rig.feed(0, &stick(30_000, 0));
    assert_eq!(rig.feed(10, &stick(24_000, 24_000)), []);
    assert_eq!(rig.tick(400), [Input::Right]);
}

#[test]
fn returning_to_the_centre_releases_the_lock() {
    let mut rig = Rig::new();
    rig.feed(0, &stick(30_000, 0));
    assert_eq!(rig.feed(100, &stick(0, 0)), []);
    assert_eq!(rig.deadline_ms(), None);
    assert_eq!(rig.feed(200, &stick(0, 30_000)), [Input::Down]);
}

#[test]
fn a_diagonal_push_engages_nothing() {
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &stick(25_000, 25_000)), []);
    assert_eq!(rig.feed(10, &stick(25_000, -22_000)), []);
    assert_eq!(rig.feed(20, &stick(-30_000, 20_000)), []);
    assert_eq!(rig.deadline_ms(), None);
}

#[test]
fn leaving_a_diagonal_towards_an_axis_engages_it() {
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &stick(25_000, 25_000)), []);
    assert_eq!(rig.feed(10, &stick(30_000, 10_000)), [Input::Right]);
}

#[test]
fn the_default_stick_needs_about_two_thirds_of_full_travel() {
    let deadzone = Deadzone::default();
    assert!(
        (20_000..=22_000).contains(&deadzone.press()),
        "{deadzone:?}"
    );
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &[axis(PAD, Axis::LeftX, 16_384)]), []);
    assert_eq!(
        rig.feed(1, &[axis(PAD, Axis::LeftX, 22_000)]),
        [Input::Right]
    );
}

#[test]
fn the_dpad_ignores_all_of_this() {
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &[press(PAD, Button::DPadDown)]), [Input::Down]);
    assert_eq!(rig.tick(400), [Input::Down]);
    rig.feed(410, &[release(PAD, Button::DPadDown)]);
    assert_eq!(
        rig.feed(500, &[press(PAD, Button::DPadLeft)]),
        [Input::Left]
    );
}

#[test]
fn the_dpad_still_works_while_the_stick_is_locked() {
    let mut rig = Rig::new();
    rig.feed(0, &stick(30_000, 0));
    assert_eq!(rig.feed(10, &[press(PAD, Button::DPadUp)]), [Input::Up]);
}

#[test]
fn the_cone_is_configurable_and_validated() {
    let wide = Deadzone::default().with_cone(44).expect("valid cone");
    assert_eq!(wide.cone(), 44);
    let mut rig = Rig::with(MapperConfig {
        deadzone: wide,
        ..MapperConfig::default()
    });
    // About 40 degrees off the X axis engages with a wide cone only.
    assert_eq!(rig.feed(0, &stick(30_000, 25_000)), [Input::Right]);
    assert!(Deadzone::default().with_cone(0).is_err());
    assert!(Deadzone::default().with_cone(46).is_err());
    assert_eq!(Deadzone::default().cone(), 30);
}
