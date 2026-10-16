//! Checks that every crate gets the workspace lints, and that `couchfin-player`
//! is the only crate where handwritten `unsafe` is possible.
//!
//! Cargo can't override one inherited lint, so `couchfin-player` and
//! `couchfin-ui` carry their own copies of the table with `unsafe_code`
//! lowered to `deny`. The player needs that for libmpv FFI; the UI only because
//! Slint's generated code has `#[allow(unsafe_code)]`, which `forbid` rejects
//! (docs/decisions/005-slint-generated-unsafe.md). This test keeps both copies
//! in step with the workspace and stops the UI's own code opting in.

use std::fs;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

const CRATES: &[&str] = &[
    "app",
    "crates/couchfin-api",
    "crates/couchfin-core",
    "crates/couchfin-input",
];
/// Crates with their own copy of the lints, `unsafe_code` lowered to `deny`.
const DENY_UNSAFE: &[&str] = &["crates/couchfin-player", "crates/couchfin-ui"];
const UI_SOURCES: &[&str] = &["crates/couchfin-ui/src", "crates/couchfin-ui/build.rs"];

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
fn every_other_crate_inherits_the_workspace_lints() {
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
fn deny_unsafe_lints_differ_from_the_workspace_only_in_unsafe_code() {
    let mut expected = workspace_lints();
    expected
        .get_mut("rust")
        .and_then(Value::as_table_mut)
        .expect("workspace has rust lints")
        .insert("unsafe_code".into(), Value::String("deny".into()));

    for dir in DENY_UNSAFE {
        let lints = manifest(dir)
            .get("lints")
            .and_then(Value::as_table)
            .cloned()
            .unwrap_or_else(|| panic!("{dir} needs its own [lints] table"));
        assert_eq!(
            lints, expected,
            "{dir}'s [lints] drifted from [workspace.lints]"
        );
    }
}

fn rust_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("directory is readable") {
            rust_files(&entry.expect("directory entry").path(), files);
        }
    } else if path.extension().is_some_and(|ext| ext == "rs") {
        files.push(path.to_path_buf());
    }
}

#[test]
fn ui_code_never_opts_in_to_unsafe() {
    let mut files = Vec::new();
    for source in UI_SOURCES {
        rust_files(&workspace_root().join(source), &mut files);
    }
    assert!(!files.is_empty(), "found no couchfin-ui sources");
    for file in files {
        let text = fs::read_to_string(&file).expect("source is readable");
        assert!(
            !text.contains("allow(unsafe_code)"),
            "{} allows unsafe_code; only couchfin-player may",
            file.display()
        );
    }
}
