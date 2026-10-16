//! Non-secret settings: the TOML file under the config directory, and the
//! in-memory fake.

use std::fs;

use couchfin_core::fake::MemorySettings;
use couchfin_core::{ConfigDir, FileSettings, Settings, SettingsError, SettingsStore};

fn settings() -> Settings {
    let mut settings = Settings::new(
        "3f2b9c1e-8d4a-4b6f-9e2d-7a1c5b8e0f42"
            .parse()
            .expect("valid device ID"),
    );
    settings.server = Some("https://jellyfin.lan/jellyfin".parse().expect("valid URL"));
    settings.username = Some("alice".to_owned());
    settings
}

#[test]
fn the_file_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileSettings::new(&ConfigDir::new(dir.path().join("couchfin")));

    assert_eq!(store.load(), Ok(None));
    store.save(&settings()).unwrap();

    assert_eq!(store.load(), Ok(Some(settings())));
    assert_eq!(
        store.path(),
        dir.path().join("couchfin").join("settings.toml")
    );
}

#[test]
fn the_file_is_plain_toml_without_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileSettings::new(&ConfigDir::new(dir.path()));

    store.save(&settings()).unwrap();

    let text = fs::read_to_string(store.path()).unwrap();
    let table: toml::Table = text.parse().unwrap();
    assert_eq!(
        table["device_id"].as_str(),
        Some("3f2b9c1e-8d4a-4b6f-9e2d-7a1c5b8e0f42")
    );
    assert_eq!(
        table["server"].as_str(),
        Some("https://jellyfin.lan/jellyfin/")
    );
    assert_eq!(table["username"].as_str(), Some("alice"));
    assert_eq!(table.len(), 3);
}

#[test]
fn settings_without_a_server_leave_it_out() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileSettings::new(&ConfigDir::new(dir.path()));
    let fresh = Settings::new("device-1".parse().unwrap());

    store.save(&fresh).unwrap();

    assert_eq!(store.load(), Ok(Some(fresh)));
}

#[test]
fn a_broken_file_is_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileSettings::new(&ConfigDir::new(dir.path()));
    fs::write(store.path(), "device_id = \"has space\"\n").unwrap();

    assert!(
        matches!(store.load(), Err(SettingsError::Invalid { .. })),
        "{:?}",
        store.load()
    );
}

#[test]
fn the_device_id_is_created_on_first_run_and_kept_after() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileSettings::new(&ConfigDir::new(dir.path().join("new")));

    let first = Settings::load_or_create(&store);
    let second =
        Settings::load_or_create(&FileSettings::new(&ConfigDir::new(dir.path().join("new"))));

    assert_eq!(first.device_id, second.device_id);
    assert_eq!(first.server, None);
}

#[test]
fn a_broken_file_is_replaced_with_fresh_settings() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileSettings::new(&ConfigDir::new(dir.path()));
    fs::write(store.path(), "not = [toml").unwrap();

    let created = Settings::load_or_create(&store);

    assert_eq!(store.load(), Ok(Some(created)));
}

#[test]
fn an_unreadable_store_still_gives_settings() {
    let store = MemorySettings::new();
    store.fail_with(SettingsError::Io {
        detail: "disk on fire".into(),
    });

    let settings = Settings::load_or_create(&store);

    assert_eq!(settings.server, None);
    store.clear_failure();
    assert_eq!(store.load(), Ok(None), "an I/O error overwrites nothing");
}

#[test]
fn the_memory_store_round_trips_and_shares_state_between_clones() {
    let store = MemorySettings::new();
    let handle = store.clone();

    store.save(&settings()).unwrap();

    assert_eq!(handle.load(), Ok(Some(settings())));
}

#[test]
fn the_system_config_dir_is_named_couchfin() {
    // Only checks the name: the parent comes from $XDG_CONFIG_HOME or $HOME.
    if let Some(dir) = ConfigDir::system() {
        assert_eq!(dir.path().file_name().unwrap(), "couchfin");
    }
}
