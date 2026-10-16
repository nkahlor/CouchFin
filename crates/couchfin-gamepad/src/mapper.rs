//! Pad events and time in, [`Input`]s out: the whole mapping, with no SDL,
//! threads or clock of its own, so tests drive it with exact instants.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::config::MapperConfig;
use crate::direction::{Direction, Held, Stick};
use crate::event::{Axis, Button, ButtonState, PadEvent, PadId};
use crate::input::Input;

/// Turns pad events into [`Input`]s, including repeats of held directions.
///
/// - Buttons map by position (SDL's south/east naming), the same on every
///   pad: south is [`Input::Select`], east is [`Input::Back`] and Start is
///   [`Input::Menu`]. They fire once, on the press.
/// - The D-pad and the left stick give directions. Each fires on the press,
///   then repeats as [`Repeat`](crate::Repeat) says until released. The
///   most recent D-pad press is the one that repeats; letting go of it
///   stops the repeat, as with a keyboard. The stick uses a
///   [`Deadzone`](crate::Deadzone): it engages only near an axis (not in the
///   diagonal wedges), locks that direction until the stick returns to
///   the centre, and is read after each batch of events, so a push
///   arriving as two axis events is judged as one position.
/// - Every pad is listened to, each with its own held directions.
///   Disconnecting a pad drops what it was holding.
/// - The right stick, triggers and other buttons are ignored for now.
///
/// # Examples
///
/// ```
/// use std::time::{Duration, Instant};
/// use couchfin_gamepad::{Button, ButtonState, Input, Mapper, MapperConfig, PadEvent, PadId};
///
/// let mut mapper = Mapper::new(MapperConfig::default());
/// let start = Instant::now();
/// let down = PadEvent::Button {
///     pad: PadId(0),
///     button: Button::DPadDown,
///     state: ButtonState::Down,
/// };
/// assert_eq!(mapper.update(&[down], start), [Input::Down]);
///
/// // Held: the first repeat comes 400 ms later.
/// let repeat_at = mapper.next_deadline().expect("a repeat is due");
/// assert_eq!(repeat_at, start + Duration::from_millis(400));
/// assert_eq!(mapper.update(&[], repeat_at), [Input::Down]);
/// ```
#[derive(Debug, Clone)]
pub struct Mapper {
    config: MapperConfig,
    pads: BTreeMap<PadId, Pad>,
}

/// What one pad is holding.
#[derive(Debug, Clone, Copy, Default)]
struct Pad {
    dpad: Option<Held>,
    stick: Stick,
}

impl Mapper {
    /// A mapper with no controllers yet.
    #[must_use]
    pub fn new(config: MapperConfig) -> Self {
        Self {
            config,
            pads: BTreeMap::new(),
        }
    }

    /// Applies `events`, oldest first, which arrived by `now`, then fires
    /// every repeat due by `now`. Call it with no events when
    /// [`next_deadline`](Self::next_deadline) passes.
    ///
    /// A release at the same instant as a repeat cancels the repeat.
    pub fn update(&mut self, events: &[PadEvent], now: Instant) -> Vec<Input> {
        let mut inputs = Vec::new();
        let mut moved = BTreeSet::new();
        for event in events {
            match *event {
                PadEvent::Added(pad) => {
                    self.pads.insert(pad, Pad::default());
                    moved.remove(&pad);
                }
                PadEvent::Removed(pad) => {
                    self.pads.remove(&pad);
                    moved.remove(&pad);
                }
                PadEvent::Button { pad, button, state } => {
                    inputs.extend(self.button(pad, button, state, now));
                }
                PadEvent::Axis { pad, axis, value } => {
                    let stick = &mut self.pads.entry(pad).or_default().stick;
                    match axis {
                        Axis::LeftX => stick.x = value,
                        Axis::LeftY => stick.y = value,
                        _ => continue,
                    }
                    moved.insert(pad);
                }
            }
        }

        let MapperConfig { repeat, deadzone } = self.config;
        for pad in moved {
            if let Some(pad) = self.pads.get_mut(&pad)
                && let Some(direction) = pad.stick.moved(deadzone, now, repeat)
            {
                inputs.push(direction.into());
            }
        }
        for pad in self.pads.values_mut() {
            for held in [pad.dpad.as_mut(), pad.stick.held.as_mut()]
                .into_iter()
                .flatten()
            {
                inputs.extend(held.fire(now, repeat));
            }
        }
        inputs
    }

    /// When the next repeat is due, or `None` while no direction is held.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.pads
            .values()
            .flat_map(|pad| [pad.dpad, pad.stick.held])
            .flatten()
            .map(|held| held.next)
            .min()
    }

    fn button(
        &mut self,
        pad: PadId,
        button: Button,
        state: ButtonState,
        now: Instant,
    ) -> Option<Input> {
        let direction = match button {
            Button::South => return pressed(state, Input::Select),
            Button::East => return pressed(state, Input::Back),
            Button::Start => return pressed(state, Input::Menu),
            Button::DPadUp => Direction::Up,
            Button::DPadDown => Direction::Down,
            Button::DPadLeft => Direction::Left,
            Button::DPadRight => Direction::Right,
            _ => return None,
        };
        let held = &mut self.pads.entry(pad).or_default().dpad;
        match state {
            ButtonState::Down => {
                *held = Some(Held::new(direction, now, self.config.repeat));
                Some(direction.into())
            }
            ButtonState::Up => {
                if held.is_some_and(|held| held.direction == direction) {
                    *held = None;
                }
                None
            }
        }
    }
}

fn pressed(state: ButtonState, input: Input) -> Option<Input> {
    (state == ButtonState::Down).then_some(input)
}
