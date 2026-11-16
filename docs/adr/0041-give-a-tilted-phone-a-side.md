# 0041. Give a tilted phone a side

- Status: accepted
- Date: 2026-11-16

## Context

The live tilt (ADR 0037) turns a flat picture in perspective, which reads as a card, not a phone.

## Decision

- `Tilt::body` projects the front face and `n` copies of it at equal steps behind it (along the phone's own
  normal), fitted to the canvas together. The renderer draws the bezel again at each step, back to front, in a
  darker metal (`extra.z` in the shader), then the front face.
- Only the live window and only when tilted. Captures and the editor's pictures are unchanged. Without a frame the
  same slices are of a plain rectangle the size of the screen.
- The slices use only the solid part of the frame art (a smoothstep on alpha), so the art's faint soft shadow does
  not turn into a grey haze.

## Alternatives considered

- **A real 3D mesh for the phone:** a stack of the existing bezel is enough for a thin side and needs no model.

## Consequences

- A tilted phone is a little smaller, because the side has to fit too, and costs 14 extra draws of the bezel.
- The metal tone is one fixed tint; a colour taken from the frame could come later.
