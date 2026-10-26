# 0021. Draw generic device frames ourselves

- Status: accepted
- Date: 2026-10-27

## Context

Handset shipped three colourways of one phone shape (ADR 0004). People mirror very different devices: notched
and punch-hole phones, tablets, foldables, rugged handsets. The internet has many device mockup sets, but
nearly all of them are a maker's artwork (Apple, Samsung and Google bezels and their trade dress) or a
third-party designer's work with its own licence terms. Bundling them in an Apache-2.0 project, in a binary
people redistribute, is a licensing and trademark risk, and each set would need its own attribution file.

## Decision

Every built-in frame is drawn from scratch by `frames/make_frames.py`, from a small `Device` description
(screen size and corner radius, glass border, rail, cutout, buttons) and a few colourways. The shapes are
generic device types (`classic`, `notch`, `island`, `drop`, `home`, `flat`, `tablet`, `fold`, `flip`,
`rugged`), not copies of a product. Frames are named `<device>-<colour>`; the original three keep their plain
names so saved settings and scripts still work. The Frame menu groups them by device. Anything else is
supplied by the user as a PNG with a `.frame` file (`docs/frames.md`).

## Alternatives considered

- **Bundling downloaded mockup sets.** Many are free to look at and not free to redistribute, and the
  artwork is trademarked. Checking each licence and shipping attributions is ongoing work with legal risk.
- **Downloading frames on demand.** Adds a network dependency and a place for the frames to disappear.
- **Only the custom-PNG route.** Works, but most people will not draw their own.

## Consequences

- The binary grows by about 1 MB (21 frames).
- Frames look like their device type, not like a particular model. Someone who wants an exact model can
  use their own PNG, which stays a supported route.
- A new device type is a `Device` entry plus a line in the registry in `app/src/frame.rs`; a test loads every
  built-in frame.
