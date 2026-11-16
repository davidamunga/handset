# 0044. Let a program drive the phone through a local socket

- Status: accepted
- Date: 2026-11-16

## Context

Agents now test apps on a real phone, and Handset is how they reach it. Today an agent has to work the
window: screenshot it, find the phone inside the frame, click at window pixels. That is slow, breaks when
the window moves or is covered, spends vision tokens on bezel and toolbar, and takes over the person's
mouse. Handset already has what an agent needs: the decoded picture, the control channel to the phone and
recording. None of it can be reached without the window.

## Decision

We will add an opt-in control socket and a `handset ctl` command.

- **Opt in.** Nothing listens unless Handset starts with `--ctl` or `ctl=on` is in the settings. Letting a
  program touch the phone is a bigger step than watching it, so it is never on by default.
- **Loopback TCP, one request per connection, a line of text.** It works the same on macOS, Windows and
  Linux, needs no new dependency (the project has no JSON library) and can be tried with `nc`. The answer is
  `ok <n>` and `n` bytes, or `err <message>`.
- **A secret in a private file.** On start Handset picks a random token and writes the port and token to
  `~/.config/handset/ctl` (mode 0600). A request without the token is refused, so a web page that finds the
  port cannot drive the phone, and another user on the machine cannot read the file. The file is removed
  when Handset stops.
- **Coordinates are screenshot pixels.** `screenshot` returns the raw picture the phone sends (no bezel,
  toolbar or tilt), and every position is checked against that size, so what an agent sees is what it taps.
  Out of range is an error that says the screen size, not a tap somewhere else.
- **Gestures are whole.** `tap`, `hold` and `swipe` play down, moves and up on the server and always lift
  the finger, even if the phone stops listening. They use their own pointer id, so they are not confused with
  the mouse or a Share viewer. Requests run one at a time.
- **The window says so.** While a program is using the socket a pill reads "AUTOMATION ACTIVE"
  with a violet dot. It is drawn in the window only, so captures and recordings do not contain it.
- **An iPhone or iPad can be looked at, not touched.** `info` and `screenshot` work; the rest says why not.

## Alternatives considered

- **A Unix socket.** Better permissions, but Windows needs a different transport and the project would carry
  two code paths. The token gives the same protection.
- **HTTP.** Easier for some clients, but a browser can send HTTP to localhost, which is the attack the token
  is there to stop; a plain line protocol is not something a page can speak.
- **Only an MCP server.** Agents want it, but it should be a thin layer on something scriptable. The socket
  comes first; `handset mcp` can wrap `handset ctl`.
- **Driving adb directly.** What agents do without Handset. It cannot see the same picture the person sees
  and bypasses the rules Handset keeps about touches.

## Consequences

- Agents get a stable, cheap way to see and drive a phone, and so do shell scripts and demos.
- Anything on this computer that can read your files can drive the phone while `--ctl` is on. That is the
  same trust as `adb`, and the pill shows when it happens.
- With several Handset windows the last one started owns the file. A choice of phone is a later addition.
- Not yet: recording from the socket, reading the screen as text (the accessibility tree, which would let an
  agent tap by label), and an MCP server.
