# 0017. Draw the connect screen with Inter and the app icon

- Status: accepted
- Date: 2026-10-26

## Context

The connect screen (ADR 0014, 0016) was drawn with the 5x7 bitmap font used for toasts: upper case only,
hard to read at length, and it looked unlike the rest of Handset. The screen is the first thing a new
person sees, and it should look like the app.

## Decision

Rasterize the screen with real text: the Inter Regular and Semibold fonts are embedded (about 830 KB, SIL
Open Font Licence, `licenses/Inter-OFL.txt`) and drawn with `ab_glyph`, anti-aliased, in sentence case and
wrapped by measured width. The app icon (`handset-icon-512.png`) is embedded, decoded once and scaled with
an alpha-weighted box filter. Colours come from the icon: a deep teal background, the icon's teal and green,
and its mint for text. Buttons are pill shapes drawn from a distance field. Toasts and the REC indicator keep
the bitmap font, since they are tiny and need no layout.

## Consequences

- The binary grows by roughly 0.85 MB.
- Text is readable at a glance, and the screen matches the icon and the README.
- A bigger or subsetted font would change the size trade-off; subsetting needs fonttools.
- Two renderers for text now exist (pills and the landing screen); merging them is possible later.

## Alternatives considered

- **System fonts through Core Text:** no embedded files, but macOS only and a second drawing path.
- **A subsetted font:** about 50 KB instead of 830 KB, at the cost of a build step and a tool to install.
