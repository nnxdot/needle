#!/usr/bin/env bash
# Builds the Mac app (apps/macos, SwiftUI with Liquid Glass) on a Mac with Xcode: Needle's core
# (crates/needle-mobile with its "desktop" feature) for Apple silicon, its Swift bindings, the
# icon and title fonts, then the app with xcodebuild, signed with Needle's own key (see
# mac-sign.sh; no Apple account, so nothing about one is in it).
#   scripts/build-mac-app.sh [release|debug|core]
# The app lands in apps/macos/build/<Release|Debug>/Needle.app, with its Dolby decoder
# (needle-ffmpeg, see build-ffmpeg-mac.sh) beside it when that has been built.
# Needs the Rust target aarch64-apple-darwin (ONNX Runtime, for stems, has no prebuilt library
# for Intel Macs).
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
kind="${1:-release}"
mac="$here/apps/macos"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
export MACOSX_DEPLOYMENT_TARGET=15.0
# No home folder's name in the app (panic messages carry source paths).
unset RUSTFLAGS
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$HOME=/build"
target_dir="${CARGO_TARGET_DIR:-$here/target}"

triple=aarch64-apple-darwin
rustup target add "$triple" > /dev/null
cargo rustc --release --locked -p needle-mobile --lib --crate-type staticlib --features desktop --target "$triple"
lib="$target_dir/$triple/release/libneedle_mobile.a"

# The Swift bindings, and the C header and module map the Swift side imports.
gen="$(mktemp -d /tmp/needle-mac.XXXXXX)"
cargo run -q --release --locked -p needle-mobile --bin uniffi-bindgen -- generate \
    --library "$lib" --language swift --out-dir "$gen" --no-format
mkdir -p "$mac/Needle/Core" "$gen/headers"
cp "$gen/needle_mobile.swift" "$mac/Needle/Core/needle_mobile.swift"
cp "$gen/needle_mobileFFI.h" "$gen/headers/"
cp "$gen/needle_mobileFFI.modulemap" "$gen/headers/module.modulemap"
rm -rf "$mac/NeedleCore.xcframework"
xcodebuild -create-xcframework -library "$lib" -headers "$gen/headers" \
    -output "$mac/NeedleCore.xcframework" > /dev/null
rm -rf "$gen"
[ "$kind" = core ] && exit 0

# The icon, from the 1024 px picture, with the sizes macOS asks for; and the title fonts.
resources="$mac/Needle/Resources"
mkdir -p "$resources"
if [ ! -f "$resources/Needle.icns" ] || [ crates/needle/assets/needle-1024.png -nt "$resources/Needle.icns" ]; then
    iconset="$(mktemp -d /tmp/needle-icon.XXXXXX)/Needle.iconset"
    mkdir -p "$iconset"
    for size in 16 32 128 256 512; do
        sips -z $size $size crates/needle/assets/needle-1024.png --out "$iconset/icon_${size}x${size}.png" > /dev/null
        double=$((size * 2))
        sips -z $double $double crates/needle/assets/needle-1024.png --out "$iconset/icon_${size}x${size}@2x.png" > /dev/null
    done
    iconutil -c icns "$iconset" -o "$resources/Needle.icns"
    rm -rf "$(dirname "$iconset")"
fi
cp crates/needle/fonts/Fraunces72ptSoft-SemiBold.ttf crates/needle/fonts/Fraunces72ptSoft-Bold.ttf "$resources/"

config=$([ "$kind" = debug ] && echo Debug || echo Release)
out="$mac/build/$config"
cd "$mac"
xcodebuild -project Needle.xcodeproj -scheme Needle -configuration "$config" \
    -destination 'platform=macOS,arch=arm64' -derivedDataPath build/derived \
    MARKETING_VERSION="$version" CONFIGURATION_BUILD_DIR="$out" build | grep -E "error|warning:|BUILD" || true
app="$out/Needle.app"
[ -d "$app" ] || { echo "The app was not built"; exit 1; }

# Next to Needle, where its core looks for the decoder.
ffmpeg="$here/third-party/ffmpeg/needle-ffmpeg-macos"
if [ -f "$ffmpeg" ]; then
    lipo -thin arm64 -output "$app/Contents/MacOS/needle-ffmpeg" "$ffmpeg" 2>/dev/null || cp "$ffmpeg" "$app/Contents/MacOS/needle-ffmpeg"
    chmod 755 "$app/Contents/MacOS/needle-ffmpeg"
fi
xattr -cr "$app"
source "$here/scripts/mac-sign.sh"
mac_sign "$app"
echo "Built $app ($version)"
