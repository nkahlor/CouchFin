//! The two things the runner needs from outside: controller events and the
//! time. SDL and the system clock implement them; [`crate::fake`] has
//! stand-ins for tests.

use std::time::{Duration, Instant};

use crate::event::PadEvent;

/// Where controller events come from. The runner owns it on its own thread,
/// so it needn't be `Send`: SDL's objects aren't.
pub trait EventSource {
    /// What another thread uses to interrupt [`wait`](Self::wait).
    type Wakeup: Wakeup;

    /// A handle that makes a current or future [`wait`](Self::wait) return.
    fn wakeup(&self) -> Self::Wakeup;

    /// Blocks until an event is pending, `timeout` has passed (`None` waits
    /// for ever) or [`Wakeup::wake`] is called. Then appends every pending
    /// event to `events`, oldest first, without blocking again, so a flood
    /// of events is handled in one go rather than one per wake.
    fn wait(&mut self, timeout: Option<Duration>, events: &mut Vec<PadEvent>);
}

/// Interrupts an [`EventSource::wait`] from another thread.
pub trait Wakeup: Send + 'static {
    /// Makes the source's current wait return, or its next one if it isn't
    /// waiting.
    fn wake(&self);
}

/// The time, for repeat deadlines.
pub trait Clock: Send + 'static {
    /// The current instant.
    fn now(&self) -> Instant;
}

/// The real clock: [`Instant::now`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}
