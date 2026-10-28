# 0037. Tilt the phone in 3D with a homography: in the window and in saved pictures

- Status: accepted
- Date: 2026-10-28

## Context

A flat phone on a gradient is the usual mockup. A turned, leaning phone with perspective looks better on a
website, in a post and on screen while presenting. Input is mapped through `video_rect` and `bezel_rect`, and
hit-testing uses the bezel's alpha mask, all of which assume an axis-aligned rectangle. Styling works on RGBA
pictures (ADR 0004 frames, `style.rs`).

## Decision

`Style` gets a `tilt` (yaw, pitch and roll in whole degrees, spec key `tilt=yaw,pitch,roll`, left out of the
spec when flat). `tilt.rs` rotates the picture's four corners, projects them with a pinhole camera, solves the
homography between the flat picture and the quad, and looks each output pixel up in the source (2x2 samples, so
edges are smooth).

- The result has the **same size** as the capture, with the quad scaled to fit. Padding, aspect ratio and
  export sizes do not change, and the editor needs no new layout code.
- The corners are rounded **before** the warp, and `Style::layout` reports a radius of 0 for a tilted style so
  the rectangle mask does not cut into the turned shape.
- The warped picture is the silhouette for the shadow, so the shadow follows the tilt.
- It applies to **pictures only** (`Style::compose`, the editor preview). `restyle::video_style` drops the tilt,
  and the editor hides the sliders for recordings.

### The live window

The same corner maths (`Tilt::corners`) gives a homography from the flat layout to the turned one, in window
space, computed every frame in `Renderer::update_uniforms`. The vertex shader applies it to the video, the black
screen fill, the bezel and the touch dots, and outputs a homogeneous position so the textures stay
perspective-correct. The window keeps its size and the quad is scaled to fit inside the bezel's flat rectangle.

- The pointer goes through the inverse (`Renderer::untilt`) before it is compared with `video_rect` and the
  bezel's alpha mask, so touch, hover and dragging land where the picture shows them.
- The toolbar, toast, banner and menu stay flat on top.
- Captures and recordings of the live window render offscreen and stay flat (the matrix is the identity unless
  the window is drawn), so a saved screenshot is turned only by the editor's tilt.
- Chosen from the right-click Tilt menu (named looks), `--tilt yaw,pitch,roll`, and remembered as `tilt`.

## Alternatives considered

- **Make the window larger to hold the corners:** the quad is fitted inside the existing rectangle instead, so
  the window, its resizing and the toolbar strip do not change.
- **ffmpeg `perspective` for recordings:** not checked for alpha with the bezel matte; left for a follow-up.
- **Warp on the GPU:** faster for a playing video, but a still takes a few milliseconds on the CPU with the
  existing `par_rows`, and the pipeline is CPU already.

## Consequences

- A tilted phone is smaller in its frame than a flat one, because the whole quad has to fit.
- Recordings, and screenshots taken from the live window, are flat until styled; recordings cannot be tilted yet.
- The flat banner and overlays do not follow the tilt.
- The camera distance is a constant; a "perspective strength" control can be added if needed.
