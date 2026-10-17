//! Builders for the fake's library content.

use std::time::Duration;

use super::FakeJellyfin;
use crate::item::Item;
use crate::library::{Library, LibraryKind};

impl FakeJellyfin {
    /// Adds an empty library. Libraries list in the order they were added.
    #[must_use]
    pub fn with_library(self, name: &str, kind: LibraryKind) -> Self {
        self.lock().catalog.add_library(name, kind);
        self
    }

    /// Adds a movie to the library called `library`.
    ///
    /// # Panics
    ///
    /// If there is no such library.
    #[must_use]
    pub fn with_movie(self, library: &str, title: &str) -> Self {
        self.lock().catalog.add_movie(library, title);
        self
    }

    /// Adds a series with `seasons` seasons of `episodes` episodes each. The
    /// seasons are named `"{title} S1"` and the episodes `"{title} S1E2"`,
    /// numbered from 1.
    ///
    /// # Panics
    ///
    /// If there is no such library.
    #[must_use]
    pub fn with_series(self, library: &str, title: &str, seasons: u32, episodes: u32) -> Self {
        self.lock()
            .catalog
            .add_series(library, title, seasons, episodes);
        self
    }

    /// Adds a movie library called `name` with `count` movies named
    /// `"{name} 00000"`, `"{name} 00001"` and so on, so they sort in the order
    /// they were made. For performance tests: Phase 3 pages through 2,000.
    #[must_use]
    pub fn with_generated_library(self, name: &str, count: usize) -> Self {
        self.lock().catalog.add_generated(name, count);
        self
    }

    /// Puts the item called `name` in Continue Watching, stopped at
    /// `position`. Items list in the order they were added.
    ///
    /// # Panics
    ///
    /// If there is no item with that name.
    #[must_use]
    pub fn with_resume(self, name: &str, position: Duration) -> Self {
        self.lock().catalog.set_resume(name, position);
        self
    }

    /// Puts the item called `name` (an episode) in Next Up.
    ///
    /// # Panics
    ///
    /// If there is no item with that name.
    #[must_use]
    pub fn with_next_up(self, name: &str) -> Self {
        self.lock().catalog.set_next_up(name);
        self
    }

    /// The library called `name`, to get its ID.
    #[must_use]
    pub fn library_named(&self, name: &str) -> Option<Library> {
        self.lock().catalog.library_named(name)
    }

    /// The first item called `name`, to get its ID.
    #[must_use]
    pub fn item_named(&self, name: &str) -> Option<Item> {
        self.lock().catalog.item_named(name)
    }
}
