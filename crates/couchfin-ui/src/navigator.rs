//! The window's navigation input, with controller and keyboard merged.
//!
//! [`Navigator`] sits in front of both paths into the window: controller
//! intents go through [`Navigator::navigate`], and the window offers every
//! key to the navigator first (a capture handler in `ui/main-window.slint`).
//! A [`DuplicateGuard`] then drops the second of the two keys Steam's
//! Desktop Layout sends for one controller press
//! (`docs/decisions/013-steam-desktop-layout-dedupe.md`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Instant;

use slint::SharedString;

use crate::generated::MainWindow;
use crate::guard::{Clock, DuplicateGuard, GuardConfig, Verdict};
use crate::navigate::{Intent, navigate};

/// Milliseconds since the first log line, on a clock that never goes
/// backwards, to put the debug lines of every input source on one timeline.
fn stamp(now: Instant) -> u128 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    now.saturating_duration_since(*EPOCH.get_or_init(Instant::now))
        .as_millis()
}

/// The monotonic timestamp (milliseconds) the debug log uses, for callers
/// that log their own input events on the same timeline.
#[must_use]
pub fn log_stamp() -> u128 {
    stamp(Instant::now())
}

/// The `tracing` target of every input line, so `RUST_LOG=couchfin=debug`
/// shows them whichever crate logs.
pub const INPUT_LOG: &str = "couchfin::input";

struct Inner {
    guard: Mutex<DuplicateGuard>,
    clock: Arc<dyn Clock>,
    /// Set while [`Navigator::navigate`] injects a key, so the capture
    /// handler lets it through instead of counting it as the keyboard.
    injecting: AtomicBool,
}

/// Feeds controller intents into a [`MainWindow`] and filters its keys, so
/// one press never moves twice. Cloning shares the guard.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use couchfin_ui::{GuardConfig, Intent, MainWindow, Navigator, SystemClock};
/// use slint::ComponentHandle;
///
/// # i_slint_backend_testing::init_no_event_loop();
/// let window = MainWindow::new()?;
/// window.show()?;
/// let navigator = Navigator::install(&window, GuardConfig::default(), Arc::new(SystemClock));
/// // The window has no focused control to act on the key here; this shows
/// // the call. In the app, the controller thread sends this on the UI thread.
/// navigator.navigate(&window, Intent::Down);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct Navigator {
    inner: Arc<Inner>,
}

impl Navigator {
    /// Starts filtering `window`'s navigation keys with a guard built from
    /// `config`, timing events with `clock`. Call it once per window, on the
    /// Slint event-loop thread.
    pub fn install(window: &MainWindow, config: GuardConfig, clock: Arc<dyn Clock>) -> Self {
        let inner = Arc::new(Inner {
            guard: Mutex::new(DuplicateGuard::new(config)),
            clock,
            injecting: AtomicBool::new(false),
        });
        let filter = Arc::clone(&inner);
        window.on_filter_key(move |text, repeat| filter.filter_key(&text, repeat));
        Self { inner }
    }

    /// Sends `intent` from a controller, unless the same press already
    /// moved as a key. Call it on the Slint event-loop thread; see
    /// [`navigate`](crate::navigate()) for what a delivered intent does.
    pub fn navigate(&self, window: &MainWindow, intent: Intent) {
        let now = self.inner.clock.now();
        let verdict = self.inner.guard().pad(intent, now);
        match verdict {
            Verdict::Deliver => {
                tracing::debug!(
                    target: INPUT_LOG, source = "gamepad", ?intent, at_ms = %stamp(now), "navigation input"
                );
                self.inner.injecting.store(true, Ordering::SeqCst);
                navigate(window, intent);
                self.inner.injecting.store(false, Ordering::SeqCst);
            }
            Verdict::Drop(reason) => {
                tracing::debug!(
                    target: INPUT_LOG,
                    source = "gamepad", ?intent, at_ms = %stamp(now), %reason,
                    "dropped a duplicate navigation input"
                );
            }
        }
    }
}

impl Inner {
    fn guard(&self) -> std::sync::MutexGuard<'_, DuplicateGuard> {
        self.guard.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether the window should swallow this key as a duplicate. Only
    /// navigation keys are looked at (and logged); anything else, such as
    /// text typed into a field, passes untouched.
    fn filter_key(&self, text: &SharedString, repeat: bool) -> bool {
        if self.injecting.load(Ordering::SeqCst) {
            return false;
        }
        let Some(intent) = Intent::from_key_text(text) else {
            return false;
        };
        let now = self.clock.now();
        let verdict = self.guard().key(intent, repeat, now);
        match verdict {
            Verdict::Deliver => {
                tracing::debug!(
                    target: INPUT_LOG,
                    source = "keyboard", ?intent, repeat, at_ms = %stamp(now), "navigation input"
                );
                false
            }
            Verdict::Drop(reason) => {
                tracing::debug!(
                    target: INPUT_LOG,
                    source = "keyboard", ?intent, repeat, at_ms = %stamp(now), %reason,
                    "dropped a duplicate navigation input"
                );
                true
            }
        }
    }
}
