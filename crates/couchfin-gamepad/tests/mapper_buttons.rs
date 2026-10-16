//! Which button gives which input.

mod common;

use common::{OTHER_PAD, PAD, Rig, press, release};
use couchfin_gamepad::{Button, Input, PadEvent};

#[test]
fn each_dpad_direction_gives_its_input_at_once() {
    for (button, input) in [
        (Button::DPadUp, Input::Up),
        (Button::DPadDown, Input::Down),
        (Button::DPadLeft, Input::Left),
        (Button::DPadRight, Input::Right),
    ] {
        let mut rig = Rig::new();
        assert_eq!(rig.feed(0, &[press(PAD, button)]), [input], "{button:?}");
    }
}

#[test]
fn south_selects_east_goes_back_and_start_opens_the_menu() {
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &[press(PAD, Button::South)]), [Input::Select]);
    assert_eq!(rig.feed(10, &[press(PAD, Button::East)]), [Input::Back]);
    assert_eq!(rig.feed(20, &[press(PAD, Button::Start)]), [Input::Menu]);
}

#[test]
fn releasing_a_button_gives_nothing() {
    let mut rig = Rig::new();
    for button in [Button::South, Button::East, Button::Start, Button::DPadUp] {
        rig.feed(0, &[press(PAD, button)]);
        assert_eq!(rig.feed(1, &[release(PAD, button)]), [], "{button:?}");
    }
}

#[test]
fn unmapped_buttons_give_nothing() {
    let mut rig = Rig::new();
    for button in [
        Button::West,
        Button::North,
        Button::View,
        Button::Guide,
        Button::LeftStick,
        Button::RightStick,
        Button::LeftShoulder,
        Button::RightShoulder,
        Button::Other,
    ] {
        assert_eq!(rig.feed(0, &[press(PAD, button)]), [], "{button:?}");
        assert_eq!(rig.deadline_ms(), None, "{button:?}");
    }
}

#[test]
fn a_batch_gives_inputs_in_event_order() {
    let mut rig = Rig::new();
    let inputs = rig.feed(
        0,
        &[
            press(PAD, Button::DPadDown),
            release(PAD, Button::DPadDown),
            press(PAD, Button::South),
            press(PAD, Button::East),
        ],
    );
    assert_eq!(inputs, [Input::Down, Input::Select, Input::Back]);
}

#[test]
fn every_connected_pad_is_listened_to() {
    let mut rig = Rig::new();
    rig.feed(0, &[PadEvent::Added(PAD), PadEvent::Added(OTHER_PAD)]);
    assert_eq!(rig.feed(1, &[press(PAD, Button::South)]), [Input::Select]);
    assert_eq!(
        rig.feed(2, &[press(OTHER_PAD, Button::East)]),
        [Input::Back]
    );
}

#[test]
fn a_pad_needs_no_added_event_first() {
    // SDL may report a button before the mapper has seen the pad's arrival,
    // for example when the runner starts while a button is down.
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &[press(PAD, Button::South)]), [Input::Select]);
}

#[test]
fn connecting_and_disconnecting_give_nothing() {
    let mut rig = Rig::new();
    assert_eq!(rig.feed(0, &[PadEvent::Added(PAD)]), []);
    assert_eq!(rig.feed(1, &[PadEvent::Removed(PAD)]), []);
}
