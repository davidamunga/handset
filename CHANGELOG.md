# Changelog

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
