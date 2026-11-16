#!/bin/sh
# Build app/assets/handset-helper.jar from helper/src. Needs a JDK (javac) and the Android SDK's build-tools
# (d8) and a platform (android.jar). See docs/runbooks/rebuild-helper.md.
set -eu
cd "$(dirname "$0")"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
D8="${D8:-$(ls -d "$SDK"/build-tools/*/d8 2>/dev/null | sort -V | tail -1)}"
PLATFORM_JAR="${ANDROID_JAR:-$(ls -d "$SDK"/platforms/android-3[0-9]/android.jar 2>/dev/null | sort -V | tail -1)}"
[ -x "$D8" ] || { echo "d8 not found; set D8 or ANDROID_HOME" >&2; exit 1; }
[ -f "$PLATFORM_JAR" ] || { echo "android.jar not found; set ANDROID_JAR or ANDROID_HOME" >&2; exit 1; }
OUT="$(cd .. && pwd)/app/assets"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
# Java 8 bytecode, so any JDK from 8 up can build it
javac -source 8 -target 8 -Xlint:-options -cp "$PLATFORM_JAR" -d "$WORK/classes" src/dev/handset/Helper.java
"$D8" --min-api 24 --lib "$PLATFORM_JAR" --output "$WORK" "$WORK"/classes/dev/handset/*.class
# a fixed date, so the same source always makes the same jar
touch -t 202001010000 "$WORK/classes.dex"
(cd "$WORK" && zip -q -X -D handset-helper.jar classes.dex)
cp "$WORK/handset-helper.jar" "$OUT/handset-helper.jar"
(cd "$OUT" && shasum -a 256 handset-helper.jar | cut -d' ' -f1 > handset-helper.sha256)
echo "built $OUT/handset-helper.jar ($(wc -c < "$OUT/handset-helper.jar") bytes, sha256 $(cat "$OUT/handset-helper.sha256"))"
