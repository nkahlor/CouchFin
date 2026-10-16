//! Non-secret settings: the device ID, the server and the last username.

use std::path::{Path, PathBuf};

use couchfin_api::{DeviceId, ServerUrl};
use serde::{Deserialize, Serialize};

use crate::config_dir::ConfigDir;
use crate::fs_util::{read_if_exists, write_atomically};

/// What Couchfin remembers between runs, apart from the access token (which
/// lives in a [`TokenStore`](crate::TokenStore)).
///
/// # Examples
///
/// ```
/// use couchfin_core::fake::MemorySettings;
/// use couchfin_core::Settings;
///
/// // The first run creates a device ID; every later run reads the same one.
/// let store = MemorySettings::new();
/// let first = Settings::load_or_create(&store);
/// assert_eq!(Settings::load_or_create(&store).device_id, first.device_id);
/// assert_eq!(first.server, None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Identifies this install to the server, which lists it under
    /// Dashboard > Devices. Created once and never changed, or every launch
    /// would show up as a new device.
    pub device_id: DeviceId,
    /// The server's canonical address (from `check_server`), once one has
    /// been checked.
    pub server: Option<ServerUrl>,
    /// The last username that logged in, to fill in the login form.
    pub username: Option<String>,
}

impl Settings {
    /// Settings for a new install: the given device ID and nothing else.
    #[must_use]
    pub fn new(device_id: DeviceId) -> Self {
        Self {
            device_id,
            server: None,
            username: None,
        }
    }

    /// Loads the stored settings, or on the first run creates them with a new
    /// [`DeviceId`] and saves them.
    ///
    /// It never fails, so a broken settings file can't stop the app
    /// starting. A file that can't be parsed is replaced with fresh settings
    /// (and a new device ID). A file that can't be read is left alone, and
    /// fresh settings are used for this run only. Both are logged as
    /// warnings.
    pub fn load_or_create(store: &impl SettingsStore) -> Self {
        let replace = match store.load() {
            Ok(Some(settings)) => return settings,
            Ok(None) => true,
            Err(error @ SettingsError::Invalid { .. }) => {
                tracing::warn!(%error, "replacing unreadable settings");
                true
            }
            Err(error @ SettingsError::Io { .. }) => {
                tracing::warn!(%error, "using fresh settings for this run");
                false
            }
        };
        let settings = Self::new(DeviceId::generate());
        if replace && let Err(error) = store.save(&settings) {
            tracing::warn!(%error, "couldn't save new settings");
        }
        settings
    }
}

/// Why settings couldn't be loaded or saved. The details are for the log.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SettingsError {
    /// The file couldn't be read or written.
    #[error("couldn't access the settings file: {detail}")]
    Io {
        /// The underlying error.
        detail: String,
    },
    /// The file isn't valid settings TOML.
    #[error("the settings file is invalid: {detail}")]
    Invalid {
        /// What was wrong.
        detail: String,
    },
}

/// Somewhere to keep [`Settings`]: [`FileSettings`] in the app,
/// [`MemorySettings`](crate::fake::MemorySettings) in tests.
pub trait SettingsStore: Send + Sync {
    /// The stored settings, or `None` if nothing has been saved yet.
    ///
    /// # Errors
    ///
    /// [`SettingsError`] if they can't be read or parsed.
    fn load(&self) -> Result<Option<Settings>, SettingsError>;

    /// Replaces the stored settings.
    ///
    /// # Errors
    ///
    /// [`SettingsError::Io`] if they can't be written.
    fn save(&self, settings: &Settings) -> Result<(), SettingsError>;
}

/// Settings in `settings.toml` in a [`ConfigDir`].
///
/// The file holds no secrets. It is replaced atomically on each save.
///
/// # Examples
///
/// ```
/// use couchfin_core::{ConfigDir, FileSettings, Settings, SettingsStore};
///
/// let dir = tempfile::tempdir()?;
/// let store = FileSettings::new(&ConfigDir::new(dir.path()));
/// let mut settings = Settings::load_or_create(&store);
/// settings.server = Some("jellyfin.lan:8096".parse()?);
/// store.save(&settings)?;
///
/// let text = std::fs::read_to_string(store.path())?;
/// assert!(text.contains("server = \"http://jellyfin.lan:8096/\""));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct FileSettings {
    path: PathBuf,
}

impl FileSettings {
    /// Settings in `dir/settings.toml`.
    #[must_use]
    pub fn new(dir: &ConfigDir) -> Self {
        Self {
            path: dir.file("settings.toml"),
        }
    }

    /// The settings file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// The file's layout. Values are validated into [`Settings`] on load.
#[derive(Serialize, Deserialize)]
struct SettingsToml {
    device_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    server: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    username: Option<String>,
}

fn invalid(error: &impl ToString) -> SettingsError {
    SettingsError::Invalid {
        detail: error.to_string(),
    }
}

fn io(error: &std::io::Error) -> SettingsError {
    SettingsError::Io {
        detail: error.to_string(),
    }
}

impl SettingsStore for FileSettings {
    fn load(&self) -> Result<Option<Settings>, SettingsError> {
        let Some(text) = read_if_exists(&self.path).map_err(|e| io(&e))? else {
            return Ok(None);
        };
        let file: SettingsToml = toml::from_str(&text).map_err(|e| invalid(&e))?;
        let server = file
            .server
            .map(|s| s.parse::<ServerUrl>())
            .transpose()
            .map_err(|e| invalid(&e))?;
        Ok(Some(Settings {
            device_id: file.device_id.parse().map_err(|e| invalid(&e))?,
            server,
            username: file.username,
        }))
    }

    fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        let file = SettingsToml {
            device_id: settings.device_id.to_string(),
            server: settings.server.as_ref().map(ToString::to_string),
            username: settings.username.clone(),
        };
        let text = toml::to_string(&file).map_err(|e| invalid(&e))?;
        write_atomically(&self.path, &text, 0o644).map_err(|e| io(&e))
    }
}
