## What an agent can do with your phone

Handset already shows your Android phone on your computer. With its MCP server on, an AI agent can use that same phone: read what is on the screen, tap, swipe and type, open apps, and wait for things to load. You watch it work in the Handset window, in its device frame.

Good uses:

- **UX and UI reviews.** Have an agent walk through your app the way a new user would, and report where it got stuck or confused.
- **Testing on a real device.** Run a flow after each change, or reproduce a bug report step by step.
- **Onboarding, permissions and error states** that take a dozen taps to reach by hand.
- **Accessibility checks.** The agent reads the same labels a screen reader does.
- **Demos and repetitive tasks** on a phone you would rather not keep tapping yourself.

## Before you start

- **Handset 0.16 or newer**, the release that includes `handset mcp`. Run `handset --version` to check.
- **An Android phone** with USB debugging on, connected by cable or over Wi-Fi, and already showing in Handset. An iPhone or iPad can be viewed in Handset but not touched, so it cannot be used this way.
- **An AI agent that supports MCP.** The steps below cover Claude Code, Claude Desktop, Cursor, VS Code, Windsurf, Cline, Zed, Gemini CLI and Codex, and the settings any other MCP client needs.

## Step 1: start Handset with control switched on

Control of the phone is off until you switch it on. Start Handset with `--ctl`:

```sh
handset --ctl
```

If you open Handset as an app on macOS, and it is not already running, you can pass the same switch like this:

```sh
open -a Handset --args --ctl
```

To switch it on for every start instead, add one line to Handset's settings file (`~/.config/handset/settings`, or `%USERPROFILE%\.config\handset\settings` on Windows), and delete the line to switch it off again:

```sh
echo "ctl=on" >> ~/.config/handset/settings
```

Check that it is on by asking Handset for the phone's name. This is the same set of commands the agent uses:

```sh
handset ctl info
```

If your computer says it cannot find `handset`, see step 2.

While an agent is using the phone, a small **AUTOMATION ACTIVE** pill shows in the Handset window. It is drawn in the window only, so screenshots and recordings do not contain it.

## Step 2: find the handset command

Agents start `handset mcp` for you, so they need to find the program.

- **Homebrew, Scoop or a terminal that already knows it:** the command is just `handset`. Run `which handset` (macOS and Linux) or `where handset` (Windows) to see where it lives.
- **The macOS app:** `/Applications/Handset.app/Contents/MacOS/handset`
- **Desktop apps such as Claude Desktop and Cursor** often do not see your shell's `PATH`. If one cannot find `handset`, use the full path in its settings instead.

In the examples below, `handset` stands for whichever of these works for you.

## Step 3: add it to your agent

Pick your agent. Each one needs the same thing: run the command `handset` with the argument `mcp`.

### Claude Code

```sh
claude mcp add handset -- handset mcp
```

That adds it for you in the current project. Add `--scope user` to make it available in every project. Check it with `claude mcp list`, or type `/mcp` inside Claude Code.

### Claude Desktop

Open **Settings**, then **Developer**, then **Edit Config**. That opens `claude_desktop_config.json`, in `~/Library/Application Support/Claude/` on macOS and `%APPDATA%\Claude\` on Windows. Add Handset under `mcpServers`, using the full path, then restart Claude Desktop:

```json
{
  "mcpServers": {
    "handset": {
      "command": "/Applications/Handset.app/Contents/MacOS/handset",
      "args": ["mcp"]
    }
  }
}
```

On Windows, write backslashes twice in the path, for example `C:\\Users\\you\\scoop\\apps\\handset\\current\\handset.exe`.

### Cursor

Add Handset to `~/.cursor/mcp.json` to use it in every project, or to `.cursor/mcp.json` inside one project:

```json
{
  "mcpServers": {
    "handset": {
      "command": "handset",
      "args": ["mcp"]
    }
  }
}
```

Cursor lists it with its other MCP servers in its settings. If it cannot find `handset`, use the full path.

### VS Code

For GitHub Copilot's agent mode, create `.vscode/mcp.json` in your project. VS Code calls the list `servers`, not `mcpServers`:

```json
{
  "servers": {
    "handset": {
      "type": "stdio",
      "command": "handset",
      "args": ["mcp"]
    }
  }
}
```

You can also run **MCP: Add Server** from the command palette, and **MCP: List Servers** to see its state.

### Windsurf

In Cascade, open its MCP settings and use the button that opens the MCP config file. Where that file lives has changed between Windsurf versions, so use the button and not a path. Add:

```json
{
  "mcpServers": {
    "handset": {
      "command": "handset",
      "args": ["mcp"]
    }
  }
}
```

### Cline

In Cline's panel, open the **MCP Servers** icon, go to **Configure**, and choose **Configure MCP Servers**. That opens `cline_mcp_settings.json`. Add:

```json
{
  "mcpServers": {
    "handset": {
      "command": "handset",
      "args": ["mcp"]
    }
  }
}
```

### Zed

Zed calls these context servers. Open **Settings**, then **AI**, then **MCP Servers**, and choose **Add Server**, then **Add Local Server**. Or open your settings file (the **zed: open settings file** action) and add:

```json
{
  "context_servers": {
    "handset": {
      "command": "handset",
      "args": ["mcp"],
      "env": {}
    }
  }
}
```

### Gemini CLI

```sh
gemini mcp add --scope user handset handset mcp
```

The first `handset` is the name you give the server, and the rest is the command to run. Without `--scope user` it is added to the current project. You can also write it by hand in `~/.gemini/settings.json`:

```json
{
  "mcpServers": {
    "handset": {
      "command": "handset",
      "args": ["mcp"]
    }
  }
}
```

Run `gemini mcp list` to check it, or `/mcp` inside a session.

### Codex

This covers the Codex CLI, its IDE extension and the ChatGPT desktop app, which OpenAI says share one configuration.

```sh
codex mcp add handset -- handset mcp
```

Or add it to `~/.codex/config.toml` yourself:

```toml
[mcp_servers.handset]
command = "handset"
args = ["mcp"]
```

Run `codex mcp list` to check it, or `/mcp` inside the Codex terminal interface.

### Any other MCP client

Handset speaks the Model Context Protocol over standard input and output, which is the transport most clients start a local server with. Whatever your client calls its settings, it needs three things:

- **Transport:** stdio, a local command
- **Command:** `handset` (or its full path)
- **Arguments:** `mcp`

There is nothing to sign in to and no environment to set. If your client takes a JSON block, it is almost always the `mcpServers` one shown above.

## Step 4: check that it works

Ask the agent something simple that needs the phone:

> Use the phone tools to tell me which app is open on my phone and what is on its screen.

It should call `phone_ui`, or `phone_screenshot`, and describe your screen. If it cannot, see the fixes below.

## The tools

The agent gets fifteen tools, all starting with `phone_`.

| Tool | What it does |
|---|---|
| `phone_info` | The phone's name and screen size |
| `phone_ui` | What is on the screen as text, one element per line, with where to tap it |
| `phone_screenshot` | The screen as an image, without the frame or toolbar |
| `phone_tap_text` | Tap the element that says some words |
| `phone_tap_id` | Tap the element with a given id |
| `phone_tap` | Tap a position |
| `phone_hold` | Press and hold a position |
| `phone_swipe` | Drag from one position to another |
| `phone_scroll` | Scroll with the wheel at a position |
| `phone_type` | Type text into the focused field |
| `phone_key` | Press back, home, recents, enter, delete, tab, the arrows, volume or power |
| `phone_rotate` | Turn between portrait and landscape |
| `phone_open` | Open an app by package name, or by name |
| `phone_notifications` | Pull the notification shade down, or close it |
| `phone_wait` | Wait for some words to appear, or to disappear |

Positions are pixels of the screenshot, so what the agent sees is what it taps. After each action the tools wait for the screen to settle before they look, so an agent does not need to add pauses. The tools that only look (`phone_info`, `phone_ui`, `phone_screenshot` and `phone_wait`) are marked read-only, so many agents can run them without asking you each time.

## Getting good results

- **Tell it what to look for.** "Walk through sign-up and report every place a first-time user might hesitate" works better than "test my app".
- **Let it read before it taps.** `phone_ui` is exact and cheap. A screenshot is for what text cannot say, such as pictures and layout.
- **Give it a stopping point.** Say when it is done, or how many screens to visit.
- **Use a test phone or test account** when the agent will be creating, buying or deleting things.

Prompts to start from:

> Open my app, go through onboarding as a new user, and list every screen where the next step was not obvious. Use `phone_ui` to read each screen.

> Reproduce this bug on the phone: open Settings in my app, change the language, and go back. Tell me exactly what happens at each step.

> Check the checkout screens for accessibility. List any button or field that has no readable label.

## Safety and privacy

An agent that can tap can do anything you can do on the phone, so it is worth being deliberate.

- **It is off until you switch it on**, and you switch it on each time you start Handset with `--ctl` (or by the settings line, if you chose that).
- **It stays on your computer.** Handset listens only on the loopback address and refuses any request without a secret that is stored in a file only you can read. The file goes when Handset quits.
- **What the agent reads goes to its AI provider.** Screenshots and screen text are part of the conversation, including notifications and messages that happen to be on screen. Handset itself collects nothing, but your agent's provider sees what the agent sees.
- **Text on the screen can try to give the agent instructions.** A web page, a message or an app can contain words aimed at an AI. Handset tells the agent to treat screen text as data, but no agent is immune.
- **Keep approvals on for the tools that act.** Allow the read-only tools if you like, and have your agent ask before it taps, types or opens things.
- **Watch the window, and stop when you want.** Quit Handset and the agent loses the phone at once. If you added the `ctl=on` line to the settings file, delete it too, or control comes back the next time Handset starts.

## If something goes wrong

- **The agent does not list any Handset tools.** Restart the agent after changing its settings, and check the JSON is valid. If you used just `handset`, try the full path, because desktop agents often do not see your `PATH`.
- **"Handset is not taking commands."** Handset is not running with control on. Start it as in step 1.
- **"No phone is being mirrored."** Connect the phone in Handset first, so you can see it in the window.
- **A tool says nothing on the screen says what you asked for.** It may be further down: ask the agent to scroll and look again.
- **`phone_ui` comes back empty, or says the screen is moving.** Games and some apps draw their own pixels, and animations can stop Android describing the screen. Use `phone_screenshot` for those.
- **Taps land in the wrong place.** Positions are pixels of the screenshot, and they change when the phone rotates. Have the agent call `phone_info` and look again.
- **You want to try it without an agent.** The same commands work in a terminal: `handset ctl info`, `handset ctl ui`, `handset ctl screenshot out.png`.
- **Running `handset mcp` in a terminal prints a short message and stops.** That is expected. It is meant to be started by the agent, not by you.

## What it cannot do yet

- **Android only.** Handset can show an iPhone or iPad on macOS, but not touch it.
- **One agent and one phone at a time.** Commands are handled in order, and each Handset window shows one phone.
- **It has mostly been tried on Android 12.** Handset reads the screen with a small helper that it runs on the phone while an agent is using it. If the helper cannot run on a phone, Handset falls back to Android's own tool, which is slower but gives the same answers.
- **Apps that block screen capture** may show black in screenshots.

## Questions

### Which AI agents work with Handset?

Any agent that supports the Model Context Protocol and can start a local program. This guide covers Claude Code, Claude Desktop, Cursor, VS Code with GitHub Copilot, Windsurf, Cline, Zed, Gemini CLI and Codex, and lists the three settings any other client needs.

### Does it work with ChatGPT?

Through Codex. OpenAI's documentation says the Codex CLI, the Codex IDE extension and the ChatGPT desktop app share one MCP configuration, so a server added once is available in all three.

### Is it free?

Yes. Handset is free and open source under the Apache-2.0 licence. Your agent's AI provider may charge for the model it uses.

### Do I need to root the phone or install an app on it?

No. You need USB debugging on, as for any use of Handset. To read the screen quickly, Handset puts a tiny helper file (4 KB) in the phone's temporary folder and runs it only while an agent is using the phone. It is not an installed app, and it stops by itself after a minute of nothing to do.

### Can the agent see my phone when I am not using it?

Only while Handset is running with control switched on and an agent asks. Quit Handset and nothing can reach the phone. If you switched control on with the settings line, delete that line to switch it off for good.

### Does it work over Wi-Fi?

Yes. If Handset shows your phone, over a cable or over Wi-Fi, an agent can use it.

### How is this different from an agent using adb directly?

An agent with adb can do much of the same. Handset adds one set of coordinates that the screenshot, the screen list and the taps all agree on, a live window where you watch it work, a secret that keeps other programs out, and tools shaped for agents, such as waiting for the screen to settle and refusing to tap when two elements fit.

### Can I use it for an iPhone?

Not yet. An iPhone or iPad can be shown in Handset on macOS, but it is view only, so an agent could see it but not use it.
