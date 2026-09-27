#!/usr/bin/env bash
# Builds Needle's small FFmpeg (Dolby Digital and Dolby Digital Plus only; see build-ffmpeg.sh)
# for Android, as third-party/ffmpeg/android/<abi>/libneedle_ffmpeg.so. Android runs programs
# only from an app's own library folder, so android-build.sh puts it there under a library's
# name.
# LGPL 2.1 or later.
#
# Run in MSYS2 (make, diffutils) with ANDROID_NDK_HOME set, as the NDK's Windows path.
set -euo pipefail

VERSION=7.1.1
SHA256=733984395e0dbbe5c046abda2dc49a5544e7e0e1e2366bba849222ae9e3a03b1
here="$(cd "$(dirname "$0")/.." && pwd)"
work="${WORK:-/tmp/needle-ffmpeg-android}"
mkdir -p "$work"
tarball="${TARBALL:-$work/ffmpeg-$VERSION.tar.xz}"
ndk="$(cygpath -u "${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME}")"
bin="$ndk/toolchains/llvm/prebuilt/windows-x86_64/bin"
api=26

if [ ! -f "$tarball" ]; then
    curl -L -o "$tarball" "https://ffmpeg.org/releases/ffmpeg-$VERSION.tar.xz"
fi
echo "$SHA256  $tarball" | sha256sum -c -

for abi in arm64-v8a x86_64; do
    case $abi in
        arm64-v8a) arch=aarch64; triple=aarch64-linux-android ;;
        x86_64) arch=x86_64; triple=x86_64-linux-android ;;
    esac
    rm -rf "$work/$abi"
    mkdir -p "$work/$abi"
    tar -xf "$tarball" -C "$work/$abi"
    (
        cd "$work/$abi/ffmpeg-$VERSION"
        ./configure \
            --enable-cross-compile --target-os=android --arch="$arch" \
            --cc="$bin/$triple$api-clang.cmd" --cxx="$bin/$triple$api-clang++.cmd" \
            --ar="$bin/llvm-ar.exe" --nm="$bin/llvm-nm.exe" --ranlib="$bin/llvm-ranlib.exe" \
            --strip="$bin/llvm-strip.exe" --disable-x86asm --disable-asm \
            --host-cc=/ucrt64/bin/gcc \
            --disable-everything --disable-autodetect --disable-network --disable-doc \
            --disable-debug --disable-ffplay --disable-ffprobe --disable-avdevice \
            --disable-swscale --disable-postproc --enable-small --enable-pthreads \
            --enable-demuxer=mov --enable-decoder=ac3,eac3 --enable-parser=ac3 \
            --enable-protocol=file,pipe --enable-encoder=pcm_f32le --enable-muxer=pcm_f32le \
            --enable-swresample \
            --enable-filter=aresample,aformat,anull,abuffer,abuffersink \
            --extra-ldflags="-static-libstdc++"
        make -j"$(nproc)" ffmpeg
        "$bin/llvm-strip.exe" ffmpeg
    )
    out="$here/third-party/ffmpeg/android/$abi"
    mkdir -p "$out"
    cp "$work/$abi/ffmpeg-$VERSION/ffmpeg" "$out/libneedle_ffmpeg.so"
    echo "Built $out/libneedle_ffmpeg.so ($(stat -c %s "$out/libneedle_ffmpeg.so") bytes)"
done
