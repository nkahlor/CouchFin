//! The left stick as four directions, with a deadzone and hysteresis.
//! The default deadzone engages at 21000 and releases below 14000.
//! Direction locking and diagonals are in `mapper_stick_lock.rs`.

mod common;

use common::{PAD, Rig, axis, press};
use couchfin_gamepad::{Axis, Button, Deadzone, Input, MapperConfig, PadEvent};

#[test]
fn pushing_past_the_press_threshold_gives_the_direction() {
    for (stick_axis, value, input) in [
        (Axis::LeftX, 21_000, Input::Right),
        (Axis::LeftX, -21_000, Input::Left),
        (Axis::LeftY, -21_000, Input::Up),
        (Axis::LeftY, 21_000, Input::Down),
    ] {
        let mut rig = Rig::new();
        assert_eq!(rig.feed(0, &[axis(PAD, stick_axis, value)]), [input]);
    }
}

#[test]
fn full_travel_either_way_counts() {
    let mut rig = Rig::new();
    assert_eq!(
        rig.feed(0, &[axis(PAD, Axis::LeftX, i16::MIN)]),
        [Input::Left]
    );
    let mut rig = Rig::new();
    assert_eq!(
        rig.feed(0, &[axis(PAD, Axis::LeftY, i16::MAX)]),
        [Input::Down]
    );
}

#[test]
fn inside_the_deadzone_gives_nothing() {
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &[axis(PAD, Axis::LeftX, 20_999)]), []);
    assert_eq!(rig.feed(1, &[axis(PAD, Axis::LeftY, -20_999)]), []);
    assert_eq!(rig.deadline_ms(), None);
}

#[test]
fn a_stick_resting_near_the_threshold_does_not_flicker() {
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &[axis(PAD, Axis::LeftY, -21_100)]), [Input::Up]);
    for (ms, value) in [(10, -20_500), (20, -21_500), (30, -15_000), (40, -21_000)] {
        assert_eq!(
            rig.feed(ms, &[axis(PAD, Axis::LeftY, value)]),
            [],
            "{value}"
        );
    }
}

#[test]
fn coming_back_below_the_release_threshold_lets_go() {
    let mut rig = Rig::new();
    rig.feed(0, &[axis(PAD, Axis::LeftX, 30_000)]);
    assert_eq!(rig.feed(100, &[axis(PAD, Axis::LeftX, 13_999)]), []);
    assert_eq!(rig.deadline_ms(), None);
    assert_eq!(
        rig.feed(200, &[axis(PAD, Axis::LeftX, 30_000)]),
        [Input::Right]
    );
}

#[test]
fn between_the_thresholds_after_letting_go_stays_released() {
    let mut rig = Rig::new();
    rig.feed(0, &[axis(PAD, Axis::LeftX, 30_000)]);
    rig.feed(100, &[axis(PAD, Axis::LeftX, 0)]);
    assert_eq!(rig.feed(200, &[axis(PAD, Axis::LeftX, 17_000)]), []);
}

#[test]
fn a_held_stick_repeats_like_the_dpad() {
    let mut rig = Rig::new();
    rig.feed(0, &[axis(PAD, Axis::LeftY, 32_767)]);
    assert_eq!(rig.tick(400), [Input::Down]);
    assert_eq!(rig.tick(460), [Input::Down]);
    rig.feed(470, &[axis(PAD, Axis::LeftY, 0)]);
    assert_eq!(rig.tick(5_000), []);
}

#[test]
fn moving_within_the_held_direction_keeps_the_repeat_timing() {
    let mut rig = Rig::new();
    rig.feed(0, &[axis(PAD, Axis::LeftY, 22_000)]);
    assert_eq!(rig.feed(200, &[axis(PAD, Axis::LeftY, 32_767)]), []);
    assert_eq!(rig.deadline_ms(), Some(400));
}

#[test]
fn the_stronger_axis_wins_when_the_other_is_small() {
    let mut rig = Rig::new();
    let inputs = rig.feed(
        0,
        &[
            axis(PAD, Axis::LeftX, 5_000),
            axis(PAD, Axis::LeftY, -25_000),
        ],
    );
    assert_eq!(inputs, [Input::Up]);

    let mut rig = Rig::new();
    let inputs = rig.feed(
        0,
        &[
            axis(PAD, Axis::LeftY, 5_000),
            axis(PAD, Axis::LeftX, -25_000),
        ],
    );
    assert_eq!(inputs, [Input::Left]);
}

#[test]
fn the_right_stick_and_triggers_are_ignored() {
    let mut rig = Rig::new();
    for stick_axis in [
        Axis::RightX,
        Axis::RightY,
        Axis::LeftTrigger,
        Axis::RightTrigger,
    ] {
        assert_eq!(rig.feed(0, &[axis(PAD, stick_axis, 32_767)]), []);
    }
    assert_eq!(rig.deadline_ms(), None);
}

#[test]
fn disconnecting_with_the_stick_held_stops_the_repeat() {
    let mut rig = Rig::new();
    rig.feed(0, &[axis(PAD, Axis::LeftX, 32_767)]);
    rig.feed(100, &[PadEvent::Removed(PAD)]);
    assert_eq!(rig.deadline_ms(), None);
}

#[test]
fn the_stick_and_dpad_are_separate_sources() {
    // Letting go of the D-pad doesn't cancel the stick's direction.
    let mut rig = Rig::new();
    rig.feed(0, &[axis(PAD, Axis::LeftX, 32_767)]);
    rig.feed(10, &[press(PAD, Button::DPadUp)]);
    rig.feed(20, &[common::release(PAD, Button::DPadUp)]);
    assert_eq!(rig.deadline_ms(), Some(400));
    assert_eq!(rig.tick(400), [Input::Right]);
}

#[test]
fn the_deadzone_is_configurable() {
    let deadzone = Deadzone::new(8_000, 4_000).expect("valid deadzone");
    let mut rig = Rig::with(MapperConfig {
        deadzone,
        ..MapperConfig::default()
    });
    assert_eq!(
        rig.feed(0, &[axis(PAD, Axis::LeftX, 8_000)]),
        [Input::Right]
    );
    assert_eq!(rig.feed(1, &[axis(PAD, Axis::LeftX, 4_000)]), []);
    assert_eq!(rig.deadline_ms(), Some(400));
    rig.feed(2, &[axis(PAD, Axis::LeftX, 3_999)]);
    assert_eq!(rig.deadline_ms(), None);
}
