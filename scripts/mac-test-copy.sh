#!/usr/bin/env bash
# Opens a test copy of the Mac app with its own library folder, on the Mac's screen:
#   scripts/mac-test-copy.sh <Needle.app> <library folder> [more arguments for Needle]
# The folder gets a secrets.json ({}) first, so the test copy keeps its sign-ins there and
# never reads or writes the Keychain: no Keychain questions for the person at the Mac, and the
# real app's sign-ins stay out of the test. Sign in inside the test copy to test an account.
set -euo pipefail

app="$1"
data="$2"
shift 2
mkdir -p "$data"
if [ ! -f "$data/secrets.json" ]; then
    (umask 077 && printf '{}\n' > "$data/secrets.json")
fi
chmod 600 "$data/secrets.json"
open -n "$app" --args --data-dir "$data" "$@"
