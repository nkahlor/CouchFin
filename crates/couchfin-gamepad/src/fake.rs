//! Scripted controllers and a virtual clock, so the runner and the app can
//! be tested without SDL or hardware. Always compiled, like the other
//! crates' fakes, so the app's tests can use them.
//!
//! A test scripts events and pauses on [`FakePads`]; the runner reads them
//! from a [`FakeEventSource`]. Time is virtual: it moves only through
//! [`FakePads::pause`], so a pause of a second takes no real time and every
//! repeat lands on its exact instant.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::event::{Axis, Button, ButtonState, PadEvent, PadId};
use crate::source::{Clock, EventSource, Wakeup};

/// How long [`FakePads::wait_until_idle`] waits before failing the test.
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// A virtual clock that moves only when told to. Clones share the time.
#[derive(Debug, Clone)]
pub struct FakeClock {
    now: Arc<Mutex<Instant>>,
}

impl FakeClock {
    /// A clock stopped at the real current instant.
    #[must_use]
    pub fn new() -> Self {
        Self {
            now: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Moves the clock forward by `by`.
    pub fn advance(&self, by: Duration) {
        *lock(&self.now) += by;
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        *lock(&self.now)
    }
}

#[derive(Debug)]
enum Step {
    Event(PadEvent),
    Pause(Duration),
}

#[derive(Debug, Default)]
struct Script {
    steps: VecDeque<Step>,
    woken: bool,
    blocked: bool,
    waits: usize,
}

#[derive(Debug, Default)]
struct Shared {
    script: Mutex<Script>,
    changed: Condvar,
}

impl Shared {
    fn edit(&self, edit: impl FnOnce(&mut Script)) {
        edit(&mut lock(&self.script));
        self.changed.notify_all();
    }
}

/// The test's side of the fake: scripts what the controllers do. Clones
/// share the script.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use couchfin_gamepad::fake::FakePads;
/// use couchfin_gamepad::{Button, EventSource, PadEvent, PadId};
///
/// let pads = FakePads::new();
/// let mut source = pads.source();
/// pads.press(PadId(0), Button::South);
///
/// let mut events = Vec::new();
/// source.wait(None, &mut events);
/// assert_eq!(events.len(), 1);
///
/// // A pause longer than the timeout: the wait times out on virtual time.
/// pads.pause(Duration::from_secs(1));
/// events.clear();
/// source.wait(Some(Duration::from_millis(400)), &mut events);
/// assert!(events.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct FakePads {
    shared: Arc<Shared>,
    clock: FakeClock,
}

impl FakePads {
    /// An empty script on a fresh [`FakeClock`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            shared: Arc::default(),
            clock: FakeClock::new(),
        }
    }

    /// The source a runner reads this script from.
    #[must_use]
    pub fn source(&self) -> FakeEventSource {
        FakeEventSource {
            shared: Arc::clone(&self.shared),
            clock: self.clock.clone(),
        }
    }

    /// The virtual clock the script moves; give it to the runner.
    #[must_use]
    pub fn clock(&self) -> FakeClock {
        self.clock.clone()
    }

    /// Queues `event`. Events queued with no pause between them arrive in
    /// one wait if the reader hasn't taken the earlier ones yet; use
    /// [`send_burst`](Self::send_burst) to be sure.
    pub fn send(&self, event: PadEvent) {
        self.shared
            .edit(|script| script.steps.push_back(Step::Event(event)));
    }

    /// Queues `events` all at once, so a reader already waiting sees them
    /// arrive together, as in one SDL update.
    pub fn send_burst(&self, events: impl IntoIterator<Item = PadEvent>) {
        self.shared.edit(|script| {
            script.steps.extend(events.into_iter().map(Step::Event));
        });
    }

    /// Queues a pause: the events after it arrive `pause` later.
    pub fn pause(&self, pause: Duration) {
        self.shared
            .edit(|script| script.steps.push_back(Step::Pause(pause)));
    }

    /// Connects `pad`.
    pub fn connect(&self, pad: PadId) {
        self.send(PadEvent::Added(pad));
    }

    /// Disconnects `pad`.
    pub fn disconnect(&self, pad: PadId) {
        self.send(PadEvent::Removed(pad));
    }

    /// Presses `button` on `pad`.
    pub fn press(&self, pad: PadId, button: Button) {
        self.send(PadEvent::Button {
            pad,
            button,
            state: ButtonState::Down,
        });
    }

    /// Releases `button` on `pad`.
    pub fn release(&self, pad: PadId, button: Button) {
        self.send(PadEvent::Button {
            pad,
            button,
            state: ButtonState::Up,
        });
    }

    /// Moves `axis` on `pad` to `value`.
    pub fn tilt(&self, pad: PadId, axis: Axis, value: i16) {
        self.send(PadEvent::Axis { pad, axis, value });
    }

    /// Blocks until the source has used the whole script and is waiting for
    /// more. Whatever reads the source has then handled every scripted
    /// event and every repeat due before the script ran out.
    ///
    /// # Panics
    ///
    /// If that hasn't happened within 10 seconds, so a stuck test fails.
    pub fn wait_until_idle(&self) {
        let script = lock(&self.shared.script);
        let (_script, timeout) = self
            .shared
            .changed
            .wait_timeout_while(script, IDLE_TIMEOUT, |script| {
                !(script.steps.is_empty() && script.blocked)
            })
            .unwrap_or_else(PoisonError::into_inner);
        assert!(
            !timeout.timed_out(),
            "the fake gamepad source never went idle"
        );
    }

    /// How many times the source has been waited on, to check that a reader
    /// handles bursts in one go and sleeps between deadlines.
    #[must_use]
    pub fn waits(&self) -> usize {
        lock(&self.shared.script).waits
    }
}

impl Default for FakePads {
    fn default() -> Self {
        Self::new()
    }
}

/// The runner's side of the fake: an [`EventSource`] reading the script
/// from [`FakePads`].
///
/// A wait with a timeout ends on virtual time when the next scripted pause
/// is longer than the timeout. With the script used up it blocks for real
/// until more is scripted or it is woken, so virtual time stands still
/// until the test says what happens next.
#[derive(Debug)]
pub struct FakeEventSource {
    shared: Arc<Shared>,
    clock: FakeClock,
}

impl EventSource for FakeEventSource {
    type Wakeup = FakeWakeup;

    fn wakeup(&self) -> FakeWakeup {
        FakeWakeup {
            shared: Arc::clone(&self.shared),
        }
    }

    fn wait(&mut self, timeout: Option<Duration>, events: &mut Vec<PadEvent>) {
        let mut script = lock(&self.shared.script);
        script.waits += 1;
        let mut left = timeout;
        loop {
            if std::mem::take(&mut script.woken) {
                return;
            }
            match script.steps.front_mut() {
                Some(Step::Event(_)) => {
                    while let Some(Step::Event(event)) = script.steps.front() {
                        events.push(*event);
                        script.steps.pop_front();
                    }
                    return;
                }
                Some(Step::Pause(pause)) => match left {
                    Some(remaining) if remaining < *pause => {
                        *pause -= remaining;
                        self.clock.advance(remaining);
                        return;
                    }
                    _ => {
                        let pause = *pause;
                        script.steps.pop_front();
                        self.clock.advance(pause);
                        left = left.map(|remaining| remaining.saturating_sub(pause));
                    }
                },
                None => {
                    script.blocked = true;
                    self.shared.changed.notify_all();
                    script = self
                        .shared
                        .changed
                        .wait(script)
                        .unwrap_or_else(PoisonError::into_inner);
                    script.blocked = false;
                }
            }
        }
    }
}

/// Wakes a [`FakeEventSource`].
#[derive(Debug, Clone)]
pub struct FakeWakeup {
    shared: Arc<Shared>,
}

impl Wakeup for FakeWakeup {
    fn wake(&self) {
        self.shared.edit(|script| script.woken = true);
    }
}

/// A test's state stays usable after a panic elsewhere in the test.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
