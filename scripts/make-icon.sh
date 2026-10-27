#!/usr/bin/env bash
# Rasterise the icon SVGs into app/packaging/AppIcon.icns (macOS). Run: scripts/make-icon.sh
# Needs rsvg-convert (brew install librsvg) and iconutil. Sizes up to 64 px use the simplified
# small drawing, larger sizes use the full one.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
src="$root/app/packaging/icon"
set_dir="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$set_dir"

render() { # <px> <svg> <out>
  rsvg-convert -w "$1" -h "$1" "$2" -o "$3"
}
pick() { if [ "$1" -le 64 ]; then echo "$src/handset-icon-small.svg"; else echo "$src/handset-icon.svg"; fi; }

for base in 16 32 128 256 512; do
  render "$base" "$(pick "$base")" "$set_dir/icon_${base}x${base}.png"
  render "$((base * 2))" "$(pick "$((base * 2))")" "$set_dir/icon_${base}x${base}@2x.png"
done

iconutil -c icns "$set_dir" -o "$root/app/packaging/AppIcon.icns"
render 512 "$src/handset-icon.svg" "$src/handset-icon-512.png"
# the style editor has its own icon, so its window is not mistaken for the mirroring one
render 512 "$src/editor-icon.svg" "$src/editor-icon-512.png"

# The Dock icon changes with what Handset is doing: the front phone's eyes take the status colour, and a red
# dot marks a recording. "live" is the icon as it is.
state() { # <name> <eye colour> [extra svg]
  local svg
  svg="$(mktemp).svg"
  sed "s/r=\"5\" fill=\"#FFFFFF\"/r=\"5\" fill=\"$2\"/g" "$src/handset-icon.svg" > "$svg"
  if [ -n "${3:-}" ]; then
    python3 - "$svg" "$3" <<'PY'
import sys
path, extra = sys.argv[1], sys.argv[2]
s = open(path).read()
i = s.rindex("</g>")
open(path, "w").write(s[:i] + extra + "\n  " + s[i:])
PY
  fi
  render 512 "$svg" "$src/state-$1.png"
  rm -f "$svg"
}
state idle "#8D9097"
state connecting "#FFB300"
state live "#FFFFFF"
state recording "#FF5449" '<circle cx="36" cy="36" r="17" fill="#FF5449" stroke="#ffffff" stroke-width="5"/>'

echo "wrote app/packaging/AppIcon.icns, handset-icon-512.png and the state-*.png Dock icons"
