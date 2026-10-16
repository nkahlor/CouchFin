//! A reusable simulation of Steam's Desktop Layout for tests.
//!
//! With Steam running but Couchfin not launched from it, Steam Input makes
//! a controller type keys as well: every pad press also arrives as the
//! matching key, a held direction as a key-down plus the OS's auto-repeat.
//! [`Steam`] scripts that next to fake pads, on a manual clock shared with
//! the app's duplicate guard, so every offset in a test is exact.
//!
//! Times are milliseconds on a timeline [`Steam`] keeps; each call to
//! [`Steam::play`] plays a [`Script`] starting at the current time.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use couchfin_gamepad::fake::FakePads;
use couchfin_gamepad::{Button, PadId};
use couchfin_ui::Clock;
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};

use super::{ui, with_window};

/// The guard's clock: moves only when the fixture moves it.
#[derive(Clone)]
pub struct ManualClock {
    base: Instant,
    ms: Arc<AtomicU64>,
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.base + Duration::from_millis(self.ms.load(Ordering::SeqCst))
    }
}

/// One thing that happens at a time.
#[derive(Clone, Copy)]
enum Action {
    Pad(Button, bool),
    Key(Key, KeyKind),
}

#[derive(Clone, Copy)]
enum KeyKind {
    Down,
    Repeat,
    Up,
}

/// Events at offsets from the script's start; offsets may be negative.
#[derive(Default)]
pub struct Script {
    events: Vec<(i64, Action)>,
}

impl Script {
    /// A pad press at `at`, the Desktop Layout's matching key `offset_ms`
    /// before (negative) or after it, and both released 20 ms later.
    #[must_use]
    pub fn steam_press(mut self, at: i64, button: Button, key: Key, offset_ms: i64) -> Self {
        self.events.push((at, Action::Pad(button, true)));
        self.events.push((at + 20, Action::Pad(button, false)));
        self.events
            .push((at + offset_ms, Action::Key(key, KeyKind::Down)));
        self.events
            .push((at + offset_ms + 20, Action::Key(key, KeyKind::Up)));
        self
    }

    /// A pad direction pressed at `at` and held for `held_ms`. Steam's key
    /// goes down `offset_ms` from the press and the OS repeats it after
    /// 500 ms, every 33 ms, until release.
    #[must_use]
    pub fn steam_hold(
        mut self,
        at: i64,
        button: Button,
        key: Key,
        offset_ms: i64,
        held_ms: i64,
    ) -> Self {
        self.events.push((at, Action::Pad(button, true)));
        self.events.push((at + held_ms, Action::Pad(button, false)));
        let down = at + offset_ms;
        self.events.push((down, Action::Key(key, KeyKind::Down)));
        let mut repeat = down + 500;
        while repeat < at + held_ms {
            self.events
                .push((repeat, Action::Key(key, KeyKind::Repeat)));
            repeat += 33;
        }
        self.events
            .push((at + held_ms, Action::Key(key, KeyKind::Up)));
        self
    }

    /// Only a pad press (no Steam), released 20 ms later.
    #[must_use]
    pub fn pad_press(mut self, at: i64, button: Button) -> Self {
        self.events.push((at, Action::Pad(button, true)));
        self.events.push((at + 20, Action::Pad(button, false)));
        self
    }

    /// Only a pad hold (no Steam).
    #[must_use]
    pub fn pad_hold(mut self, at: i64, button: Button, held_ms: i64) -> Self {
        self.events.push((at, Action::Pad(button, true)));
        self.events.push((at + held_ms, Action::Pad(button, false)));
        self
    }

    /// A real keyboard press at `at`, released 20 ms later.
    #[must_use]
    pub fn key_press(mut self, at: i64, key: Key) -> Self {
        self.events.push((at, Action::Key(key, KeyKind::Down)));
        self.events.push((at + 20, Action::Key(key, KeyKind::Up)));
        self
    }

    /// A real keyboard hold: down at `at`, repeats after 500 ms every 33 ms
    /// while held for `held_ms`.
    #[must_use]
    pub fn key_hold(mut self, at: i64, key: Key, held_ms: i64) -> Self {
        self.events.push((at, Action::Key(key, KeyKind::Down)));
        let mut repeat = at + 500;
        while repeat < at + held_ms {
            self.events
                .push((repeat, Action::Key(key, KeyKind::Repeat)));
            repeat += 33;
        }
        self.events
            .push((at + held_ms, Action::Key(key, KeyKind::Up)));
        self
    }
}

/// Fake pads and a manual clock, played together.
pub struct Steam {
    pads: FakePads,
    clock: ManualClock,
    now_ms: i64,
}

/// The pad the scripts press on.
const PAD: PadId = PadId(0);

impl Steam {
    /// A fixture over `pads`; give [`Steam::clock`] to the app's guard.
    pub fn new(pads: &FakePads) -> Self {
        Self {
            pads: pads.clone(),
            clock: ManualClock {
                base: Instant::now(),
                ms: Arc::new(AtomicU64::new(0)),
            },
            now_ms: 0,
        }
    }

    /// The clock for `connect_gamepads_with`.
    pub fn clock(&self) -> Arc<dyn Clock> {
        Arc::new(self.clock.clone())
    }

    /// Plays `script` with its time zero 1 s after the previous script's
    /// end, so scripts never merge with each other.
    pub fn play(&mut self, script: &Script) {
        let mut events = script.events.clone();
        // Stable: same-instant events keep the order they were scripted in.
        events.sort_by_key(|(at, _)| *at);
        let origin = self.now_ms + 1_000 - events.first().map_or(0, |(at, _)| *at);
        let mut last = self.now_ms;
        // Gamepad repeats fire inside the fake runner at their own virtual
        // times; step to each so the guard's clock is right when one lands.
        let mut steps: Vec<i64> = events.iter().map(|(at, _)| origin + at).collect();
        let end = steps.iter().copied().max().unwrap_or(origin);
        let downs: Vec<i64> = events
            .iter()
            .filter(|(_, action)| matches!(action, Action::Pad(_, true)))
            .map(|(at, _)| origin + at)
            .collect();
        for down in downs {
            let mut repeat = down + 400;
            while repeat <= end {
                steps.push(repeat);
                repeat += 60;
            }
        }
        steps.sort_unstable();
        steps.dedup();
        for step in steps {
            self.move_to(step, &mut last);
            for (at, action) in &events {
                if origin + at == step {
                    self.fire(*action);
                }
            }
            self.pads.wait_until_idle();
            ui(|| ());
        }
        self.now_ms = last;
    }

    fn move_to(&self, ms: i64, last: &mut i64) {
        let gap = u64::try_from(ms - *last).unwrap_or(0);
        if gap > 0 {
            self.pads.pause(Duration::from_millis(gap));
        }
        self.clock
            .ms
            .store(u64::try_from(ms).unwrap_or(0), Ordering::SeqCst);
        *last = ms;
    }

    fn fire(&self, action: Action) {
        match action {
            Action::Pad(button, true) => self.pads.press(PAD, button),
            Action::Pad(button, false) => self.pads.release(PAD, button),
            Action::Key(key, kind) => {
                let text: slint::SharedString = key.into();
                with_window(move |window| {
                    let window = window.window();
                    window.dispatch_event(match kind {
                        KeyKind::Down => WindowEvent::KeyPressed { text },
                        KeyKind::Repeat => WindowEvent::KeyPressRepeated { text },
                        KeyKind::Up => WindowEvent::KeyReleased { text },
                    });
                });
            }
        }
    }
}
