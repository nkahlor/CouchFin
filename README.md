# Couchfin

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev)

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

1. Create the box and enter it. The image is public, so no registry login is
   needed:

   ```sh
   distrobox create --name couchfin-dev --image ghcr.io/nkahlor/couchfin-dev:latest
   distrobox enter couchfin-dev
   ```

2. Check that every tool and library is present, then build and run:

   ```sh
   check-devenv
   cargo run
   ```

   For now this opens a fullscreen window that says "Hello". Escape (or
   Backspace) quits.

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

### Checking a real Jellyfin server

`couchfin-api` has a small example that checks a server, logs in and calls
`/Users/Me`, so you can try the client against your own server before the
login screen exists. If the address redirects (for example a proxy sending
`http://` to `https://`), it prints the address the server really answers on
and uses that from then on. It asks for the password without echoing it and
never prints it or the token:

```sh
cargo run -p couchfin-api --example probe -- http://jellyfin.lan:8096 alice
```

Afterwards, the server's Dashboard > Devices lists "Couchfin". Each run uses a
new device ID unless `COUCHFIN_DEVICE_ID` is set to one it printed before. See
[decision 006](docs/decisions/006-api-client-design.md) for how the client is
built.

## Checks

CI runs these on every pull request and on `main`, in the dev container image,
one job per check (the comments give the job names). CI also passes
`--locked`, so an out-of-date `Cargo.lock` fails.

```sh
cargo fmt --all --check                                # fmt
taplo fmt --check                                      # fmt
cargo clippy --workspace --all-targets -- -D warnings  # clippy
cargo test --workspace                                 # test
cargo doc --workspace --no-deps                        # doc (RUSTDOCFLAGS=-D warnings)
cargo deny check                                       # deny
cargo llvm-cov --workspace                             # coverage
```

The `coverage` job fails if `couchfin-api` or `couchfin-core` drops below 80%
line coverage. See [docs/dev-environment.md](docs/dev-environment.md#coverage)
for how each crate is measured.

Lint levels live in `[workspace.lints]` in the root `Cargo.toml`: Clippy's
pedantic group warns, `unwrap()` outside tests is an error, and `unsafe` is
forbidden everywhere except `couchfin-player`. `couchfin-ui` allows it only in
Slint's generated code; handwritten UI code may not use it, and
`app/tests/lint_config.rs` enforces that
([decision 005](docs/decisions/005-slint-generated-unsafe.md)). CI turns every
warning into an error. Formatting is set by `rustfmt.toml`, `taplo.toml` and
`.editorconfig`, licence and advisory rules by `deny.toml`. Format Slint files with
`slint-lsp format <file.slint>`.

To catch formatting and Clippy failures before CI does, install the pre-commit
hook once per clone, inside the dev container:

```sh
pre-commit install
```

The hook runs `cargo fmt --check`, `cargo clippy -D warnings` and
`taplo fmt --check`, and rejects the commit if any fails.

## Releases

Pushing a tag such as `v0.1.0` runs the CI checks on it, builds `couchfin` in
release mode in the dev image, and publishes a GitHub Release with a tarball of
the binary, its SHA-256 checksum, and notes generated by
[git-cliff](https://git-cliff.org) from the Conventional Commits since the
previous release. The tag must match the `version` in the root `Cargo.toml`. A
tag with a hyphen, such as `v0.2.0-rc.1`, becomes a pre-release. The binary
needs Fedora 44 or newer (current Bazzite), because it links against the dev
image's libraries. See [decision 004](docs/decisions/004-release-notes.md) for
how the notes are put together.

## Licence

Couchfin is [MIT](LICENSE). It uses [Slint](https://slint.dev) under the Slint
Royalty-free Desktop, Mobile, and Web Applications License 2.0. That licence
requires the "Made with Slint" badge on the page where Couchfin is downloaded,
so it appears at the top of this README and in every release's notes. See
[decision 003](docs/decisions/003-slint-licence.md).
