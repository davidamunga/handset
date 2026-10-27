# 0025. Mirror iOS devices through AVFoundation screen capture, view only for now

- Status: proposed
- Date: 2026-10-27

## Context

People asked whether Handset could show an iPhone as well as an Android phone. iOS has no adb and no
equivalent of the scrcpy server: nothing can be pushed to the phone and started from the Mac. But macOS can
already capture a connected iPhone's or iPad's screen and sound over USB. QuickTime records it this way:
once CoreMediaIO is told to allow screen capture devices
(`kCMIOHardwarePropertyAllowScreenCaptureDevices`), the device appears as an external AVFoundation capture
device that carries both video and audio (a "muxed" device). Nothing is installed on the phone, and it does
not need developer mode.

Control is a different problem. The only practical route without extra hardware is WebDriverAgent (an
XCUITest runner signed with the person's Apple ID). It sends taps and drags as whole actions, roughly 100 to
300 ms each, and needs Xcode signing and, from iOS 17, a root tunnel.

## Decision

`handset --ios [name|id]` (macOS only) mirrors the first connected iOS device, or the one named, without
control:

- `ios.rs` sets the CoreMediaIO property, waits up to four seconds for the device, and runs an
  `AVCaptureSession` with a video output (NV12, video range, split into I420 for the existing slot and
  renderer) and an audio output (48 kHz stereo signed 16-bit, the same PCM the scrcpy server sends).
- `Mirror` wraps either a scrcpy session or an iOS capture. The window, frame, toolbar, screenshots,
  recordings and styling do not know the difference.
- The control is `Control::disconnected()`: input is ignored. Dropping files is refused with a toast.
- iOS only sends audio while something plays, so silence fills gaps in what the recorder receives.
  Otherwise ffmpeg's `-shortest` cuts the video at the end of the sound.
- When the device goes away, Handset finishes any recording and quits. The connect screen is Android only.
- The app bundle declares `NSCameraUsageDescription` and `NSMicrophoneUsageDescription`. macOS treats the
  capture as camera and microphone use. Without access the session runs but delivers nothing, so Handset
  asks first and stops with a clear message if access is denied.

## Alternatives considered

- **ffmpeg's avfoundation input:** already a dependency, but it only lists video and audio devices, never
  muxed ones, so it does not see the iPhone's screen.
- **A Swift helper process:** simpler Objective-C interop, but another binary to build, sign and ship.
  The `objc2` crates Handset already uses cover the APIs.
- **AirPlay receiver (as UxPlay does):** wireless and cross-platform, but the person must start mirroring
  from Control Center each time, latency is higher, and it means implementing AirPlay.
- **Re-implementing the USB protocol (as quicktime_video_hack does):** would reach Windows, but needs
  driver swaps there and tracks a private protocol.

## Consequences

- iOS mirroring is macOS only and USB only.
- Run from a terminal, macOS attributes camera access to the terminal (or whatever launched Handset). If
  that app cannot be granted access, the capture is refused. The packaged `Handset.app` gets its own prompt.
- iOS replaces the status bar with a clean one (9:41, full signal and battery) while it is captured. That
  suits demo captures.
- The frames are drawn for Android phones. The iPhone's taller screen fits with small bands, and an iPhone
  bezel would be its own change.
- Control (WebDriverAgent, or a Bluetooth HID accessory) and an iOS entry on the connect screen are left
  for later. The `Mirror` split is where they plug in.
