# Share: what the page and Handset say to each other

This is the contract between Handset and the page it serves for [Share in a browser](share.md). It is written down because
other pages and tools may speak it, and so a change to it is a decision rather than an accident. The reasoning behind
Share is in [ADR 0039](adr/0039-share-the-phone-by-forwarding-its-h264-over-webrtc.md). The code is in
`app/src/share.rs` (the server and the connection), `share_input.rs` (what a viewer may do) and `share_audio.rs` (sound),
and the page is `app/assets/share-viewer.html`.

There is no version number yet. The first change that is not backward compatible should add one.

## Over HTTP

Handset listens on port 8099, or on a free port if that one is taken. It answers four requests and nothing else.

| Request | Answer |
|---|---|
| `GET /` | The viewer page. |
| `GET /styles.json` | The editor's backgrounds, looks and limits, so the page offers what the editor does: `gradients`, `wallpapers`, `looks`, `aspects`, `maxPadding`, `maxRadius`. |
| `GET /frame.png` | The frame the host has chosen, as the PNG itself, or `404` when the host uses none. Never cached. |
| `POST /offer` | The viewer's WebRTC offer (SDP text, at most 64 KB) in the body. The answer is the host's SDP. |

`POST /offer` can be refused, and the page tells the viewer why:

| Status | Meaning |
|---|---|
| `200` | Allowed. The body is the SDP answer. |
| `400` | The offer could not be read. The host is not asked. |
| `403` | The host refused, or refused this address in the last 30 seconds. |
| `429` | Three people are already waiting for the host to answer. |
| `503` | Four viewers are already connected, or sharing has stopped. |

The request waits until the host has answered the dialog. Only one dialog is open at a time.

## Over WebRTC

The offer has one receive-only video line and one receive-only audio line, and the page opens a data channel called
`control` before it makes the offer. Handset answers with host candidates only, on the address it has on the network the
viewer is on. It uses no STUN and no TURN, so both ends must be on the same network.

- **Video** is the phone's H.264, forwarded untouched in packetization mode 1. Every keyframe starts with the SPS and
  PPS. A new viewer starts at a keyframe. Keyframe requests from all viewers together are limited to one every 700 ms.
- **Audio** is Opus, 48 kHz stereo, in 20 ms packets, sent only while the phone sends sound. The audio and the video
  are stamped from the same clock on the phone.

## On the data channel

Every message is text. Handset speaks first, once the channel opens, and again whenever the look changes.

### From Handset to the page

| Message | Meaning |
|---|---|
| `audio` | The phone is sending sound, so show a sound button. |
| `control` | This viewer may touch the phone. |
| `view` | This viewer may only watch. Every viewer gets `control` or `view`. |
| `look,{json}` | How to draw the phone: `{"device":"Pixel 8","frame":null,"v":1}`, or with a frame `"frame":{"hole":[x,y,w,h]}`. `hole` is the screen's place in the frame picture as fractions of it, and `v` changes whenever the frame does, so the page fetches `/frame.png` again. |

### From the page to Handset

Handset reads these only from a viewer it told `control`, and ignores anything else. All numbers are decimals.

| Message | Meaning |
|---|---|
| `d,x,y` | Finger down at `x`, `y`, each a fraction of the picture from 0 to 1. Outside that is held on the edge. |
| `m,x,y` | Finger moved. Ignored unless that viewer put a finger down. |
| `u,x,y` | Finger up. Ignored unless that viewer put a finger down. |
| `s,x,y,h,v` | Scroll at `x`, `y` by `h` and `v` notches, held between -16 and 16. |
| `k,name` | A key: `back`, `home`, `recents`, `enter` or `del`. |
| `t,text` | Type text. Up to 200 characters; control characters are dropped. |
| `r` | Turn the phone. |

Each viewer touches with its own pointer id, so two people cannot take hold of each other's finger. A viewer that leaves
while its finger is down has it lifted. A viewer that sends nothing for 10 seconds is dropped.

## What this does not cover

The page and the connection setup travel over plain HTTP. See [SECURITY.md](../SECURITY.md) for what that means. Sharing
across networks needs a signalling service that does not exist yet, so nothing here describes one.
