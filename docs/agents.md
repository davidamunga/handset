# Driving the phone from a program

Handset can take commands from a script or an AI agent, so it can see and use the phone without anyone
working the window. It is off until you ask for it.

```sh
handset --ctl                         # or put ctl=on in ~/.config/handset/settings
handset ctl info
handset ctl screenshot screen.png     # the phone's picture, without frame or toolbar
handset ctl ui                        # what is on screen, as text, with where to tap
handset ctl tap-text "Sign in"        # tap by what it says
handset ctl wait-text "Welcome" 15s   # wait for something slow to appear
handset ctl tap 540 1200
handset ctl swipe 540 1600 540 600 300
handset ctl type "hello"
handset ctl key back
handset ctl open com.android.settings
```

Run `handset ctl help` for every command.

## Coordinates

Positions are pixels of the picture `screenshot` returns, with 0, 0 at the top left. `info` gives the size.
The phone turns when it rotates, so read `info` again after `rotate`. A position outside the screen is an
error that names the screen size.

## Reading the screen as text

`handset ctl ui` lists what is on the screen, one element per line, in the same pixels as `screenshot`:

```
screen 720x1600 package com.android.settings
3 TextView "Search settings" @250,298 id=search_action_bar_title
5 TextView "Network & internet" @271,423 clickable id=title
```

`tap-text <words>` taps the element that says them (an exact match first, then one that contains them), and
`tap-id <id>` the one with that resource id. If several fit, nothing is tapped and the error lists them with
positions. It works on Android only.

It uses a small helper that Handset puts on the phone (4 KB, nothing to install, ended by itself after a minute
idle), so `ui` takes a few hundredths of a second, and about 1.3 s the first time after a pause while the helper
starts. If the helper cannot run on a phone (a new Android version, say) Handset falls back to Android's own
`uiautomator`, which takes about three seconds, and keeps the last answer while the picture has not changed.
`HANDSET_NO_HELPER=1` forces the fallback. A screen that blocks description, or an app that draws its own pixels
(many games), may come back empty or as an error; use `screenshot` then. Web pages describe their buttons. See
[ADR 0045](adr/0045-read-the-screen-as-text-with-uiautomator.md) and
[ADR 0046](adr/0046-describe-the-screen-with-a-small-helper-on-the-phone.md).

## Looking right after an action

An action returns when it has been sent, and the phone takes a moment to show it (an app takes about a second to
open). So `ui`, `tap-text` and `tap-id` wait for the picture to change and then hold still before they look, and
the next touch, key or text waits the same way, so a tap is not sent into a page that is turning. You can
send commands one after another without sleeps; each costs about 0.7 s when the one before it changed the
screen. An animation that never stops (a game) is cut off after 2.5 s.

Some things take longer, such as search results or a network call. Wait for them on purpose:

```sh
handset ctl wait-text "Wi-Fi certificate" 15s     # until it is on the screen (default 10 s, at most 30)
handset ctl wait-gone "Loading" 20s               # until it is gone
```

If the text is not found and something on the screen scrolls, the error says it may be out of view: scroll and
look again.

After each action, once the screen settles, Handset also describes it in the background, so the next `ui` or
`tap-text` is ready when you ask. That read-ahead only runs while you are using `ui`. A `ui` straight after an
action still waits for a fresh answer, never a stale one.

## What an agent should know

- Prefer `ui` and `tap-text` where they work: they are exact and cheap. Fall back to `screenshot` and `tap`.
- Look, then act: `screenshot` (or `ui`), decide, tap, and look again. A tap returns when it has been sent,
  not when the app has finished drawing; give a screen a moment before reading it.
- Text goes where the phone's keyboard focus is. Tap a field first. At most 300 characters at a time.
- `open` and `type` can do nothing without saying so (a name that fits two apps; no text field in focus).
  Look at the `package` on the first line of `ui`, and at the text of the field, to see what happened.
- `open` takes a package name. `?<app name>` also works, but only when it matches exactly one app; if it matches
  several or none, nothing happens (the phone's server only logs why). A package name never fails that way.
- `info` can show the old size for a moment after `rotate`; wait a second or two.
- An iPhone or iPad can be looked at (`info`, `screenshot`), not touched.
- While a program is using the socket the window shows "AUTOMATION ACTIVE".

## Safety

Handset listens only on the loopback address and refuses a request that does not carry the secret in
`~/.config/handset/ctl`, a file only you can read, removed when Handset stops. Anything that can read your
files can drive the phone while this is on, so leave it off when you do not need it. The reasons are in
[ADR 0044](adr/0044-let-a-program-drive-the-phone-through-a-local-socket.md).

## The protocol

One line in, one answer out, over a TCP connection to `127.0.0.1:<port>` (the port and token are in the
file above): `<token> <command> [arguments]`. The answer is `ok <n>` and `n` bytes (text, or a PNG for
`screenshot`), or `err <message>`.
