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

- A recording is flattened onto an opaque colour, so its transparency is lost. The phone's outline does not
  change during a recording, so Handset saves it once at the start as `<name>.matte` (a greyscale PNG) beside
  the recording. Styling uses it as the shape of the hole in the backdrop and as the shadow's silhouette, so
  the phone sits directly on the background. A recording without a matte (made by an older version, or moved
  without it) is styled as a rounded card on its own backdrop colour; the corner radius only applies then.
- Styling a video re-encodes it (libx264, crf 18); audio is copied.
- Styling needs `ffmpeg` for video and for non-PNG background images only.

## The editor window

`handset --edit <file>` opens an `eframe`/`egui` window (egui 0.37 uses the same wgpu 30 and winit 0.30 as the
renderer, so nothing is duplicated). It runs as its own process, started from Recent Captures > Edit Style...,
so it does not touch the mirroring window's event loop and works for any saved file. The preview is a
downscaled still (a picture, or a video's first frame) styled by the same `Style::compose` as the export.
Choosing a picture background uses a native file dialog (`rfd`). Exporting remembers the look as the
`style` setting. egui adds compile time and some binary size; the alternative, hand-drawing sliders and a
colour picker in the bitmap-font overlay, would have been a lot more code for a worse result.

Playing a recording in the preview uses a worker thread that has ffmpeg decode small RGBA frames at their real
pace (a single frame per seek while scrubbing, newest request wins). The backdrop is drawn once per style
change and only the picture is blended again per frame. The preview is silent; audio is only in the export.

Trimming (`trim.rs`) is applied at the ffmpeg input (`-ss` and `-t` before `-i`), so only the kept part is
decoded and styled and the audio is cut with it. The start and end are clamped so that a clip is never shorter
than 0.2 s, and a trim within 0.05 s of the whole recording counts as no trim. It is not applied to pictures.

Saved looks (`presets.rs`) are a style under a name. Yours are kept in `~/.config/handset/looks`, one
`name<TAB>style` per line, in the same text form the `style` setting uses, so the file is easy to read and
edit; a line that cannot be read is skipped rather than losing the rest. Five built-in looks ship with the app.
`--style` accepts a name (yours first, then the built-in ones, ignoring case) or a style written out.
