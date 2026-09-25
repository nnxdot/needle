#!/usr/bin/env bash
# Build Needle's Linux packages (dist/linux/needle_<version>_amd64.deb and
# needle-<version>-1.x86_64.rpm) in an Ubuntu 24.04 container. Needs Docker. The compiled
# code is kept in the Docker volume needle-linux-build, so later builds are faster; remove it
# with: docker volume rm needle-linux-build
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd -W 2>/dev/null || pwd)
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$here:/src" -v needle-linux-build:/build \
  -e CARGO_HOME=/build/cargo -e CARGO_TARGET_DIR=/build/target -e RUSTUP_HOME=/build/rustup \
  ubuntu:24.04 bash -c "bash /src/scripts/linux-compile.sh && bash /src/scripts/linux-package.sh"
