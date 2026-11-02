# 0039. Share the phone by forwarding its H.264 over WebRTC

- Status: accepted (the same-network version is built; the cross-network service is still to build)
- Date: 2026-10-31

## Context

People want to show or hand their phone to someone who is not at the same desk: a colleague checking a bug,
a client watching a demo. Vysor sells this as Vysor Share. Handset already receives the phone's H.264 from
the scrcpy server and decodes it for its own window, so it has the stream; the question is whether a
browser can play that same stream, and take taps back, without a second encode.

A spike (`spikes/share`, notes in `SPIKE.md`) answered that with a Nokia G21 (Android 12, USB):

- The browser plays the phone's packets untouched. The encoder sends Constrained Baseline,
  profile-level-id `42c014`, and labels it level 2.0 for a 576x1280 picture; browsers follow the SPS and
  ignore the label. A High 4.1 test file also played under a Baseline payload type, so the negotiated
  profile is a hint. Chrome, Safari and Firefox on a Mac all played it.
- A new viewer needs a keyframe at once. scrcpy's reset-video control message (type 17) produces one, and
  each keyframe must be sent with the config packet (SPS and PPS) in front of it.
- Rotation sends a new session packet; once the host followed the new size and kept prefixing config,
  the viewer switched between 576x1280 and 1280x576 without a freeze.
- A viewer on another network played the stream and controlled the phone with no relay, using a STUN
  lookup for the host's public address and a viewer that waits for its candidates before it posts its
  offer. That is one network pair; carrier NAT and strict firewalls are expected to need a relay.
- Touch control over a data channel works. Median touch-to-first-changed-frame was about 155 ms (118 to
  189 ms over six trials), including about 50 ms of Android's touch slop and animation start. There is no
  baseline against the desktop window.

Not measured: hardware decoders in phone browsers, encoders that send High profile or B-frames, loss or a
throttled link, audio, and load with many viewers.

## Decision

We will build Share as a WebRTC link from Handset to a browser:

- **Video is forwarded, not re-encoded.** The host hands the phone's Annex B packets, with the latest config
  in front of each keyframe, to the WebRTC stack. The video thread's decoder path stays as it is.
- **The host speaks WebRTC with `str0m`**, which is sans-IO and fits Handset's threads without an async
  runtime. `webrtc-rs` is the fallback if `str0m` cannot do something we need.
- **A small signalling service** carries the offer and answer, and nothing else. It is written to be
  self-hosted, and Handset takes the server address as a setting. A TURN relay covers networks that
  direct connections cannot cross.
- **Control travels on a data channel** and is checked by the host: coordinates are validated, a move or
  up without a matching down is dropped, and the viewer is view-only unless the host allows control.
- **The viewer is a static page** that draws the video inside the device frame and sends pointer events.
- **The host approves each viewer** and shows that it is sharing, with a stop button. The spike's secret
  link is not the final design.
- **The server and the viewer are open source** under the same licence as Handset, in their own repository,
  so anyone can read what the service sees (the handshake, never the video) and run their own.

## What the first version does

The same-network version follows the decision above, with these choices made while building it (tracking
issue #134):

- **No signalling service and no STUN yet.** Handset serves the viewer page and answers the offer itself,
  with host candidates only. Nothing leaves the local network, so no third party sees a viewer's address.
- **The host decides about each viewer, and what that viewer may do.** A dialog offers Allow and Control,
  Watch Only or Don't Allow. A refused address is not asked again for 30 seconds, only one dialog is open at
  a time, and at most four viewers and three waiting requests are accepted. A malformed offer never reaches
  the dialog. The window shows how many are watching and controlling, and Stop Sharing ends everything.
- **A viewer that sends nothing for 10 seconds is gone.** A closed tab does not say goodbye and the ICE
  state alone never reported it.
- **Keyframe requests are limited to one per 700 ms for all viewers together.**
- **Control is parsed strictly and gated.** Messages are short text. Coordinates are clamped to the picture,
  text is capped at 200 characters without control characters, each viewer touches with its own pointer id,
  and a viewer that leaves mid-touch lifts its finger. `user_rotation` is not restored after a rotate, the
  same as the app's own Rotate Device.
- **Sound is encoded once.** The phone's raw PCM is cut into 20 ms frames and encoded to Opus (libopus, built
  from source, so building needs `cmake`) for all viewers. The browser's sound button starts off because
  browsers do not play sound unprompted.
- **The host sees what is shared before it agrees.** The dialog and the viewer page say what is shown and
  what control can reach. Share asks before starting on a computer with a public internet address; it cannot
  tell a cafe's Wi-Fi from a home one.
- **No bitrate adaptation.** The phone's encoder is fixed when mirroring starts. A slow viewer loses frames
  and resumes at the next keyframe.

## Alternatives considered

- **WebSocket with fragmented MP4 in a `<video>` (Media Source Extensions).** Simpler server, but it adds
  latency and sends every byte through the server, so there is no direct path and bandwidth is always ours.
- **WebCodecs in the viewer.** More control over decoding, more code to get wrong; `<video>` over WebRTC
  already worked in three browsers.
- **A media server (an SFU such as LiveKit).** Fast to adopt, but a heavy dependency and cost for one host
  and a few viewers.
- **Only a tunnel to a local web server.** No infrastructure of ours, but every viewer needs the tunnel
  address, it carries all the video, and it gives the host no approval step.
- **Re-encode for the browser.** Would guarantee a profile every decoder accepts, at the price of a decode
  and an encode per stream on the host. Held back as the fix if a decoder refuses the phone's stream.

## Consequences

- No new codec work on the host in the common case, so the cost of Share on the host should be small. This
  is not measured yet.
- The relay is the main running cost, and some sessions will always need it. How it is paid for is open.
- Still to do from the spike's list: the signalling service and TURN, and a versioned spec for the
  signalling and data-channel messages.
- A phone whose encoder sends something a browser's decoder refuses would need a codec option passed to the
  scrcpy server, or a re-encode.
- The signalling protocol and the data-channel messages become a public contract once other people run
  servers, so they need a version and a written spec before the first release.
- Sharing a screen sends notifications and passwords to whoever watches. The host must say so before a
  session starts.
