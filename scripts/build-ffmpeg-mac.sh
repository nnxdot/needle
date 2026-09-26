#!/usr/bin/env bash
# Builds third-party/ffmpeg/needle-ffmpeg-macos: the macOS twin of needle-ffmpeg.exe (see
# build-ffmpeg.sh), with only what Dolby Digital and Dolby Digital Plus (Atmos) music needs.
# One file for both Apple silicon and Intel Macs. LGPL 2.1 or later.
#
# Run on a Mac with the command line tools (xcode-select --install); full Xcode is not needed.
# Optionally set TARBALL to an already downloaded ffmpeg-$VERSION.tar.xz.
set -euo pipefail

VERSION=7.1.1
SHA256=733984395e0dbbe5c046abda2dc49a5544e7e0e1e2366bba849222ae9e3a03b1
here="$(cd "$(dirname "$0")/.." && pwd)"
# A fixed folder, so no home folder's name is written into the program.
work="${WORK:-/tmp/needle-ffmpeg}"
mkdir -p "$work"
tarball="${TARBALL:-$work/ffmpeg-$VERSION.tar.xz}"
export MACOSX_DEPLOYMENT_TARGET=11.0

if [ ! -f "$tarball" ]; then
    curl -L -o "$tarball" "https://ffmpeg.org/releases/ffmpeg-$VERSION.tar.xz"
fi
echo "$SHA256  $tarball" | shasum -a 256 -c -

for arch in arm64 x86_64; do
    rm -rf "$work/$arch"
    mkdir -p "$work/$arch"
    tar -xf "$tarball" -C "$work/$arch"
    (
        cd "$work/$arch/ffmpeg-$VERSION"
        ./configure \
            --enable-cross-compile --target-os=darwin --arch="$arch" \
            --cc="clang -arch $arch" --disable-x86asm \
            --disable-everything --disable-autodetect --disable-network --disable-doc \
            --disable-debug --disable-ffplay --disable-ffprobe --disable-avdevice \
            --disable-swscale --disable-postproc --enable-small --enable-pthreads \
            --enable-demuxer=mov --enable-decoder=ac3,eac3 --enable-parser=ac3 \
            --enable-protocol=file,pipe --enable-encoder=pcm_f32le --enable-muxer=pcm_f32le \
            --enable-swresample \
            --enable-filter=aresample,aformat,anull,abuffer,abuffersink
        make -j"$(sysctl -n hw.ncpu)" ffmpeg
        strip ffmpeg
    )
done

mkdir -p "$here/third-party/ffmpeg"
out="$here/third-party/ffmpeg/needle-ffmpeg-macos"
lipo -create -output "$out" \
    "$work/arm64/ffmpeg-$VERSION/ffmpeg" "$work/x86_64/ffmpeg-$VERSION/ffmpeg"
"$out" -hide_banner -version | head -3
echo "Built $out ($(stat -f %z "$out") bytes, $(lipo -archs "$out"))"
