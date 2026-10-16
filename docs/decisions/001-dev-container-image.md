# 001: Dev container image

Status: accepted · 2026-10-15

## Context

Bazzite is immutable, so build dependencies can't be installed on the host. The
plan asks for one container image, used through Distrobox on the TV PC and by
CI, so both see the same toolchain and libraries.

## Decision

- **Base:** `fedora:44`, the newest Fedora release. Bazzite is built on Fedora,
  so library versions stay close to the host's, and Distrobox supports Fedora
  well.
- **Codecs from RPM Fusion:** Fedora's `ffmpeg-free` and `mesa-va-drivers` leave
  out H.264 and HEVC, which the Phase 4 check (4K HEVC and 1080p H.264 with
  hardware decoding) needs. The image installs RPM Fusion's `ffmpeg-libs` and
  `mesa-va-drivers-freeworld` before `mpv-devel`, so libmpv links against them.
  `check-devenv` fails if the Fedora builds sneak back in.
- **Dependencies by pkg-config name** (`pkgconfig(mpv)`, `pkgconfig(sdl2)`), so
  package renames such as SDL2 becoming `sdl2-compat` don't break the build.
- **Rust via rustup, pinned** (`RUST_VERSION`), installed under `/usr/local` and
  readable by everyone, because Distrobox runs as the host user and shares
  `$HOME`. Cargo's registry cache stays in the user's `~/.cargo`.
- **Prebuilt tool binaries** (cargo-binstall for cargo-deny and cargo-llvm-cov,
  release downloads for taplo and slint-lsp), with compiling from source turned
  off, to keep image builds short. Versions are pinned as `ARG`s.
- **Published to GHCR** (`ghcr.io/nkahlor/couchfin-dev`) from `main` by
  `.github/workflows/devcontainer.yml`. Pull requests build the image and run
  the smoke test, both as root and as an unprivileged login shell (as Distrobox
  runs it), but push nothing.

## Consequences

- Distrobox users must log in to GHCR while the repo is private.
- The image is x86_64 and aarch64 only, the architectures all pinned tools
  publish binaries for.
- A workspace `rust-toolchain.toml` must match `RUST_VERSION`, since rustup
  can't install extra toolchains into the read-only `/usr/local/rustup`.
- This only covers development and CI. The Phase 6 Flatpak bundles its own
  libraries and must choose its own codec source.
