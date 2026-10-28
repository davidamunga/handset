# 0033. Release Linux tarballs, signed like the other files

- Status: accepted
- Date: 2026-10-28

## Context

ADR 0031 kept the Linux tarball as a week-long CI artifact until it had run on real systems, and ADR 0032 fixed its
glibc floor at 2.35 by building on Ubuntu 22.04. A tarball that disappears after a week cannot be installed by a
package such as an Arch `handset-bin`, linked from the website, or checked against a signature. GitHub now offers
`ubuntu-22.04-arm` runners for public repositories, so aarch64 can be built natively on the same glibc.

## Decision

- `build-release-files.yml` builds `Handset-<version>-linux-x86_64.tar.gz` on `ubuntu-22.04` and
  `Handset-<version>-linux-aarch64.tar.gz` on `ubuntu-22.04-arm` with `scripts/package-linux.sh`, which keeps its
  `MAX_GLIBC` guard. The asset name uses `uname -m` for the architecture and does not change.
- A small job checks each build's `SHA256SUMS` and joins them into one `SHA256SUMS-linux`, so the release has one
  Linux list and one signature, as Windows has `SHA256SUMS-windows`. `release.yml` signs it with the same key
  and step as the others, and `handset --verify-release` checks it.
- The release notes and the website call Linux experimental. The website's Linux card offers the tarball for the
  visitor's architecture when the release has one, with `cargo install` as the alternative.
- The AppImage is still not released (it does not bundle ALSA, ADR 0031), and the updater does not install Linux
  updates in place.

## Consequences

- People on glibc 2.35 or newer get a prebuilt binary, and packagers have a stable URL and a signed checksum.
- A release now waits for two more runners; an aarch64 build failure blocks the release like any other platform.
- Linux tarballs are not code-signed (Linux has no common scheme for that); the signed checksum list is the check.
- Updating in place on Linux, if wanted, can use the signed list later without changing the release.

## Alternatives considered

- **Keep Linux out of releases until someone tests it on a desktop:** safer, but it blocks the Arch package and the
  website, and testers still need a file that lasts.
- **One `SHA256SUMS` for every platform:** simpler to verify, but macOS and Windows are built and listed separately
  today, and the updater reads the platform files by name; joining all three would change that for no gain.
- **Cross-compile aarch64 from x86_64:** no ARM runner needed, but it needs a cross toolchain and sysroot for ALSA
  and openh264's C code, and nothing would run the result.
