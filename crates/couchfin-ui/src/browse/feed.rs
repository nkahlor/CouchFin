//! The way in for everything that arrives off the UI thread: new library
//! states, the cue to show or drop the library, and finished images.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Weak;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use couchfin_core::{ImageLoad, LibraryState};

use super::Inner;
use super::loader::Slot;

pub(super) enum Msg {
    State(Box<LibraryState>),
    Show,
    Reset,
    Image {
        slot: Slot,
        ticket: u64,
        loaded: ImageLoad,
    },
}

#[derive(Default)]
struct Mailbox {
    queue: Mutex<VecDeque<Msg>>,
    /// A wake-up is already queued on the event loop.
    waking: AtomicBool,
}

impl Mailbox {
    fn queue(&self) -> MutexGuard<'_, VecDeque<Msg>> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

thread_local! {
    /// The browsers of this (UI) thread. Weak, so a window that goes away
    /// takes its browser with it.
    static BROWSERS: RefCell<HashMap<u64, Weak<Inner>>> = RefCell::new(HashMap::new());
}

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Sends library states and commands to the browsing screens from any
/// thread. Cheap to clone. Messages queue up and are applied in order on the
/// UI thread; consecutive states collapse to the latest one, so a burst of
/// page loads costs one update.
///
/// Made by [`install`](crate::install).
#[derive(Clone)]
pub struct Feed {
    id: u64,
    mailbox: Arc<Mailbox>,
}

impl Feed {
    pub(super) fn new() -> Self {
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            mailbox: Arc::new(Mailbox::default()),
        }
    }

    pub(super) fn register(&self, inner: Weak<Inner>) {
        BROWSERS.with(|browsers| browsers.borrow_mut().insert(self.id, inner));
    }

    /// Shows this library state: Home, a grid or a detail page, whichever is
    /// on top of the stack. Nothing is shown on the window until [`show`]
    /// (the screens belong to the sign-in flow until then).
    ///
    /// [`show`]: Feed::show
    pub fn state(&self, state: LibraryState) {
        self.send(Msg::State(Box::new(state)));
    }

    /// Puts the library's screen on the window, as the sign-in flow reaches
    /// Home. Call it after rendering that Home.
    pub fn show(&self) {
        self.send(Msg::Show);
    }

    /// Forgets the library: empties the screens, cancels every image load
    /// and stops putting screens on the window. Call it when the sign-in
    /// flow leaves Home.
    pub fn reset(&self) {
        self.send(Msg::Reset);
    }

    /// Applies every queued message now. The event loop does this by
    /// itself after each message arrives; this is for code that has no
    /// event loop running, such as tests. Call it on the UI thread.
    pub fn pump(&self) {
        let messages: Vec<Msg> = self.mailbox.queue().drain(..).collect();
        let inner = BROWSERS.with(|browsers| browsers.borrow().get(&self.id)?.upgrade());
        if let Some(inner) = inner {
            inner.receive(messages);
        }
    }

    pub(super) fn image(&self, slot: Slot, ticket: u64, loaded: ImageLoad) {
        self.send(Msg::Image {
            slot,
            ticket,
            loaded,
        });
    }

    fn send(&self, msg: Msg) {
        self.mailbox.queue().push_back(msg);
        if self.mailbox.waking.swap(true, Ordering::AcqRel) {
            return;
        }
        let feed = self.clone();
        let woken = slint::invoke_from_event_loop(move || {
            // Cleared first, so a message that arrives while we apply this
            // batch wakes the loop again.
            feed.mailbox.waking.store(false, Ordering::Release);
            feed.pump();
        });
        if let Err(error) = woken {
            // No event loop: the app is quitting, or this is a test that
            // calls `pump` itself.
            self.mailbox.waking.store(false, Ordering::Release);
            tracing::debug!(%error, "no event loop to wake");
        }
    }
}
