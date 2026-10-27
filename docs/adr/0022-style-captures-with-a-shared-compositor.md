# 0022. Style captures with a shared compositor

- Status: accepted
- Date: 2026-10-27

## Context

People want to post a screenshot or recording on a styled background (gradient, wallpaper, plain colour or
their own picture) with padding, a shadow, rounded corners and a chosen aspect ratio, as CleanShot X does.
This has to work for PNG, MP4 and GIF, both right after capturing and later on a saved file, and it must
never destroy the original.

## Decision

- A pure-pixel module, `style.rs`, owns the look (`Style`), the layout (output size and where the capture
  sits, for auto and fixed aspect ratios) and the drawing of background and shadow. All amounts are
  percentages of the capture's size, so the editor's small preview and the full export match.
- Pictures are composed in process. The shadow follows the capture's own alpha, so a phone frame with
  transparent corners gets a shadow shaped like the phone.
- Recordings go through ffmpeg (ADR 0008). Handset draws one backdrop PNG with the capture's rounded
  rectangle cut out (transparent, anti-aliased at the corners); ffmpeg pads the video to the output size and
  lays the backdrop over it. This keeps the video's own timing and audio and needs no mask filters.
- Backgrounds are made by us: ten gradients, six procedural wallpapers (soft colour blobs with a warp) and
  any image the person picks. No third-party artwork is bundled, for the reason given in ADR 0021.
  Images other than PNG are decoded by ffmpeg, so no image-decoding crate is added.
- The styled file is always a new file, `<name>-styled.<ext>`, beside the original.
- Styling happens both ways: `--style-captures` (menu: Save a styled copy of each capture) styles each new
  capture on a background thread, and `--restyle <file>` or Recent Captures > Save Styled Copy styles an
  existing file. The look is the `style` setting or `--style <spec>`.

## Alternatives considered

- **Style in the wgpu renderer at capture time.** Gives video a real alpha silhouette, but ties styling to
  capture time and cannot restyle saved files.
- **An ffmpeg filter graph for everything** (gradients through `gradients`/`geq`, blur for shadows): slow and
  hard to keep identical to the preview.

## Consequences

- A recording has an opaque background colour, so its styled copy shows the phone and that colour as a
  rounded card, not the bare phone silhouette. Recording the phone with transparency would fix that; it is
  not done yet.
- Styling a video re-encodes it (libx264, crf 18); audio is copied.
- Styling needs `ffmpeg` for video and for non-PNG background images only.
