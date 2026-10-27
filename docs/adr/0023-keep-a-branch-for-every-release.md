# 0023. Keep a branch for every release

- Status: accepted
- Date: 2026-10-27

## Context

Releases were a tag on `main` (ADR 0015). By the time a bug is found in a shipped version, `main` has moved
on, so there is no place to build a fix on top of exactly what shipped, and the release workflow could only
run from `main`, so a patch to an older version could not be published without also publishing everything
that had landed since.

## Decision

- `release/X.Y.Z` is the line that produced the tag `vX.Y.Z`. The `release` workflow creates it at the
  commit it tags. The existing tags were given their branches once by `scripts/release-branches.sh backfill`,
  which only creates missing branches and never moves or deletes one.
- The `release` workflow runs from `main`, or from the branch `release/<version>` of the very version being
  released (`scripts/release-branches.sh check-ref`). A patch to an older line is prepared on a branch
  created from the old release branch and named for the new version, then released from there.
- A release from a release branch is not marked latest and does not change the Homebrew cask or the Scoop
  manifest, so the newest version keeps that place. Only releases from `main` do.
- CI runs on pushes to `release/**`, and the helper script is tested in CI.
- The workflow's own pull request branch for the Scoop manifest was `release/scoop-X`; it is now
  `chore/scoop-X`, so `release/` holds only versions.

## Alternatives considered

- **Tags only:** a fix can start from a tag, but a branch is easier to find, protect, build and run CI on,
  and the release workflow needs a branch to run from.
- **One long-lived `release` branch:** it would hold only the latest line, which is what `main` already is.
- **Create the branch only when a patch is needed:** the commit is easy to find from the tag, but nobody has
  to remember to do it before `main` has moved, and a branch per release costs nothing.

## Consequences

- Every release has a branch, and older lines can be patched without taking newer changes.
- Release branches are meant to be left alone after the release; a patch gets its own new branch.
- Protecting `release/*` with a repository ruleset (no force pushes or deletion) is a repository setting and
  is not part of this change.
