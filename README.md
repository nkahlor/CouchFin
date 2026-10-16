# Couchfin

A controller-first [Jellyfin](https://jellyfin.org) client for Bazzite/SteamOS
and Steam Big Picture, written in Rust with libmpv for playback and Slint for
the UI.

Couchfin is in early development. The roadmap for the first milestone is in
[docs/PLAN.md](docs/PLAN.md), and contributor conventions are in
[CLAUDE.md](CLAUDE.md).

## Development setup

Bazzite's root filesystem is immutable, so development happens inside the
Couchfin dev container, entered through Distrobox. Podman and Distrobox come
preinstalled on Bazzite. The container shares your home directory, display,
GPU and controllers, and CI builds and tests in the same image. See
[docs/dev-environment.md](docs/dev-environment.md) for what the image contains
and how to update it.

1. While the repo is private, log in to GitHub's container registry with a
   token that has the `read:packages` scope:

   ```sh
   podman login ghcr.io -u <github-user>
   ```

2. Create the box and enter it:

   ```sh
   distrobox create --name couchfin-dev --image ghcr.io/nkahlor/couchfin-dev:latest
   distrobox enter couchfin-dev
   ```

3. Check that every tool and library is present, then build and run:

   ```sh
   check-devenv
   cargo run
   ```

To try changes to the Containerfile before they're merged, build the image
locally:

```sh
podman build -t localhost/couchfin-dev -f .devcontainer/Containerfile .devcontainer
distrobox create --name couchfin-dev-local --image localhost/couchfin-dev
```

## Workspace

| Crate | Owns |
|---|---|
| [`couchfin-api`](crates/couchfin-api) | Typed Jellyfin HTTP client: auth, items, images, playback reporting |
| [`couchfin-core`](crates/couchfin-core) | App state, navigation stack, settings, image cache |
| [`couchfin-input`](crates/couchfin-input) | SDL controller and keyboard events mapped to one `Action` enum |
| [`couchfin-player`](crates/couchfin-player) | libmpv wrapper: load, play/pause, seek, tracks, position events |
| [`couchfin-ui`](crates/couchfin-ui) | Slint screens and components; focus handling |
| [`app`](app) | The `couchfin` binary: wires everything together, window, main loop |

`couchfin-api`, `couchfin-input` and `couchfin-player` never depend on each
other or on the UI, and `couchfin-core` depends only on `couchfin-api`.
`app/tests/workspace_layout.rs` enforces this, so a forbidden dependency fails
`cargo test`.

The Rust toolchain is pinned in [rust-toolchain.toml](rust-toolchain.toml).

## Checks

CI runs these on every pull request:

```sh
cargo fmt --all --check
taplo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
cargo deny check
```

Lint levels live in `[workspace.lints]` in the root `Cargo.toml`: Clippy's
pedantic group warns, `unwrap()` outside tests is an error, and `unsafe` is
forbidden everywhere except `couchfin-player`. CI turns every warning into an
error. Formatting is set by `rustfmt.toml`, `taplo.toml` and `.editorconfig`,
licence and advisory rules by `deny.toml`. Format Slint files with
`slint-lsp format <file.slint>`.

To catch formatting and Clippy failures before CI does, install the pre-commit
hook once per clone, inside the dev container:

```sh
pre-commit install
```

The hook runs `cargo fmt --check`, `cargo clippy -D warnings` and
`taplo fmt --check`, and rejects the commit if any fails.

## Licence

[MIT](LICENSE)
