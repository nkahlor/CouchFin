//! Checks that every crate gets the workspace lints, and that `couchfin-player`
//! is the only crate where `unsafe` is possible.
//!
//! Cargo can't override one inherited lint, so `couchfin-player` carries its
//! own copy of the table. This test keeps that copy in step with the workspace.

use std::fs;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

const CRATES: &[&str] = &[
    "app",
    "crates/couchfin-api",
    "crates/couchfin-core",
    "crates/couchfin-input",
    "crates/couchfin-ui",
];
const PLAYER: &str = "crates/couchfin-player";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("app has a parent directory")
        .to_path_buf()
}

fn manifest(dir: &str) -> Table {
    let path = workspace_root().join(dir).join("Cargo.toml");
    let text = fs::read_to_string(&path).expect("manifest is readable");
    text.parse().expect("manifest is valid TOML")
}

fn workspace_lints() -> Table {
    manifest(".")
        .get("workspace")
        .and_then(|w| w.get("lints"))
        .and_then(Value::as_table)
        .expect("root Cargo.toml has [workspace.lints]")
        .clone()
}

fn level(lints: &Table, tool: &str, lint: &str) -> Option<String> {
    match lints.get(tool)?.get(lint)? {
        Value::String(level) => Some(level.clone()),
        Value::Table(table) => table.get("level")?.as_str().map(str::to_owned),
        _ => None,
    }
}

#[test]
fn workspace_lints_match_the_plan() {
    let lints = workspace_lints();
    assert_eq!(
        level(&lints, "rust", "unsafe_code").as_deref(),
        Some("forbid")
    );
    assert_eq!(
        level(&lints, "rust", "missing_docs").as_deref(),
        Some("warn")
    );
    assert_eq!(level(&lints, "clippy", "pedantic").as_deref(), Some("warn"));
    assert_eq!(
        level(&lints, "clippy", "unwrap_used").as_deref(),
        Some("deny")
    );
}

#[test]
fn every_crate_but_the_player_inherits_the_workspace_lints() {
    for dir in CRATES {
        let lints = manifest(dir).get("lints").cloned();
        assert_eq!(
            lints.and_then(|l| l.get("workspace").and_then(Value::as_bool)),
            Some(true),
            "{dir}/Cargo.toml needs `[lints] workspace = true`"
        );
    }
}

#[test]
fn player_lints_differ_from_the_workspace_only_in_allowing_unsafe() {
    let player = manifest(PLAYER)
        .get("lints")
        .and_then(Value::as_table)
        .cloned()
        .expect("couchfin-player has its own [lints] table");
    assert_eq!(
        level(&player, "rust", "unsafe_code").as_deref(),
        Some("deny")
    );

    let mut expected = workspace_lints();
    expected
        .get_mut("rust")
        .and_then(Value::as_table_mut)
        .expect("workspace has rust lints")
        .insert("unsafe_code".into(), Value::String("deny".into()));
    assert_eq!(
        player, expected,
        "couchfin-player's [lints] drifted from [workspace.lints]"
    );
}
