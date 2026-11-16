# Rebuild the on-phone helper

The helper that describes the screen is embedded as `app/assets/handset-helper.jar` (see
[ADR 0046](../adr/0046-describe-the-screen-with-a-small-helper-on-the-phone.md)). Its source is
`helper/src/dev/handset/Helper.java`. Rebuild it after you change the source.

## Prerequisites

- A JDK (`javac`), any version from 8 up
- The Android SDK with build-tools (for `d8`) and one platform (for `android.jar`); `ANDROID_HOME` set, or
  `D8` and `ANDROID_JAR` pointing at the files

## Steps

1. Build:

   ```sh
   helper/build.sh
   ```

   It writes `app/assets/handset-helper.jar` and `app/assets/handset-helper.sha256`. Run it twice: the
   checksum must not change, because the jar is reproducible.
2. Try it on a phone:

   ```sh
   cd app && cargo run -- --ctl
   handset ctl ui          # in another terminal; the first call takes a second, later ones are quick
   ```

   `HANDSET_NO_HELPER=1` runs the same without the helper, for comparing the two.
3. `cargo test` checks that the recorded checksum is the embedded file's.
4. Commit the source, the jar and the checksum together.

## Notes

- The helper reaches Android internals by reflection, so a new Android version can break it. When it does,
  Handset falls back to `uiautomator` and logs `the helper on the phone ...` on the terminal.
- On the phone the jar is `/data/local/tmp/handset-helper-<checksum>.jar`. It is safe to delete.
