# 0032. Support older glibc, musl and Nix; leave out Flatpak and Snap for now

- Status: accepted
- Date: 2026-10-27

## Context

The Linux build (ADR 0030) and its tarball (ADR 0031) were first made on Ubuntu 24.04, whose glibc is newer than
the one on many systems, so the file would fail there with "GLIBC_2.xx not found". Alpine uses musl, NixOS has no
shared library path, and Flatpak and Snap run programs in sandboxes. ADR 0030 also said Linux needs GTK; building
on a system without GTK showed that is wrong: `rfd` uses the desktop portal, and the binary links only ALSA,
libstdc++ and libgcc (the window, GPU and portal libraries are loaded at run time).

## Decision

- **glibc:** build and test in CI on Ubuntu 22.04 (glibc 2.35), the oldest runner GitHub offers. The binary needs
  glibc 2.35, so it runs on Ubuntu 22.04 and newer and Debian 12 and newer. `scripts/package-linux.sh` reads the
  newest glibc symbol the binary uses and refuses to package one that needs more than `MAX_GLIBC` (default
  2.35), so this cannot drift back by accident. Older systems build from source.
- **No GTK:** GTK is dropped from the build packages, the PKGBUILD and the CI job.
- **musl:** checked on Alpine 3.20: it builds, and the tests pass, with `-C target-feature=-crt-static`. A static musl
  binary cannot load the Wayland, X11 and Vulkan libraries, and `alsa-lib-dev` has no static library, so a dynamic
  link is the only way. Documented as build from source; no prebuilt musl file.
- **Nix:** `app/packaging/nix/package.nix` builds the app and adds the run-time libraries to its search path and the
  helper programs to its `PATH`. It is not in nixpkgs and needs a nixpkgs whose Rust is 1.91 or newer.
- **Flatpak and Snap:** not supported yet. Both need `adb` to reach USB devices and the network (mDNS) from inside the
  sandbox, which means broad permissions (`--device=all`, host networking) and the host's udev rules, and a manifest
  cannot be built or tested here. Revisit when someone can test one on a real desktop.

## Consequences

- Ubuntu 22.04 and newer, Debian 12 and newer, Fedora and Arch can use the CI tarball; others build from source.
- The CI job now builds on an old system, so newer-only breakage is caught by the other platforms' jobs, not this one.
- Raising the glibc floor is a deliberate change to `MAX_GLIBC` and the runner.

## Alternatives considered

- **Build in a manylinux-style container instead of an old runner:** reaches glibc 2.17, but it needs a custom
  image and is harder to keep working; the runner is enough until someone needs older systems.
- **Ship an AppImage that bundles its libraries:** would hide the glibc problem for the bundled libraries, but not
  the base C library, so the floor would be the same.
- **Write Flatpak and Snap manifests now:** untestable here, and a broken manifest is worse than none.
