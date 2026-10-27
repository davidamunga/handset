#!/usr/bin/env bash
# Build the Linux release files in dist/linux/: Handset-<version>-linux-<arch>.tar.gz (the binary, the desktop
# entry, the icon and the licences) and SHA256SUMS. When `appimagetool` is on PATH it also builds an AppImage.
# Run on Linux: scripts/package-linux.sh
#
# The binary uses the system's GTK and ALSA (see "Linux" in the README), so the AppImage does not bundle them
# and runs only where they are installed. Unsigned, like the Windows zip.
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
