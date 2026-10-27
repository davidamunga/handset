# AGENTS.md

Guidance for coding agents (and humans) working in this repository.

## What this is

Handset mirrors and controls an Android phone from a Mac, inside a realistic phone frame. It is a
small native Rust app that implements the desktop side of the scrcpy protocol and embeds the
**unmodified** scrcpy server. It is tested on macOS and Windows 11; macOS is the primary platform.

## Layout

| Path | What lives there |
|---|---|
| `app/` | The Rust crate (binary `handset`) |
| `app/src/adb.rs`, `server.rs` | Run adb, push and start the server, read its sockets |
| `app/src/stream.rs` | Video thread: read packets, decode H.264, publish the latest frame |
| `app/src/render.rs`, `shader.wgsl` | wgpu renderer: YUV to RGB on the GPU, bezel, overlays, offscreen capture |
| `app/src/app.rs` | winit window, input, toolbar, toast, menu events, recording timer |
| `app/src/control.rs` | Control messages sent to the phone (touch, scroll, keys, text, rotate) |
| `app/src/frame.rs`, `overlay.rs`, `toolbar.rs` | Bezel loading, bitmap-font pills, toolbar icons |
| `app/src/audio.rs`, `capture.rs` | Raw PCM audio and playback; screenshots and ffmpeg recording |
| `app/src/ios.rs` | `--ios` (macOS): an iPhone or iPad over USB through AVFoundation screen capture, video and audio, view only |
| `app/src/audio_preview.rs` | Sound for the editor's video preview: ffmpeg decodes the audio and a feeder paces it to the speakers |
| `app/src/connect.rs`, `mirror.rs` | The landing screen drawn in the frame while no phone is ready (USB status, Wi-Fi list, QR pairing, clickable buttons); one mirroring session (scrcpy server or iOS capture, video, audio) |
| `app/src/style.rs`, `restyle.rs` | Capture styling: background, padding, shadow, radius, aspect (pure pixels); styling saved PNG, MP4 and GIF files (ffmpeg for video) |
| `app/src/trim.rs` | The part of a recording to keep (start and end, clamped), used by the editor's trim handles and the export |
| `app/src/presets.rs` | Named looks (a style saved under a name): built-in ones, yours in `~/.config/handset/looks`, and `--style <name>` |
| `app/src/editor.rs` | `handset --edit <file>`: egui window with live preview and playback, run as its own process |
| `app/src/update.rs`, `install.rs` | Looking for a newer release; how this copy was installed (Homebrew, Scoop, winget, a download) and what the update notice says for each |
| `app/src/updater.rs` | Updating a downloaded install in place: download, verify, unpack, then a helper script swaps the app (with rollback) |
| `app/src/verify.rs` | Release signature and checksum checks (Ed25519 over `SHA256SUMS`), and `--verify-release` |
| `app/src/menu.rs`, `tools.rs` | Native macOS menu; tool lookup, error dialogs, default arguments |
| `app/assets/` | Embedded scrcpy server, the built-in phone frames and the Inter fonts |
| `site/` | The website (Vite, React, Tailwind): its own package, never shipped in a release; see `site/README.md` |
| `frames/` | Script that generates the frame PNGs |
| `app/packaging/` | App icon: SVG sources, `AppIcon.icns` and a README PNG (rebuild with `scripts/make-icon.sh`) |
| `scripts/` | Packaging, icon, version-sync, release-branch (`release-branches.sh`, tested by `test-release-branches.sh`), update-key (`update-key.sh`, tested by `test-update-key.sh`) and hook scripts |
| `.changeset/`, `package.json` | Changesets config and the version it bumps (release tooling only, see `docs/runbooks/release.md`) |
| `docs/adr/`, `docs/runbooks/` | Architecture decisions and repeatable procedures |

## Commands

```sh
cd app
cargo run --release -- --audio     # run against a connected phone
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test -- --ignored clipboard  # changes the real clipboard, run on purpose only
../scripts/package-macos.sh        # builds dist/Handset.app
../scripts/install-hooks.sh        # enable the commit message hook
npm run changeset                  # (repo root) add a changeset for a user-visible change
```

## Conventions

- Commits and PR titles use Conventional Commits. Scopes are in `.commit-scopes`.
- Do not add attribution lines to commit messages or pull request descriptions.
- Rust 2024 edition, `rustfmt.toml` in `app/`, clippy clean with `-D warnings`.
- Comments explain why, not what. Keep them as sparse as the surrounding code.
- `app/packaging/windows/handset.res` holds the exe icon; regenerate it with `scripts/make-windows-res.ps1`.
- Windows-only code is behind `cfg(windows)`; shell out via `tools::command` so no console window flashes.
- macOS-only code is behind `cfg(target_os = "macos")` and must not break other targets.
- Do not commit build output, the local `scrcpy/` checkout, `frames/out/` or personal captures.
- Record architectural decisions as ADRs (see `docs/adr/README.md`) in the same change.

## Testing and safety

- Most behaviour needs a real phone. Check `adb devices` first.
- Do not send synthetic mouse or keyboard events to the developer's machine, or take full-screen
  screenshots of it, without asking. Capture only the app window (`screencapture -l <window id>`).
- Anything that changes the phone (rotation, volume, settings, launched apps) must be restored
  afterwards. Use `--mute` when testing audio so the Mac stays quiet.
- Delete test captures from `~/Pictures/handset` and `~/Movies/handset` when you are done, and do not
  overwrite the user's clipboard without saying so.
- When writing files from a shell, quote heredocs (`<<'EOF'`) so backticks and `$` in Markdown are
  never executed.

## Architecture in one paragraph

adb forwards a local port to the server on the phone. `server.rs` opens the video, optional audio and
control sockets in that order. A video thread decodes H.264 with openh264 and writes the newest
frame into a shared slot, waking the UI thread through an event-loop proxy. The UI thread uploads the
planes to wgpu, draws video then bezel, and writes control messages straight to the control socket.
The toolbar, toast and REC pill are drawn only in the window pass, so screenshots and recordings,
which render offscreen through `Renderer::capture`, never contain them.

## Gotchas

- `SERVER_VERSION` in `server.rs` must equal the embedded server's version exactly.
- A Finder-launched app has a bare `PATH`; always locate `adb` and `ffmpeg` through `tools::find`.
- The window is transparent and borderless when a frame is active; hit-testing uses a quarter-
  resolution alpha mask of the bezel.
- Frame metadata is a sidecar `<name>.frame` file holding `x y w h` of the screen area in pixels.
- wgpu 30 differs from older tutorials (for example `queue.present(frame)`); read the crate source.
