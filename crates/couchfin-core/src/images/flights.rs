//! One load per image at a time: later callers for the same key wait for the
//! first, then find its result in the caches.
//!
//! The work runs inside the first caller's own future, not in a detached
//! task, so a caller that gives up (drops its future) cancels it, and the
//! next waiter simply takes over.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use super::key::Key;

#[derive(Debug, Default)]
pub(super) struct Flights {
    locks: Mutex<HashMap<Key, Arc<AsyncMutex<()>>>>,
}

impl Flights {
    /// Waits until no one else is loading `key`.
    pub(super) async fn enter<'a>(&'a self, key: &Key) -> Flight<'a> {
        let lock = {
            let mut locks = self.locks.lock().unwrap_or_else(PoisonError::into_inner);
            Arc::clone(locks.entry(key.clone()).or_default())
        };
        // The `Flight` exists before the wait, so dropping the future while
        // waiting still tidies the map.
        let mut flight = Flight {
            flights: self,
            key: key.clone(),
            lock: Arc::clone(&lock),
            guard: None,
        };
        flight.guard = Some(lock.lock_owned().await);
        flight
    }
}

/// Holds the right to load one key; ends when dropped.
pub(super) struct Flight<'a> {
    flights: &'a Flights,
    key: Key,
    lock: Arc<AsyncMutex<()>>,
    guard: Option<OwnedMutexGuard<()>>,
}

impl Drop for Flight<'_> {
    fn drop(&mut self) {
        self.guard = None;
        let mut locks = self
            .flights
            .locks
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        // Only the map and this flight hold it: nobody is waiting.
        if Arc::strong_count(&self.lock) == 2 {
            locks.remove(&self.key);
        }
    }
}
