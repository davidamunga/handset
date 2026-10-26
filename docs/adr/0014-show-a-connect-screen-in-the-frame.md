# 0014. Show a connect screen inside the frame

- Status: accepted (the bitmap typeface is superseded by [0017](0017-draw-the-connect-screen-with-inter-and-the-app-icon.md))
- Date: 2026-10-26

## Context

With no phone ready, Handset printed "No Android device found" in a dialog and quit. That hides the actual
problem (no cable, a phone waiting for the Allow prompt, a stale connection) and makes Wi-Fi and pairing
flows impossible to add, because there is nowhere to show progress.

## Decision

Open the window anyway and draw a connect screen in the phone's screen area. A background thread runs
`adb devices` about every second and sends the result to the app, which picks the phone to use (or the one
named on the command line) and otherwise explains what it sees. When a phone is ready the app starts the
session itself and the first video frame replaces the screen. The screen is rasterized on the CPU with the
overlay's bitmap font and uploaded as a video frame, so the renderer needs no new code. A session is now a
`Mirror` value that `main` or the app can start, and `Control` accepts messages before a phone is attached.

## Consequences

- No-phone, unauthorised and offline are visible, self-healing states instead of a fatal error.
- The bitmap font is upper case only and plain; a nicer typeface would mean text rendering in the GPU path.
- Wi-Fi lists, QR pairing and reconnect-on-drop can reuse the same screen.
- `--connect` and a failing `--wifi` still stop with a dialog, because the person asked for something
  specific that did not work.

## Alternatives considered

- **A separate setup window:** more room for controls, but a second window to manage and a jump between two.
- **Native dialogs:** quick, but cannot update live as the phone appears.
