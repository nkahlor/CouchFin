//! Starts the real SDL backend. There is no controller in CI, so this only
//! checks that SDL's gamepad support starts on the gamepad thread, waits,
//! and stops promptly when asked.
//!
//! One test only: SDL can run once per process at a time, and tests in a
//! file run in parallel.

use std::time::{Duration, Instant};

use couchfin_gamepad::{Gamepads, MapperConfig, SdlHints};

#[test]
fn sdl_starts_on_the_gamepad_thread_and_stops_when_asked() {
    let gamepads = Gamepads::start_sdl(MapperConfig::default(), SdlHints::default(), |_| {})
        .expect("SDL's gamepad support starts");
    // Let it settle into waiting for events.
    std::thread::sleep(Duration::from_millis(100));

    let stopping = Instant::now();
    gamepads.stop();
    assert!(
        stopping.elapsed() < Duration::from_secs(2),
        "stopping took {:?}",
        stopping.elapsed()
    );

    // SDL shut down with it, so it can start again.
    Gamepads::start_sdl(MapperConfig::default(), SdlHints::none(), |_| {})
        .expect("SDL starts a second time")
        .stop();
}
