//! Keyring first, the 0600 file when the keyring is unavailable.

use std::os::unix::fs::PermissionsExt;

use couchfin_core::fake::MemoryTokens;
use couchfin_core::{
    AccessToken, ConfigDir, FallbackTokens, FileTokens, ServerUrl, TokenStore, TokenStoreError,
};

fn server() -> ServerUrl {
    "http://jellyfin.lan:8096"
        .parse()
        .expect("test URL is valid")
}

fn token(text: &str) -> AccessToken {
    AccessToken::new(text).expect("test token is valid")
}

fn unavailable() -> TokenStoreError {
    TokenStoreError::Unavailable {
        detail: "no Secret Service".into(),
    }
}

struct Stores {
    keyring: MemoryTokens,
    file: FileTokens,
    both: FallbackTokens<MemoryTokens, FileTokens>,
    _dir: tempfile::TempDir,
}

fn stores() -> Stores {
    let dir = tempfile::tempdir().expect("temp dir");
    let keyring = MemoryTokens::new();
    let file = FileTokens::new(&ConfigDir::new(dir.path()));
    let both = FallbackTokens::new(keyring.clone(), file.clone());
    Stores {
        keyring,
        file,
        both,
        _dir: dir,
    }
}

#[test]
fn a_working_keyring_is_used_and_no_file_is_written() {
    let s = stores();

    s.both.save(&server(), &token("secret")).unwrap();

    assert_eq!(s.keyring.load(&server()), Ok(Some(token("secret"))));
    assert!(!s.file.path().exists());
    assert_eq!(s.both.load(&server()), Ok(Some(token("secret"))));
}

#[test]
fn an_unavailable_keyring_falls_back_to_the_private_file() {
    let s = stores();
    s.keyring.fail_with(unavailable());

    s.both.save(&server(), &token("secret")).unwrap();

    assert_eq!(s.file.load(&server()), Ok(Some(token("secret"))));
    let mode = std::fs::metadata(s.file.path())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(s.both.load(&server()), Ok(Some(token("secret"))));
}

#[test]
fn a_token_saved_to_the_file_is_found_once_the_keyring_works_again() {
    let s = stores();
    s.keyring.fail_with(unavailable());
    s.both.save(&server(), &token("secret")).unwrap();

    s.keyring.clear_failure();

    assert_eq!(s.both.load(&server()), Ok(Some(token("secret"))));
}

#[test]
fn saving_to_the_keyring_removes_an_older_copy_from_the_file() {
    let s = stores();
    s.file.save(&server(), &token("old")).unwrap();

    s.both.save(&server(), &token("new")).unwrap();

    assert_eq!(s.file.load(&server()), Ok(None));
    assert_eq!(s.both.load(&server()), Ok(Some(token("new"))));
}

#[test]
fn delete_removes_the_token_from_both() {
    let s = stores();
    s.keyring.save(&server(), &token("a")).unwrap();
    s.file.save(&server(), &token("b")).unwrap();

    s.both.delete(&server()).unwrap();

    assert_eq!(s.keyring.load(&server()), Ok(None));
    assert_eq!(s.file.load(&server()), Ok(None));
}

#[test]
fn delete_clears_the_file_and_reports_an_unavailable_keyring() {
    let s = stores();
    s.file.save(&server(), &token("b")).unwrap();
    s.keyring.fail_with(unavailable());

    // The token may still be in the keyring, so the caller hears about it.
    assert_eq!(s.both.delete(&server()), Err(unavailable()));

    assert_eq!(s.file.load(&server()), Ok(None));
}

#[test]
fn other_keyring_errors_are_reported_rather_than_hidden() {
    let s = stores();
    let broken = TokenStoreError::Invalid {
        detail: "not a token".into(),
    };
    s.keyring.fail_with(broken.clone());

    assert_eq!(s.both.load(&server()), Err(broken.clone()));
    assert_eq!(s.both.save(&server(), &token("x")), Err(broken));
    assert!(!s.file.path().exists());
}
