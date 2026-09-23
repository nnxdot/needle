#!/usr/bin/env bash
# Builds third-party/ffmpeg/needle-ffmpeg.exe: FFmpeg with only what Dolby Digital and
# Dolby Digital Plus (Atmos) music needs — MP4 reading, the AC-3 and E-AC-3 decoders, and
# mixing down to 48 kHz stereo floats on a pipe. LGPL 2.1 or later (no GPL or version 3 parts).
#
# Run in an MSYS2 UCRT64 shell with: pacman -S make diffutils mingw-w64-ucrt-x86_64-gcc
#   mingw-w64-ucrt-x86_64-pkgconf mingw-w64-ucrt-x86_64-nasm
# Optionally set TARBALL to an already downloaded ffmpeg-$VERSION.tar.xz.
set -euo pipefail

VERSION=7.1.1
SHA256=733984395e0dbbe5c046abda2dc49a5544e7e0e1e2366bba849222ae9e3a03b1
here="$(cd "$(dirname "$0")/.." && pwd)"
work="${WORK:-$(mktemp -d)}"
tarball="${TARBALL:-$work/ffmpeg-$VERSION.tar.xz}"

if [ ! -f "$tarball" ]; then
    curl -L -o "$tarball" "https://ffmpeg.org/releases/ffmpeg-$VERSION.tar.xz"
fi
echo "$SHA256  $tarball" | sha256sum -c -
rm -rf "$work/ffmpeg-$VERSION"
tar -xf "$tarball" -C "$work"
cd "$work/ffmpeg-$VERSION"

./configure \
    --disable-everything --disable-autodetect --disable-network --disable-doc \
    --disable-debug --disable-ffplay --disable-ffprobe --disable-avdevice \
    --disable-swscale --disable-postproc --enable-small --enable-w32threads \
    --enable-demuxer=mov --enable-decoder=ac3,eac3 --enable-parser=ac3 \
    --enable-protocol=file,pipe --enable-encoder=pcm_f32le --enable-muxer=pcm_f32le \
    --enable-swresample \
    --enable-filter=aresample,aformat,anull,abuffer,abuffersink \
    --extra-ldflags=-static --pkg-config-flags=--static
make -j"$(nproc)" ffmpeg.exe
strip ffmpeg.exe

mkdir -p "$here/third-party/ffmpeg"
cp ffmpeg.exe "$here/third-party/ffmpeg/needle-ffmpeg.exe"
cp COPYING.LGPLv2.1 "$here/third-party/ffmpeg/COPYING.LGPLv2.1"
./ffmpeg.exe -hide_banner -version | head -3 > "$here/third-party/ffmpeg/VERSION.txt"
echo "Built $here/third-party/ffmpeg/needle-ffmpeg.exe ($(stat -c %s ffmpeg.exe) bytes)"
