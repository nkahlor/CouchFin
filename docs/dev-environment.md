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

## Updating the toolchain

Tool versions are `ARG`s at the top of the Containerfile. The Rust toolchain is
installed read-only under `/usr/local/rustup`, so `rustup` cannot add other
toolchains from inside the box: if the workspace pins a toolchain, keep it equal
to `RUST_VERSION`. Cargo's registry cache stays in your own `~/.cargo`.
