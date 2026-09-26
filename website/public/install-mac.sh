#!/bin/bash
# Installs Needle on a Mac with Apple silicon (M1 or newer):
#   curl -fsSL https://needle.nnx.fyi/install-mac.sh | bash
# It downloads the latest Needle from needle.nnx.fyi, checks it against the published SHA-256
# list, and puts Needle.app in Applications. Nothing else is sent or changed.
#
# Needle is signed without an Apple developer account, so a copy downloaded in a browser is
# held back by macOS the first time. A copy installed this way is not, because curl does not
# mark its downloads.
set -euo pipefail

site="https://needle.nnx.fyi"
say() { printf '\033[1m%s\033[0m\n' "$*"; }
fail() { printf 'Needle was not installed: %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = Darwin ] || fail "this script is for macOS."
[ "$(uname -m)" = arm64 ] || fail "Needle for macOS needs a Mac with Apple silicon (M1 or newer)."
major=$(sw_vers -productVersion | cut -d. -f1)
[ "$major" -ge 11 ] || fail "Needle needs macOS 11 (Big Sur) or newer."

work=$(mktemp -d /tmp/needle-install.XXXXXX)
mount="$work/mounted"
cleanup() {
    hdiutil detach -quiet "$mount" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT

say "Finding the latest Needle…"
version=$(curl -fsSL "$site/latest.json" | sed -n 's/.*"tag_name"[^"]*"v\{0,1\}\([0-9][0-9.]*\)".*/\1/p' | head -1)
[ -n "$version" ] || fail "could not read the latest version from $site."
image="Needle-$version-macos.dmg"

say "Downloading Needle $version…"
curl -fL --progress-bar -o "$work/$image" "$site/download/$image" || fail "the download did not work."
curl -fsSL -o "$work/SHA256SUMS.txt" "$site/download/SHA256SUMS.txt" || fail "could not get the checksum list."
expected=$(awk -v name="$image" '$2 == name || $2 == "*" name { print tolower($1) }' "$work/SHA256SUMS.txt")
[ -n "$expected" ] || fail "the checksum list has no entry for $image."
actual=$(shasum -a 256 "$work/$image" | cut -d' ' -f1)
[ "$actual" = "$expected" ] || fail "the download does not match its published checksum."
say "The download matches its published checksum."

mkdir "$mount"
hdiutil attach -quiet -nobrowse -readonly -noautoopen -mountpoint "$mount" "$work/$image" \
    || fail "could not open the disk image."

# Applications when this user can write there (admins can); otherwise their own Applications.
target=/Applications
if [ ! -w "$target" ]; then
    target="$HOME/Applications"
    mkdir -p "$target"
fi
if pgrep -xq Needle; then
    say "Closing Needle…"
    osascript -e 'tell application id "fyi.nnx.Needle" to quit' 2>/dev/null || true
    for _ in $(seq 20); do pgrep -xq Needle || break; sleep 0.5; done
fi
rm -rf "$target/Needle.app.installing"
ditto "$mount/Needle.app" "$target/Needle.app.installing"
xattr -dr com.apple.quarantine "$target/Needle.app.installing" 2>/dev/null || true
rm -rf "$target/Needle.app"
mv "$target/Needle.app.installing" "$target/Needle.app"

say "Needle $version is in $target. Opening it…"
open "$target/Needle.app"
