# 0034. Type addresses and pairing codes in the window

- Status: accepted
- Date: 2026-10-28

## Context

"Type an address" and "Pair with a code" on the connect screen asked for their text with a system dialog:
AppleScript on macOS, a PowerShell InputBox on Windows, and `zenity` or `kdialog` on Linux (ADR 0030). Many
minimal Linux desktops, a stock Omarchy among them, have neither, so both buttons did nothing there. The
connect screen is already drawn by Handset itself, buttons and all (ADR 0014, 0017).

## Decision

We draw a text field on the connect screen instead, on every platform: a labelled box with a caret, a hint
saying where the phone shows the address or code, and Cancel and Connect (or Pair) buttons. While it is open
it takes the keyboard: typed text, Backspace, Enter, Escape, and Cmd-V or Ctrl-V to paste. The code field takes
six digits only. There is no selection or moving the caret: an address or a six digit code does not need them.
`zenity` and `kdialog` are now used only for error dialogs when there is no terminal.

## Alternatives considered

- **Keep the dialogs and add more fallbacks on Linux:** still nothing on desktops with none of them, and three
  different looks for one feature.
- **A text field from egui (used by the editor):** a second UI toolkit in the mirroring window, for one field.

## Consequences

- The prompts work wherever the window does, and look the same on macOS, Windows and Linux.
- No input method support: the field takes what winit gives as key text, which is enough for digits, dots and
  colons.
