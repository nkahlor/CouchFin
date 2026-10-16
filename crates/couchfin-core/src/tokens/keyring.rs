//! The system keyring (Secret Service over D-Bus), through keyring-core.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use couchfin_api::{AccessToken, ServerUrl};
use keyring_core::{CredentialStore, Entry, Error};

use super::{TokenStore, TokenStoreError};

/// Tokens in a keyring-core credential store: on Linux the Secret Service
/// (GNOME Keyring, `KWallet`), reached over D-Bus by a pure-Rust client.
///
/// Each token is an entry with service [`KeyringTokens::SERVICE`] and the
/// server's canonical address as the user name.
///
/// Every call runs on its own thread and gives up after a timeout
/// ([`KeyringTokens::DEFAULT_TIMEOUT`]), because a locked keyring can wait
/// forever on an unlock prompt that nobody sees (under gamescope, for one).
/// After a timeout the store reports [`TokenStoreError::Unavailable`] at once
/// for the rest of the run, instead of waiting again.
///
/// # Examples
///
/// keyring-core's mock store stands in for the Secret Service here:
///
/// ```
/// use couchfin_core::{AccessToken, KeyringTokens, TokenStore};
///
/// let tokens = KeyringTokens::new(keyring_core::mock::Store::new()?);
/// let server = "jellyfin.lan:8096".parse()?;
/// tokens.save(&server, &AccessToken::new("0123456789abcdef")?)?;
/// assert!(tokens.load(&server)?.is_some());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct KeyringTokens {
    store: Result<Arc<CredentialStore>, String>,
    timeout: Duration,
    timed_out: Arc<AtomicBool>,
}

impl KeyringTokens {
    /// The keyring service name every Couchfin token is stored under.
    pub const SERVICE: &str = "Couchfin";

    /// How long one keyring call may take before the keyring is treated as
    /// unavailable. D-Bus calls normally take milliseconds.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

    /// Tokens in `store`.
    #[must_use]
    pub fn new(store: Arc<CredentialStore>) -> Self {
        Self {
            store: Ok(store),
            timeout: Self::DEFAULT_TIMEOUT,
            timed_out: Arc::new(AtomicBool::new(false)),
        }
    }

    /// A store whose every call fails with [`TokenStoreError::Unavailable`]
    /// carrying `detail`, for when no keyring could be opened.
    #[must_use]
    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self {
            store: Err(detail.into()),
            timeout: Self::DEFAULT_TIMEOUT,
            timed_out: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Connects to the Secret Service on the D-Bus session bus. If that fails
    /// (no session bus, no Secret Service, which is common under gamescope
    /// or in a container) or takes longer than the timeout, the result is
    /// [`KeyringTokens::unavailable`], and the failure is logged.
    #[must_use]
    pub fn secret_service() -> Self {
        let connect =
            || zbus_secret_service_keyring_store::Store::new().map(|s| s as Arc<CredentialStore>);
        match run_with_timeout(Self::DEFAULT_TIMEOUT, connect) {
            Ok(store) => Self::new(store),
            Err(error) => {
                tracing::warn!(%error, "no system keyring; tokens will go to the token file");
                Self::unavailable(error.to_string())
            }
        }
    }

    /// Changes how long a call may take.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Runs `op` on the entry for `server`, on another thread, with the
    /// timeout.
    fn run<R: Send + 'static>(
        &self,
        server: &ServerUrl,
        op: impl FnOnce(&Entry) -> Result<R, Error> + Send + 'static,
    ) -> Result<R, TokenStoreError> {
        let store = self.store.as_ref().map_err(unavailable)?;
        if self.timed_out.load(Ordering::Relaxed) {
            return Err(unavailable("an earlier call timed out"));
        }
        let store = Arc::clone(store);
        let user = server.as_str().to_owned();
        let result = run_with_timeout(self.timeout, move || {
            op(&store.build(Self::SERVICE, &user, None)?)
        });
        if let Err(KeyringFailure::TimedOut) = result {
            self.timed_out.store(true, Ordering::Relaxed);
        }
        result.map_err(KeyringFailure::into_store_error)
    }
}

impl fmt::Debug for KeyringTokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let store = match &self.store {
            Ok(store) => store.vendor(),
            Err(detail) => format!("unavailable: {detail}"),
        };
        f.debug_struct("KeyringTokens")
            .field("store", &store)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

fn unavailable(detail: impl Into<String>) -> TokenStoreError {
    TokenStoreError::Unavailable {
        detail: detail.into(),
    }
}

/// Why a keyring call on the worker thread didn't produce a value.
#[derive(Debug, thiserror::Error)]
enum KeyringFailure {
    #[error("the keyring didn't answer in time")]
    TimedOut,
    #[error("couldn't start a keyring thread: {0}")]
    Thread(String),
    #[error(transparent)]
    Keyring(#[from] Error),
}

impl KeyringFailure {
    /// A value that isn't a token is `Invalid`. Anything else (no access, a
    /// locked collection, D-Bus or platform failures, a timeout) means the
    /// keyring can't be used now, so the caller may fall back.
    fn into_store_error(self) -> TokenStoreError {
        match self {
            Self::Keyring(Error::BadEncoding(_) | Error::BadDataFormat(..)) => {
                TokenStoreError::Invalid {
                    detail: "the keyring entry isn't text".to_owned(),
                }
            }
            other => unavailable(other.to_string()),
        }
    }
}

/// Runs `op` on a new thread and waits up to `timeout` for it. A thread that
/// never finishes is left behind; it holds nothing but its own connection.
fn run_with_timeout<R: Send + 'static>(
    timeout: Duration,
    op: impl FnOnce() -> Result<R, Error> + Send + 'static,
) -> Result<R, KeyringFailure> {
    let (send, receive) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("couchfin-keyring".to_owned())
        .spawn(move || {
            // The receiver is gone if the caller already gave up.
            let _ = send.send(op());
        })
        .map_err(|e| KeyringFailure::Thread(e.to_string()))?;
    match receive.recv_timeout(timeout) {
        Ok(result) => result.map_err(KeyringFailure::from),
        Err(_) => Err(KeyringFailure::TimedOut),
    }
}

impl TokenStore for KeyringTokens {
    fn load(&self, server: &ServerUrl) -> Result<Option<AccessToken>, TokenStoreError> {
        let password = self.run(server, |entry| match entry.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(Error::NoEntry) => Ok(None),
            Err(error) => Err(error),
        })?;
        password
            .map(AccessToken::new)
            .transpose()
            .map_err(|e| TokenStoreError::Invalid {
                detail: e.to_string(),
            })
    }

    fn save(&self, server: &ServerUrl, token: &AccessToken) -> Result<(), TokenStoreError> {
        let token = token.expose_secret().to_owned();
        self.run(server, move |entry| entry.set_password(&token))
    }

    fn delete(&self, server: &ServerUrl) -> Result<(), TokenStoreError> {
        self.run(server, |entry| match entry.delete_credential() {
            Err(Error::NoEntry) => Ok(()),
            other => other,
        })
    }
}
