#!/usr/bin/env bash
# Compile Needle for Linux inside Ubuntu 24.04 (glibc 2.39, which ONNX Runtime needs: the build
# runs on Ubuntu 24.04 and newer, Mint 22 and newer, Debian 13, and Fedora 40 and newer).
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq build-essential pkg-config cmake clang curl git ca-certificates \
  libasound2-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev \
  libx11-xcb-dev libxcb1-dev libxi-dev libxcursor-dev libxrandr-dev libfontconfig-dev \
  libfreetype-dev libssl-dev libzstd-dev libvulkan-dev rpm file >/dev/null
if [ ! -x "$CARGO_HOME/bin/cargo" ]; then
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path >/dev/null
fi
export PATH="$CARGO_HOME/bin:$PATH"
cd /src
cargo build --release --locked -p needle --bins
strip "$CARGO_TARGET_DIR/release/needle-desktop" "$CARGO_TARGET_DIR/release/needle"
mkdir -p /src/dist/linux/bin
cp "$CARGO_TARGET_DIR/release/needle-desktop" "$CARGO_TARGET_DIR/release/needle" /src/dist/linux/bin/
echo "NEEDED:"
objdump -p /src/dist/linux/bin/needle-desktop | grep NEEDED
objdump -T /src/dist/linux/bin/needle-desktop | grep -o 'GLIBC_[0-9.]*' | sort -Vu | tail -1
