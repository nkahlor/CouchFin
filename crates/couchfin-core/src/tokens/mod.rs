//! Where access tokens are kept: the system keyring, with a private file as
//! the fallback.

mod file;
mod keyring;

pub use file::FileTokens;
pub use keyring::KeyringTokens;

use couchfin_api::{AccessToken, ServerUrl};

use crate::config_dir::ConfigDir;

/// Why a token couldn't be loaded, saved or deleted. The details are for the
/// log and never contain a token.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TokenStoreError {
    /// The store can't be used right now: no D-Bus session or Secret
    /// Service, a locked keyring, or no answer in time. [`FallbackTokens`]
    /// then uses its fallback.
    #[error("the keyring is unavailable: {detail}")]
    Unavailable {
        /// The underlying error.
        detail: String,
    },
    /// The token file couldn't be read or written.
    #[error("couldn't access the token file: {detail}")]
    Io {
        /// The underlying error.
        detail: String,
    },
    /// What is stored isn't a valid token (or token file).
    #[error("the stored token is invalid: {detail}")]
    Invalid {
        /// What was wrong.
        detail: String,
    },
}

/// Keeps one access token per server, keyed by the server's canonical
/// address, so a token is only ever sent back to the server that issued it.
///
/// Implementations: [`KeyringTokens`], [`FileTokens`], [`FallbackTokens`]
/// (one, then the other) and, for tests,
/// [`MemoryTokens`](crate::fake::MemoryTokens). Calls block (on D-Bus or the
/// disk) but are short.
pub trait TokenStore: Send + Sync {
    /// The token stored for `server`, if any.
    ///
    /// # Errors
    ///
    /// [`TokenStoreError`] if the store can't be read.
    fn load(&self, server: &ServerUrl) -> Result<Option<AccessToken>, TokenStoreError>;

    /// Stores `token` for `server`, replacing any earlier one.
    ///
    /// # Errors
    ///
    /// [`TokenStoreError`] if the store can't be written.
    fn save(&self, server: &ServerUrl, token: &AccessToken) -> Result<(), TokenStoreError>;

    /// Forgets the token for `server`. Deleting a token that isn't there
    /// succeeds.
    ///
    /// # Errors
    ///
    /// [`TokenStoreError`] if the store can't be written.
    fn delete(&self, server: &ServerUrl) -> Result<(), TokenStoreError>;
}

/// Uses `primary` (the keyring) and, only when it reports
/// [`TokenStoreError::Unavailable`], `fallback` (the file).
///
/// - `save` writes to the primary and then removes any copy from the
///   fallback, so a token doesn't stay in the file once the keyring works.
///   If the primary is unavailable, it writes to the fallback.
/// - `load` reads the primary, and the fallback when the primary has
///   nothing or is unavailable, so a token saved while the keyring was down
///   is still found later.
/// - `delete` deletes from both, and reports the primary's error, if any,
///   after clearing the fallback.
///
/// Other errors from the primary are returned, not hidden by the fallback.
///
/// # Examples
///
/// ```
/// use couchfin_core::fake::MemoryTokens;
/// use couchfin_core::{
///     AccessToken, ConfigDir, FallbackTokens, FileTokens, TokenStore, TokenStoreError,
/// };
///
/// let dir = tempfile::tempdir()?;
/// let keyring = MemoryTokens::new();
/// keyring.fail_with(TokenStoreError::Unavailable { detail: "no D-Bus".into() });
/// let tokens = FallbackTokens::new(keyring, FileTokens::new(&ConfigDir::new(dir.path())));
///
/// let server = "jellyfin.lan:8096".parse()?;
/// tokens.save(&server, &AccessToken::new("0123456789abcdef")?)?;
/// assert!(dir.path().join("tokens.toml").exists());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct FallbackTokens<P, F> {
    primary: P,
    fallback: F,
}

/// The app's token store: the Secret Service keyring, falling back to
/// `tokens.toml` (mode 0600) in the config directory.
pub type SystemTokens = FallbackTokens<KeyringTokens, FileTokens>;

impl<P: TokenStore, F: TokenStore> FallbackTokens<P, F> {
    /// Tries `primary` first and `fallback` when it is unavailable.
    #[must_use]
    pub fn new(primary: P, fallback: F) -> Self {
        Self { primary, fallback }
    }
}

impl SystemTokens {
    /// The keyring from [`KeyringTokens::secret_service`], with
    /// `dir/tokens.toml` as the fallback. Connecting to D-Bus can take up to
    /// [`KeyringTokens::DEFAULT_TIMEOUT`].
    #[must_use]
    pub fn system(dir: &ConfigDir) -> Self {
        Self::new(KeyringTokens::secret_service(), FileTokens::new(dir))
    }
}

fn log_fallback(error: &TokenStoreError) {
    tracing::warn!(%error, "using the token file instead of the keyring");
}

impl<P: TokenStore, F: TokenStore> TokenStore for FallbackTokens<P, F> {
    fn load(&self, server: &ServerUrl) -> Result<Option<AccessToken>, TokenStoreError> {
        match self.primary.load(server) {
            Ok(Some(token)) => Ok(Some(token)),
            Ok(None) => self.fallback.load(server),
            Err(error @ TokenStoreError::Unavailable { .. }) => {
                log_fallback(&error);
                self.fallback.load(server)
            }
            Err(error) => Err(error),
        }
    }

    fn save(&self, server: &ServerUrl, token: &AccessToken) -> Result<(), TokenStoreError> {
        match self.primary.save(server, token) {
            Ok(()) => {
                if let Err(error) = self.fallback.delete(server) {
                    tracing::warn!(%error, "couldn't remove the old token from the token file");
                }
                Ok(())
            }
            Err(error @ TokenStoreError::Unavailable { .. }) => {
                log_fallback(&error);
                self.fallback.save(server, token)
            }
            Err(error) => Err(error),
        }
    }

    fn delete(&self, server: &ServerUrl) -> Result<(), TokenStoreError> {
        // Clear the file even when the keyring fails, then report the
        // keyring's error: the token may still be in it.
        let primary = self.primary.delete(server);
        self.fallback.delete(server)?;
        primary
    }
}
