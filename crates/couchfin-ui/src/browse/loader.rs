//! Which images are being fetched for which place on screen, and the
//! plug the app fills with couchfin-core's image cache.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use couchfin_core::{ImageLoad, ImageRef, Pixels, RowKind, Variant};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use super::feed::Feed;

/// Loads one image. The app implements it over couchfin-core's
/// `ImageCache`, so this crate needs no async runtime and tests use a fake.
pub trait ImageLoader: Send + Sync {
    /// Starts fetching `image` at `max_width` pixels and calls `done` with the
    /// result from any thread. Dropping the returned [`Guard`] cancels the
    /// load: `done` must not be called afterwards, or may be and is ignored.
    /// A failed load should be logged by the loader and reported as
    /// [`ImageLoad::NoImage`], which keeps the placeholder.
    fn load(
        &self,
        image: &ImageRef,
        max_width: u32,
        variant: Variant,
        done: Box<dyn FnOnce(ImageLoad) + Send>,
    ) -> Guard;
}

/// Held while a load is wanted; dropping it cancels the load.
pub struct Guard(#[allow(dead_code)] Box<dyn Any + Send>);

impl Guard {
    /// Wraps whatever cancels the load when dropped, such as a tokio task's
    /// abort handle.
    #[must_use]
    pub fn new(on_drop: impl Any + Send) -> Self {
        Self(Box::new(on_drop))
    }
}

/// A place on screen that shows a loaded image.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Slot {
    /// A Home card: its row, its place in the row and which picture.
    Card(String, usize, Variant),
    /// A place in the library grid.
    Grid(usize),
    /// The detail page's backdrop.
    Backdrop,
}

/// A row's identity across reloads, as text a [`Slot`] can hold.
pub(crate) fn row_key(kind: &RowKind) -> String {
    format!("{kind:?}")
}

/// What a slot should show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wish {
    pub image: ImageRef,
    pub width: u32,
    pub variant: Variant,
}

enum Load {
    Pending(#[allow(dead_code)] Guard),
    Done,
}

struct Entry {
    wish: Wish,
    ticket: u64,
    load: Load,
}

/// The loads started and finished, one per slot.
pub(crate) struct Loads {
    loader: Arc<dyn ImageLoader>,
    feed: Feed,
    entries: HashMap<Slot, Entry>,
    next_ticket: u64,
}

impl Loads {
    pub(crate) fn new(loader: Arc<dyn ImageLoader>, feed: Feed) -> Self {
        Self {
            loader,
            feed,
            entries: HashMap::new(),
            next_ticket: 0,
        }
    }

    /// Makes the loads match `wanted`, in priority order: loads for slots
    /// no longer wanted are cancelled, new ones start. A finished image
    /// outside `wanted` stays while `keep` says so. Returns the slots whose
    /// image the caller should now clear.
    pub(crate) fn reconcile(
        &mut self,
        wanted: Vec<(Slot, Wish)>,
        keep: impl Fn(&Slot) -> bool,
    ) -> Vec<Slot> {
        let mut cleared = Vec::new();
        self.entries.retain(|slot, entry| {
            let still_wanted = wanted
                .iter()
                .any(|(wanted, wish)| wanted == slot && *wish == entry.wish);
            let finished = matches!(entry.load, Load::Done);
            let kept = still_wanted || (finished && keep(slot));
            if !kept && finished {
                cleared.push(slot.clone());
            }
            kept
        });
        for (slot, wish) in wanted {
            if self.entries.get(&slot).is_some_and(|e| e.wish == wish) {
                continue;
            }
            let ticket = self.next_ticket;
            self.next_ticket += 1;
            let feed = self.feed.clone();
            let to = slot.clone();
            let guard = self.loader.load(
                &wish.image,
                wish.width,
                wish.variant,
                Box::new(move |loaded| feed.image(to, ticket, loaded)),
            );
            self.entries.insert(
                slot,
                Entry {
                    wish,
                    ticket,
                    load: Load::Pending(guard),
                },
            );
        }
        cleared
    }

    /// Notes that the load with this ticket finished. False if the slot has
    /// moved on and the result should be dropped.
    pub(crate) fn finish(&mut self, slot: &Slot, ticket: u64) -> bool {
        match self.entries.get_mut(slot) {
            Some(entry) if entry.ticket == ticket => {
                entry.load = Load::Done;
                true
            }
            _ => false,
        }
    }

    /// Cancels every load and forgets every finished one.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    /// Forgets the finished images of slots that no longer exist.
    pub(crate) fn forget(&mut self, gone: impl Fn(&Slot) -> bool) {
        self.entries.retain(|slot, _| !gone(slot));
    }
}

/// Copies decoded pixels into a Slint image. The copy is a memcpy; the
/// decoding already happened in couchfin-core.
pub(crate) fn to_image(pixels: &Pixels) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        pixels.rgba(),
        pixels.width(),
        pixels.height(),
    ))
}
