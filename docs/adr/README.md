# Architecture decision records

Short notes that capture why Handset is built the way it is. One decision per file, numbered in
the order they were made. Never rewrite history: if a decision changes, add a new ADR and mark the old
one as superseded.

Statuses: `proposed`, `accepted`, `superseded by NNNN`, `rejected`.

Copy [`0000-template.md`](0000-template.md) to start a new one.

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | accepted |
| [0002](0002-build-a-native-rust-client.md) | Build a native Rust client instead of forking scrcpy | accepted |
| [0003](0003-reuse-the-unmodified-scrcpy-server.md) | Reuse the unmodified scrcpy server | accepted |
| [0004](0004-draw-device-frames-from-png-bezels.md) | Draw device frames from PNG bezels | accepted |
| [0005](0005-render-with-wgpu-in-a-transparent-window.md) | Render with wgpu in a transparent window | accepted |
| [0006](0006-decode-h264-in-software-with-openh264.md) | Decode H.264 in software with openh264 | accepted |
| [0007](0007-capture-raw-pcm-audio-and-play-it-with-cpal.md) | Capture raw PCM audio and play it with cpal | accepted |
| [0008](0008-record-through-an-ffmpeg-sidecar.md) | Record through an ffmpeg sidecar | accepted |
| [0009](0009-native-menu-on-macos-and-self-drawn-toolbar.md) | Native menu on macOS and a self-drawn toolbar | accepted |
| [0010](0010-package-as-an-ad-hoc-signed-app-bundle.md) | Package as an ad-hoc signed app bundle | accepted |
| [0011](0011-use-a-scrcpy-derived-icon.md) | Use an icon derived from scrcpy's | accepted |
| [0012](0012-release-with-changesets.md) | Release with Changesets | accepted |
| [0013](0013-sign-and-notarise-releases.md) | Sign with Developer ID and notarise releases | accepted |
| [0014](0014-show-a-connect-screen-in-the-frame.md) | Show a connect screen inside the frame | accepted |
| [0015](0015-release-only-when-asked.md) | Release only when asked, and never as a pre-release | accepted |
| [0016](0016-pair-with-a-qr-code-and-find-phones-with-mdns.md) | Pair with a QR code and find phones with mDNS | accepted |
| [0017](0017-draw-the-connect-screen-with-inter-and-the-app-icon.md) | Draw the connect screen with Inter and the app icon | accepted |
| [0018](0018-decode-h264-with-videotoolbox.md) | Decode H.264 with VideoToolbox | proposed |
