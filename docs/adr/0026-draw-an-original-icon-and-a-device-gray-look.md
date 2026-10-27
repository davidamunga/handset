# 0026. Draw an original icon and a Device Gray look

- Status: accepted
- Date: 2026-10-27

## Context

The icon (ADR 0011) was a monitor with the Android robot and a phone, derived from scrcpy's icon. Handset
now mirrors an iPhone as well (ADR 0025) and runs on macOS and Windows, so a robot says the wrong thing,
and the resemblance to scrcpy's icon was already a risk for store distribution. The connect screen and the
style editor also carried a green theme taken from the robot, and the editor was dark only.

## Decision

- The icon is an original mark: a phone with two eyes in front, and two echo phones fanned behind it, one in
  Android green and one in iOS blue, on a light grey tile. `handset-icon.svg` is the full drawing,
  `handset-icon-small.svg` the simplified one for sizes up to 64 px, and `editor-icon.svg` the dark-tile
  variant that keeps the editor window apart from the mirroring window. `scripts/make-icon.sh` builds them.
- The Dock and window icon still shows state, now through the front phone's eyes: grey while idle, amber
  while connecting, white while live, red with a dot while recording.
- The connect screen is black, like a phone screen, with white text and the colour mark without its tile
  (`handset-mark.svg`). Buttons have three kinds: a near-white primary for the one thing to do, dark grey secondary
  buttons for the other ways, and a quiet one for going back, each with a small drawn symbol. While Handset is
  looking for a phone the logo's eyes glance about. Status colours (amber, red, and green for passed checks)
  remain, for meaning only.
- The style editor uses Device Gray, a grey palette with one accent (near white on dark, graphite on
  light), and follows the operating system's light or dark setting through egui's system theme. The
  palette lives in `theme` in `editor.rs` and is read at draw time.
- The README opens with the brand Open Graph image. `docs/images/og/` holds it and the product one.

## Consequences

- Nothing in the icon is derived from scrcpy or Google's robot, so `NOTICE` no longer carries those
  attributions for it. The green and blue are colours, not artwork.
- The Windows `handset.res` still holds the old icon until `scripts/make-windows-res.ps1` is run on
  Windows.
- Colour in the interface is for status only, which leaves room for a brand colour later.

## Alternatives considered

- **Keep the robot and add an Apple mark:** two platform logos in one icon, and still scrcpy's idea.
- **A colour theme instead of grey for the editor:** tried in Figma (indigo and ocean palettes); grey was
  chosen so the icon's two platform colours are the only colour in the brand.
