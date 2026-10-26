# Troubleshoot a device connection

Use this when Handset does not show the phone, or shows a black or frozen window.

With no phone ready, Handset opens on a connect screen drawn in the frame. Its status line says what adb sees
(looking for a phone, allow this Mac on the phone, the phone is offline, no access, adb is missing) and what to
do, and it starts mirroring on its own once the phone is ready. The table below is the same advice in more
detail.

## 1. Is adb seeing the phone?

```sh
adb devices
```

| Output | Meaning | Fix |
|---|---|---|
| nothing listed | no connection | try another cable or port; enable **USB debugging** in Developer options |
| `unauthorized` | the phone has not trusted this Mac | unlock the phone and accept the prompt; if it never appears, revoke USB debugging authorisations and reconnect |
| `offline` | stale adb session | `adb kill-server && adb start-server`, then replug |
| serial and `device` | fine | go to step 2 |

If `adb` itself is missing, install the Android platform-tools. Handset finds `adb` on `PATH`, in
`/opt/homebrew/bin`, `/usr/local/bin` and the Android SDK folder, and the `ADB` environment variable
overrides the lookup.

### Over Wi-Fi instead of USB

On the connect screen, **Wi-Fi** lists paired phones on the network and lets you type an address or switch a
USB phone, and **QR code** pairs a new phone with no typing. The command line does the same with `--connect`,
`--pair` and `--wifi`.

QR pairing needs Android 11 or later, Wireless debugging on, and the phone and the Mac on the same network
(guest and office networks often block devices from finding each other). Scan with Developer options >
Wireless debugging > Pair device with QR code, not with the camera app. If the screen says pairing failed, it
shows a fresh code: scan again. `adb mdns check` should report a daemon version; if it does not, run
`adb kill-server` and retry.

- `handset --wifi` with the phone on USB switches it to adb over Wi-Fi (port 5555) and connects.
- On Android 11 and later you can skip the cable: Developer options > Wireless debugging > Pair device
  with pairing code, then `handset --pair <ip:port> <code>` (the pairing port is not the connection
  port), then `handset --connect <ip:port>` using the address on the main Wireless debugging screen.
- "Could not connect": the phone and the Mac must be on the same Wi-Fi, and guest or office networks often
  isolate devices from each other. Turning Wireless debugging off and on gets a fresh port. A phone left in
  `adb tcpip` mode goes back to USB with `adb usb`.
- When the phone is listed as `unauthorized` or `offline`, Handset now says so and what to do instead of
  "No Android device found".

## 2. Run from a terminal to see the real error

```sh
HANDSET_DEBUG=1 handset
```

Errors are printed to the terminal. When launched from Finder they appear in an alert instead.

## 3. Common causes

- **Mirroring starts but touch does nothing.** Some manufacturers (for example Xiaomi and Oppo
  phones) have a second switch for controlling the device, often called **USB debugging (Security
  settings)**. Turn it on in Developer options.
- **"Could not start mirroring" or a version error.** The embedded server is pinned to one scrcpy
  version. Check `SERVER_VERSION` in `app/src/server.rs` against the asset
  ([rebuild runbook](rebuild-scrcpy-server.md)).
- **Stale port forwards.** List them with `adb forward --list` and clear with
  `adb forward --remove-all`.
- **Black rectangle inside an app.** Apps that mark their window as secure (banking, some streaming)
  are not captured by the server. That is expected.
- **No audio.** Audio is opt-in (`--audio`) and needs Android 11 or later. Without it Handset prints
  that audio is unavailable and carries on.
- **A recording has video but no audio.** Run `HANDSET_DEBUG=1 handset --audio` and look for ffmpeg
  messages. An `Input contains (near) NaN/+-Inf` error means the loudness normaliser produced bad
  samples; Handset guards against this, so please report it with the log if you still see it. As a
  workaround, `--no-normalize` skips the normaliser. Check the file with
  `ffprobe -show_entries stream=codec_type file.mp4`.
- **Recording fails.** Recording needs `ffmpeg` (`brew install ffmpeg`). The `FFMPEG` environment
  variable overrides the lookup.
- **First launch of the app from Finder.** Right-click the app and choose Open the first time (it is
  ad-hoc signed), and allow access to Pictures and Movies when macOS asks.

## 4. Still stuck?

Open a bug report and include the output of `HANDSET_DEBUG=1 handset`, your phone model and Android
version, the output of `adb version`, and what you tried above.
