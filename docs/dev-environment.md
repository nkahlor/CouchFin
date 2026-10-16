# Dev environment

Bazzite's root filesystem is immutable, so all development happens inside a
container built from [`.devcontainer/Containerfile`](../.devcontainer/Containerfile).
On Bazzite you enter it through Distrobox (Podman is already installed), which
shares your home directory, display, GPU (`/dev/dri`) and input devices
(`/dev/input`, including Steam Input's virtual pad) with the container. CI
builds and tests in the same image.

The image contains:

- the pinned Rust toolchain with rustfmt, clippy and llvm-tools
- cargo-deny, cargo-llvm-cov, taplo, slint-lsp and pre-commit
- the libmpv, SDL2, Mesa (GL/EGL, VA-API, Vulkan), Slint windowing, D-Bus and
  OpenSSL development packages
- ffmpeg and the Mesa VA-API drivers from RPM Fusion, which include the
  H.264/HEVC support Fedora's own builds leave out (see
  [decision 001](decisions/001-dev-container-image.md))

## Using the published image

CI pushes the image to `ghcr.io/nkahlor/couchfin-dev` whenever `.devcontainer/`
changes on `main`. While the repo is private, log in once with a GitHub token
that has the `read:packages` scope:

```sh
podman login ghcr.io -u <github-user>
```

Then create and enter the box:

```sh
distrobox create --name couchfin-dev --image ghcr.io/nkahlor/couchfin-dev:latest
distrobox enter couchfin-dev
```

## Building the image locally

To try changes to the Containerfile before they are merged:

```sh
podman build -t localhost/couchfin-dev -f .devcontainer/Containerfile .devcontainer
distrobox create --name couchfin-dev-local --image localhost/couchfin-dev
```

## Checking the environment

`check-devenv` runs inside the container and lists every tool and library the
workspace needs. CI runs it against each image it builds.

```sh
check-devenv
vainfo   # should list H264 and HEVC profiles on the TV PC's GPU
```

## CI

`.github/workflows/ci.yml` runs each check from `CLAUDE.md` as its own job in
`ghcr.io/nkahlor/couchfin-dev:latest`: `fmt` (rustfmt and taplo), `clippy`,
`test`, `doc`, `deny` and `coverage`. Nothing is installed at runtime, so a
tool CI needs must be added to the Containerfile first, and reaches CI only
once that change is merged and the image is pushed. The image is private while
the repo is, so each job pulls it with the workflow's `GITHUB_TOKEN`
(`packages: read`). See [decision 004](decisions/004-release-notes.md).

## Coverage

The `coverage` job runs `cargo llvm-cov --workspace`, then
`.github/scripts/coverage_gate.py` checks the line coverage of each of
`couchfin-api` and `couchfin-core` separately (the files under the crate's
`src/`, covered by any test in the workspace). Each must be at or above 80%.

- A crate with no executable lines yet, such as an empty library, is skipped
  and the job shows a warning saying so. The gate applies from its first
  executable line.
- A crate name the gate doesn't find fails the job, so renaming a crate can't
  silently turn the gate off.

To see the same numbers locally:

```sh
cargo llvm-cov --workspace --summary-only
```

The gate's own tests run in the same job:
`python3 -m unittest discover -s .github/scripts`.

## Updating the toolchain

Tool versions are `ARG`s at the top of the Containerfile. The Rust toolchain is
installed read-only under `/usr/local/rustup`, so `rustup` cannot add other
toolchains from inside the box: if the workspace pins a toolchain, keep it equal
to `RUST_VERSION`. Cargo's registry cache stays in your own `~/.cargo`.
