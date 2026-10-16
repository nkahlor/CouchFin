//! The token file used when there is no keyring: readable by the user only.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use couchfin_core::{AccessToken, ConfigDir, FileTokens, ServerUrl, TokenStore, TokenStoreError};

fn server() -> ServerUrl {
    "http://jellyfin.lan:8096"
        .parse()
        .expect("test URL is valid")
}

fn token(text: &str) -> AccessToken {
    AccessToken::new(text).expect("test token is valid")
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path)
        .expect("file exists")
        .permissions()
        .mode()
        & 0o777
}

#[test]
fn tokens_round_trip_per_server() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileTokens::new(&ConfigDir::new(dir.path().join("couchfin")));
    let other: ServerUrl = "https://other.example".parse().unwrap();

    assert_eq!(store.load(&server()), Ok(None));
    store.save(&server(), &token("first")).unwrap();
    store.save(&other, &token("second")).unwrap();

    assert_eq!(store.load(&server()), Ok(Some(token("first"))));
    assert_eq!(store.load(&other), Ok(Some(token("second"))));
    assert_eq!(
        store.path(),
        dir.path().join("couchfin").join("tokens.toml")
    );
}

#[test]
fn delete_removes_only_that_servers_token() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileTokens::new(&ConfigDir::new(dir.path()));
    let other: ServerUrl = "https://other.example".parse().unwrap();
    store.save(&server(), &token("first")).unwrap();
    store.save(&other, &token("second")).unwrap();

    store.delete(&server()).unwrap();
    store.delete(&server()).unwrap();

    assert_eq!(store.load(&server()), Ok(None));
    assert_eq!(store.load(&other), Ok(Some(token("second"))));
}

#[test]
fn deleting_without_a_file_is_fine() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileTokens::new(&ConfigDir::new(dir.path()));

    assert_eq!(store.delete(&server()), Ok(()));
    assert!(!store.path().exists());
}

#[test]
fn the_file_is_created_readable_by_the_user_only() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("couchfin");
    let store = FileTokens::new(&ConfigDir::new(&config));

    store.save(&server(), &token("secret")).unwrap();

    assert_eq!(mode(store.path()), 0o600);
    assert_eq!(
        mode(&config),
        0o700,
        "a new config directory is private too"
    );
}

#[test]
fn a_file_with_loose_permissions_is_tightened_when_read() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileTokens::new(&ConfigDir::new(dir.path()));
    store.save(&server(), &token("secret")).unwrap();
    fs::set_permissions(store.path(), fs::Permissions::from_mode(0o644)).unwrap();

    assert_eq!(store.load(&server()), Ok(Some(token("secret"))));

    assert_eq!(mode(store.path()), 0o600);
}

#[test]
fn saving_over_a_file_with_loose_permissions_leaves_it_private() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileTokens::new(&ConfigDir::new(dir.path()));
    fs::write(store.path(), "").unwrap();
    fs::set_permissions(store.path(), fs::Permissions::from_mode(0o666)).unwrap();

    store.save(&server(), &token("secret")).unwrap();

    assert_eq!(mode(store.path()), 0o600);
    assert_eq!(store.load(&server()), Ok(Some(token("secret"))));
}

#[test]
fn a_broken_file_is_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileTokens::new(&ConfigDir::new(dir.path()));
    fs::write(
        store.path(),
        "\"http://jellyfin.lan:8096/\" = \"not a token\"\n",
    )
    .unwrap();

    let error = store.load(&server()).unwrap_err();

    assert!(
        matches!(error, TokenStoreError::Invalid { .. }),
        "{error:?}"
    );
    assert!(
        !error.to_string().contains("not a token"),
        "errors never echo the file's contents: {error}"
    );
}

#[test]
fn an_unwritable_directory_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    // A file where the config directory should be.
    let blocker = dir.path().join("couchfin");
    fs::write(&blocker, "").unwrap();
    let store = FileTokens::new(&ConfigDir::new(&blocker));

    let error = store.save(&server(), &token("secret")).unwrap_err();

    assert!(matches!(error, TokenStoreError::Io { .. }), "{error:?}");
}
