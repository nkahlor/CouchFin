//! `DuplicateGuard` merges a controller press and the key Steam's Desktop
//! Layout adds to it, in either order, and nothing else (decision 013).
//! Every instant is exact: the guard only knows the time it is given.

use std::time::{Duration, Instant};

use couchfin_ui::{Dropped, DuplicateGuard, GuardConfig, Intent, Verdict};

struct Rig {
    guard: DuplicateGuard,
    start: Instant,
}

impl Rig {
    fn new() -> Self {
        Self::with(GuardConfig::default())
    }

    fn with(config: GuardConfig) -> Self {
        Self {
            guard: DuplicateGuard::new(config),
            start: Instant::now(),
        }
    }

    fn at(&self, ms: u64) -> Instant {
        self.start + Duration::from_millis(ms)
    }

    fn pad(&mut self, intent: Intent, ms: u64) -> Verdict {
        self.guard.pad(intent, self.at(ms))
    }

    fn key(&mut self, intent: Intent, ms: u64) -> Verdict {
        self.guard.key(intent, false, self.at(ms))
    }

    fn repeat(&mut self, intent: Intent, ms: u64) -> Verdict {
        self.guard.key(intent, true, self.at(ms))
    }
}

const DELIVER: Verdict = Verdict::Deliver;

#[test]
fn a_key_just_after_the_pad_press_is_dropped() {
    let mut rig = Rig::new();
    assert_eq!(rig.pad(Intent::Right, 100), DELIVER);
    assert_eq!(
        rig.key(Intent::Right, 115),
        Verdict::Drop(Dropped::KeyAfterPad)
    );
}

#[test]
fn a_key_just_before_the_pad_press_drops_the_pad_press() {
    let mut rig = Rig::new();
    assert_eq!(rig.key(Intent::Right, 95), DELIVER);
    assert_eq!(
        rig.pad(Intent::Right, 100),
        Verdict::Drop(Dropped::PadAfterKey)
    );
}

#[test]
fn presses_further_apart_than_the_window_both_count() {
    let mut rig = Rig::new();
    assert_eq!(rig.pad(Intent::Right, 0), DELIVER);
    assert_eq!(rig.key(Intent::Right, 50), DELIVER);
    assert_eq!(rig.key(Intent::Right, 500), DELIVER);
    assert_eq!(rig.pad(Intent::Right, 550), DELIVER);
}

#[test]
fn one_press_merges_with_one_event_only() {
    // Two quick pad presses with one key between them: the key belongs to
    // the first, the second press is its own.
    let mut rig = Rig::new();
    assert_eq!(rig.pad(Intent::Right, 0), DELIVER);
    assert_eq!(
        rig.key(Intent::Right, 5),
        Verdict::Drop(Dropped::KeyAfterPad)
    );
    assert_eq!(rig.pad(Intent::Right, 30), DELIVER);
}

#[test]
fn a_different_intent_is_never_merged() {
    let mut rig = Rig::new();
    assert_eq!(rig.pad(Intent::Right, 0), DELIVER);
    assert_eq!(rig.key(Intent::Left, 1), DELIVER);
    assert_eq!(rig.key(Intent::Down, 2), DELIVER);
    assert_eq!(rig.key(Intent::Activate, 3), DELIVER);
}

#[test]
fn key_repeats_while_the_pad_holds_are_dropped() {
    let mut rig = Rig::new();
    rig.pad(Intent::Right, 0);
    rig.key(Intent::Right, 2);
    // The pad repeats at 400, 460, ...; the OS repeats from 500 every 33.
    rig.pad(Intent::Right, 400);
    rig.pad(Intent::Right, 460);
    for ms in [500, 533, 566] {
        assert_eq!(
            rig.repeat(Intent::Right, ms),
            Verdict::Drop(Dropped::KeyRepeatDuringPadHold),
            "{ms}"
        );
    }
}

#[test]
fn repeats_of_a_merged_press_stay_dropped_even_if_the_pad_is_silent() {
    // A one-shot button such as A: the pad never repeats, Steam's key does.
    let mut rig = Rig::new();
    rig.pad(Intent::Activate, 0);
    rig.key(Intent::Activate, 3);
    for ms in [600, 633, 666, 699] {
        assert_eq!(
            rig.repeat(Intent::Activate, ms),
            Verdict::Drop(Dropped::KeyRepeatOfMergedRun),
            "{ms}"
        );
    }
}

#[test]
fn a_new_key_press_ends_a_dropped_run() {
    let mut rig = Rig::new();
    rig.pad(Intent::Activate, 0);
    rig.key(Intent::Activate, 3);
    rig.repeat(Intent::Activate, 600);
    assert_eq!(rig.key(Intent::Activate, 2_000), DELIVER);
    assert_eq!(rig.repeat(Intent::Activate, 2_500), DELIVER);
}

#[test]
fn a_keyboard_alone_repeats_as_it_always_did() {
    let mut rig = Rig::new();
    assert_eq!(rig.key(Intent::Left, 0), DELIVER);
    for ms in [500, 533, 566, 599] {
        assert_eq!(rig.repeat(Intent::Left, ms), DELIVER, "{ms}");
    }
}

#[test]
fn the_controller_alone_is_never_dropped() {
    let mut rig = Rig::new();
    for ms in [0, 400, 460, 520, 580, 3_000] {
        assert_eq!(rig.pad(Intent::Down, ms), DELIVER, "{ms}");
    }
}

#[test]
fn the_windows_are_configurable() {
    let mut rig = Rig::with(GuardConfig {
        merge_window: Duration::from_millis(10),
        ..GuardConfig::default()
    });
    rig.pad(Intent::Right, 0);
    assert_eq!(rig.key(Intent::Right, 15), DELIVER);
}
