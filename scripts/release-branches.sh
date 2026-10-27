#!/usr/bin/env bash
# Release branches: `release/X.Y.Z` is the line that produced the tag `vX.Y.Z`.
#
#   scripts/release-branches.sh check-ref <ref> <version>   is a release of <version> allowed from <ref>?
#   scripts/release-branches.sh backfill [--dry-run] [<remote>]   create a branch for every release tag that has none
#
# `check-ref` is what the release workflow runs before it builds anything: releases come from `main`, or from
# the branch `release/<version>` of the very version being released (a patch for an older line is prepared on
# that branch before it is released). `backfill` is idempotent: it only creates what is missing and never moves
# or deletes a branch. See docs/runbooks/release.md and ADR 0023.
set -euo pipefail

semver='[0-9]+\.[0-9]+\.[0-9]+'

die() { echo "error: $*" >&2; exit 1; }

check_ref() {
  local ref="${1:-}" version="${2:-}"
  [[ "$version" =~ ^${semver}$ ]] || die "'$version' is not a version (X.Y.Z)"
  case "$ref" in
    refs/heads/main) ;;
    "refs/heads/release/$version") ;;
    refs/heads/release/*) die "run the release from main, or from release/$version (this is ${ref#refs/heads/}; the version in package.json is $version)" ;;
    *) die "run the release from main, or from release/$version (not from ${ref:-an empty ref})" ;;
  esac
}

backfill() {
  local dry=0 remote=origin
  for a in "$@"; do
    case "$a" in
      --dry-run) dry=1 ;;
      -*) die "unknown option $a" ;;
      *) remote="$a" ;;
    esac
  done
  local tag version branch sha made=0
  while read -r tag; do
    [[ "$tag" =~ ^v(${semver})$ ]] || continue
    version="${BASH_REMATCH[1]}"
    branch="release/$version"
    if git ls-remote --exit-code --heads "$remote" "refs/heads/$branch" >/dev/null 2>&1; then
      echo "have     $branch"
      continue
    fi
    sha="$(git rev-list -n 1 "refs/tags/$tag")"
    if [ "$dry" = 1 ]; then
      echo "would create $branch at $tag (${sha:0:7})"
    else
      git push --quiet "$remote" "$sha:refs/heads/$branch"
      echo "created  $branch at $tag (${sha:0:7})"
    fi
    made=$((made + 1))
  done < <(git tag --list 'v*' | sort -V)
  echo "$made branch(es) $([ "$dry" = 1 ] && echo 'would be ')created"
}

case "${1:-}" in
  check-ref) shift; check_ref "$@" ;;
  backfill) shift; backfill "$@" ;;
  *) echo "usage: $0 check-ref <ref> <version> | backfill [--dry-run] [<remote>]" >&2; exit 2 ;;
esac
