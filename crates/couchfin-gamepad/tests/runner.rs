//! The gamepad thread, driven by scripted pads on a virtual clock.

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use couchfin_gamepad::fake::FakePads;
use couchfin_gamepad::{
    Axis, Button, ButtonState, Clock, Gamepads, Input, MapperConfig, PadEvent, PadId, StartError,
};

const PAD: PadId = PadId(7);

fn start(pads: &FakePads) -> (Gamepads, Receiver<Input>) {
    let (send, inputs) = mpsc::channel();
    let source = pads.source();
    let gamepads = Gamepads::start(
        MapperConfig::default(),
        pads.clock(),
        move || Ok(source),
        move |input| {
            let _ = send.send(input);
        },
    )
    .expect("the runner starts");
    (gamepads, inputs)
}

fn received(pads: &FakePads, inputs: &Receiver<Input>) -> Vec<Input> {
    pads.wait_until_idle();
    inputs.try_iter().collect()
}

#[test]
fn inputs_reach_the_callback_in_order() {
    let pads = FakePads::new();
    let (_gamepads, inputs) = start(&pads);
    pads.connect(PAD);
    pads.press(PAD, Button::DPadRight);
    pads.release(PAD, Button::DPadRight);
    pads.press(PAD, Button::South);
    pads.press(PAD, Button::East);
    assert_eq!(
        received(&pads, &inputs),
        [Input::Right, Input::Select, Input::Back]
    );
}

#[test]
fn a_held_direction_repeats_on_time() {
    let pads = FakePads::new();
    let start_time = pads.clock().now();
    let (_gamepads, inputs) = start(&pads);
    pads.press(PAD, Button::DPadDown);
    pads.pause(Duration::from_millis(1_000));
    pads.release(PAD, Button::DPadDown);
    // The press, then repeats at 400, 460, ..., 940 ms.
    assert_eq!(received(&pads, &inputs), [Input::Down; 11]);
    assert_eq!(
        pads.clock().now() - start_time,
        Duration::from_millis(1_000)
    );
}

#[test]
fn the_thread_sleeps_until_the_next_repeat() {
    let pads = FakePads::new();
    let (_gamepads, inputs) = start(&pads);
    pads.press(PAD, Button::DPadDown);
    pads.pause(Duration::from_millis(1_000));
    pads.release(PAD, Button::DPadDown);
    assert_eq!(received(&pads, &inputs).len(), 11);
    // One wait for the press, one per repeat, one for the release and the
    // last one now blocking: no polling in between.
    assert!(pads.waits() <= 14, "waited {} times", pads.waits());
}

#[test]
fn a_burst_of_events_is_handled_in_one_wake() {
    let pads = FakePads::new();
    let (_gamepads, inputs) = start(&pads);
    pads.wait_until_idle();
    let before = pads.waits();
    // A Steam Deck streams stick noise; the press behind it must not wait.
    let noise = (0..2_000).map(|value| PadEvent::Axis {
        pad: PAD,
        axis: Axis::RightX,
        value,
    });
    let press = PadEvent::Button {
        pad: PAD,
        button: Button::South,
        state: ButtonState::Down,
    };
    pads.send_burst(noise.chain([press]));
    assert_eq!(received(&pads, &inputs), [Input::Select]);
    assert!(
        pads.waits() - before <= 2,
        "waited {} times",
        pads.waits() - before
    );
}

#[test]
fn disconnecting_mid_hold_stops_the_repeat() {
    let pads = FakePads::new();
    let (_gamepads, inputs) = start(&pads);
    pads.connect(PAD);
    pads.press(PAD, Button::DPadUp);
    pads.pause(Duration::from_millis(100));
    pads.disconnect(PAD);
    pads.pause(Duration::from_secs(5));
    pads.connect(PAD);
    assert_eq!(received(&pads, &inputs), [Input::Up]);
}

#[test]
fn the_stick_repeats_too() {
    let pads = FakePads::new();
    let (_gamepads, inputs) = start(&pads);
    pads.tilt(PAD, Axis::LeftX, -32_000);
    // Repeats at 400 and 460 ms, just before the stick comes back.
    pads.pause(Duration::from_millis(461));
    pads.tilt(PAD, Axis::LeftX, 0);
    assert_eq!(received(&pads, &inputs), [Input::Left; 3]);
}

#[test]
fn a_source_that_fails_to_open_fails_the_start() {
    let result = Gamepads::start::<couchfin_gamepad::fake::FakeEventSource, _>(
        MapperConfig::default(),
        FakePads::new().clock(),
        || Err(StartError::Sdl("no gamepad support".into())),
        |_| {},
    );
    assert!(
        matches!(&result, Err(StartError::Sdl(message)) if message == "no gamepad support"),
        "{result:?}"
    );
}

#[test]
fn stopping_ends_the_thread_while_it_waits() {
    let pads = FakePads::new();
    let (gamepads, inputs) = start(&pads);
    pads.wait_until_idle();
    let started = Instant::now();
    gamepads.stop();
    assert!(started.elapsed() < Duration::from_secs(5));
    // The thread has gone, and its callback with it.
    assert_eq!(inputs.recv(), Err(mpsc::RecvError));
}

#[test]
fn dropping_stops_the_thread() {
    let pads = FakePads::new();
    let (gamepads, inputs) = start(&pads);
    pads.press(PAD, Button::DPadUp);
    pads.wait_until_idle();
    drop(gamepads);
    assert_eq!(inputs.try_iter().collect::<Vec<_>>(), [Input::Up]);
    assert_eq!(inputs.recv(), Err(mpsc::RecvError));
}
