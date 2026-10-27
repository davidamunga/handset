#!/usr/bin/env bash
# Build the Linux release files in dist/linux/: Handset-<version>-linux-<arch>.tar.gz (the binary, the desktop
# entry, the icon and the licences) and SHA256SUMS. When `appimagetool` is on PATH it also builds an AppImage.
# Run on Linux: scripts/package-linux.sh
#
# The binary uses the system's ALSA (see "Linux" in the README), so the AppImage does not bundle it and runs
# only where it is installed. It also needs a glibc at least as new as the one it was built against, so build on
# the oldest system you want to support (CI uses Ubuntu 22.04, glibc 2.35); MAX_GLIBC makes the script refuse
# a binary that needs more. Unsigned, like the Windows zip.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(grep -m1 '^version' "$root/app/Cargo.toml" | cut -d'"' -f2)"
arch="$(uname -m)"
out="$root/dist/linux"
name="Handset-$version-linux-$arch"

if [ "$(uname -s)" != "Linux" ]; then
  echo "package-linux.sh builds for Linux; run it on Linux" >&2
  exit 1
fi

(cd "$root/app" && cargo build --release)
bin="${CARGO_TARGET_DIR:-$root/app/target}/release/handset"

# the newest glibc symbol the binary uses is the oldest glibc it can run on
max_glibc="${MAX_GLIBC:-2.35}"
if command -v objdump >/dev/null 2>&1; then
  need="$(objdump -T "$bin" | grep -o 'GLIBC_[0-9.]*' | sed 's/^GLIBC_//' | sort -uV | tail -1)"
  if [ -n "$need" ] && [ "$(printf '%s\n%s\n' "$need" "$max_glibc" | sort -V | tail -1)" != "$max_glibc" ]; then
    echo "the binary needs glibc $need but at most $max_glibc is allowed: build on an older system" >&2
    exit 1
  fi
  echo "needs glibc $need or newer (limit $max_glibc)"
else
  echo "objdump not found: not checking which glibc the binary needs" >&2
fi

rm -rf "$out"
stage="$out/$name"
mkdir -p "$stage"
cp "$bin" "$stage/handset"
cp "$root/app/packaging/linux/handset.desktop" "$stage/"
cp "$root/app/packaging/icon/handset-icon-512.png" "$stage/handset.png"
cp "$root/LICENSE" "$root/NOTICE" "$stage/"
tar -C "$out" -czf "$out/$name.tar.gz" "$name"

if command -v appimagetool >/dev/null 2>&1; then
  appdir="$out/Handset.AppDir"
  mkdir -p "$appdir/usr/bin"
  cp "$bin" "$appdir/usr/bin/handset"
  cp "$root/app/packaging/linux/handset.desktop" "$appdir/"
  cp "$root/app/packaging/icon/handset-icon-512.png" "$appdir/handset.png"
  ln -s usr/bin/handset "$appdir/AppRun"
  ARCH="$arch" appimagetool "$appdir" "$out/Handset-$version-$arch.AppImage"
  rm -rf "$appdir"
fi

rm -rf "$stage"
(
  cd "$out"
  for f in *.tar.gz *.AppImage; do
    # an unmatched pattern stays literal: skip it (no AppImage without appimagetool)
    if [ -f "$f" ]; then sha256sum -- "$f"; fi
  done > SHA256SUMS
)
echo "built:"
ls -l "$out"
