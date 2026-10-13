#!/usr/bin/env bash
# A test only: builds the Discord status bridge (apps/android/discord/bridge.cpp) for phones and
# the emulator, against Discord's Social SDK. Run after scripts/android-build.sh has filled
# jniLibs, then build the app again.
#   scripts/android-discord.sh
# Needs Discord's SDK for Android from the Developer Portal, saved as
# apps/android/app/libs/discord_partner_sdk.aar, and ANDROID_NDK_HOME.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
app="$here/apps/android/app"
aar="$app/libs/discord_partner_sdk.aar"
[ -f "$aar" ] || { echo "Put Discord's SDK at $aar first." >&2; exit 1; }
sdk="$app/build/discord-sdk"
rm -rf "$sdk"
mkdir -p "$sdk"
unzip -q "$aar" -d "$sdk"
include="$(dirname "$(find "$sdk" -name discordpp.h | head -1)")"

bin="$(ls -d "$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/*/bin | head -1)"
for abi in arm64-v8a x86_64; do
    target=$([ "$abi" = arm64-v8a ] && echo aarch64-linux-android26 || echo x86_64-linux-android26)
    libs="$(dirname "$(find "$sdk" -path "*$abi*" -name libdiscord_partner_sdk.so | head -1)")"
    # Only C types cross into Discord's library (discordpp.h wraps its C interface), so the
    # bridge can carry its own C++ library.
    "$bin/clang++" --target="$target" -std=c++17 -O2 -fPIC -shared -static-libstdc++ \
        -I"$include" "$here/apps/android/discord/bridge.cpp" \
        -L"$libs" -ldiscord_partner_sdk -llog \
        -o "$app/src/main/jniLibs/$abi/libneedle_discord.so"
done
ls -la "$app"/src/main/jniLibs/*/libneedle_discord.so
