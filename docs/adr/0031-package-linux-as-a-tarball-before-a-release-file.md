# 0031. Package Linux as a tarball in CI before it becomes a release file

- Status: accepted
- Date: 2026-10-27

## Context

Linux builds and is checked in CI (ADR 0030), but nobody can try it without building it, and nobody has tested the
window on a real desktop. A release file would be signed into `SHA256SUMS` and checked by the updater's rules
(ADR 0024), and the release workflow cannot be run here to see that a change to it works.

## Decision

- `scripts/package-linux.sh` builds `Handset-<version>-linux-<arch>.tar.gz` (binary, `handset.desktop`, icon, licences)
  and an AppImage when `appimagetool` is installed. The Linux CI job runs it and keeps the result as a build
  artifact for a week. The release workflow is not changed.
- The files do not bundle ALSA: the AppImage runs only where they are installed, and says so.
- The window gets the class `handset` (X11 and Wayland) and the app icon, so the desktop entry matches it.
- `app/packaging/linux/PKGBUILD` builds an Arch package from a release tag; it is for people who build
  locally and is not published.
- `handset --doctor` reports missing helper programs and the udev-rules case (`no permissions`) with the fix.

## Consequences

- A tester can download an artifact and try Linux without a toolchain.
- Linux stays out of releases, Homebrew-style updates and the signed `SHA256SUMS` until it has run on real systems;
  adding it later means a release job and, if wanted, signing the files like the others.
- The AppImage is not self-contained, so it is a convenience, not a distribution answer (Flatpak, `.deb` or a bundle
  with the libraries remain open).

## Alternatives considered

- **Add Linux to the release workflow now:** one step further, but unverifiable here and it would publish a file
  nobody has run.
- **Bundle the libraries into the AppImage:** more portable, but heavier and it needs a build container pinned to an
  old glibc, which is better done when someone can test the result.
