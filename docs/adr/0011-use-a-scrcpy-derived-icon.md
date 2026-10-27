# 0011. Use an icon derived from scrcpy's

- Status: superseded by [0026](0026-draw-an-original-icon-and-a-device-gray-look.md)
- Date: 2026-10-26

## Context

Handset needs an app icon. Several original marks were tried (frames, clappers, mascots, a unified
set of scenes). The one chosen is a monitor showing the Android robot with a phone in front of it: the
phone is what is being mirrored, the monitor is where it appears. It is the scrcpy icon's idea, drawn
again with a phone added.

## Decision

Use `app/packaging/icon/handset-icon.svg` (and a simplified `handset-icon-small.svg` for sizes up to
64 px) as the icon, built into `AppIcon.icns` by `scripts/make-icon.sh`.

The robot, head and monitor are derived from scrcpy's `app/data/scrcpy.svg`, which is Apache-2.0
licensed. The Android robot is a work created and shared by Google and used according to the terms of
the Creative Commons 3.0 Attribution licence. `NOTICE` carries both attributions. Status colours
(antenna and eyes) are part of the artwork and can be used for a live Dock icon later.

## Consequences

- The icon is recognisably in the scrcpy family. That suits a client of the scrcpy server, but it can
  be mistaken for scrcpy's own icon, so the README says Handset is independent.
- Distribution to a store would need a second look: Google Play and the Mac App Store both reject
  icons that are easily confused with another app's or that imply Google's endorsement.
- Replacing the icon later means editing two SVGs and running one script.

## Alternatives considered

- **An original mascot or object** (frame-bot, clapper, picture frame): distinct, and avoids every
  question above. Kept as the fallback if the resemblance becomes a problem.
- **The scrcpy icon unchanged:** simplest, but gives no sign that this is a different app.
