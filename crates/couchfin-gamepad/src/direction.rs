//! A held direction and its repeat timer, shared by the D-pad and the stick.

use std::time::Instant;

use crate::config::{Deadzone, Repeat};
use crate::input::Input;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl From<Direction> for Input {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Up => Input::Up,
            Direction::Down => Input::Down,
            Direction::Left => Input::Left,
            Direction::Right => Input::Right,
        }
    }
}

/// A direction being held, and when it next repeats.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Held {
    pub(crate) direction: Direction,
    pub(crate) next: Instant,
}

impl Held {
    pub(crate) fn new(direction: Direction, now: Instant, repeat: Repeat) -> Self {
        Self {
            direction,
            next: now + repeat.delay(),
        }
    }

    /// Fires the repeat if it is due. A late wake fires once and restarts
    /// the interval from `now`, rather than catching up in a burst that
    /// would overshoot what the user meant to reach.
    pub(crate) fn fire(&mut self, now: Instant, repeat: Repeat) -> Option<Input> {
        if self.next > now {
            return None;
        }
        self.next += repeat.interval();
        if self.next <= now {
            self.next = now + repeat.interval();
        }
        Some(self.direction.into())
    }
}

/// The left stick's position and the direction it is holding.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Stick {
    pub(crate) x: i16,
    pub(crate) y: i16,
    pub(crate) held: Option<Held>,
}

impl Stick {
    /// Re-reads the position after it moved. Returns the direction if the
    /// stick has just engaged one.
    ///
    /// A held direction stays until its own axis falls below the release
    /// threshold, whatever the other axis does, so a stick near a diagonal
    /// can't flick between two directions. Engaging picks the stronger axis
    /// (horizontal on an exact tie).
    pub(crate) fn moved(
        &mut self,
        deadzone: Deadzone,
        now: Instant,
        repeat: Repeat,
    ) -> Option<Direction> {
        if let Some(held) = self.held {
            if self.along(held.direction) >= deadzone.release() {
                return None;
            }
            self.held = None;
        }
        let (across, up_down) = (self.x.unsigned_abs(), self.y.unsigned_abs());
        let (strength, direction) = if up_down > across {
            let direction = if self.y < 0 {
                Direction::Up
            } else {
                Direction::Down
            };
            (up_down, direction)
        } else {
            let direction = if self.x < 0 {
                Direction::Left
            } else {
                Direction::Right
            };
            (across, direction)
        };
        if strength < deadzone.press() {
            return None;
        }
        self.held = Some(Held::new(direction, now, repeat));
        Some(direction)
    }

    /// How far the stick points in `direction` (zero if the other way).
    fn along(&self, direction: Direction) -> u16 {
        let (value, sign) = match direction {
            Direction::Up => (self.y, -1),
            Direction::Down => (self.y, 1),
            Direction::Left => (self.x, -1),
            Direction::Right => (self.x, 1),
        };
        if value.signum() == sign {
            value.unsigned_abs()
        } else {
            0
        }
    }
}
