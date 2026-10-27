#!/usr/bin/env bash
# Build dist/Handset.app (macOS). Run: scripts/package-macos.sh
# UNIVERSAL=1 builds for Apple Silicon and Intel and joins them with lipo (needs both rust targets).
# CODESIGN_IDENTITY="Developer ID Application: ..." (or a certificate hash) signs with the hardened
# runtime and a timestamp, ready for notarisation. Without it the app is signed ad hoc.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
app="$root/dist/Handset.app"
version="$(grep -m1 '^version' "$root/app/Cargo.toml" | cut -d'"' -f2)"

target="${CARGO_TARGET_DIR:-$root/app/target}"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
if [ "${UNIVERSAL:-0}" = 1 ]; then
  (cd "$root/app" && cargo build --release --target aarch64-apple-darwin && cargo build --release --target x86_64-apple-darwin)
  lipo -create "$target/aarch64-apple-darwin/release/handset" "$target/x86_64-apple-darwin/release/handset" \
    -output "$app/Contents/MacOS/handset"
else
  (cd "$root/app" && cargo build --release)
  cp "$target/release/handset" "$app/Contents/MacOS/handset"
fi
cp "$root/app/packaging/AppIcon.icns" "$app/Contents/Resources/AppIcon.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Handset</string>
  <key>CFBundleDisplayName</key><string>Handset</string>
  <key>CFBundleIdentifier</key><string>com.davidamunga.handset</string>
  <key>CFBundleExecutable</key><string>handset</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSCameraUsageDescription</key><string>Handset shows a connected iPhone or iPad screen, which macOS treats as a camera.</string>
  <key>NSMicrophoneUsageDescription</key><string>Handset plays and records the sound of a connected iPhone or iPad.</string>
  <key>NSLocalNetworkUsageDescription</key><string>Handset finds Android phones on your Wi-Fi to pair and connect with them.</string>
  <key>NSBonjourServices</key>
  <array>
    <string>_adb-tls-pairing._tcp</string>
    <string>_adb-tls-connect._tcp</string>
  </array>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
</dict>
</plist>
PLIST

identity="${CODESIGN_IDENTITY:--}"
if [ "$identity" = "-" ]; then
  # ad-hoc signature: enough to run on this Mac (not for distribution)
  codesign --force --deep --sign - "$app"
else
  codesign --force --options runtime --timestamp --sign "$identity" "$app"
  codesign --verify --strict --verbose=2 "$app"
fi
plutil -lint "$app/Contents/Info.plist" >/dev/null
echo "built $app"
