# 0016. Pair with a QR code and find phones with mDNS

- Status: accepted
- Date: 2026-10-26

## Context

ADR 0014 put a connect screen in the frame. USB was covered, but Wi-Fi needed typed commands, and
Android 11's wireless pairing needs an address, a pairing port and a six digit code that change each time.

## Decision

The screen gets two buttons, Wi-Fi and QR code, and is drawn with clickable buttons: each render returns
the rectangles it drew, and the app maps clicks and hover from window space to the picture.

- **Finding phones:** while the Wi-Fi or QR view is open, the poll thread also runs `adb mdns services`. A
  `_adb-tls-connect` entry is a phone that can be connected to; a `_adb-tls-pairing` entry is a phone on its
  pairing screen. adb usually connects paired phones to itself, and they then show up in `adb devices`.
- **QR pairing:** Handset shows `WIFI:T:ADB;S:<name>;P:<password>;;` with a random name and password
  (from `/dev/urandom`, drawn with the `qrcode` crate). A phone that scans it announces a pairing service
  called `<name>`; when it appears, the app runs `adb pair <ip:port> <password>` and then connects to that
  phone if adb has not already.
- **Typing an address:** the screen has no text field, so a native prompt (`osascript`) asks for it.
- Network work (`adb connect`, pair, the Wi-Fi switch, the prompt) runs on threads and reports back with a
  user event, so the window never blocks.

## Consequences

- No typing is needed to add a phone, only to scan.
- Pairing depends on adb's mDNS support and on the network allowing multicast; the troubleshooting runbook
  covers `adb mdns check`.
- The password is a one-off value shown on screen for the length of the pairing; it is not stored.
- One new dependency, `qrcode` (without its image features).

## Alternatives considered

- **Draw our own QR encoder:** no dependency, but QR error correction is easy to get subtly wrong.
- **Ask for the pairing address and code in the app:** works on every Android 11 phone but needs two typed
  values on a timer.
