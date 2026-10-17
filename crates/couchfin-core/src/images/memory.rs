//! Recently decoded images, bounded by their size in bytes.

use std::collections::HashMap;

use super::key::Key;
use super::pixels::Pixels;

/// A least-recently-used map. Eviction scans for the oldest entry, which is
/// fine for the few hundred posters a 256 MB budget holds.
#[derive(Debug)]
pub(super) struct Memory {
    limit: usize,
    used: usize,
    clock: u64,
    entries: HashMap<Key, (Pixels, u64)>,
}

impl Memory {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            limit,
            used: 0,
            clock: 0,
            entries: HashMap::new(),
        }
    }

    pub(super) fn get(&mut self, key: &Key) -> Option<Pixels> {
        self.clock += 1;
        let clock = self.clock;
        let (pixels, used_at) = self.entries.get_mut(key)?;
        *used_at = clock;
        Some(pixels.clone())
    }

    pub(super) fn insert(&mut self, key: Key, pixels: Pixels) {
        let size = pixels.rgba().len();
        // An image bigger than the whole budget would only flush the rest.
        if size > self.limit {
            return;
        }
        self.clock += 1;
        if let Some((old, _)) = self.entries.insert(key, (pixels, self.clock)) {
            self.used -= old.rgba().len();
        }
        self.used += size;
        while self.used > self.limit {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, used_at))| *used_at)
                .map(|(entry, _)| entry.clone())
            else {
                break;
            };
            if let Some((evicted, _)) = self.entries.remove(&oldest) {
                self.used -= evicted.rgba().len();
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.used = 0;
    }
}
