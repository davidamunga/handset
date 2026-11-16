# 0040. Let people change the keyboard shortcuts

- Status: accepted
- Date: 2026-11-16

## Context

The shortcuts (screenshot, record, rotate, zoom) were matched by hand in `app.rs` and written by hand in
`menu.rs`, so adding one meant editing both and nobody could change them.

## Decision

- `shortcuts.rs` holds the actions (a menu item id and a label), their defaults and the bindings a person
  changed. An action is the menu item's id, so a key press runs `App::on_menu(id)`, exactly what choosing the
  item does. Menus ask `shortcuts::text_for(id)` for what to show, so the menu and the keys cannot disagree.
- Bindings are saved in the existing settings file as `key.<id>=cmd+shift+s` (`none` removes a default,
  `default` or no line keeps it).
- A binding needs `cmd` (Command or Ctrl) or `alt`, or is an F-key: a plain letter is typed into the phone.
  Binding a key another action has takes it from that action, and says so.
- `handset --shortcuts` is an egui window in its own process, like the editor. The mirroring window
  re-reads the file when it gets focus, so no channel between the processes is needed.

## Alternatives considered

- **Native accelerators on the muda menu:** they would tie the keys to the platform's menu, which is not used on
  Linux and cannot be changed while it is shown.
- **Rebuild the editor-style window in the mirroring process:** a second event loop in one process is fragile; a
  separate process matches how the editor runs.

## Consequences

- A new action needs one line in `ACTIONS` and an arm in `on_menu` (a test checks the second).
