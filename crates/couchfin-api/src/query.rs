//! The request for one page of items.

use crate::ids::ItemId;
use crate::item::ItemKind;

/// Which items to list. Results are always sorted by name.
///
/// The default lists everything directly under no parent, from the first
/// item, without a limit; build up from there.
///
/// # Examples
///
/// The first 50 movies and series of a library, however deep they sit:
///
/// ```
/// use couchfin_api::{ItemKind, ItemsQuery};
///
/// let query = ItemsQuery::default()
///     .in_parent("abc".parse()?)
///     .of_kinds([ItemKind::Movie, ItemKind::Series])
///     .recursive()
///     .page(0, 50);
/// assert_eq!(query.limit, Some(50));
/// # Ok::<(), couchfin_api::InvalidValueError>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemsQuery {
    /// List the children of this library or folder.
    pub parent: Option<ItemId>,
    /// Only these kinds; empty means every kind.
    pub kinds: Vec<ItemKind>,
    /// Include descendants, not just direct children.
    pub recursive: bool,
    /// How many items to skip.
    pub start_index: usize,
    /// The most items to return; `None` means all of them.
    pub limit: Option<usize>,
}

impl ItemsQuery {
    /// Lists the children of `parent`.
    #[must_use]
    pub fn in_parent(mut self, parent: ItemId) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Keeps only items of these kinds.
    #[must_use]
    pub fn of_kinds(mut self, kinds: impl IntoIterator<Item = ItemKind>) -> Self {
        self.kinds = kinds.into_iter().collect();
        self
    }

    /// Includes descendants.
    #[must_use]
    pub fn recursive(mut self) -> Self {
        self.recursive = true;
        self
    }

    /// Skips `start_index` items and returns at most `limit`.
    #[must_use]
    pub fn page(mut self, start_index: usize, limit: usize) -> Self {
        self.start_index = start_index;
        self.limit = Some(limit);
        self
    }
}
