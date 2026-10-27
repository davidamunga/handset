#!/usr/bin/env bash
# Tests for scripts/update-key.sh, with a throwaway HOME so the real signing key is never touched.
# Run: scripts/test-update-key.sh   (needs OpenSSL 3; set OPENSSL=/path/to/openssl otherwise)
set -uo pipefail

script="$(cd "$(dirname "$0")" && pwd)/update-key.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export HOME="$tmp/home"; mkdir -p "$HOME"
unset UPDATE_SIGNING_KEY
openssl_bin="${OPENSSL:-openssl}"
pass=0 fail=0

ok() { pass=$((pass + 1)); echo "ok   - $1"; }
bad() { fail=$((fail + 1)); echo "FAIL - $1"; }
expect() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$d"; else bad "$d"; fi; }
refuse() { local d="$1"; shift; if "$@" >/dev/null 2>&1; then bad "$d"; else ok "$d"; fi; }

key="$HOME/.config/handset/update-signing-key.pem"

refuse "signing without a key says so" "$script" sign "$tmp/nothing"
out="$("$script" generate 2>&1)"; code=$?
[ $code = 0 ] && [ -f "$key" ] && ok "generate makes a key" || bad "generate makes a key: $out"
# the permission string from ls is the same on macOS and Linux (stat's flags are not)
[ "$(ls -l "$key" | cut -c1-10)" = "-rw-------" ] && ok "the key file is private to its owner" || bad "the key file is private to its owner"
pub="$("$script" public)"
[[ "$pub" =~ ^[0-9a-f]{64}$ ]] && ok "the public key is 64 hex characters" || bad "the public key is 64 hex characters: $pub"
case "$out" in *"$pub"*) ok "generate prints the public key" ;; *) bad "generate prints the public key" ;; esac
case "$out" in *"BACK THIS FILE UP"*) ok "generate reminds you to back the key up" ;; *) bad "generate reminds you to back the key up" ;; esac
refuse "generate never overwrites an existing key" "$script" generate
[ "$("$script" public)" = "$pub" ] && ok "the key is unchanged after a refused generate" || bad "the key is unchanged after a refused generate"

printf 'abc  file.zip\n' > "$tmp/SHA256SUMS"
expect "sign writes a signature" "$script" sign "$tmp/SHA256SUMS"
[ "$(wc -c < "$tmp/SHA256SUMS.sig" | tr -d ' ')" = 64 ] && ok "the signature is the raw 64 bytes" || bad "the signature is the raw 64 bytes"

# an independent check with openssl itself: derive the public key file and verify
"$openssl_bin" pkey -in "$key" -pubout -out "$tmp/pub.pem" 2>/dev/null
expect "openssl verifies the signature" "$openssl_bin" pkeyutl -verify -pubin -inkey "$tmp/pub.pem" -rawin -in "$tmp/SHA256SUMS" -sigfile "$tmp/SHA256SUMS.sig"
printf 'tampered\n' >> "$tmp/SHA256SUMS"
refuse "openssl refuses it once the file changes" "$openssl_bin" pkeyutl -verify -pubin -inkey "$tmp/pub.pem" -rawin -in "$tmp/SHA256SUMS" -sigfile "$tmp/SHA256SUMS.sig"

# the key can come from the environment, as in the release workflow
export UPDATE_SIGNING_KEY="$(cat "$key")"
rm "$key"
[ "$("$script" public)" = "$pub" ] && ok "the key can come from UPDATE_SIGNING_KEY" || bad "the key can come from UPDATE_SIGNING_KEY"
unset UPDATE_SIGNING_KEY

# check: a key the app does not trust is reported
"$script" generate >/dev/null 2>&1 || true
refuse "check refuses a key that is not built into the app" "$script" check
refuse "an unknown command prints usage" "$script" nonsense

echo "$pass passed, $fail failed"
[ "$fail" = 0 ]
