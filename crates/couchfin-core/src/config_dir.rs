//! Where Couchfin keeps its files, injected so tests can use a temporary
//! directory.

use std::path::{Path, PathBuf};

/// The directory holding `settings.toml` and, when there is no keyring,
/// `tokens.toml`.
///
/// The app uses [`ConfigDir::system`]; tests pass a temporary directory to
/// [`ConfigDir::new`]. The directory is created on the first save.
///
/// # Examples
///
/// ```
/// use couchfin_core::ConfigDir;
///
/// let dir = ConfigDir::new("/tmp/couchfin-example");
/// assert_eq!(dir.path(), std::path::Path::new("/tmp/couchfin-example"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDir(PathBuf);

impl ConfigDir {
    /// Uses `path` as the config directory.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// The per-user config directory, `$XDG_CONFIG_HOME/couchfin` (usually
    /// `~/.config/couchfin`). `None` if there is no home directory.
    #[must_use]
    pub fn system() -> Option<Self> {
        directories::ProjectDirs::from("", "", "Couchfin")
            .map(|dirs| Self(dirs.config_dir().to_path_buf()))
    }

    /// The directory itself.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    pub(crate) fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
