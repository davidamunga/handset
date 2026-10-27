# 0028. Find phones on the network ourselves, next to adb

- Status: accepted
- Date: 2026-10-27

## Context

QR pairing waits for the phone to announce an `_adb-tls-pairing` service named after the code, and Wi-Fi
connecting lists `_adb-tls-connect` services. Both came only from `adb mdns services`. On some machines that
list is empty or misses the short pairing window while the system's own mDNS sees the phone at once (checked with
`dns-sd`, with several network interfaces and VPN tunnels present). The screen then waited forever.

## Decision

`discover.rs` browses both service types with the `mdns-sd` crate (pure Rust, the same on macOS, Windows and
Linux) and keeps the result. The poll merges it with adb's list, each service once. The QR screen also offers
"Pair with a code" (a native prompt, then `adb pair` against the phone's own pairing service) and, after 12
seconds without a phone, says what usually gets in the way.

## Consequences

- One more dependency, and a background thread per service type once a Wi-Fi or QR view has been opened.
- Pairing no longer depends on adb's discovery working. Phones whose Wi-Fi drops multicast are still not found;
  the hint and the code path are for them.
- Linux needs no extra system library for this.

## Alternatives considered

- **A macOS-only `dns-sd` fallback:** smaller, but parses text from three commands and helps only one platform.
- **Forcing `ADB_MDNS_OPENSCREEN`:** tried; it found the connect service but not the pairing one, and adb is
  started by other tools too, so we cannot rely on its environment.
