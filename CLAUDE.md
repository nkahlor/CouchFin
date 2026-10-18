# CLAUDE.md

Couchfin is a controller-first Jellyfin client for Bazzite/SteamOS and Steam Big Picture, written in Rust with libmpv for playback and Slint for the UI. The full roadmap, phases and "Done when" checks are in `docs/PLAN.md`. Read the section for your task before starting.

## Your task

Each session works on exactly one task from `docs/PLAN.md`, on its own git worktree and branch. The prompt gives you the task, its "Done when" check and the crate you may touch.

- Stay inside the assigned crate. If you must change another crate's public API, explain why in the PR description.
- If the task is unclear or conflicts with this file or the plan, stop and ask rather than guessing.
- Each task has a GitHub issue (label `phase-N`, milestone M1). Link it from the PR.
- Finish by opening a PR (see "Before opening a PR" below). `.github/pull_request_template.md` carries the same checklist.

## Workspace layout

| Crate | Owns |
|---|---|
| `couchfin-api` | Typed Jellyfin HTTP client: auth, items, images, playback reporting |
| `couchfin-core` | App state, navigation stack, settings, image cache |
| `couchfin-gamepad` | Game controllers via SDL3, mapped to one `Input` enum (the keyboard is handled in `couchfin-ui`) |
| `couchfin-player` | libmpv wrapper: load, play/pause, seek, tracks, position events |
| `couchfin-ui` | Slint screens and components; focus handling |
| `app` | Binary: wires everything together, window, main loop |

**Dependency rule:** `couchfin-api`, `couchfin-gamepad` and `couchfin-player` never depend on each other or on the UI. `couchfin-core` depends only on `couchfin-api`. Each crate provides a fake or mock so others can be tested without it.

## Commands

Run everything inside the Distrobox dev container (see README). CI runs the same checks with the same image. Run `pre-commit install` once per clone so fmt, clippy and taplo run on every commit.

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
- No `unsafe` outside `couchfin-player`. `couchfin-ui` allows it only in Slint's generated code; handwritten UI code may not use it (enforced by `app/tests/lint_config.rs`).
- Make invalid states unrepresentable: newtypes for IDs (`ItemId`, `UserId`), enums instead of booleans and strings.
- Keep business logic out of the UI. Screens render state from `couchfin-core` and send actions (such as `LoginAction`) back to it (one-way data flow).
- Keep modules small and single-purpose, with `pub` only where needed. Aim for at most about 300 lines per file.
- Every UI action must be reachable by controller. Check focus on every new screen.
- Styling comes from `Theme` in `crates/couchfin-ui/ui/theme.slint`: every colour, length and font weight in a screen or widget is a `Theme` property, never a literal. Add a named property when you need a new value (enforced by `crates/couchfin-ui/tests/theme_tokens.rs`; see decision 009).
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
- [ ] The PR is small and covers one task. UI tasks cover one screen or one component per PR, so each can really be reviewed
- [ ] **UI changes only: you looked at it** (see "Looking at the UI" below) and the PR lists what you checked
- [ ] The PR description states which "Done when" check the human still needs to run on the TV

## Looking at the UI

Passing tests don't mean it looks right (#44 passed every test and shipped overlapping titles). Every PR that changes `crates/couchfin-ui`, `app` or a `.slint` file must be looked at, cheaply, in this order (decision 018):

1. **Layout checks, no images:** `cargo test -p couchfin-ui --test layout_checks`. It renders the screens from the example's fixtures at 1920x1080 and 1280x720 and fails on text overlapping text, text spilling out of its card, and text or buttons off screen. A test marked `#[ignore = "fixed by #N"]` is a known problem: if your PR fixes it, remove the `ignore`.
2. **Screenshots:** `cargo run -p couchfin-ui --example screenshots -- target/screenshots`. It writes every screen and state as `<screen>-<height>-<state>.png` (full size, uploaded by CI as `screenshots-<sha>`) and four contact sheets, `sheet-login`, `sheet-home`, `sheet-browse` and `sheet-detail` (six screens each, shrunk, about 1920x1630). Add a state for your change to `crates/couchfin-ui/examples/screenshots/` when no shot shows it.
3. **Look, within a budget.** An image costs about 2k tokens now and again on every later turn. So open **only the contact sheets for the screens your PR touches, once, as the last step before opening the PR**, never one full-size file after another. Open a single full-size PNG only to inspect a specific problem you already know about.
4. **Check against this list**, and put what you checked in the PR: no overlapping or clipped text; alignment and spacing consistent; focus clearly visible; contrast readable; nothing cut off at 1280x720; images fill their slots (also the wrong-shaped and missing ones).

The screenshots use Slint's software renderer, which draws no shadows, glow or other femtovg-only effects (the focused card's glow, for one), and uses the host's fonts. Anything depending on those still needs the TV: say so in the PR.
