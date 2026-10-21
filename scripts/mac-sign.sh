# Sourced by the Mac build scripts: mac_sign <app> signs the app, and the programs inside it.
#
# With Needle's own signing key on this Mac (scripts/make-mac-signing-key.sh makes it, in
# ~/.needle-signing), every build carries the same signature, so the Keychain knows each new
# build and each update as the same app: "Always Allow" is asked once per key, not after every
# build. Without the key, the app is signed ad hoc, and the Keychain asks again each time.
# The key's certificate is named only "Needle"; nothing about an Apple account or this Mac is
# in it.

signing="${NEEDLE_SIGNING_DIR:-$HOME/.needle-signing}"

mac_sign() {
    local app="$1"
    if [ -f "$signing/needle.keychain-db" ] && [ -f "$signing/keychain.pass" ]; then
        local keychain="$signing/needle.keychain-db"
        security unlock-keychain -p "$(cat "$signing/keychain.pass")" "$keychain"
        local identity
        identity=$(security find-identity -p codesigning "$keychain" | awk '/"Needle"/ { print $2; exit }')
        [ -n "$identity" ] || { echo "No \"Needle\" signing key in $keychain"; return 1; }
        codesign --force --deep --sign "$identity" --keychain "$keychain" --identifier fyi.nnx.Needle "$app"
    else
        echo "No signing key in $signing (see scripts/make-mac-signing-key.sh): signing ad hoc."
        codesign --force --deep --sign - --identifier fyi.nnx.Needle "$app"
    fi
}
