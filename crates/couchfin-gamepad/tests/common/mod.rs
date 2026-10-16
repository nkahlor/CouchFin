//! Shorthand for building pad events and exact instants.

#![allow(dead_code)]

use std::time::{Duration, Instant};

use couchfin_gamepad::{Axis, Button, ButtonState, Mapper, MapperConfig, PadEvent, PadId};

pub const PAD: PadId = PadId(1);
pub const OTHER_PAD: PadId = PadId(2);

/// A mapper with the default config and a fixed start time, so every
/// instant in a test is exact.
pub struct Rig {
    pub mapper: Mapper,
    pub start: Instant,
}

impl Rig {
    pub fn new() -> Self {
        Self::with(MapperConfig::default())
    }

    pub fn with(config: MapperConfig) -> Self {
        Self {
            mapper: Mapper::new(config),
            start: Instant::now(),
        }
    }

    /// `ms` milliseconds after the start.
    pub fn at(&self, ms: u64) -> Instant {
        self.start + Duration::from_millis(ms)
    }

    /// Feeds `events` at `ms`.
    pub fn feed(&mut self, ms: u64, events: &[PadEvent]) -> Vec<couchfin_gamepad::Input> {
        let now = self.at(ms);
        self.mapper.update(events, now)
    }

    /// Lets time pass to `ms` with no events.
    pub fn tick(&mut self, ms: u64) -> Vec<couchfin_gamepad::Input> {
        self.feed(ms, &[])
    }

    /// The next deadline as milliseconds after the start.
    pub fn deadline_ms(&self) -> Option<u128> {
        self.mapper
            .next_deadline()
            .map(|deadline| deadline.duration_since(self.start).as_millis())
    }
}

pub fn press(pad: PadId, button: Button) -> PadEvent {
    PadEvent::Button {
        pad,
        button,
        state: ButtonState::Down,
    }
}

pub fn release(pad: PadId, button: Button) -> PadEvent {
    PadEvent::Button {
        pad,
        button,
        state: ButtonState::Up,
    }
}

pub fn axis(pad: PadId, axis: Axis, value: i16) -> PadEvent {
    PadEvent::Axis { pad, axis, value }
}
