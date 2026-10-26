# 0020. Publish an unsigned Windows zip with each release

- Status: accepted
- Date: 2026-10-26

## Context

ADR 0019 added Windows support, but the release workflow (ADR 0015) only built the macOS files, so a
release had nothing for Windows users. Code signing needs a certificate that the project does not have yet.
Windows blocks unsigned code on PCs with Smart App Control and warns on first run elsewhere, whatever way the
code is packaged, so an installer would not remove that.

## Decision

The release files are built by a reusable workflow, `build-release-files.yml`: the signed macOS files, and the
Windows zip on `windows-latest` with `scripts/package-windows.ps1`. The `release` workflow, started by hand and
only from `main`, calls it and then attaches both sets of files to the one GitHub release. The Windows checksum
file is named `SHA256SUMS-windows` so it does not clash with the macOS one, and the release notes say that the
Windows build is unsigned and what that means for users. The Homebrew cask is unchanged.

Rehearsing is a separate workflow, `release-dry-run`, which calls the same build and only keeps the files as
artifacts. It replaces the `dry_run` checkbox, so the workflow that publishes has no switch that decides whether
it publishes, and the dry run can run from any branch.

## Alternatives considered

- **An installer (MSI or setup exe).** More to build and maintain, and it does not get past Smart App Control.
- **Waiting until the build can be signed.** Windows users would get nothing until then.
- **Keeping the `dry_run` checkbox on `release`.** One mis-click away from publishing, and the dry run could
  not be started from a branch without also passing the "run from main" check.
- **Bundling `adb` and `ffmpeg` in the zip.** Downloading them in CI adds a supply-chain step. The script can
  bundle them with `-ToolsDir`, and the README tells users to install them with winget.

## Consequences

- Every release has a Windows zip, built in the same run as the macOS files.
- Signing is a follow-up: add a signing step (for example Azure Trusted Signing) to the Windows job, and drop
  the "not signed" paragraph from the release notes.
- A Windows build failure stops the whole release, so a release never ships for one platform only.
