#!/usr/bin/env bash
# Builds the iPhone app on a Mac with Xcode: Needle's core (crates/needle-mobile) for iPhones and
# the simulator, its Swift bindings, then the app with xcodebuild.
#   scripts/build-ios.sh [sim|device|ipa|core]
# sim: the app for the simulator (apps/ios/build/sim). device: an unsigned build for a phone
# (apps/ios/build/device), to sign in Xcode. ipa: that build as apps/ios/build/Needle.ipa, for
# SideStore and AltStore, which sign it on each phone with the person's own Apple ID. core:
# only the core and its bindings.
# Needs the Rust targets aarch64-apple-ios and aarch64-apple-ios-sim.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
# Keys built into the app (fanart.tv's, for artists' logos), from a private file outside the
# code: lines such as `export NEEDLE_FANART_KEY=...`. Without it, people add their own in Settings.
if [ -f "$HOME/.needle-keys" ]; then
    # shellcheck disable=SC1091
    . "$HOME/.needle-keys"
fi
kind="${1:-sim}"
ios="$here/apps/ios"
export IPHONEOS_DEPLOYMENT_TARGET=26.0
# No home folder's name in the app (panic messages carry source paths).
unset RUSTFLAGS
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$HOME=/build"
target_dir="${CARGO_TARGET_DIR:-$here/target}"

triples=(aarch64-apple-ios aarch64-apple-ios-sim)
for triple in "${triples[@]}"; do
    cargo rustc --release --locked -p needle-mobile --lib --crate-type staticlib --target "$triple"
done

# The Swift bindings, and the C header and module map the Swift side imports.
gen="$(mktemp -d /tmp/needle-ios.XXXXXX)"
cargo run -q --release -p needle-mobile --bin uniffi-bindgen -- generate \
    --library "$target_dir/aarch64-apple-ios-sim/release/libneedle_mobile.a" \
    --language swift --out-dir "$gen" --no-format
mkdir -p "$ios/Needle/Core"
cp "$gen/needle_mobile.swift" "$ios/Needle/Core/needle_mobile.swift"
mkdir -p "$gen/headers"
cp "$gen/needle_mobileFFI.h" "$gen/headers/"
cp "$gen/needle_mobileFFI.modulemap" "$gen/headers/module.modulemap"

rm -rf "$ios/NeedleCore.xcframework"
args=()
for triple in "${triples[@]}"; do
    args+=(-library "$target_dir/$triple/release/libneedle_mobile.a" -headers "$gen/headers")
done
xcodebuild -create-xcframework "${args[@]}" -output "$ios/NeedleCore.xcframework" > /dev/null
rm -rf "$gen"
[ "$kind" = core ] && exit 0

cd "$ios"
if [ "$kind" = device ] || [ "$kind" = ipa ]; then
    xcodebuild -project Needle.xcodeproj -scheme Needle -configuration Release \
        -sdk iphoneos -destination 'generic/platform=iOS' -derivedDataPath build/derived \
        CODE_SIGNING_ALLOWED=NO CONFIGURATION_BUILD_DIR="$ios/build/device" build | tail -3
else
    xcodebuild -project Needle.xcodeproj -scheme Needle -configuration Debug \
        -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' -derivedDataPath build/derived \
        CONFIGURATION_BUILD_DIR="$ios/build/sim" build | tail -3
fi
[ "$kind" = ipa ] || exit 0

# No Apple ID or team: only an ad hoc signature, which writes down the entitlements (the
# widgets' shared folder) for SideStore and AltStore to ask for when they sign.
rm -rf build/ipa build/Needle.ipa
mkdir -p build/ipa/Payload
cp -R build/device/Needle.app build/ipa/Payload/
app=build/ipa/Payload/Needle.app
for framework in "$app"/Frameworks/*.framework; do
    if [ -e "$framework" ]; then
        codesign --force --sign - --timestamp=none "$framework"
    fi
done
codesign --force --sign - --timestamp=none --entitlements NeedleWidgets.entitlements "$app/PlugIns/NeedleWidgets.appex"
codesign --force --sign - --timestamp=none --entitlements Needle.entitlements "$app"
(cd build/ipa && zip -qry ../Needle.ipa Payload)
rm -rf build/ipa
echo "$ios/build/Needle.ipa"
