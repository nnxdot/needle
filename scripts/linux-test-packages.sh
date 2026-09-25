#!/usr/bin/env bash
# Install the built packages in clean Ubuntu 24.04 and Fedora containers and check that
# Needle starts (needle-cli --version) and that the updater sees how it was installed.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd -W 2>/dev/null || pwd)
version=$(grep -m1 '^version' "$(dirname "$0")/../Cargo.toml" | cut -d'"' -f2)
echo "== Ubuntu 24.04 (.deb)"
MSYS_NO_PATHCONV=1 docker run --rm -v "$here/dist/linux:/pkg:ro" ubuntu:24.04 bash -c "
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq >/dev/null && apt-get install -y -qq /pkg/needle_${version}_amd64.deb >/dev/null 2>&1
  needle-cli --version && dpkg-query -S /usr/bin/needle && ls /usr/share/applications/fyi.nnx.Needle.desktop /usr/share/icons/hicolor/256x256/apps/fyi.nnx.Needle.png"
echo "== Fedora (.rpm)"
MSYS_NO_PATHCONV=1 docker run --rm -v "$here/dist/linux:/pkg:ro" fedora:latest bash -c "
  dnf install -y -q /pkg/needle-${version}-1.x86_64.rpm >/dev/null 2>&1
  needle-cli --version && rpm -qf /usr/bin/needle && cat /etc/fedora-release"
