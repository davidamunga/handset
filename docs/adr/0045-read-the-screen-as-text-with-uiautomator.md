# 0045. Read the screen as text with uiautomator

- Status: accepted
- Date: 2026-11-16

## Context

The control socket ([ADR 0044](0044-let-a-program-drive-the-phone-through-a-local-socket.md)) gives an agent a
picture and a way to touch, so it has to guess where a button is from pixels. That is slow, costs vision
tokens and misses when the guess is a few pixels off. Android already knows what is on the screen and what
each element says.

## Decision

We will add `ui`, `tap-text` and `tap-id` to the control socket, and read the screen with the phone's own
`uiautomator dump` through adb.

- **No new thing on the phone.** `uiautomator` ships with Android and needs no install, no accessibility
  permission and no change to the scrcpy server. It is run in one `adb exec-out` call together with
  `wm size`, which gives the real screen size.
- **Positions are the screenshot's pixels.** The dump is in screen pixels; Handset's picture can be scaled
  down (`--max-size`) and turns with the phone. The positions are scaled by the picture's width over the
  screen's, so `ui`, `screenshot`, `tap` and `tap-text` all agree.
- **A short list, not the whole tree.** Only elements that say something or can be used are listed, one line
  each with its kind, label, centre, flags and id. A tappable row made of several text views is shown as its
  words, marked clickable.
- **Tap by what it says, and refuse to guess.** `tap-text` takes an exact match first, then one that
  contains the words. If several fit and one of them can be tapped it takes that one; otherwise it fails and
  lists them with positions. A wrong tap on someone's phone costs more than an error.
- **Android only.** An iPhone has no such description; the commands say so and point to `screenshot`.

## Alternatives considered

- **An accessibility service on the phone.** Faster and could stream changes, but it needs an app, a
  permission the user must grant and a way to install it. The dump is slower (about three seconds) and in
  exchange needs nothing.
- **Our own addition to the scrcpy server.** Breaks "reuse the unmodified server" (ADR 0003).
- **The whole tree as JSON.** Larger, and the project has no JSON library; agents read the short list well.

## Consequences

- An agent can tap by label, and read text that a screenshot would need vision for.
- A dump takes about three seconds on the phone (about one is `uiautomator` waiting for the screen to go
  quiet, the rest is starting it; the adb round trip is 0.03 s), so it is hidden, not made faster. The last
  description is kept while a small grey copy of the picture is unchanged, two askers share one dump, and after
  each action, once the picture settles, the next dump starts in the background (only while `ui` is in use).
  Anything done to the phone discards the kept description at once, because the picture takes a moment to show
  the change and would otherwise look unchanged. A dump that overlaps an action is not kept. The real fix
  would be our own small helper on the phone, which needs a build step for Android code and is not done.
- A dump fails while something is still animating (Android reports no idle
  state); Handset retries once, then says so and points to `screenshot`. Games that draw their own pixels
  describe nothing; web pages do describe their buttons.
- Screens that block it (some banking apps) come back empty.
- Not yet: reading only part of the screen, waiting for a label to appear, and an MCP server.
