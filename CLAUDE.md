# CLAUDE.md

Couchfin is a controller-first Jellyfin client for Bazzite/SteamOS and Steam Big Picture, written in Rust with libmpv for playback and Slint for the UI. The full roadmap, phases and "Done when" checks are in `docs/PLAN.md`. Read the section for your task before starting.

## Your task

Each session works on exactly one task from `docs/PLAN.md`, on its own git worktree and branch. The prompt gives you the task, its "Done when" check and the crate you may touch.

- Stay inside the assigned crate. If you must change another crate's public API, explain why in the PR description.
- If the task is unclear or conflicts with this file or the plan, stop and ask rather than guessing.
- Finish by opening a PR (see "Before opening a PR" below).

## Workspace layout

| Crate | Owns |
|---|---|
| `couchfin-api` | Typed Jellyfin HTTP client: auth, items, images, playback reporting |
| `couchfin-core` | App state, navigation stack, settings, image cache |
| `couchfin-input` | SDL controller and keyboard events mapped to one `Action` enum |
| `couchfin-player` | libmpv wrapper: load, play/pause, seek, tracks, position events |
| `couchfin-ui` | Slint screens and components; focus handling |
| `app` | Binary: wires everything together, window, main loop |

**Dependency rule:** `couchfin-api`, `couchfin-input` and `couchfin-player` never depend on each other or on the UI. `couchfin-core` depends only on `couchfin-api`. Each crate provides a fake or mock so others can be tested without it.

## Commands

Run everything inside the Distrobox dev container (see README). CI runs the same checks with the same image.

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace            # includes doctests
cargo llvm-cov --workspace        # api and core must stay at or above 80%
cargo deny check
cargo doc --workspace --no-deps
taplo fmt --check
slint-lsp format <file.slint>
```

## How to work: red-green-refactor TDD

1. Write the test first, run it, and confirm it fails for the expected reason (red). Save that output for the PR description.
2. Write just enough code to make it pass (green).
3. Refactor with the tests still green.

- Test public interfaces only. Unit tests go through each crate's public API; integration tests live in `tests/`. Never test private functions directly.
- For `couchfin-api` and `couchfin-core`, use wiremock, never a real Jellyfin server.
- Put external things (HTTP, mpv, SDL, clock, filesystem) behind traits and inject them, so every crate can be tested with fakes.

## Code conventions

- No `unwrap()` outside tests. Use `thiserror` in libraries and `anyhow` in `app`.
- No `unsafe` outside `couchfin-player`.
- Make invalid states unrepresentable: newtypes for IDs (`ItemId`, `UserId`), enums instead of booleans and strings.
- Keep business logic out of the UI. Screens render state from `couchfin-core` and send `Action`s back (one-way data flow).
- Keep modules small and single-purpose, with `pub` only where needed. Aim for at most about 300 lines per file.
- Every UI action must be reachable by controller. Check focus on every new screen.
- Never log secrets (access tokens, passwords). Tokens live in the system keyring, with a 0600 file as the fallback.
- Log through `tracing`; stdout is hidden when launched from Steam.

## Docs

- Docs never drift from code. Any change in behaviour updates the affected doc comments, README, `docs/` and this file in the same PR.
- Every public item gets a doc comment. Public APIs carry runnable rustdoc examples, so a wrong example fails `cargo test`.
- Comments explain why, not what. Delete stale comments rather than leaving them.
- Record design choices in `docs/decisions/NNN-title.md` (next free number, short kebab-case title).

## Before opening a PR

- [ ] fmt, clippy (`-D warnings`), tests, coverage, `cargo deny` and `cargo doc` all pass locally
- [ ] Every change has tests, and the PR description includes the red (failing) test output
- [ ] You re-read every comment and doc touching the changed code, and listed them in the description (or ticked "no docs affected")
- [ ] The commit messages and PR title follow Conventional Commits (`feat(api): ...`, `fix(ui): ...`)
- [ ] The PR is small and covers one task
- [ ] The PR description states which "Done when" check the human still needs to run on the TV
