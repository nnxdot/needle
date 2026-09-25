#!/usr/bin/env bash
# Clippy and the tests on Linux, in the same Ubuntu 24.04 container the packages are built in.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd -W 2>/dev/null || pwd)
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$here:/src" -v needle-linux-build:/build \
  -e CARGO_HOME=/build/cargo -e CARGO_TARGET_DIR=/build/target -e RUSTUP_HOME=/build/rustup \
  ubuntu:24.04 bash -c '
    export DEBIAN_FRONTEND=noninteractive PATH=/build/cargo/bin:$PATH
    apt-get update -qq && apt-get install -y -qq build-essential pkg-config cmake clang curl git ca-certificates libasound2-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libx11-xcb-dev libxcb1-dev libxi-dev libxcursor-dev libxrandr-dev libfontconfig-dev libfreetype-dev libssl-dev libzstd-dev libvulkan-dev ffmpeg >/dev/null
    rustup component add clippy >/dev/null 2>&1 || true
    cd /src
    cargo clippy --workspace --all-targets 2>&1 | grep -E "^(warning|error)" -A8 | grep -v future || true
    cargo test --workspace 2>&1 | grep -E "test result|FAILED|panicked"'
