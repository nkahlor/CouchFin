FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# System libraries Slint (femtovg + winit) needs to build and run
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
    pkg-config \
    libfontconfig1-dev \
    libxkbcommon-dev libxkbcommon-x11-dev \
    libwayland-dev \
    libx11-dev libxcursor-dev libxrandr-dev libxi-dev \
    libgl1-mesa-dev \
    dbus-x11 \
    libinput10 \
    && rm -rf /var/lib/apt/lists/*
