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
- the libmpv, SDL3, Mesa (GL/EGL, VA-API, Vulkan), Slint windowing, D-Bus and
  OpenSSL development packages
- ffmpeg and the Mesa VA-API drivers from RPM Fusion, which include the
  H.264/HEVC support Fedora's own builds leave out (see
  [decision 001](decisions/001-dev-container-image.md))

## Developing natively on Ubuntu or WSL

The dev box can also be a plain Ubuntu 24.04 machine (including WSL2 with
WSLg), without the container. Day-to-day coding happens here, and so do
Claude Code agents. The TV PC stays the place for checks that need its GPU,
controllers or Steam. Install once:

1. System packages:

   ```sh
   sudo apt install -y build-essential cmake pkg-config git libfontconfig-dev libfreetype-dev libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libdbus-1-dev libssl-dev libudev-dev
   ```

2. SDL3. Ubuntu 24.04 doesn't package it, so build the same version the dev
   image uses. Couchfin uses SDL only for game controllers (decision 010), so
   video, audio and the rest are switched off, and `SDL_UNIX_CONSOLE_BUILD`
   confirms that a build without windows is intended:

   ```sh
   git clone --depth 1 --branch release-3.4.18 https://github.com/libsdl-org/SDL.git ~/src/SDL
   cmake -S ~/src/SDL -B ~/src/SDL/build -DCMAKE_BUILD_TYPE=Release -DSDL_TESTS=OFF -DSDL_EXAMPLES=OFF -DSDL_VIDEO=OFF -DSDL_AUDIO=OFF -DSDL_RENDER=OFF -DSDL_GPU=OFF -DSDL_CAMERA=OFF -DSDL_UNIX_CONSOLE_BUILD=ON
   cmake --build ~/src/SDL/build -j"$(nproc)"
   sudo cmake --install ~/src/SDL/build && sudo ldconfig
   pkg-config --modversion sdl3   # 3.4.18
   ```

   When the dev image's SDL3 version changes, rebuild with the new
   `release-x.y.z` tag.

3. Rust comes from `rust-toolchain.toml` through rustup. The other check tools
   are optional, since CI runs them, but having them saves CI round trips:

   ```sh
   cargo install --locked cargo-deny@0.20.2 cargo-llvm-cov@0.9.1 taplo-cli@0.10.0
   ```

Then `cargo run`, `cargo test --workspace` and the screenshot example work as
they do in the box.

**Differences from the TV:**
- **Controllers:** WSL sees no USB controllers unless they're passed through
  with `usbipd-win`, so expect keyboard only.
- **Keyring:** there's no Secret Service, so Couchfin logs that it is using
  the token file.
- **Rendering:** WSLg may render on the GPU or in software, so judge frame
  rate (`perf_grid`) on the TV.

## Using the published image

CI pushes the image to `ghcr.io/nkahlor/couchfin-dev` whenever `.devcontainer/`
changes on `main`. The image is public, so create and enter the box without
logging in:

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
once that change is merged and the image is pushed. Each job pulls the image
with the workflow's `GITHUB_TOKEN` (`packages: read`), which keeps CI working
even if the package is made private again. See
[decision 004](decisions/004-release-notes.md).

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
