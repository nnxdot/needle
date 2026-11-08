#!/usr/bin/env bash
# Builds the iPhone app on a Mac with Xcode: Needle's core (crates/needle-mobile) for iPhones and
# the simulator, its Swift bindings, then the app with xcodebuild.
#   scripts/build-ios.sh [sim|device|ipa|testflight|core]
# sim: the app for the simulator (apps/ios/build/sim). device: an unsigned build for a phone
# (apps/ios/build/device), to sign in Xcode. ipa: that build as apps/ios/build/Needle.ipa, for
# SideStore and AltStore, which sign it on each phone with the person's own Apple ID. testflight:
# the App Store build (see below). core: only the core and its bindings.
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

# Needle's fonts for titles and text (SIL Open Font License; see assets/fonts).
mkdir -p "$ios/Needle/Fonts"
cp "$here/assets/fonts/"*.ttf "$here/assets/fonts/"*-OFL.txt "$ios/Needle/Fonts/"

cd "$ios"
if [ "$kind" = testflight ]; then
    # The App Store build for TestFlight, with the Watch app, signed by the paid team signed in
    # to Xcode. Run it in the Mac's own session (Terminal), not over ssh: signing needs the login
    # keychain. The team id comes from outside the code. Each upload needs a new, higher build
    # number: the time, unless NEEDLE_BUILD gives one. NEEDLE_UPLOAD=1 sends the build to App
    # Store Connect; without it, the signed .ipa only lands in apps/ios/build/testflight.
    # NEEDLE_ID is the app's id (fyi.nnx.needle2 on the TestFlight team, because another team
    # holds fyi.nnx.needle); the widgets, Watch app, and shared folder follow it.
    team="${NEEDLE_TEAM:?NEEDLE_TEAM must hold the Apple team id}"
    needle_id="${NEEDLE_ID:-fyi.nnx.needle2}"
    build="${NEEDLE_BUILD:-$(date -u +%Y%m%d%H%M)}"
    destination=export
    [ "${NEEDLE_UPLOAD:-0}" = 1 ] && destination=upload
    rm -rf build/Needle.xcarchive build/testflight
    xcodebuild -project Needle.xcodeproj -scheme Needle -configuration Release \
        -destination 'generic/platform=iOS' -derivedDataPath build/derived \
        -archivePath build/Needle.xcarchive -allowProvisioningUpdates \
        DEVELOPMENT_TEAM="$team" NEEDLE_ID="$needle_id" CURRENT_PROJECT_VERSION="$build" \
        archive | tail -3
    options="$(mktemp /tmp/needle-testflight.XXXXXX)"
    cat > "$options" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key><string>app-store-connect</string>
    <key>destination</key><string>$destination</string>
    <key>teamID</key><string>$team</string>
    <key>signingStyle</key><string>automatic</string>
    <key>manageAppVersionAndBuildNumber</key><false/>
    <key>uploadSymbols</key><true/>
</dict>
</plist>
EOF
    xcodebuild -exportArchive -archivePath build/Needle.xcarchive -exportOptionsPlist "$options" \
        -exportPath build/testflight -allowProvisioningUpdates | tail -3
    rm -f "$options"
    echo "build $build: $destination done"
    exit 0
fi
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
# The entitlements files name the shared folder with $(NEEDLE_ID); codesign takes them as written.
ents="$(mktemp -d /tmp/needle-ents.XXXXXX)"
for file in Needle NeedleWidgets; do
    sed 's/\$(NEEDLE_ID)/fyi.nnx.needle/g' "$file.entitlements" > "$ents/$file.entitlements"
done
codesign --force --sign - --timestamp=none --entitlements "$ents/NeedleWidgets.entitlements" "$app/PlugIns/NeedleWidgets.appex"
# SideStore and AltStore sign only PlugIns and Frameworks, so a Watch app would stay unsigned for
# them and could stop the iPhone app installing or updating.
rm -rf "$app/Watch"
codesign --force --sign - --timestamp=none --entitlements "$ents/Needle.entitlements" "$app"
rm -rf "$ents"
(cd build/ipa && zip -qry ../Needle.ipa Payload)
rm -rf build/ipa
echo "$ios/build/Needle.ipa"
