//! Decides when a controller press and a key press are the same move.
//!
//! With Steam running but Couchfin not launched from it, Steam Input's
//! Desktop Layout makes the controller type arrow keys, Enter and Escape as
//! well. Each press then reaches the window twice: as an [`Intent`] from
//! the gamepad thread and as a real key. [`DuplicateGuard`] merges them
//! (`docs/decisions/013-steam-desktop-layout-dedupe.md`). It is pure: the
//! caller passes the time of each event, so tests are exact.

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};

use crate::Intent;

/// A source of time, injected so tests don't sleep.
pub trait Clock: Send + Sync {
    /// The current instant; it never goes backwards.
    fn now(&self) -> Instant;
}

/// The real clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Timing for [`DuplicateGuard`].
///
/// - `merge_window`: a controller press and a key press of the same intent
///   closer together than this, in either order, are one press. Steam's key
///   lands within a few milliseconds of the pad event; the default of 50 ms
///   is far below a deliberate double press.
/// - `hold_window`: while a controller has fired an intent within this long,
///   a key *repeat* of it is dropped. It must outlast the gap between the
///   controller's own repeats, which is 400 ms before the first and 60 ms
///   after, so the default is 450 ms.
/// - `run_gap`: a run of key repeats that started as a duplicate stays
///   dropped while each repeat follows the last within this long. It covers
///   the operating system's delay before the first repeat (about 500 ms,
///   up to a second on slow settings) and ends with the next real key press.
///
/// ```
/// use std::time::Duration;
/// use couchfin_ui::GuardConfig;
///
/// let config = GuardConfig::default();
/// assert_eq!(config.merge_window, Duration::from_millis(50));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuardConfig {
    /// How close a controller press and a key press must be to merge.
    pub merge_window: Duration,
    /// How long after a controller event key repeats of it are dropped.
    pub hold_window: Duration,
    /// The longest pause inside a run of duplicate key repeats.
    pub run_gap: Duration,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            merge_window: Duration::from_millis(50),
            hold_window: Duration::from_millis(450),
            run_gap: Duration::from_millis(1_000),
        }
    }
}

/// Why an event was dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    /// A key arrived just after the controller press that already moved.
    KeyAfterPad,
    /// A controller press arrived just after the key that already moved.
    PadAfterKey,
    /// A key repeat while a controller holds the same intent.
    KeyRepeatDuringPadHold,
    /// A key repeat in a run that began as a duplicate of a controller press.
    KeyRepeatOfMergedRun,
}

impl fmt::Display for Dropped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Dropped::KeyAfterPad => "key just after the same controller press",
            Dropped::PadAfterKey => "controller press just after the same key",
            Dropped::KeyRepeatDuringPadHold => "key repeat while the controller holds it",
            Dropped::KeyRepeatOfMergedRun => "key repeat of a run that duplicated the controller",
        })
    }
}

/// What to do with an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Let it through.
    Deliver,
    /// It repeats a move already made.
    Drop(Dropped),
}

/// What was last seen for one intent from one source.
#[derive(Debug, Clone, Copy)]
struct Seen {
    at: Instant,
    /// Already merged with the other source, so it can't merge again.
    merged: bool,
}

#[derive(Debug, Default, Clone, Copy)]
struct PerIntent {
    pad: Option<Seen>,
    key: Option<Seen>,
    /// When the current run of duplicate key repeats last dropped one.
    run: Option<Instant>,
}

/// Merges the controller and keyboard paths: a key that duplicates a
/// controller press, or the other way round, is dropped.
///
/// ```
/// use std::time::{Duration, Instant};
/// use couchfin_ui::{Dropped, DuplicateGuard, GuardConfig, Intent, Verdict};
///
/// let mut guard = DuplicateGuard::new(GuardConfig::default());
/// let start = Instant::now();
/// assert_eq!(guard.pad(Intent::Right, start), Verdict::Deliver);
/// // Steam's matching key, 2 ms later, is the same press.
/// let key_at = start + Duration::from_millis(2);
/// assert_eq!(
///     guard.key(Intent::Right, false, key_at),
///     Verdict::Drop(Dropped::KeyAfterPad)
/// );
/// ```
#[derive(Debug, Clone, Default)]
pub struct DuplicateGuard {
    config: GuardConfig,
    seen: HashMap<Intent, PerIntent>,
}

impl DuplicateGuard {
    /// A guard with `config` and nothing seen yet.
    #[must_use]
    pub fn new(config: GuardConfig) -> Self {
        Self {
            config,
            seen: HashMap::new(),
        }
    }

    /// A controller intent is about to be sent at `now`.
    pub fn pad(&mut self, intent: Intent, now: Instant) -> Verdict {
        let merge = self.config.merge_window;
        let state = self.seen.entry(intent).or_default();
        let merged = match &mut state.key {
            Some(key) if !key.merged && within(key.at, now, merge) => {
                key.merged = true;
                state.run = Some(key.at);
                true
            }
            _ => false,
        };
        state.pad = Some(Seen { at: now, merged });
        if merged {
            Verdict::Drop(Dropped::PadAfterKey)
        } else {
            Verdict::Deliver
        }
    }

    /// A navigation key arrived at `now`; `repeat` is true for the
    /// operating system's auto-repeat.
    pub fn key(&mut self, intent: Intent, repeat: bool, now: Instant) -> Verdict {
        let config = self.config;
        let state = self.seen.entry(intent).or_default();
        if repeat {
            if state
                .pad
                .is_some_and(|pad| within(pad.at, now, config.hold_window))
            {
                state.run = Some(now);
                return Verdict::Drop(Dropped::KeyRepeatDuringPadHold);
            }
            if state
                .run
                .is_some_and(|last| within(last, now, config.run_gap))
            {
                state.run = Some(now);
                return Verdict::Drop(Dropped::KeyRepeatOfMergedRun);
            }
            // A keyboard's own repeat: not ours to touch.
            return Verdict::Deliver;
        }
        state.run = None;
        match &mut state.pad {
            Some(pad) if !pad.merged && within(pad.at, now, config.merge_window) => {
                pad.merged = true;
                state.run = Some(now);
                Verdict::Drop(Dropped::KeyAfterPad)
            }
            _ => {
                state.key = Some(Seen {
                    at: now,
                    merged: false,
                });
                Verdict::Deliver
            }
        }
    }
}

/// Whether `later` is no earlier than `earlier` and less than `window` after it.
fn within(earlier: Instant, later: Instant, window: Duration) -> bool {
    later
        .checked_duration_since(earlier)
        .is_some_and(|gap| gap < window)
}
