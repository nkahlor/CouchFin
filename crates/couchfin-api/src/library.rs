//! Libraries (the user's views) and the paged list wrapper.

use std::collections::HashMap;

use serde::Deserialize;

use crate::ids::{ImageTag, ItemId};
use crate::item::image_tag;

/// What a library holds, from Jellyfin's `CollectionType`.
///
/// # Examples
///
/// ```
/// use couchfin_api::LibraryKind;
///
/// assert_eq!(LibraryKind::from(Some("tvshows")), LibraryKind::TvShows);
/// assert_eq!(LibraryKind::from(None), LibraryKind::Mixed);
/// assert_eq!(LibraryKind::from(Some("music")), LibraryKind::Other("music".to_owned()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LibraryKind {
    /// Films.
    Movies,
    /// TV shows.
    TvShows,
    /// No collection type: a mix of content.
    Mixed,
    /// Any other collection type, as Jellyfin names it.
    Other(String),
}

impl From<Option<&str>> for LibraryKind {
    fn from(name: Option<&str>) -> Self {
        match name {
            Some("movies") => Self::Movies,
            Some("tvshows") => Self::TvShows,
            None => Self::Mixed,
            Some(other) => Self::Other(other.to_owned()),
        }
    }
}

/// One of the user's libraries, such as "Movies".
///
/// # Examples
///
/// ```
/// use couchfin_api::{Library, LibraryKind};
///
/// let library: Library = serde_json::from_str(
///     r#"{ "Id": "abc", "Name": "Movies", "CollectionType": "movies" }"#,
/// )?;
/// assert_eq!(library.kind, LibraryKind::Movies);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawLibrary")]
pub struct Library {
    /// The library's ID; the parent to browse it with.
    pub id: ItemId,
    /// The name the admin gave it.
    pub name: String,
    /// What it holds.
    pub kind: LibraryKind,
    /// The tag of its cover image, if it has one.
    pub primary_image: Option<ImageTag>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawLibrary {
    id: ItemId,
    name: String,
    collection_type: Option<String>,
    #[serde(default)]
    image_tags: HashMap<String, Option<String>>,
}

impl From<RawLibrary> for Library {
    fn from(raw: RawLibrary) -> Self {
        Self {
            id: raw.id,
            name: raw.name,
            kind: LibraryKind::from(raw.collection_type.as_deref()),
            primary_image: image_tag(&raw.image_tags, "Primary"),
        }
    }
}

/// One page of a longer list, with the size of the whole list.
///
/// # Examples
///
/// ```
/// use couchfin_api::{Library, Page};
///
/// let page: Page<Library> = serde_json::from_str(
///     r#"{ "Items": [], "TotalRecordCount": 2000, "StartIndex": 100 }"#,
/// )?;
/// assert_eq!(page.total, 2000);
/// assert_eq!(page.start_index, 100);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "RawPage<T>", bound(deserialize = "T: Deserialize<'de>"))]
pub struct Page<T> {
    /// The items on this page.
    pub items: Vec<T>,
    /// How many items the whole list has, so a grid can size its scrollbar.
    /// If the server leaves it out, the number of items on this page.
    pub total: usize,
    /// The position of the first item in the whole list.
    pub start_index: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawPage<T> {
    items: Vec<T>,
    total_record_count: Option<usize>,
    #[serde(default)]
    start_index: usize,
}

impl<T> From<RawPage<T>> for Page<T> {
    fn from(raw: RawPage<T>) -> Self {
        Self {
            total: raw.total_record_count.unwrap_or(raw.items.len()),
            items: raw.items,
            start_index: raw.start_index,
        }
    }
}
