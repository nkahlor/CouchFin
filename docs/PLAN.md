# Couchfin — Milestone 1 Plan

Oct 15, 2026 · @Nick

> This is the source of truth for Milestone 1. Agents: read `CLAUDE.md` first, then the phase and task you've been assigned here. Tick a task's box only after its PR is merged.

## Overview

Milestone 1 is a controller-first Jellyfin client for Bazzite/SteamOS and Steam Big Picture. It logs in, browses a library, plays video with resume and subtitle selection, and is fully usable from a gamepad, with native support for Steam Controller or any other controller. It's built in small phases so each one ships something runnable.

**Name:** Couchfin. The "-fin" suffix tells people it's a Jellyfin client, as with other community clients. We avoided "SteamFin" because Steam is a Valve trademark (branding guidelines) and it's close to Streamyfin. The crates are prefixed `couchfin-` (couchfin-api, couchfin-core and so on).

**Playback scope for M1:** direct play only. mpv plays the original file as-is, with no server transcoding. mpv handles almost every codec locally, so this should cover nearly everything. Transcoding can come in a later milestone if a file won't play.

## Goals

- Playback that works as reliably as the official clients (libmpv does the decoding)
- A 10-foot browsing UI that's fully usable with a controller
- Feels like Netflix/YouTube TV/console apps
- Launches from Steam as a non-Steam game, fullscreen, with no mouse or keyboard needed

## Non-goals for M1

- Jellyfin server plugins and web extensions
- A client-side extension system (stretch goal; keep module boundaries clean so it's possible later)
- Music, live TV, downloads and multi-user switching

## Tech stack

The app is Rust throughout, with libmpv for playback and Slint for the UI, rendered together in one OpenGL window.

| Concern | Choice | Notes |
|---|---|---|
| Language | Rust (stable, 2024 edition) | Cargo workspace |
| Playback | libmpv2 crate | Render API into an OpenGL texture/FBO; hardware decoding via `hwdec=auto-safe` |
| UI | Slint | Use the femtovg/OpenGL renderer so mpv frames can draw under the UI |
| Input | sdl2 crate (GameController API) | Works with Steam Input's virtual Xbox pad; fall back to keyboard for dev |
| HTTP / API | reqwest + serde + tokio | Hand-written typed client for the endpoints M1 needs |
| Images | reqwest + image crate | Disk cache under `$XDG_CACHE_HOME` |
| Config | serde + toml, directories crate | Server URL and access token under `$XDG_CONFIG_HOME` |
| Logging | tracing + tracing-subscriber | Log to a rotating file, since Steam hides stdout; a panic hook writes crashes and backtraces to the log |
| Packaging | Flatpak, built by CI and attached to GitHub Releases (Phase 6) | Bazzite is immutable; dev in a Distrobox |

**Open question:** if the Slint and mpv OpenGL integration fights back, the fallback is to let mpv own the window and draw the UI as an overlay. Phase 4 should spike this early.

## Repo layout

The workspace is split into crates with narrow interfaces, so agents can work in parallel and extensions can plug in later.

| Crate | Owns | Depends on |
|---|---|---|
| couchfin-api | Typed Jellyfin HTTP client: auth, items, images, playback reporting | reqwest, serde |
| couchfin-core | App state, navigation stack, settings, image cache | couchfin-api |
| couchfin-input | SDL controller events mapped to app actions (Up, Down, Select, Back, Menu, Seek) | sdl2 |
| couchfin-player | libmpv wrapper: load, play/pause, seek, tracks, position events | libmpv2 |
| couchfin-ui | Slint screens and components; focus handling | slint, couchfin-core |
| app | Binary: wires everything together, window, main loop | all of the above |

**Rule:** couchfin-api, couchfin-input and couchfin-player never depend on each other or on the UI. Each one gets a mock or fake so it can be tested on its own.

## Phases and tasks

There are six phases, and each ends with something runnable. Tasks within a phase can go to parallel agents, but phases run in order.

### Phase 0: Skeleton

- [x] Confirm the Couchfin name is free on GitHub and Flathub, then create a GitHub repo named couchfin
- [ ] Repo settings: protected main (PRs only, CI must pass), PR and issue templates, Dependabot for Cargo, and one GitHub issue per task in this plan
- [ ] Dev environment as a container image: a Containerfile (Fedora base, Rust toolchain, mpv-devel, SDL2-devel, Mesa) in `.devcontainer/`. Use it on Bazzite through Distrobox (Podman, preinstalled) so the app can reach the GPU, display and controllers; CI builds and tests in the same image on GitHub Actions
- [ ] Set up the Cargo workspace with the six crates, CI (fmt, clippy, test), and a README covering the Distrobox setup
- [ ] Linting and formatting: a `rustfmt.toml`; workspace-wide Clippy settings in `[workspace.lints]` in the root Cargo.toml (`clippy::pedantic` on as warnings, `unwrap_used` denied, `unsafe_code` forbidden except in couchfin-player); cargo-deny for licences and advisories; taplo for TOML formatting; `.editorconfig`; a pre-commit hook (via cargo-husky or pre-commit) that runs fmt and clippy; CI runs the same checks with `-D warnings`. Slint files are formatted with `slint-lsp format`.
- [ ] app opens a fullscreen Slint window that says "Hello" and quits on Back/Escape
- [ ] CI/CD: GitHub Actions runs every check on each PR in the dev container image. Pushing a `v*` tag builds a release binary and publishes it to a GitHub Release with auto-generated notes from the Conventional Commits

**Done when:** `cargo run` shows the window and CI passes; a deliberately badly formatted or Clippy-failing change is rejected by both the pre-commit hook and CI

### Phase 1: Login

- [ ] couchfin-api: `POST /Users/AuthenticateByName`, the `Authorization: MediaBrowser ...` header, and `GET /System/Info/Public` to check the server, plus `GET /Users/Me`. The auth header sends a stable per-install device ID and the device name "Couchfin", so the server lists it under Devices
- [ ] couchfin-core: persist the server URL and token (token in the system keyring via the keyring crate, falling back to a file only the user can read, with permissions 0600), and skip login when the token is still valid
- [ ] couchfin-ui: server URL and username/password screen; keyboard entry is fine for now (Steam's on-screen keyboard handles the couch case)

**Done when:**
1. Home shows the server name and version from `/System/Info/Public` plus the logged-in username from `/Users/Me`.
2. The server's Dashboard > Devices lists this client by name.
3. After a restart, it skips login and `/Users/Me` returns 200.
4. A wrong password or an unreachable URL shows a clear error, not a crash.
5. Integration tests cover each of these against a wiremock server.

### Phase 2: Controller navigation and placeholder cards

- [ ] couchfin-input: SDL controller and keyboard (arrow keys, Enter, Escape/Backspace) both mapped to one `Action` enum, with key repeat on held D-pad and stick
- [ ] couchfin-ui: Home screen with a horizontal row of 10 hard-coded placeholder title cards and a clear focus highlight

**Done when:** the D-pad and the arrow keys both move focus, A/Enter selects, and B/Escape goes back, with no mouse needed

### Phase 3: Real library

- [ ] couchfin-api: `GET /UserViews`, `GET /Items` (paged), `GET /Items/{id}/Images/Primary`, and `GET /UserItems/Resume`
- [ ] couchfin-core: async image loading with a disk cache
- [ ] couchfin-ui: Home rows for Continue Watching plus one row per library, a library grid and a detail page

**Done when:** your real server's libraries all browse correctly, and an automated test runs against a fake Jellyfin server (wiremock with a generated fixture of 2,000 items and placeholder posters). In that test, scrolling the grid stays at 60 fps and the first row of posters appears within 1 s. The functional checks run in CI. The frame-rate check runs on the TV PC or a self-hosted GPU runner, because GitHub's runners have no GPU

### Phase 4: Playback

- [ ] Spike first: mpv rendering inside the Slint window. Timebox this to a day and record the result in `docs/decisions/`
- [ ] couchfin-player: load a direct-stream URL (`/Videos/{id}/stream?static=true`), play/pause and seek ±10 s/30 s
- [ ] couchfin-ui: a player overlay with a progress bar that auto-hides

**Done when:** a 4K HEVC file and a 1080p H.264 file both play with hardware decoding, controlled by controller

### Phase 5: Resume and subtitles

- [ ] couchfin-api: playback reporting (`/Sessions/Playing`, `/Progress` every 10 s, `/Stopped`)
- [ ] Resume from `UserData.PlaybackPositionTicks`, with a "Resume or Start over" prompt
- [ ] A subtitle and audio track picker in the overlay, using mpv's track list, including external subtitles from the server

**Done when:** you stop halfway, the position shows on the web client, and it resumes on relaunch; subtitles switch mid-playback

### Phase 6: Steam integration

- [ ] Launch script plus instructions for adding it as a non-Steam game, and a Steam Input template
- [ ] Flatpak: a manifest (`io.github.<user>.Couchfin`) bundling libmpv and SDL2, built in CI on every tag and attached to the GitHub Release; Flathub submission comes later

**Done when:** it launches from Big Picture, runs fullscreen and returns to Steam cleanly on quit

## Agent workflow

Use one agent per task, each on its own git worktree and branch. You review and merge at the end of each phase.

1. This plan lives in the repo as `docs/PLAN.md`, with the conventions in `CLAUDE.md` at the repo root.
2. For each task, start a Claude Code session with: the task line, its "Done when" check, and the crate it may touch.
3. The agent writes tests first for couchfin-api and couchfin-core. It uses a mock HTTP server (wiremock) rather than your real server.
4. The agent opens a PR. CI must pass, and you run the "Done when" check yourself on the TV.
5. After merging, tick the box here and start the next phase.

## Engineering standards

- **Red-green-refactor TDD:** write the test first, run it and confirm it fails for the expected reason (red), write just enough code to pass (green), then refactor. Each PR description notes the red run, e.g. the failing test output, so reviewers know the test can actually catch the bug
- **Test public interfaces only:** unit tests go through each crate's public API, integration tests live in `tests/`, and private functions are never tested directly
- Aim for comprehensive coverage of behaviour, measured with `cargo llvm-cov`; CI fails if coverage of couchfin-api and couchfin-core falls below 80%
- Put external things (HTTP, mpv, SDL, clock, filesystem) behind traits, and inject them, so every crate can be tested with fakes
- Keep business logic out of the UI: screens render state from couchfin-core and send Actions back (one-way data flow)
- Keep modules small and single-purpose, with `pub` only where needed; prefer roughly 300 lines per file at most
- Make invalid states unrepresentable: newtypes for IDs (`ItemId`, `UserId`), and enums instead of booleans and strings
- Every public item gets a doc comment; run `cargo doc` in CI
- **Docs never drift from code.** Any PR that changes behaviour updates the affected doc comments, README, `docs/` and CLAUDE.md in the same PR. Before opening a PR, the agent re-reads every comment and doc that touches the code it changed, and lists them in the description. The PR template has a "Docs updated / no docs affected" checkbox
- Prefer docs that are checked by machines: public APIs carry runnable rustdoc examples, which `cargo test` runs as doctests, so a wrong example fails CI. README setup commands are run by a CI job, so broken instructions fail too
- Comments explain why, not what; the code and tests say what. Delete stale comments rather than leave them
- **Reviewer pass:** a separate agent reviews each PR specifically for doc/code mismatches before you review it
- Run rustfmt and `clippy -D warnings` (pedantic where sensible); no unsafe outside couchfin-player
- Use small PRs with Conventional Commits messages; every PR includes tests for what it changes
