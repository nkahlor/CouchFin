# 002: Lint and pre-commit hook setup

Status: accepted · 2026-10-15

## Context

The plan asks for workspace-wide Clippy settings, with `unsafe_code` forbidden
everywhere except `couchfin-player`, and a pre-commit hook (via cargo-husky or
pre-commit) that runs fmt and Clippy.

## Decision

- **Pre-commit framework, not cargo-husky.** cargo-husky's last release (1.5.0)
  was in January 2020, and it installs hooks as a side effect of a build script.
  Agents each work in their own git worktree, and pre-commit is known to work
  there: it installs into the shared hooks directory, so one install covers all
  worktrees. It's packaged by Fedora (so it's in the dev image), and its hooks
  are plain `language: system` commands calling the tools already in the image.
  The cost is one `pre-commit install` per clone.
- **`couchfin-player` copies the workspace lint table.** Cargo can't override
  one inherited lint, and `forbid` can't be relaxed with `#[allow]`. So the
  workspace forbids `unsafe_code`, and `couchfin-player` has its own copy of
  `[workspace.lints]` with `unsafe_code = "deny"`, so each FFI block opts in
  with `#[allow(unsafe_code)]`. `app/tests/lint_config.rs` fails if the copy
  drifts, or if another crate stops inheriting the workspace lints.
- **`missing_docs` warns** (an error in CI), to enforce "every public item gets
  a doc comment" from `CLAUDE.md`.
- **`unwrap()` is allowed in tests** through `allow-unwrap-in-tests` in
  `clippy.toml`, matching "no `unwrap()` outside tests".
- **cargo-deny** allows only permissive licences, denies unknown registries and
  git sources, and fails on vulnerable, unmaintained or yanked crates.

## Consequences

- A dependency with a licence that isn't on the allow list (for example Slint,
  which is GPL-3.0 or its own royalty-free licence) needs a deliberate
  `deny.toml` change in the PR that adds it.
- The pre-commit hook runs Clippy on the whole workspace, so commits get slower
  as the workspace grows. If that hurts, move Clippy to `pre-push`.
