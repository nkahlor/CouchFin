//! In-memory stand-ins for testing code that uses couchfin-core without a
//! disk, a keyring or a server. Always compiled, like
//! [`couchfin_api::fake`], so couchfin-ui and the app can use them without
//! feature plumbing.
//!
//! Clones of each fake share state, so a test keeps a handle to inspect or
//! script it while the code under test owns another.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

pub use couchfin_api::fake::FakeJellyfin;
use couchfin_api::{AccessToken, ServerUrl};

use crate::settings::{Settings, SettingsError, SettingsStore};
use crate::tokens::{TokenStore, TokenStoreError};

/// A test's state stays usable after a panic elsewhere in the test.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Settings kept in memory, empty at first.
///
/// # Examples
///
/// ```
/// use couchfin_core::fake::MemorySettings;
/// use couchfin_core::{Settings, SettingsError, SettingsStore};
///
/// let store = MemorySettings::new();
/// let settings = Settings::load_or_create(&store);
/// assert_eq!(store.load(), Ok(Some(settings)));
///
/// store.fail_with(SettingsError::Io { detail: "read-only".into() });
/// assert!(store.load().is_err());
/// ```
#[derive(Debug, Clone, Default)]
pub struct MemorySettings {
    state: Arc<Mutex<(Option<Settings>, Option<SettingsError>)>>,
}

impl MemorySettings {
    /// A store with nothing saved yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes every call fail with `error` until
    /// [`MemorySettings::clear_failure`].
    pub fn fail_with(&self, error: SettingsError) {
        lock(&self.state).1 = Some(error);
    }

    /// Stops failing calls.
    pub fn clear_failure(&self) {
        lock(&self.state).1 = None;
    }
}

impl SettingsStore for MemorySettings {
    fn load(&self) -> Result<Option<Settings>, SettingsError> {
        let state = lock(&self.state);
        match &state.1 {
            Some(error) => Err(error.clone()),
            None => Ok(state.0.clone()),
        }
    }

    fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        let mut state = lock(&self.state);
        if let Some(error) = &state.1 {
            return Err(error.clone());
        }
        state.0 = Some(settings.clone());
        Ok(())
    }
}

/// Tokens kept in memory, standing in for the keyring.
///
/// # Examples
///
/// ```
/// use couchfin_core::fake::MemoryTokens;
/// use couchfin_core::{AccessToken, TokenStore, TokenStoreError};
///
/// let tokens = MemoryTokens::new();
/// let server = "jellyfin.lan:8096".parse()?;
/// tokens.save(&server, &AccessToken::new("0123456789abcdef")?)?;
/// assert!(tokens.load(&server)?.is_some());
///
/// // Behave like a keyring that can't be reached.
/// tokens.fail_with(TokenStoreError::Unavailable { detail: "no D-Bus".into() });
/// assert!(tokens.load(&server).is_err());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default)]
pub struct MemoryTokens {
    state: Arc<Mutex<TokensState>>,
}

#[derive(Debug, Default)]
struct TokensState {
    tokens: BTreeMap<String, AccessToken>,
    failure: Option<TokenStoreError>,
}

impl MemoryTokens {
    /// A store with no tokens.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes every call fail with `error` until
    /// [`MemoryTokens::clear_failure`].
    pub fn fail_with(&self, error: TokenStoreError) {
        lock(&self.state).failure = Some(error);
    }

    /// Stops failing calls.
    pub fn clear_failure(&self) {
        lock(&self.state).failure = None;
    }

    fn answer(&self) -> Result<MutexGuard<'_, TokensState>, TokenStoreError> {
        let state = lock(&self.state);
        match &state.failure {
            Some(error) => Err(error.clone()),
            None => Ok(state),
        }
    }
}

impl TokenStore for MemoryTokens {
    fn load(&self, server: &ServerUrl) -> Result<Option<AccessToken>, TokenStoreError> {
        Ok(self.answer()?.tokens.get(server.as_str()).cloned())
    }

    fn save(&self, server: &ServerUrl, token: &AccessToken) -> Result<(), TokenStoreError> {
        self.answer()?
            .tokens
            .insert(server.as_str().to_owned(), token.clone());
        Ok(())
    }

    fn delete(&self, server: &ServerUrl) -> Result<(), TokenStoreError> {
        self.answer()?.tokens.remove(server.as_str());
        Ok(())
    }
}
