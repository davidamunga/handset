# Driving the phone from a program

Handset can take commands from a script or an AI agent, so it can see and use the phone without anyone
working the window. It is off until you ask for it.

```sh
handset --ctl                         # or put ctl=on in ~/.config/handset/settings
handset ctl info
handset ctl screenshot screen.png     # the phone's picture, without frame or toolbar
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

## What an agent should know

- Look, then act: `screenshot`, decide, `tap`, and `screenshot` again. A tap returns when it has been sent,
  not when the app has finished drawing; give a screen a moment before reading it.
- Text goes where the phone's keyboard focus is. Tap a field first. At most 300 characters at a time.
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
