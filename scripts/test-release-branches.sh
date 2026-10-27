#!/usr/bin/env bash
# Tests for scripts/release-branches.sh, in a throwaway repository and a throwaway remote.
# Run: scripts/test-release-branches.sh
set -uo pipefail

script="$(cd "$(dirname "$0")" && pwd)/release-branches.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
pass=0 fail=0

ok() { pass=$((pass + 1)); echo "ok   - $1"; }
bad() { fail=$((fail + 1)); echo "FAIL - $1"; }
expect() { # <description> <command...>   passes when the command succeeds
  local d="$1"; shift
  if "$@" >/dev/null 2>&1; then ok "$d"; else bad "$d"; fi
}
refuse() { # <description> <command...>   passes when the command fails
  local d="$1"; shift
  if "$@" >/dev/null 2>&1; then bad "$d"; else ok "$d"; fi
}

git init -q --bare "$tmp/origin.git"
git init -q "$tmp/work" && cd "$tmp/work"
git config user.email t@example.com && git config user.name t
git remote add origin "$tmp/origin.git"
commit() { echo "$1" > f && git add f && git commit -q -m "$1"; }

commit one;   git tag v0.1.0
commit two;   git tag -a v0.2.0 -m "annotated"
commit three; git tag v0.2.1
commit four;  git tag v1.0 && git tag v0.3.0-rc1 && git tag nightly && git tag v10.20.30
sha() { git rev-list -n 1 "$1"; }
remote_branches() { git ls-remote --heads origin 'refs/heads/release/*' | sed 's#.*refs/heads/##' | sort -V | tr '\n' ' '; }

# --- check-ref
refuse "an empty version is refused"                  "$script" check-ref refs/heads/main ""
refuse "a version that is not X.Y.Z is refused"       "$script" check-ref refs/heads/main 1.0
expect "main may release any version"                 "$script" check-ref refs/heads/main 0.4.0
expect "release/X.Y.Z may release X.Y.Z"              "$script" check-ref refs/heads/release/0.4.1 0.4.1
refuse "release/X.Y.Z may not release another version" "$script" check-ref refs/heads/release/0.4.0 0.4.1
refuse "a feature branch may not release"             "$script" check-ref refs/heads/feat/x 0.4.0
refuse "a tag ref may not release"                    "$script" check-ref refs/tags/v0.4.0 0.4.0
refuse "an empty ref may not release"                 "$script" check-ref "" 0.4.0
msg="$("$script" check-ref refs/heads/release/0.4.0 0.4.1 2>&1 || true)"
case "$msg" in *"release/0.4.1"*) ok "the refusal says which branch to use" ;; *) bad "the refusal says which branch to use: $msg" ;; esac

# --- backfill
before="$(remote_branches)"
out="$("$script" backfill --dry-run origin 2>&1)"
[ "$(remote_branches)" = "$before" ] && ok "a dry run creates nothing" || bad "a dry run creates nothing"
case "$out" in *"would create release/0.1.0"*"would create release/10.20.30"*) ok "a dry run lists what it would create, oldest first" ;; *) bad "a dry run lists what it would create: $out" ;; esac

"$script" backfill origin >/dev/null 2>&1
[ "$(remote_branches)" = "release/0.1.0 release/0.2.0 release/0.2.1 release/10.20.30 " ] && ok "a branch is made for each X.Y.Z tag and no others" || bad "a branch is made for each X.Y.Z tag and no others (got: $(remote_branches))"
for pair in "0.1.0:v0.1.0" "0.2.0:v0.2.0" "0.2.1:v0.2.1"; do
  b="${pair%%:*}" t="${pair##*:}"
  [ "$(git ls-remote origin "refs/heads/release/$b" | cut -f1)" = "$(sha "$t")" ] && ok "release/$b points at $t" || bad "release/$b points at $t"
done

out="$("$script" backfill origin 2>&1)"
case "$out" in *"0 branch(es)"*) ok "running it again creates nothing" ;; *) bad "running it again creates nothing: $out" ;; esac

# an existing branch is never moved, even if it differs from its tag
git push -q origin "$(sha v0.2.1):refs/heads/release/0.3.5"; git tag v0.3.5 "$(sha v0.1.0)"
"$script" backfill origin >/dev/null 2>&1
[ "$(git ls-remote origin refs/heads/release/0.3.5 | cut -f1)" = "$(sha v0.2.1)" ] && ok "an existing branch is left where it is" || bad "an existing branch is left where it is"

refuse "an unknown option is refused" "$script" backfill --nope
refuse "no command prints usage and fails" "$script"

echo "$pass passed, $fail failed"
[ "$fail" = 0 ]
