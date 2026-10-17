//! The fake's library content: libraries, items, the Home lists and the
//! rules for paging them the way a Jellyfin server does.

use std::time::Duration;

use crate::blur_hash::{BlurHash, ImageBlurHashes};
use crate::ids::{ImageTag, ItemId};
use crate::image::ImageKind;
use crate::item::{ImageTags, Item, ItemKind, UserData};
use crate::library::{Library, LibraryKind};

use super::fake_id;

mod reading;

/// A valid 1x1 grey PNG, standing in for every poster, thumbnail and
/// backdrop.
const PLACEHOLDER_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x08, 0x0D, 0x4D, 0xFB,
    0x0F, 0x00, 0x04, 0x23, 0x02, 0x10, 0x5F, 0x24, 0x6F, 0xD6, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

#[derive(Debug, Default)]
pub(super) struct Catalog {
    libraries: Vec<Library>,
    /// Every item in the order it was added, which is also its age.
    entries: Vec<Entry>,
    resume: Vec<ItemId>,
    next_up: Vec<ItemId>,
    issued: usize,
}

#[derive(Debug)]
struct Entry {
    item: Item,
    /// The library, series or season that contains it.
    parent: ItemId,
}

/// A valid BlurHash every fake image has, so placeholders can be tested.
pub(crate) const FAKE_BLUR_HASH: &str = "LEHV6nWB2yk8pyo0adR*.7kCMdnj";

fn fake_blur_hashes(id: &ItemId, wide: bool) -> ImageBlurHashes {
    let hash = || FAKE_BLUR_HASH.parse::<BlurHash>().expect("a valid hash");
    let kinds = [ImageKind::Primary, ImageKind::Thumb]
        .into_iter()
        .chain(wide.then_some(ImageKind::Backdrop));
    kinds.fold(ImageBlurHashes::default(), |hashes, kind| {
        hashes.with(kind, tag(id, kind), hash())
    })
}

fn tag(id: &ItemId, kind: ImageKind) -> ImageTag {
    ImageTag::try_from(format!("fake-{}-{id}", kind.as_str().to_lowercase()))
        .expect("letters, digits and '-' are always a valid tag")
}

// Building.
impl Catalog {
    fn new_id(&mut self) -> ItemId {
        self.issued += 1;
        fake_id("item", self.issued)
    }

    pub(super) fn add_library(&mut self, name: &str, kind: LibraryKind) -> ItemId {
        let id = self.new_id();
        self.libraries.push(Library {
            primary_image: Some(tag(&id, ImageKind::Primary)),
            primary_blur_hash: FAKE_BLUR_HASH.parse().ok(),
            id: id.clone(),
            name: name.to_owned(),
            kind,
        });
        id
    }

    pub(super) fn library_id(&self, name: &str) -> ItemId {
        self.library_named(name)
            .unwrap_or_else(|| panic!("the fake has no library called {name:?}"))
            .id
    }

    pub(super) fn library_named(&self, name: &str) -> Option<Library> {
        self.libraries.iter().find(|l| l.name == name).cloned()
    }

    fn add(&mut self, parent: &ItemId, name: String, kind: ItemKind) -> Item {
        let id = self.new_id();
        let wide = matches!(kind, ItemKind::Movie | ItemKind::Series);
        let runtime = match kind {
            ItemKind::Movie => Some(Duration::from_mins(100)),
            ItemKind::Episode => Some(Duration::from_mins(45)),
            _ => None,
        };
        let item = Item {
            overview: Some(format!("About {name}.")),
            name,
            kind,
            production_year: Some(2000),
            runtime,
            official_rating: Some("PG".to_owned()),
            image_tags: ImageTags {
                primary: Some(tag(&id, ImageKind::Primary)),
                thumb: Some(tag(&id, ImageKind::Thumb)),
                backdrop: wide.then(|| tag(&id, ImageKind::Backdrop)),
            },
            blur_hashes: fake_blur_hashes(&id, wide),
            user_data: Some(UserData {
                played: false,
                position: Duration::ZERO,
                played_percentage: None,
            }),
            series_name: None,
            series_id: None,
            season_number: None,
            episode_number: None,
            id,
        };
        self.entries.push(Entry {
            item: item.clone(),
            parent: parent.clone(),
        });
        item
    }

    pub(super) fn add_movie(&mut self, library: &str, title: &str) {
        let parent = self.library_id(library);
        self.add(&parent, title.to_owned(), ItemKind::Movie);
    }

    pub(super) fn add_series(&mut self, library: &str, title: &str, seasons: u32, episodes: u32) {
        let parent = self.library_id(library);
        let series = self.add(&parent, title.to_owned(), ItemKind::Series);
        for s in 1..=seasons {
            let mut season = self.add(&series.id, format!("{title} S{s}"), ItemKind::Season);
            season.series_name = Some(title.to_owned());
            season.series_id = Some(series.id.clone());
            season.season_number = Some(s);
            self.update(&season);
            for e in 1..=episodes {
                let mut episode =
                    self.add(&season.id, format!("{title} S{s}E{e}"), ItemKind::Episode);
                episode.series_name = Some(title.to_owned());
                episode.series_id = Some(series.id.clone());
                episode.season_number = Some(s);
                episode.episode_number = Some(e);
                self.update(&episode);
            }
        }
    }

    pub(super) fn add_generated(&mut self, name: &str, count: usize) {
        let parent = self.add_library(name, LibraryKind::Movies);
        for n in 0..count {
            // Zero-padded, so sorting by name keeps the order they are made in.
            self.add(&parent, format!("{name} {n:05}"), ItemKind::Movie);
        }
    }

    /// Replaces the stored copy of `item`, after its fields were filled in.
    fn update(&mut self, item: &Item) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.item.id == item.id) {
            entry.item = item.clone();
        }
    }

    fn id_named(&self, name: &str) -> ItemId {
        self.item_named(name)
            .unwrap_or_else(|| panic!("the fake has no item called {name:?}"))
            .id
    }

    pub(super) fn item_named(&self, name: &str) -> Option<Item> {
        self.entries
            .iter()
            .find(|e| e.item.name == name)
            .map(|e| e.item.clone())
    }

    pub(super) fn set_resume(&mut self, name: &str, position: Duration) {
        let id = self.id_named(name);
        if let Some(entry) = self.entries.iter_mut().find(|e| e.item.id == id) {
            let percentage = entry
                .item
                .runtime
                .map(|runtime| position.as_secs_f64() / runtime.as_secs_f64() * 100.0);
            entry.item.user_data = Some(UserData {
                played: false,
                position,
                played_percentage: percentage,
            });
        }
        self.resume.push(id);
    }

    pub(super) fn set_next_up(&mut self, name: &str) {
        let id = self.id_named(name);
        self.next_up.push(id);
    }
}
