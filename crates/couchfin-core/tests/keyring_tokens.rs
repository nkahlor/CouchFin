//! The keyring-backed token store, run against keyring-core's in-memory mock
//! store and a store that hangs, never the real system keyring.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use couchfin_core::{AccessToken, KeyringTokens, ServerUrl, TokenStore, TokenStoreError};
use keyring_core::api::CredentialStoreApi;
use keyring_core::{CredentialStore, Entry, Error, mock};

fn server() -> ServerUrl {
    "http://jellyfin.lan:8096"
        .parse()
        .expect("test URL is valid")
}

fn token() -> AccessToken {
    AccessToken::new("0123456789abcdef0123456789abcdef").expect("test token is valid")
}

fn mock_store() -> Arc<CredentialStore> {
    mock::Store::new().expect("mock store builds")
}

/// The mock credential the store uses for `server`, to script errors on it.
fn credential(store: &Arc<CredentialStore>, server: &ServerUrl) -> Entry {
    store
        .build(KeyringTokens::SERVICE, server.as_str(), None)
        .expect("mock store builds entries")
}

fn fail_next(entry: &Entry, error: Error) {
    let mock: &mock::Cred = entry
        .as_any()
        .downcast_ref()
        .expect("entry is a mock credential");
    mock.set_error(error);
}

#[test]
fn tokens_round_trip_under_the_couchfin_service_and_the_server_address() {
    let store = mock_store();
    let tokens = KeyringTokens::new(Arc::clone(&store));

    assert_eq!(tokens.load(&server()), Ok(None));
    tokens.save(&server(), &token()).unwrap();

    assert_eq!(tokens.load(&server()), Ok(Some(token())));
    assert_eq!(
        credential(&store, &server()).get_password().unwrap(),
        token().expose_secret()
    );

    tokens.delete(&server()).unwrap();
    assert_eq!(tokens.load(&server()), Ok(None));
    assert_eq!(tokens.delete(&server()), Ok(()), "deleting twice is fine");
}

#[test]
fn a_locked_keyring_is_unavailable() {
    let store = mock_store();
    let tokens = KeyringTokens::new(Arc::clone(&store));
    let locked = || Error::NoStorageAccess(Box::new(std::io::Error::other("locked")));

    fail_next(&credential(&store, &server()), locked());
    let load = tokens.load(&server());
    fail_next(&credential(&store, &server()), locked());
    let save = tokens.save(&server(), &token());

    assert!(
        matches!(load, Err(TokenStoreError::Unavailable { .. })),
        "{load:?}"
    );
    assert!(
        matches!(save, Err(TokenStoreError::Unavailable { .. })),
        "{save:?}"
    );
}

#[test]
fn a_stored_value_that_is_not_a_token_is_invalid() {
    let store = mock_store();
    credential(&store, &server())
        .set_password("not a token")
        .unwrap();

    let load = KeyringTokens::new(store).load(&server());

    assert!(
        matches!(load, Err(TokenStoreError::Invalid { .. })),
        "{load:?}"
    );
}

#[test]
fn no_store_at_all_is_unavailable() {
    let tokens = KeyringTokens::unavailable("no D-Bus session bus");

    let load = tokens.load(&server());

    assert_eq!(
        load,
        Err(TokenStoreError::Unavailable {
            detail: "no D-Bus session bus".into()
        })
    );
}

/// A store whose every call hangs, like a Secret Service waiting on an
/// unlock prompt that nobody can see under gamescope.
#[derive(Debug)]
struct Hanging;

impl CredentialStoreApi for Hanging {
    fn vendor(&self) -> String {
        "hanging".into()
    }

    fn id(&self) -> String {
        "hanging".into()
    }

    fn build(
        &self,
        _service: &str,
        _user: &str,
        _modifiers: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<Entry> {
        std::thread::sleep(Duration::from_secs(60));
        Err(Error::NoEntry)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn a_hanging_keyring_times_out_and_is_not_asked_again() {
    let tokens = KeyringTokens::new(Arc::new(Hanging)).with_timeout(Duration::from_millis(50));

    let first = tokens.load(&server());
    let started = Instant::now();
    let second = tokens.save(&server(), &token());

    assert!(
        matches!(first, Err(TokenStoreError::Unavailable { .. })),
        "{first:?}"
    );
    assert!(
        matches!(second, Err(TokenStoreError::Unavailable { .. })),
        "{second:?}"
    );
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "the second call doesn't wait again"
    );
}

#[test]
fn debug_output_names_no_token() {
    let tokens = KeyringTokens::new(mock_store());
    tokens.save(&server(), &token()).unwrap();

    assert!(!format!("{tokens:?}").contains(token().expose_secret()));
}
