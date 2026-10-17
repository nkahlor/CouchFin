//! The endpoints' side of the fake catalog: reading and paging what the
//! builders added.

use crate::error::ApiError;
use crate::ids::ItemId;
use crate::image::{Image, ImageKind, ImageRequest};
use crate::item::{Item, ItemKind};
use crate::library::{Library, Page};
use crate::query::ItemsQuery;

use super::{Catalog, Entry, PLACEHOLDER_PNG};

// Reading, as the endpoints do.
impl Catalog {
    pub(in crate::fake) fn libraries(&self) -> Vec<Library> {
        self.libraries.clone()
    }

    fn entry(&self, id: &ItemId) -> Option<&Entry> {
        self.entries.iter().find(|e| e.item.id == *id)
    }

    /// Whether `ancestor` is the parent of `entry`, or the parent of its
    /// parent, and so on.
    fn within(&self, entry: &Entry, ancestor: &ItemId) -> bool {
        let mut parent = &entry.parent;
        loop {
            if parent == ancestor {
                return true;
            }
            match self.entry(parent) {
                Some(next) => parent = &next.parent,
                None => return false,
            }
        }
    }

    pub(in crate::fake) fn page(&self, query: &ItemsQuery) -> Page<Item> {
        let wanted = |entry: &Entry| {
            let kind_ok = query.kinds.is_empty() || query.kinds.contains(&entry.item.kind);
            let place_ok = match &query.parent {
                Some(parent) if query.recursive => self.within(entry, parent),
                Some(parent) => entry.parent == *parent,
                // Without a parent the server lists the user's root folders,
                // which the fake doesn't model; recursive lists everything.
                None => query.recursive,
            };
            kind_ok && place_ok
        };
        let mut matching: Vec<&Item> = self
            .entries
            .iter()
            .filter(|e| wanted(e))
            .map(|e| &e.item)
            .collect();
        // Jellyfin sorts names case-insensitively; ties keep their order.
        matching.sort_by_cached_key(|item| item.name.to_lowercase());
        Page {
            total: matching.len(),
            start_index: query.start_index,
            items: matching
                .into_iter()
                .skip(query.start_index)
                .take(query.limit.unwrap_or(usize::MAX))
                .cloned()
                .collect(),
        }
    }

    pub(in crate::fake) fn item(&self, id: &ItemId) -> Result<Item, ApiError> {
        self.entry(id)
            .map(|e| e.item.clone())
            .ok_or(ApiError::ItemNotFound)
    }

    /// The scripted ids, as items, at most `limit` of them.
    fn listed(&self, ids: &[ItemId], limit: u32) -> Vec<Item> {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        ids.iter()
            .filter_map(|id| self.entry(id))
            .map(|e| e.item.clone())
            .take(limit)
            .collect()
    }

    pub(in crate::fake) fn resume(&self, limit: u32) -> Vec<Item> {
        self.listed(&self.resume, limit)
    }

    pub(in crate::fake) fn next_up(&self, limit: u32) -> Vec<Item> {
        self.listed(&self.next_up, limit)
    }

    pub(in crate::fake) fn latest(&self, parent: &ItemId, limit: u32) -> Vec<Item> {
        let limit = usize::try_from(limit).unwrap_or(usize::MAX);
        self.entries
            .iter()
            .rev()
            .filter(|e| matches!(e.item.kind, ItemKind::Movie | ItemKind::Series))
            .filter(|e| self.within(e, parent))
            .map(|e| e.item.clone())
            .take(limit)
            .collect()
    }

    fn children(&self, parent: &ItemId, kind: &ItemKind) -> Vec<Item> {
        self.entries
            .iter()
            .filter(|e| e.parent == *parent && e.item.kind == *kind)
            .map(|e| e.item.clone())
            .collect()
    }

    fn require_series(&self, series: &ItemId) -> Result<(), ApiError> {
        match self.entry(series) {
            Some(e) if e.item.kind == ItemKind::Series => Ok(()),
            _ => Err(ApiError::ItemNotFound),
        }
    }

    pub(in crate::fake) fn seasons(&self, series: &ItemId) -> Result<Vec<Item>, ApiError> {
        self.require_series(series)?;
        Ok(self.children(series, &ItemKind::Season))
    }

    pub(in crate::fake) fn episodes(
        &self,
        series: &ItemId,
        season: &ItemId,
    ) -> Result<Vec<Item>, ApiError> {
        self.require_series(series)?;
        // A season of another series has no episodes here.
        let belongs = self.entry(season).is_some_and(|e| e.parent == *series);
        Ok(if belongs {
            self.children(season, &ItemKind::Episode)
        } else {
            Vec::new()
        })
    }

    pub(in crate::fake) fn image(&self, request: &ImageRequest) -> Result<Image, ApiError> {
        let has_image = if let Some(entry) = self.entry(&request.item) {
            let tags = &entry.item.image_tags;
            match request.kind {
                ImageKind::Primary => tags.primary.is_some(),
                ImageKind::Thumb => tags.thumb.is_some(),
                ImageKind::Backdrop => tags.backdrop.is_some(),
            }
        } else {
            request.kind == ImageKind::Primary
                && self.libraries.iter().any(|l| l.id == request.item)
        };
        if !has_image {
            return Err(ApiError::ItemNotFound);
        }
        Ok(Image {
            bytes: PLACEHOLDER_PNG.to_vec(),
            content_type: "image/png".to_owned(),
        })
    }
}
