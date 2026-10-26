# Changelog

## 0.1.4

### Patch Changes

- [#14](https://github.com/davidamunga/handset/pull/14) [`6dc4e20`](https://github.com/davidamunga/handset/commit/6dc4e207305194a923c0ead8d147d6ce9e777d19) Thanks [@davidamunga](https://github.com/davidamunga)! - When the phone's screen stays black for a few seconds, Handset now says so ("Black screen: phone asleep or app blocks capture") instead of looking frozen.

- [#16](https://github.com/davidamunga/handset/pull/16) [`2b02ce3`](https://github.com/davidamunga/handset/commit/2b02ce325e80b0aa2ac694317058428f74c105d1) Thanks [@davidamunga](https://github.com/davidamunga)! - The Dock icon now shows what Handset is doing: the robot's antennae are grey while waiting for a phone, amber while connecting, green while mirroring, and red with a dot while recording.

- [#15](https://github.com/davidamunga/handset/pull/15) [`0519c46`](https://github.com/davidamunga/handset/commit/0519c46fe5c0e302570ec7cd3b406c520856b00f) Thanks [@davidamunga](https://github.com/davidamunga)! - Open apps on the phone from the Mac: the right-click menu has a new **Open App** list (your apps first, system apps in a submenu), and `handset --start-app <package>` opens one when the phone is mirrored.

- [#12](https://github.com/davidamunga/handset/pull/12) [`b261abc`](https://github.com/davidamunga/handset/commit/b261abcd3269e5841a1e94c1ef6a04445fc25e1f) Thanks [@davidamunga](https://github.com/davidamunga)! - Right-click a running session and use the new **Phone** menu: **Switch to Wi-Fi** for a USB phone, **Connect Another Phone**, and **Disconnect**. After Disconnect, nothing restarts by itself; the connect screen offers the phone that is plugged in, or the Wi-Fi and QR code options.

- [#11](https://github.com/davidamunga/handset/pull/11) [`6094955`](https://github.com/davidamunga/handset/commit/60949554f2098f01de5e18e3ce0f54436b144179) Thanks [@davidamunga](https://github.com/davidamunga)! - Handset remembers the last few phones you connected to over Wi-Fi. The connect screen offers "Reconnect to <phone>" and lists them under Wi-Fi, and `handset --last` reconnects to the most recent one.

- [#13](https://github.com/davidamunga/handset/pull/13) [`a000b86`](https://github.com/davidamunga/handset/commit/a000b8625c3fceaebadf009c681df531d1b34e4d) Thanks [@davidamunga](https://github.com/davidamunga)! - Clipboard text is now shared with the phone. Copy on the phone and it is on your Mac's clipboard; copy on the Mac, click the Handset window, and it is on the phone's. Turn it off with `--no-clipboard-sync` or the right-click menu.

- [#10](https://github.com/davidamunga/handset/pull/10) [`29c0f99`](https://github.com/davidamunga/handset/commit/29c0f99a95721a46dc367d523a6bf18948c595df) Thanks [@davidamunga](https://github.com/davidamunga)! - When the connection to the phone drops, Handset now shows "Connection lost" on the connect screen and starts mirroring again by itself when the phone is back, instead of quitting. Phones on Wi-Fi are reconnected with `adb connect` every few seconds.

## 0.1.3

### Patch Changes

- [`a9cc45f`](https://github.com/davidamunga/handset/commit/a9cc45fc0c2245c8d2afb4178e5aa62b9eebf76a) Thanks [@davidamunga](https://github.com/davidamunga)! - The connect screen has a new look and buttons. It uses the Handset icon and colours and a proper typeface instead of pixel letters. **Wi-Fi** lists paired phones found on your network, lets you type an address, and switches a USB phone to Wi-Fi. **QR code** shows a code to scan on the phone (Wireless debugging, Pair device with QR code) and pairs and connects without typing.

## 0.1.2

### Patch Changes

- [`c1cb884`](https://github.com/davidamunga/handset/commit/c1cb8840faa9309d7f10124baa058169befb743a) Thanks [@davidamunga](https://github.com/davidamunga)! - With no phone plugged in, Handset now opens on a "connect a phone" screen inside the frame instead of quitting with an error. It shows what it sees (nothing yet, a phone waiting for you to tap Allow, an offline connection), and starts mirroring by itself as soon as a phone is ready.

- [`37285e4`](https://github.com/davidamunga/handset/commit/37285e4780b43beb62be2d786bb423ae1f1886d4) Thanks [@davidamunga](https://github.com/davidamunga)! - `handset --connect` now gives up after 10 seconds with a clear message instead of waiting over a minute for adb to time out.

## 0.1.1

### Patch Changes

- [`edf3465`](https://github.com/davidamunga/handset/commit/edf3465c8b2c7195de620548cfc99695a1230719) Thanks [@davidamunga](https://github.com/davidamunga)! - Mirror over Wi-Fi: `handset --connect <ip[:port]>`, `handset --pair <ip:port> <code>` for Android 11 wireless pairing, and `handset --wifi` to switch a USB phone to Wi-Fi. When no phone is found, the message now says whether it is unauthorised or offline and what to do about it.

## 0.1.0

### Fixed

- The window froze on some phones (seen on a UNIWA W888): OpenH264's per-packet flush leaked pictures until its
  pool ran out. Decoding no longer flushes, and a failed decoder is rebuilt instead of looping
- Recordings could lose their audio track when ffmpeg's loudness normaliser produced NaN samples

### Added

- Mirror and control an Android phone over adb using the embedded, unmodified scrcpy server
- Video decoded with openh264 and drawn with wgpu, with YUV to RGB conversion on the GPU
- Phone frames (`obsidian`, `porcelain`, `hazel` or a custom PNG) on a transparent window, switchable at run time
- Touch, scroll, keyboard and text input, and landscape rotation
- Device audio captured as raw PCM and played locally (`--audio`, `--mute`)
- Framed screenshots and MP4 or GIF recordings with audio through ffmpeg, with loudness normalisation
- Toolbar, native right-click menu, REC indicator and clickable "saved" toast
- Recent captures menu and automatic clipboard copy (`--no-clipboard` to disable)
- `--record-for` for hands-off recordings and `~/.config/handset/args` for default options
- macOS app bundle packaging with an app icon (monitor, Android robot and phone) rebuilt from SVG by `scripts/make-icon.sh`
- Repository setup: license, contribution and security docs, commit linting, issue and pull request
  templates, CI, ADRs and runbooks
