# Changelog

## 0.7.2

### Patch Changes

- [#70](https://github.com/davidamunga/handset/pull/70) [`4652a33`](https://github.com/davidamunga/handset/commit/4652a3319b188922f2488339fbcae8069dd90637) Thanks [@davidamunga](https://github.com/davidamunga)! - QR pairing no longer waits forever when adb cannot see the phone on the network. Handset now looks for phones itself and merges that with adb's list. The QR screen has a "Pair with a code" button, and after 12 seconds without a phone it says what usually gets in the way (Wireless debugging screen, same Wi-Fi, no VPN).

  On macOS 15 and later, the app now declares why it uses the local network, so the system can ask for the Local Network permission that finding phones needs.

## 0.7.1

### Patch Changes

- [#65](https://github.com/davidamunga/handset/pull/65) [`1952aab`](https://github.com/davidamunga/handset/commit/1952aab31e9748673c7e92ff305941fded530054) Thanks [@davidamunga](https://github.com/davidamunga)! - The connect screen is now black with white text, shows the Handset mark and name at the top, and has clearer buttons: one bright button for the main action, quieter ones for the other ways to connect, each with a small symbol. The mark's eyes glance about while Handset looks for a phone, and the step numbers are centred in their circles.

## 0.7.0

### Minor Changes

- [#63](https://github.com/davidamunga/handset/pull/63) [`3f3a288`](https://github.com/davidamunga/handset/commit/3f3a2880511f6a1521fbf15de27ae0c8615eae29) Thanks [@davidamunga](https://github.com/davidamunga)! - Handset has a new icon: a phone with two eyes in front of an Android-green and an iOS-blue echo, so it no longer looks like it is only for Android. The connect screen is now grey and white, and the style editor uses a grey look that follows your system's light or dark setting. The Dock icon still shows what Handset is doing, now through the phone's eyes. The editor's **Looks** menu is now a **Presets** card at the top of the sidebar, with a thumbnail of your picture for each preset.

- [#59](https://github.com/davidamunga/handset/pull/59) [`88884c0`](https://github.com/davidamunga/handset/commit/88884c06bf02bd98e84d627b628e7ce32376d0a2) Thanks [@davidamunga](https://github.com/davidamunga)! - Handset can show an iPhone or iPad connected over USB on macOS, with its sound: click **Mirror iPhone** on the connect screen, or start it with `handset --ios`. The frame, screenshots and recordings work as they do for Android phones. Controlling the device is not supported yet, so the toolbar and menu offer only what works without it. The frame list shows only the iPhone and iPad shapes, and an iPhone starts in the Dynamic Island frame.

### Patch Changes

- [#61](https://github.com/davidamunga/handset/pull/61) [`4fa6d03`](https://github.com/davidamunga/handset/commit/4fa6d037b9201222594a3525524c731507aba263) Thanks [@davidamunga](https://github.com/davidamunga)! - Fixed Handset using more and more memory, up to tens of gigabytes, while its window was hidden or had not appeared yet. New phone frames and the animated connect screen are now sent to the graphics card only when the window draws.

## 0.6.1

### Patch Changes

- [#56](https://github.com/davidamunga/handset/pull/56) [`f045456`](https://github.com/davidamunga/handset/commit/f04545659cb271e33fa3d07152c636d2185183ba) Thanks [@davidamunga](https://github.com/davidamunga)! - After Handset has been updated, by any means (its own installer, Homebrew, Scoop or a download), the first start says `UPDATED TO X`. Handset now remembers the last version it ran in `last-version` in the settings file.

## 0.6.0

### Minor Changes

- [#51](https://github.com/davidamunga/handset/pull/51) [`33feb02`](https://github.com/davidamunga/handset/commit/33feb025f4cbb4ccd52c24d5596e8574441dede7) Thanks [@davidamunga](https://github.com/davidamunga)! - A downloaded Handset (the app on macOS, the unzipped release on Windows) can now update itself: the update notice and **Install Handset X and Restart** in the menu download the release, check its signature and checksum, replace the app and start the new version, putting the old one back if anything fails. Releases without a valid signature are never installed this way.

- [#49](https://github.com/davidamunga/handset/pull/49) [`9e8c55a`](https://github.com/davidamunga/handset/commit/9e8c55afbe229405f11c0c2c951592727d8ef3d7) Thanks [@davidamunga](https://github.com/davidamunga)! - Update notices know how Handset was installed. With Homebrew, Scoop or winget the toast gives the command to run (`brew upgrade --cask handset`, `scoop update handset`, `winget upgrade davidamunga.Handset`) and a click copies it. The menu gains **What's New in X**, **Skip X** (that version stays quiet) and **Remind Me in 3 Days**.

- [#50](https://github.com/davidamunga/handset/pull/50) [`e2a6df6`](https://github.com/davidamunga/handset/commit/e2a6df635626b49cb0d05b2a0ab164a801cf6914) Thanks [@davidamunga](https://github.com/davidamunga)! - Releases are now signed. `handset --verify-release <folder>` checks downloaded release files against the signed `SHA256SUMS` and says whether they can be trusted. Handset uses the same check before it installs an update.

### Patch Changes

- [#53](https://github.com/davidamunga/handset/pull/53) [`48e8b81`](https://github.com/davidamunga/handset/commit/48e8b81352ee000e0389b73f47354ef3d00fea90) Thanks [@davidamunga](https://github.com/davidamunga)! - When Handset cannot replace itself where it is installed (for example in `/Applications` without administrator rights), it now opens the download page and says why instead of failing. Old update folders in the temporary folder are removed after a week.

## 0.5.0

### Minor Changes

- [#45](https://github.com/davidamunga/handset/pull/45) [`02423ca`](https://github.com/davidamunga/handset/commit/02423ca97c2d6ad3b970e55912a37fb11d5ef954) Thanks [@davidamunga](https://github.com/davidamunga)! - The style editor now plays a recording's sound while you preview it. A **Sound** switch next to Loop mutes it (it only shows for recordings that have audio).

- [#44](https://github.com/davidamunga/handset/pull/44) [`28e89e0`](https://github.com/davidamunga/handset/commit/28e89e0acc146c68b1b5a2c56497968ea04a4745) Thanks [@davidamunga](https://github.com/davidamunga)! - Looks: save the editor's current style under a name and pick it again later from the **Looks** menu at the top of the editor. Five come with Handset (Clean white, Dark card, Sunset 4:5, Aurora 16:9, Dusk square). `--style` also accepts a look's name, for example `handset --restyle shot.png --style "Aurora 16:9"`.

- [#42](https://github.com/davidamunga/handset/pull/42) [`653b2c6`](https://github.com/davidamunga/handset/commit/653b2c6d1941c58d0ce6fcc83b8fbb599badba3c) Thanks [@davidamunga](https://github.com/davidamunga)! - A style editor with a live preview: **EDIT** on the toast that appears after a capture is saved (next to **SHOW**), **EDIT** on the toast that appears after a capture is saved (next to **SHOW**), **Recent Captures > Edit Style...** (or `handset --edit <file>`) opens a window to pick the background (gradients, wallpapers, your own picture or a colour), padding, corner radius, shadow and aspect ratio, then exports a styled copy of a PNG, MP4 or GIF. Your last export's look is remembered. Recordings can be played and scrubbed in the editor's preview (Space plays and pauses). The editor has its own icon in the Dock. Cmd/Ctrl+Z undoes a change, a Loop switch repeats the preview, video exports show their progress and can be cancelled, and a capture dropped on the editor opens in a new window.

- [#39](https://github.com/davidamunga/handset/pull/39) [`96db140`](https://github.com/davidamunga/handset/commit/96db140e92997f717b487b0ada553db903f979f1) Thanks [@davidamunga](https://github.com/davidamunga)! - Captures can be put on a styled background: gradients, wallpapers, plain colours or your own picture, with padding, shadow, rounded corners and an aspect ratio such as 1:1, 16:9 or 4:5. Works for PNG, MP4 and GIF. Turn on **Recording > Save a styled copy of each capture**, use **Recent Captures > Save Styled Copy**, or run `handset --restyle <file> --style "bg=gradient:dusk;padding=12;radius=6;shadow=60;aspect=16:9"`. The original is always kept.

- [#43](https://github.com/davidamunga/handset/pull/43) [`ac7589a`](https://github.com/davidamunga/handset/commit/ac7589a2d4433a782823b7beded79df38db0b7b4) Thanks [@davidamunga](https://github.com/davidamunga)! - Recordings can be trimmed in the style editor: drag the two green handles at the ends of the scrubber to keep only part of a recording. Playback, Loop and Export follow the kept part, the exported file is cut to it (audio included), and **Reset trim** brings the whole recording back.

## 0.4.0

### Minor Changes

- [#35](https://github.com/davidamunga/handset/pull/35) [`dc8b2b5`](https://github.com/davidamunga/handset/commit/dc8b2b567de25de0bcec0daeba98b510b98a89f0) Thanks [@davidamunga](https://github.com/davidamunga)! - Eighteen new device frames: notch, island, waterdrop, older home-button phone, flat-edge phone, tablet, foldable, flip and rugged, each in two colours. The Frame menu groups them by device, and `docs/frames.md` explains how to make your own.

- [#36](https://github.com/davidamunga/handset/pull/36) [`407253c`](https://github.com/davidamunga/handset/commit/407253c6ff453bfc171e559ea5b0768fb6b267c8) Thanks [@davidamunga](https://github.com/davidamunga)! - Handset tells you when a newer release exists: a clickable toast after start (at most once a day), **Check for Updates...** and **Download Handset X.Y.Z...** in the menu, and `--no-update-check` or **Check for Updates Daily** to turn it off. `handset --version` prints the version.

## 0.3.0

### Minor Changes

- [#30](https://github.com/davidamunga/handset/pull/30) [`44c919c`](https://github.com/davidamunga/handset/commit/44c919cbe03e55a95b42361f80c7f127ba2b8500) Thanks [@davidamunga](https://github.com/davidamunga)! - `handset --doctor` checks that `adb` and `ffmpeg` are found and that each connected phone is online, authorised, running Android 5.0 or later and awake, and tells you what to do about anything that is not.

- [#29](https://github.com/davidamunga/handset/pull/29) [`8a6e95b`](https://github.com/davidamunga/handset/commit/8a6e95bac7b04b6c63ff406715e227de13f5ed20) Thanks [@davidamunga](https://github.com/davidamunga)! - Drop an `.apk` on the window to install it on the phone, or any other file to copy it to the phone's Downloads folder. A toast shows progress and the result.

- [#32](https://github.com/davidamunga/handset/pull/32) [`f46a48d`](https://github.com/davidamunga/handset/commit/f46a48da21ca102d8ec78922539864995810aecf) Thanks [@davidamunga](https://github.com/davidamunga)! - Show Touches (in the menu, or `--show-touches`) draws a translucent dot where you touch the phone and fades it when you let go, in the window and in screenshots and recordings.

- [#26](https://github.com/davidamunga/handset/pull/26) [`c3ac755`](https://github.com/davidamunga/handset/commit/c3ac7557ab5e77a8f9b2712878ab6e9dfd8fdec0) Thanks [@davidamunga](https://github.com/davidamunga)! - The toolbar now has Back, Home, Recents, Power, Volume Up, Volume Down and Notifications buttons, and shows a button's name when you hover it. Menu choices (frame, toolbar, capture format, normalise, clipboard copy and sync, mute) are remembered in `~/.config/handset/settings`.

  The window opens smaller on macOS so it fits above the Dock, and can be resized by dragging a corner, or with Cmd/Ctrl plus `+`, `-` and `0` or the Larger, Smaller and Default Size menu items. Scroll-resizing no longer grows past the screen.

### Patch Changes

- [#31](https://github.com/davidamunga/handset/pull/31) [`1cff6e4`](https://github.com/davidamunga/handset/commit/1cff6e4cd6632ff5a64904b7eb40a07e7ef8d3f3) Thanks [@davidamunga](https://github.com/davidamunga)! - The connect screen has a Run checks button that shows the results of `handset --doctor` in the frame.

## 0.2.0

### Minor Changes

- [#22](https://github.com/davidamunga/handset/pull/22) [`0e9e027`](https://github.com/davidamunga/handset/commit/0e9e027b7da9881366fd84e38939bcadfb173a3a) Thanks [@davidamunga](https://github.com/davidamunga)! - Run on Windows 10 and 11: the right-click menu, clipboard sync, screenshots and recordings (copied to the clipboard), the connect prompt and Explorer actions all work, and `adb` and `ffmpeg` are found from `PATH`, the Android SDK, winget, scoop or Chocolatey. The window is sized to fit the screen.

### Patch Changes

- [#22](https://github.com/davidamunga/handset/pull/22) [`0e9e027`](https://github.com/davidamunga/handset/commit/0e9e027b7da9881366fd84e38939bcadfb173a3a) Thanks [@davidamunga](https://github.com/davidamunga)! - Phone > Connect Another Phone no longer quits the app: the old session's late end-of-stream message is ignored while the connect screen is showing.

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
