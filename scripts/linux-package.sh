#!/usr/bin/env bash
# Package the Linux build in dist/linux/bin as a .deb and an .rpm, in dist/linux. Runs inside
# the same Ubuntu container as linux-compile.sh (see build-linux.sh).
#
# Names must stay as they are: Needle's updater looks for needle_<version>_amd64.deb and
# needle-<version>-1.x86_64.rpm (crates/needle-core/src/update.rs).
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
cd /src
version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
arch_deb=amd64
arch_rpm=x86_64
out=/src/dist/linux
stage=$(mktemp -d)
root="$stage/root"

# The files, where Linux expects them.
install -Dm755 "$out/bin/needle-desktop" "$root/usr/bin/needle"
install -Dm755 "$out/bin/needle" "$root/usr/bin/needle-cli"
install -Dm644 packaging/linux/fyi.nnx.Needle.desktop "$root/usr/share/applications/fyi.nnx.Needle.desktop"
install -Dm644 packaging/linux/fyi.nnx.Needle.metainfo.xml "$root/usr/share/metainfo/fyi.nnx.Needle.metainfo.xml"
for icon in packaging/linux/icons/hicolor/*/apps/fyi.nnx.Needle.png; do
  size=$(basename "$(dirname "$(dirname "$icon")")")
  install -Dm644 "$icon" "$root/usr/share/icons/hicolor/$size/apps/fyi.nnx.Needle.png"
done
printf "Needle %s
Copyright (c) 2026 nnx. All rights reserved.
https://needle.nnx.fyi

Third-party components and their licenses: THIRD-PARTY-NOTICES.md and licenses.txt in this folder.
" "$version" > "$stage/copyright"
install -Dm644 "$stage/copyright" "$root/usr/share/doc/needle/copyright"
install -Dm644 THIRD-PARTY-NOTICES.md "$root/usr/share/doc/needle/THIRD-PARTY-NOTICES.md"
install -Dm644 third-party/licenses.txt "$root/usr/share/doc/needle/licenses.txt"

summary="A fast, careful player for the music you own"
description="Needle plays the music on your computer, lossless and private: synced lyrics, smart playlists, themes, plugins, speakers, and your own music server."

# ---- .deb: the libraries it needs, worked out from the program itself.
mkdir -p "$stage/debian"
printf 'Source: needle\n' > "$stage/debian/control"
depends=$(cd "$stage" && dpkg-shlibdeps -O -e root/usr/bin/needle -e root/usr/bin/needle-cli 2>/dev/null | sed -n 's/^shlibs:Depends=//p')
size=$(du -sk "$root/usr" | cut -f1)
mkdir -p "$root/DEBIAN"
cat > "$root/DEBIAN/control" <<CONTROL
Package: needle
Version: $version
Architecture: $arch_deb
Maintainer: nnx <dot@nnx.fyi>
Installed-Size: $size
Depends: $depends
Recommends: ffmpeg, xdg-desktop-portal, mesa-vulkan-drivers
Section: sound
Priority: optional
Homepage: https://needle.nnx.fyi
Description: $summary
 $description
CONTROL
dpkg-deb --build --root-owner-group "$root" "$out/needle_${version}_${arch_deb}.deb" >/dev/null
rm -rf "$root/DEBIAN"

# ---- .rpm: rpmbuild finds the libraries it needs by itself.
mkdir -p "$stage/rpm"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
cat > "$stage/rpm/SPECS/needle.spec" <<SPEC
Name: needle
Version: $version
Release: 1
Summary: $summary
License: Proprietary
URL: https://needle.nnx.fyi
Recommends: (ffmpeg-free or ffmpeg)
Recommends: xdg-desktop-portal
Recommends: mesa-vulkan-drivers
%global debug_package %{nil}
%global __strip /bin/true

%description
$description

%install
mkdir -p %{buildroot}
cp -a $root/. %{buildroot}/

%files
/usr/bin/needle
/usr/bin/needle-cli
/usr/share/applications/fyi.nnx.Needle.desktop
/usr/share/metainfo/fyi.nnx.Needle.metainfo.xml
/usr/share/icons/hicolor/*/apps/fyi.nnx.Needle.png
%doc /usr/share/doc/needle
SPEC
rpmbuild --quiet --define "_topdir $stage/rpm" --target "$arch_rpm" -bb "$stage/rpm/SPECS/needle.spec" 2>&1 | grep -v "^warning" || true
cp "$stage/rpm/RPMS/$arch_rpm/needle-${version}-1.${arch_rpm}.rpm" "$out/"

rm -rf "$stage"
cd "$out"
ls -la ./*.deb ./*.rpm
echo "deb depends: $depends"
rpm -qpR "needle-${version}-1.${arch_rpm}.rpm" | grep -v '^rpmlib' | tr '\n' ' '
echo
