#!/usr/bin/env bash
# Makes Needle's own signing key on this Mac, once: a self-signed code signing certificate
# named "Needle", in its own keychain in ~/.needle-signing (with a random password beside it,
# so builds over SSH can open it). scripts/mac-sign.sh signs every build with it, so the
# Keychain knows new builds and updates as the same app.
#
# Keep ~/.needle-signing: a new key makes the Keychain ask once more for each saved key.
# Nothing in it names you or this Mac.
set -euo pipefail

dir="${NEEDLE_SIGNING_DIR:-$HOME/.needle-signing}"
[ -e "$dir" ] && { echo "$dir exists already"; exit 1; }
mkdir -m 700 "$dir"
cd "$dir"
umask 077

openssl rand -hex 24 > keychain.pass
openssl req -x509 -newkey rsa:3072 -keyout key.pem -out cert.pem -days 7300 -nodes -subj "/CN=Needle" \
    -addext "keyUsage=critical,digitalSignature" -addext "extendedKeyUsage=critical,codeSigning" \
    -addext "basicConstraints=critical,CA:false"
bundle_pass=$(openssl rand -hex 16)
openssl pkcs12 -export -inkey key.pem -in cert.pem -name Needle -out needle.p12 -passout "pass:$bundle_pass"

keychain="$dir/needle.keychain-db"
keychain_pass=$(cat keychain.pass)
security create-keychain -p "$keychain_pass" "$keychain"
security set-keychain-settings "$keychain"
security unlock-keychain -p "$keychain_pass" "$keychain"
security import needle.p12 -k "$keychain" -P "$bundle_pass" -T /usr/bin/codesign > /dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$keychain_pass" "$keychain" > /dev/null
# The private key lives in the keychain only.
rm needle.p12 key.pem
security find-identity -p codesigning "$keychain"
