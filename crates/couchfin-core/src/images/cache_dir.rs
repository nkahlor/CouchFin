//! Where the image cache keeps its files, injected so tests can use a
//! temporary directory.

use std::path::{Path, PathBuf};

/// The directory holding cached images.
///
/// The app uses [`CacheDir::system`]; tests pass a temporary directory to
/// [`CacheDir::new`]. The directory is created on the first write.
///
/// # Examples
///
/// ```
/// use couchfin_core::CacheDir;
///
/// let dir = CacheDir::new("/tmp/couchfin-images");
/// assert_eq!(dir.path(), std::path::Path::new("/tmp/couchfin-images"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheDir(PathBuf);

impl CacheDir {
    /// Uses `path` as the cache directory.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// The per-user cache directory, `$XDG_CACHE_HOME/couchfin/images`
    /// (usually `~/.cache/couchfin/images`). `None` if there is no home
    /// directory.
    #[must_use]
    pub fn system() -> Option<Self> {
        directories::ProjectDirs::from("", "", "Couchfin")
            .map(|dirs| Self(dirs.cache_dir().join("images")))
    }

    /// The directory itself.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}
