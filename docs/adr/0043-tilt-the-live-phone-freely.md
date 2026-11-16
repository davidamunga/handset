# 0043. Tilt the live phone freely

- Status: accepted
- Date: 2026-11-16

## Context

The live tilt (ADR 0037, ADR 0041) could only be one of five named presets, `--tilt`, or the next preset. People
want to turn the phone to any angle, snap to tidy ones, and push it the way they would a real object.

## Decision

- The tilt stays three whole degrees (yaw, pitch, roll) inside the existing limits, so the presets, `--tilt` and the
  saved `tilt` setting are unchanged. `Tilt` gains `dragged`, `nudged`, `snapped` and `clamped` as pure functions.
- Holding Alt (Option on a Mac) and dragging turns the phone toward the drag: right turns it right, down brings the
  top edge closer. Alt and scroll rolls it. The drag is tracked in the window's event handler before any touch
  handling, so no touch reaches the phone while turning.
- Snapping is a saved setting (`tilt-snap`: free, 5 or 15 degrees, in the Tilt menu). Holding Shift while dragging
  snaps to 15 degrees when the setting is free. A tilt near flat snaps to flat.
- Keys nudge by the snap step (5 degrees when free): shortcut actions `tilt:left|right|up|down|ccw|cw` and
  `tilt:flat`, with default Alt and the arrows, `,` `.` and `0`. They are ordinary shortcuts (ADR 0040).
- A drag changes the tilt live and saves it when the button is released, not on every move.
- Touch still lands where the pointer is, since `Renderer::untilt` uses whatever tilt is current. Captures stay flat.

## Alternatives considered

- **Plain drag turns the phone:** the phone is also the touch surface, so a modifier is needed.
- **Fractional degrees:** nothing is gained visually, and whole degrees keep the setting format and snapping simple.

## Consequences

- Alt and the arrow keys no longer reach the phone as D-pad presses unless the shortcut is changed.
