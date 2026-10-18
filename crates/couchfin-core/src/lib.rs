//! App state, navigation stack, settings and image cache.
//!
//! The only Couchfin crate it depends on is `couchfin-api`, and it
//! re-exports the API types the UI needs, so `couchfin-ui` depends on this
//! crate alone. Screens render this state and send actions back.
//!
//! - [`LoginFlow`] runs Phase 1's login: it restores a stored session at
//!   startup, checks the server address, logs in, and persists the session.
//!   Its state is a [`LoginScreen`]; the UI sends [`LoginAction`]s.
//! - [`LibraryFlow`] runs the screens after login: a navigation stack of
//!   Home (rows), a paged library grid and movie and series details. Its
//!   state is a [`LibraryState`]; the UI sends [`LibraryAction`]s, and the
//!   app watches [`LibraryState::session_expired`] to return to sign-in.
//! - [`Settings`] (device ID, server, last username) live in a
//!   [`SettingsStore`]: [`FileSettings`] writes `settings.toml` in a
//!   [`ConfigDir`].
//! - Access tokens live in a [`TokenStore`]: [`SystemTokens`] uses the
//!   system keyring ([`KeyringTokens`]) and falls back to a file only the
//!   user can read ([`FileTokens`], mode 0600).
//! - [`ImageCache`] loads posters: decoded pixels from a disk and memory
//!   cache (decision 016).
//! - [`fake`] has in-memory stores and the fake Jellyfin server, for tests.
//!
//! The designs are recorded in `docs/decisions/007-session-persistence.md`
//! and `docs/decisions/017-library-state.md`.
//!
//! # Examples
//!
//! How the app wires it together (not run here: it opens the real keyring):
//!
//! ```no_run
//! use std::sync::Arc;
//! use couchfin_api::JellyfinClient;
//! use couchfin_core::{ConfigDir, FileSettings, LoginFlow, Settings, SystemTokens};
//!
//! let dir = ConfigDir::system().ok_or("no home directory")?;
//! let settings_file = FileSettings::new(&dir);
//! let settings = Settings::load_or_create(&settings_file);
//! let api = JellyfinClient::new(settings.device_id.clone())?;
//! let flow = Arc::new(
//!     LoginFlow::new(api, settings, settings_file, SystemTokens::system(&dir))
//!         .on_change(|screen| println!("now showing {screen:?}")),
//! );
//!
//! let runtime = tokio::runtime::Runtime::new()?;
//! runtime.spawn({
//!     let flow = Arc::clone(&flow);
//!     async move { flow.start().await }
//! });
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod config_dir;
pub mod fake;
mod fs_util;
mod images;
mod library;
mod login;
mod settings;
mod tokens;

pub use config_dir::ConfigDir;
pub use images::{
    CacheDir, ImageCache, ImageCacheConfig, ImageError, ImageLoad, ImageShape, ImageSize, Pixels,
    placeholder,
};
pub use library::{
    Card, CardShape, Episode, GridState, Header, HomeState, ImageRef, LibraryAction, LibraryFlow,
    LibraryState, MovieState, Row, RowKind, ScreenState, Season, SeriesState, Target,
};
pub use login::{
    EnterAddress, Home, LogIn, LoginAction, LoginError, LoginFlow, LoginScreen, Request,
    RestoreStatus, Restoring,
};
pub use settings::{FileSettings, Settings, SettingsError, SettingsStore};
pub use tokens::{
    FallbackTokens, FileTokens, KeyringTokens, SystemTokens, TokenStore, TokenStoreError,
};

pub use couchfin_api::{
    AccessToken, ApiError, DeviceId, JellyfinApi, JellyfinClient, Password, PublicSystemInfo,
    ServerUrl, ServerUrlError, Session, User, UserId,
};
