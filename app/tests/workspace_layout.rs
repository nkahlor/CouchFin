//! Checks the workspace shape and the dependency rule from `CLAUDE.md`, so a
//! stray `path` dependency between crates fails CI instead of slipping through
//! review.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// Every workspace crate, mapped to the workspace crates it may depend on.
/// `app` wires everything together, so it is the only crate allowed to use all.
const ALLOWED: &[(&str, &[&str])] = &[
    ("couchfin-api", &[]),
    ("couchfin-input", &[]),
    ("couchfin-player", &[]),
    ("couchfin-core", &["couchfin-api"]),
    ("couchfin-ui", &["couchfin-core"]),
    (
        "app",
        &[
            "couchfin-api",
            "couchfin-core",
            "couchfin-input",
            "couchfin-player",
            "couchfin-ui",
        ],
    ),
];

/// Workspace package name to the set of workspace packages it depends on
/// (any dependency kind).
fn workspace_graph() -> BTreeMap<String, BTreeSet<String>> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(&manifest)
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("metadata is JSON");
    let packages = metadata["packages"].as_array().expect("packages array");

    let names: BTreeSet<String> = packages
        .iter()
        .map(|p| p["name"].as_str().expect("package name").to_owned())
        .collect();

    packages
        .iter()
        .map(|p| {
            let deps = p["dependencies"]
                .as_array()
                .expect("dependencies array")
                .iter()
                .map(|d| d["name"].as_str().expect("dependency name").to_owned())
                .filter(|d| names.contains(d))
                .collect();
            (p["name"].as_str().expect("package name").to_owned(), deps)
        })
        .collect()
}

#[test]
fn workspace_has_exactly_the_six_planned_crates() {
    let actual: BTreeSet<String> = workspace_graph().into_keys().collect();
    let expected: BTreeSet<String> = ALLOWED.iter().map(|(name, _)| (*name).to_owned()).collect();
    assert_eq!(actual, expected);
}

#[test]
fn crates_only_depend_on_what_the_dependency_rule_allows() {
    let graph = workspace_graph();
    for (name, allowed) in ALLOWED {
        let allowed: BTreeSet<String> = allowed.iter().map(|d| (*d).to_owned()).collect();
        let Some(actual) = graph.get(*name) else {
            panic!("{name} is not a workspace member");
        };
        let forbidden: Vec<_> = actual.difference(&allowed).collect();
        assert!(
            forbidden.is_empty(),
            "{name} depends on {forbidden:?}, which the dependency rule in CLAUDE.md forbids"
        );
    }
}
