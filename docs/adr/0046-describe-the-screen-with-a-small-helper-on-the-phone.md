# 0046. Describe the screen with a small helper on the phone

- Status: accepted
- Date: 2026-11-16

## Context

[ADR 0045](0045-read-the-screen-as-text-with-uiautomator.md) reads the screen with `uiautomator dump`. That
takes about three seconds on the phone, and measuring showed where it goes: the adb round trip is 0.03 s, and
the rest is Android starting `uiautomator` from nothing for every request. Caching and reading ahead hide the
wait when an agent pauses between calls, but an agent that asks straight after an action still waits.

## Decision

We will ship a small helper, written in Java, that Handset pushes to the phone and keeps running while
`handset ctl ui` is in use. It answers a request in a few hundredths of a second. The source is in `helper/`,
and the built jar is embedded as `app/assets/handset-helper.jar`, the way the scrcpy server is.

- **It uses what is already on the phone.** It makes a `UiAutomation` connection through the same internal
  classes `uiautomator` uses, reached by reflection, and writes the screen in `uiautomator`'s own XML, so the
  reader in `ui.rs` is the same for both. It is 4 KB. It needs no app, no permission and no install step; it
  runs as the shell user under `app_process`, like the scrcpy server.
- **A connection per request.** Each request connects, reads and disconnects, so the connection is held for
  tens of milliseconds, not the whole session. It asks Android not to switch off the person's own
  accessibility services (`FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES`). Checked on a phone with a third
  party accessibility service enabled: it stayed bound through 80 requests.
- **Speaks lines on standard input and output.** `dump` answers `ok <n>` and the XML, `ping` checks it is
  alive, `quit` or the end of the input stops it. The input ends when Handset or the adb connection goes, so
  nothing is left running if Handset is killed.
- **Ends itself after a minute without a request.** It holds about a hundred megabytes while it runs. Handset
  replaces a helper that has been idle for 45 seconds instead of asking it, so the two never race.
- **It is only ever an optimisation.** If the helper cannot be pushed or started, answers that it cannot, or
  stops, Handset uses `uiautomator` for that request. Two failures in a row turn it off for the run.
  `HANDSET_NO_HELPER=1` turns it off by hand.
- **A shared, named copy on the phone.** The jar goes to `/data/local/tmp/handset-helper-<checksum>.jar` and
  is pushed only if that file is not already there at the right size, so Handsets share it and a running copy
  is never pushed over (that makes it abort).
- **Built from source, reproducibly.** `helper/build.sh` compiles with `javac` against the SDK's `android.jar`
  and converts with `d8`, with a fixed file date, so the same source gives the same jar. A test checks that the
  recorded checksum is the embedded file's. See `docs/runbooks/rebuild-helper.md`.

## Alternatives considered

- **Keep `uiautomator` and the cache.** Needs nothing, but an agent that asks straight after each action
  waits three seconds every time.
- **An accessibility service app on the phone.** Could stream changes, but the person has to install it and
  turn it on in Settings.
- **Hold one `UiAutomation` connection for the whole session.** A little faster, but it keeps the phone's
  accessibility machinery under our control the whole time.
- **Extend the scrcpy server.** Breaks "reuse the unmodified server" (ADR 0003). ADR 0003 turned down writing
  our own agent for mirroring, which is large; this one is 200 lines and does something scrcpy does not.

## Consequences

- `ui` takes about 0.04 to 0.1 s instead of 3 s on the test phone (Nokia G21, Android 12), and 0.3 s on an
  animated game page that `uiautomator` took 3.8 s on. The first call after a minute idle takes about 1.3 s
  to start the helper.
- The helper uses Android internals, which change between versions. It was only tried on Android 12. Its
  failure costs speed, not function, and a new Android version can be checked with the build script and a
  phone. Android 14 and later should be tried before this is relied on there.
- A new binary asset and a build step that needs a JDK and the Android SDK, only when the helper changes.
- Element bounds are clipped to the whole screen, where `uiautomator` clips to the area above the navigation
  bar, so a few centres differ by some pixels. Both are inside the element.
- Not yet: waiting for a label to appear (the helper could watch accessibility events), and reading only part
  of the screen.
