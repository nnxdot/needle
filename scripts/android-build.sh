#!/usr/bin/env bash
# Builds the Android app: Needle's core (crates/needle-mobile) for phones, 32-bit TVs, and
# the emulator, its Kotlin bindings, then the app with Gradle (64-bit; -PneedleAbis=armeabi-v7a
# packages the 32-bit APK from the same libraries).
#   scripts/android-build.sh [debug|release]
# Needs ANDROID_HOME, ANDROID_NDK_HOME, and JAVA_HOME (see MOBILE.md), cargo-ndk, and the Rust
# targets aarch64-linux-android, armv7-linux-androideabi, and x86_64-linux-android.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
cd "$here"
kind="${1:-debug}"
app="$here/apps/android/app"
# On Windows (Git Bash), give the tools Windows paths: with MSYS_NO_PATHCONV set, a path like
# /c/Users/... would otherwise be read as C:\c\Users\...
if command -v cygpath > /dev/null; then app="$(cygpath -m "$app")"; fi

cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -P 26 -o "$app/src/main/jniLibs" \
    build --release -p needle-mobile
# Needle's Dolby decoder (see build-ffmpeg-android.sh), beside the core.
for abi in arm64-v8a armeabi-v7a x86_64; do
    cp "$here/third-party/ffmpeg/android/$abi/libneedle_ffmpeg.so" "$app/src/main/jniLibs/$abi/"
done
cargo run -q -p needle-mobile --bin uniffi-bindgen -- generate \
    --library target/x86_64-linux-android/release/libneedle_mobile.so \
    --language kotlin \
    --out-dir "$app/src/main/java" --no-format

# Discord's library is not in the repository; without it the APK has no Discord status.
if [ "$kind" = release ] && [ ! -f "$here/apps/android/app/libs/discord_partner_sdk.aar" ]; then
    echo "warning: apps/android/app/libs/discord_partner_sdk.aar is missing: this APK has no Discord status" >&2
fi

cd "$here/apps/android"
task=$([ "$kind" = release ] && echo assembleRelease || echo assembleDebug)
./gradlew --quiet "$task"
ls -la app/build/outputs/apk/"$kind"/*.apk
