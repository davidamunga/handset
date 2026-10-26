# 0018. Decode H.264 with VideoToolbox (proposed)

- Status: proposed
- Date: 2026-10-26

## Context

ADR 0006 chose software decoding with openh264: it builds from source, needs no platform code and decodes
everything the scrcpy server produces. It has cost us two things since: a decoder that leaks pictures with its
default flush (fixed by not flushing) and CPU time that a laptop spends on battery.

Measured with `scripts/measure-cpu.sh` on an Apple M4 Pro, mirroring a UNIWA W888 (886x1920 H.264, High 4.1) while
the phone's launcher was scrolled about every three seconds, 20 s run:

| Decoder | Average CPU | Peak CPU |
|---|---|---|
| openh264 (software) | 10.1 % of one core | 38.8 % of one core |

That is whole-process use (decode, upload, render, audio) on a fast chip. On an older Intel Mac, with a larger
`--max-size` or while recording (which renders and encodes through ffmpeg too), the decode share grows. The
measurement script is in the repository so the numbers can be repeated and compared.

## Proposal

Decode with VideoToolbox (`VTDecompressionSession`), which runs on the media engine, and keep openh264 as the
fallback.

1. **Input:** the server sends Annex B. VideoToolbox wants AVCC (length-prefixed) NAL units and a format
   description built from the SPS and PPS (`CMVideoFormatDescriptionCreateFromH264ParameterSets`). Convert each
   packet and rebuild the session when the SPS changes (rotation sends a new one).
2. **Output:** ask for NV12 (`kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange`), the hardware's native layout.
   The renderer currently takes three I420 planes, so it needs a two-plane path (Y, and interleaved CbCr) in
   `render.rs` and `shader.wgsl`; the BT.709 limited-range maths stays.
3. **Selection:** use VideoToolbox when the session is created, fall back to openh264 if it fails (older
   streams, odd profiles, a failed session), and say which one is in use under `HANDSET_DEBUG=1`.
4. **Bindings:** `objc2-video-toolbox`, `objc2-core-media` and `objc2-core-video`, behind
   `cfg(target_os = "macos")`, like the Dock code.

## Decision rule

Do not switch unless it clearly helps. Build step 2 behind a flag, measure the same scenario with
`scripts/measure-cpu.sh`, and accept it only if average CPU drops by at least half and latency (touch to pixels)
does not get worse. If it does not, record the numbers here and mark this ADR rejected.

## Consequences

- Lower CPU and heat on battery, and headroom for higher resolutions.
- A second decode path to test and keep correct: AVCC conversion, session rebuilds on resolution change, and
  the fallback. Both paths need the same recovery behaviour (ADR 0006 and the decoder reset code in `stream.rs`).
- A larger macOS-only footprint in the code and in dependencies.
- Hardware decoders are stricter than openh264 about broken streams, so the fallback matters.

## Alternatives considered

- **Stay on openh264:** simplest, already works, costs the CPU above.
- **Link ffmpeg's libavcodec:** hardware decode through the same VideoToolbox, plus every codec, but it
  brings a large C dependency and its licence terms; the ffmpeg already used for recording is a separate process.
- **Ask the phone for a cheaper stream** (lower `--max-size` or bitrate): works today and is the first thing to
  try, but trades picture quality for CPU.
