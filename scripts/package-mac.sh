#!/usr/bin/env bash
# Builds dist/Needle-<version>-macos.dmg on a Mac: the Mac app (apps/macos, SwiftUI with Liquid
# Glass, see build-mac-app.sh) for Apple silicon (ONNX Runtime, for stems, has no prebuilt
# library for Intel Macs), with its Dolby decoder (needle-ffmpeg-macos, see build-ffmpeg-mac.sh)
# and command line (needle-cli). Signed with Needle's own key (see mac-sign.sh; no Apple
# account, so nothing about one is in it); the install script (website/public/install-mac.sh)
# copies it without the download quarantine.
#
# Needs Xcode (for the app) and rustup.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
# No home folder's name in the programs (panic messages carry source paths).
# CARGO_ENCODED_RUSTFLAGS (split on 0x1f, not spaces), so a home folder with a space works.
unset RUSTFLAGS
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$HOME=/build"
target_dir="${CARGO_TARGET_DIR:-$here/target}"

ffmpeg="$here/third-party/ffmpeg/needle-ffmpeg-macos"
[ -f "$ffmpeg" ] || { echo "$ffmpeg is missing: run scripts/build-ffmpeg-mac.sh"; exit 1; }

bash scripts/build-mac-app.sh release

# The command line, as before.
triple=aarch64-apple-darwin
MACOSX_DEPLOYMENT_TARGET=15.0 cargo build --release --locked -p needle --bin needle --target "$triple"

stage="$(mktemp -d /tmp/needle-package.XXXXXX)"
app="$stage/Needle.app"
ditto --norsrc --noextattr --noqtn apps/macos/build/Release/Needle.app "$app"
cp "$target_dir/$triple/release/needle" "$app/Contents/MacOS/needle-cli"
strip -x "$app/Contents/MacOS/needle-cli"
chmod 755 "$app/Contents/MacOS/"*

# The notices, as the other builds ship them.
mkdir -p "$app/Contents/Resources/licenses"
cp THIRD-PARTY-NOTICES.md third-party/licenses.txt third-party/ffmpeg/COPYING.LGPLv2.1 \
    "$app/Contents/Resources/licenses/"

# No extended attributes (Finder data, quarantine) from this Mac; then sign.
xattr -cr "$app"
source scripts/mac-sign.sh
mac_sign "$app"
codesign --verify --deep --strict "$app"

# The disk image: the app and a link to Applications, to drag it across.
ln -s /Applications "$stage/Applications"
mkdir -p dist
dmg="dist/Needle-$version-macos.dmg"
rm -f "$dmg"
hdiutil create -quiet -volname "Needle $version" -srcfolder "$stage" -fs HFS+ -format UDZO "$dmg"
rm -rf "$stage"
shasum -a 256 "$dmg"
echo "Built $dmg ($(stat -f %z "$dmg") bytes)"
