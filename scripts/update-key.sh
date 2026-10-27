#!/usr/bin/env bash
# The key that signs Handset releases, so that the app only installs updates that really come from this project.
#
#   scripts/update-key.sh generate [--set-secret]   make a key (once) and print the public half
#   scripts/update-key.sh public [<key.pem>]        print the public key, as 64 hex characters
#   scripts/update-key.sh sign <file> [<key.pem>]   write <file>.sig (the raw 64-byte Ed25519 signature)
#   scripts/update-key.sh check [<key.pem>]         does the key match the public key built into the app?
#
# The private key lives in ~/.config/handset/update-signing-key.pem (mode 600) unless a path is given, or in the
# environment as UPDATE_SIGNING_KEY (the PEM text), which is how the release workflow supplies it. Back the file
# up somewhere safe: a lost key cannot be recovered. Needs OpenSSL 3 (`brew install openssl`), set OPENSSL to
# its path if the default `openssl` is LibreSSL.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
default_key="${HOME}/.config/handset/update-signing-key.pem"
openssl_bin="${OPENSSL:-openssl}"

die() { echo "error: $*" >&2; exit 1; }

need_openssl3() {
  "$openssl_bin" version 2>/dev/null | grep -q '^OpenSSL 3' || die "OpenSSL 3 is needed ($("$openssl_bin" version 2>&1 | head -1)); set OPENSSL=/path/to/openssl"
}

# print the path of a PEM file holding the key (from an argument, the environment or the default location)
key_file() {
  local arg="${1:-}"
  if [ -n "$arg" ]; then [ -f "$arg" ] || die "no key at $arg"; echo "$arg"; return; fi
  if [ -n "${UPDATE_SIGNING_KEY:-}" ]; then
    local tmp; tmp="$(mktemp)"; chmod 600 "$tmp"
    printf '%s\n' "$UPDATE_SIGNING_KEY" > "$tmp"
    echo "$tmp"; return
  fi
  [ -f "$default_key" ] || die "no key: run '$0 generate' first, or set UPDATE_SIGNING_KEY"
  echo "$default_key"
}

public_hex() {
  "$openssl_bin" pkey -in "$1" -pubout -outform DER | tail -c 32 | od -An -tx1 | tr -d ' \n'
  echo
}

cmd="${1:-}"; shift || true
case "$cmd" in
  generate)
    need_openssl3
    [ -e "$default_key" ] && die "$default_key already exists; keeping it. Use 'public' to print its public key."
    mkdir -p "$(dirname "$default_key")"
    (umask 077; "$openssl_bin" genpkey -algorithm ed25519 -out "$default_key")
    echo "made $default_key"
    echo "BACK THIS FILE UP: if it is lost, updates can no longer be signed with this key."
    echo "public key: $(public_hex "$default_key")"
    if [ "${1:-}" = "--set-secret" ]; then
      gh secret set UPDATE_SIGNING_KEY < "$default_key"
      echo "stored it as the repository secret UPDATE_SIGNING_KEY"
    else
      echo "to let the release workflow sign: gh secret set UPDATE_SIGNING_KEY < $default_key"
    fi
    ;;
  public)
    need_openssl3
    public_hex "$(key_file "${1:-}")"
    ;;
  sign)
    need_openssl3
    file="${1:-}"; [ -f "$file" ] || die "no file to sign: '$file'"
    "$openssl_bin" pkeyutl -sign -inkey "$(key_file "${2:-}")" -rawin -in "$file" -out "$file.sig"
    echo "wrote $file.sig"
    ;;
  check)
    need_openssl3
    have="$(public_hex "$(key_file "${1:-}")")"
    grep -q "\"$have\"" "$root/app/src/verify.rs" || die "the public key $have is not among the keys built into app/src/verify.rs"
    echo "ok: $have is a key the app trusts"
    ;;
  *) echo "usage: $0 generate [--set-secret] | public [key] | sign <file> [key] | check [key]" >&2; exit 2 ;;
esac
