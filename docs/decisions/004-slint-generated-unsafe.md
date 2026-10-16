# 004: unsafe_code in couchfin-ui

Status: accepted · 2026-10-15

## Context

The workspace forbids `unsafe_code`, and only `couchfin-player` lowers that to
`deny` ([decision 002](002-lint-and-hook-setup.md)). Slint compiles `.slint`
markup into Rust inside `couchfin-ui`, and that code uses the `vtable` crate's
macros, which put `#[allow(unsafe_code)]` on their generated `unsafe` blocks.
Under `forbid` this fails to compile (`E0453: allow(unsafe_code) incompatible
with previous forbid`). No Slint option avoids it.

## Decision

- `couchfin-ui` gets a copy of `[workspace.lints]` with `unsafe_code = "deny"`,
  the same way `couchfin-player` does. `app/tests/lint_config.rs` checks that
  both copies stay equal to the workspace table apart from that one lint.
- Handwritten code in `couchfin-ui` still may not use `unsafe`.
  `app/tests/lint_config.rs` fails if any file under `crates/couchfin-ui/src`,
  or its `build.rs`, contains `allow(unsafe_code)`. `deny` then rejects any
  `unsafe` written there.
- Slint's generated module in `couchfin-ui/src/lib.rs` also allows
  `missing_docs` and `clippy::unwrap_used`, because Slint generates neither doc
  comments nor `expect` calls. The public re-exports carry the documentation.

## Consequences

- "No `unsafe` outside `couchfin-player`" still holds for code we write.
  The only `unsafe` in `couchfin-ui` is Slint's generated code.
- If a future Slint release stops emitting `#[allow(unsafe_code)]`,
  `couchfin-ui` can go back to inheriting the workspace lints.
