#!/usr/bin/env bash
# Smoke test for the dev container: every tool and library the workspace
# needs must be present. CI runs this inside the freshly built image, so a
# Containerfile change that drops a dependency fails before it reaches anyone.
set -uo pipefail

failures=0

check() {
    local name=$1
    shift
    if "$@" >/dev/null 2>&1; then
        printf 'ok    %s\n' "$name"
    else
        printf 'FAIL  %s\n' "$name"
        failures=$((failures + 1))
    fi
}

# Rust toolchain and the checks CLAUDE.md lists.
check "cargo"              cargo --version
check "rustfmt"            cargo fmt --version
check "clippy"             cargo clippy --version
check "rustdoc"            rustdoc --version
check "cargo-llvm-cov"     cargo llvm-cov --version
check "llvm-tools"         bash -c 'rustup component list --installed | grep -q llvm-tools'
check "cargo-deny"         cargo deny --version
check "taplo"              taplo --version
check "slint-lsp format"   slint-lsp format --help
check "pre-commit"         pre-commit --version
check "C compiler"         cc --version
check "clang (bindgen)"    clang --version

# Native libraries the crates link against.
for module in mpv sdl3 egl gl fontconfig freetype2 xkbcommon wayland-client x11 dbus-1 openssl; do
    check "pkg-config $module" pkg-config --exists "$module"
done

# Fedora's stock ffmpeg and Mesa VA drivers leave out H.264/HEVC, which the
# Phase 4 "Done when" check needs. These must be the RPM Fusion builds.
check "ffmpeg from RPM Fusion"        rpm -q ffmpeg-libs
check "no ffmpeg-free"                bash -c '! rpm -q libavcodec-free'
check "mesa-va-drivers-freeworld"     rpm -q mesa-va-drivers-freeworld
check "vainfo"                        command -v vainfo

if ((failures > 0)); then
    printf '\n%d check(s) failed\n' "$failures"
    exit 1
fi
printf '\nall checks passed\n'
