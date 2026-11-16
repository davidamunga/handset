# 0048. Offer the phone to agents as an MCP server

- Status: accepted
- Date: 2026-11-16

## Context

`handset ctl` ([ADR 0044](0044-let-a-program-drive-the-phone-through-a-local-socket.md)) gives a script a
way to see and drive the phone. An agent in Claude Code, Claude Desktop, Cursor or VS Code does not want to
shell out and parse text: it wants tools it can discover, with descriptions and typed arguments, that it
calls the way it calls any other. The Model Context Protocol is how those clients find and call tools.

## Decision

We will add `handset mcp`, an MCP server that speaks over standard input and output, and is a thin layer over
the control socket.

- **A subcommand, started by the agent.** MCP clients start a local server as a child process and talk to it
  on its pipes, so `handset mcp` is the command to put in their configuration. No port is opened and there is
  nothing new to authenticate: the child reads the same secret file as `handset ctl` and so is allowed to
  exactly what the person allowed by starting Handset with `--ctl`.
- **It does not start Handset or switch the socket on.** Turning on control of the phone is the person's
  choice, made when they start Handset, and not something an agent's configuration should make for them.
  With Handset not running, the server still starts and lists its tools, and a call says how to start it.
- **Fifteen tools, named `phone_*`**, each one a `handset ctl` command: `phone_info`, `phone_ui`,
  `phone_screenshot`, `phone_tap_text`, `phone_tap_id`, `phone_tap`, `phone_hold`, `phone_swipe`,
  `phone_scroll`, `phone_type`, `phone_key`, `phone_rotate`, `phone_open`, `phone_notifications` and
  `phone_wait`. The two notification commands are one tool, and the two waits are one tool, to keep the list
  short. Each tool builds the control request and the control socket's own checks apply. A test requires that
  every tool has a case that produces a request `ctl::parse` accepts, so the two cannot drift apart.
- **A screenshot is an image, the rest is text.** `phone_screenshot` returns an MCP image (base64 PNG); the
  others return the socket's text, or `ok`.
- **Failures an agent can act on are results, not protocol errors.** A wrong argument, a position off the
  screen, a phone that is not there, text that is not found: these come back as a result with `isError` set,
  so the model reads the message and tries again. Protocol errors are kept for a bad message, a method that
  does not exist and a tool that is not in the list.
- **Descriptions carry the lessons.** What each tool says, and the server's instructions, say to prefer
  `phone_ui`, that positions are screenshot pixels, that tools wait for the screen to settle, and that `open`
  and `type` can do nothing without saying so. The read-only tools say so (`readOnlyHint`), so a client can
  allow them without asking.
- **Text from the screen is data, not instructions.** The server's instructions say so. What an app shows can
  be written by anyone, and an agent that reads the screen can be told to do things by it.
- **No JSON library.** `json.rs` is a small strict reader and writer with a depth limit, since the project
  has none and this needs little. Messages longer than 4 MB are refused.
- **Protocol versions 2025-06-18, 2025-03-26 and 2024-11-05**, and a client that asks for another gets the
  newest. Batches are answered as batches. Notifications are not answered.

## Alternatives considered

- **A network transport (HTTP).** Needs a port and its own authentication, and the common clients start a
  local server over standard input and output anyway.
- **A separate program.** The server needs the socket client and the settings folder, which are in Handset;
  a subcommand keeps one thing to install and to keep in step.
- **A `serde_json` dependency.** Well tested, but one more thing to build, audit and ship for what is a small,
  fixed set of messages.
- **Starting Handset with `--ctl` from `handset mcp`.** Convenient, but it would let a line in an agent's
  configuration turn on control of the phone without the person choosing it.
- **One tool that takes a command line.** Fewer tools, but the model would have to learn the syntax from
  the description, and gets no typed arguments or checks.

## Consequences

- An agent in an MCP client can use the phone with one line of configuration, and a person watches it in the
  Handset window, with the "AUTOMATION ACTIVE" pill showing while it is used.
- Anything the agent can do through the socket, a prompt injection through the screen can try to do. The
  tools cannot tell the two apart. Starting Handset with `--ctl` only for the time wanted, and watching the
  window, is the protection.
- The tools were tried with a test client written for this and a real one is still to be tried against
  each of the common clients; their configuration formats differ slightly.
- Not yet: progress or cancellation of a long `phone_wait`, a list of the phone's apps, and resources (such as
  the screenshot as a resource that updates).
