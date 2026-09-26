#!/usr/bin/env bash
# Builds dist/Needle-<version>-macos.dmg on a Mac: Needle.app for Apple silicon (ONNX Runtime,
# for stems, has no prebuilt library for Intel Macs), with its Dolby decoder (needle-ffmpeg-macos, see build-ffmpeg-mac.sh) and command line
# (needle-cli). Signed ad hoc (no Apple account, so nothing about one is in it); the install
# script (website/public/install-mac.sh) copies it without the download quarantine.
#
# Needs the command line tools (xcode-select --install) and rustup; full Xcode is not needed.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
export MACOSX_DEPLOYMENT_TARGET=11.0
# No home folder's name in the programs (panic messages carry source paths).
# CARGO_ENCODED_RUSTFLAGS (split on 0x1f, not spaces), so a home folder with a space works.
unset RUSTFLAGS
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$HOME=/build"
target_dir="${CARGO_TARGET_DIR:-$here/target}"

ffmpeg="$here/third-party/ffmpeg/needle-ffmpeg-macos"
[ -f "$ffmpeg" ] || { echo "$ffmpeg is missing: run scripts/build-ffmpeg-mac.sh"; exit 1; }

triple=aarch64-apple-darwin
rustup target add "$triple" > /dev/null
cargo build --release --locked -p needle --target "$triple"

stage="$(mktemp -d /tmp/needle-package.XXXXXX)"
app="$stage/Needle.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
for bin in needle-desktop needle; do
    name=$([ "$bin" = needle-desktop ] && echo Needle || echo needle-cli)
    cp "$target_dir/$triple/release/$bin" "$app/Contents/MacOS/$name"
    strip -x "$app/Contents/MacOS/$name"
done
# Next to Needle, where it looks for its decoder.
lipo -thin arm64 -output "$app/Contents/MacOS/needle-ffmpeg" "$ffmpeg"
chmod 755 "$app/Contents/MacOS/"*

# The icon, from the 1024 px picture, with the sizes macOS asks for.
iconset="$stage/Needle.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    sips -z $size $size crates/needle/assets/needle-1024.png --out "$iconset/icon_${size}x${size}.png" > /dev/null
    double=$((size * 2))
    sips -z $double $double crates/needle/assets/needle-1024.png --out "$iconset/icon_${size}x${size}@2x.png" > /dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/Needle.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Needle</string>
    <key>CFBundleDisplayName</key><string>Needle</string>
    <key>CFBundleIdentifier</key><string>fyi.nnx.Needle</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleExecutable</key><string>Needle</string>
    <key>CFBundleIconFile</key><string>Needle</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.music</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSHumanReadableCopyright</key><string>Copyright © 2026 nnx</string>
</dict>
</plist>
PLIST

# The notices, as the other builds ship them.
mkdir -p "$app/Contents/Resources/licenses"
cp THIRD-PARTY-NOTICES.md third-party/licenses.txt third-party/ffmpeg/COPYING.LGPLv2.1 \
    "$app/Contents/Resources/licenses/"

# No extended attributes (Finder data, quarantine) from this Mac; then sign ad hoc.
xattr -cr "$app"
codesign --force --deep --sign - --identifier fyi.nnx.Needle "$app"
codesign --verify --deep --strict "$app"

# The disk image: the app and a link to Applications, to drag it across.
ln -s /Applications "$stage/Applications"
rm -rf "$iconset"
mkdir -p dist
dmg="dist/Needle-$version-macos.dmg"
rm -f "$dmg"
hdiutil create -quiet -volname "Needle $version" -srcfolder "$stage" -fs HFS+ -format UDZO "$dmg"
rm -rf "$stage"
shasum -a 256 "$dmg"
echo "Built $dmg ($(stat -f %z "$dmg") bytes)"
