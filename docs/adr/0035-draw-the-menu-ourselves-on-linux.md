# 0035. Draw the right-click menu ourselves on Linux

- Status: accepted
- Date: 2026-10-28

## Context

Everything that is not on the toolbar (frames, recording settings, recent captures, apps, connection, updates with
Skip and Remind) lives in the right-click menu. On macOS and Windows that is a native `muda` menu. ADR 0030 left
Linux without one: `muda` builds its Linux menus with GTK and needs a GTK window to show a context menu, and winit
gives us an X11 or Wayland window, not a GTK one. So on Linux right-click and the toolbar's **…** button did
nothing, and those features could not be reached.

## Decision

- Build the menu's content once as plain data (`menu::build`, from `menu::State`): items with ids, labels,
  shortcuts, check marks and enabled state, separators, submenus and an About entry.
- macOS and Windows keep the native `muda` menu, now built from that data. It shows the same items with the same
  ids as before, so `App::on_menu` is unchanged.
- Linux draws the menu itself in the window (`popup.rs`), the way the toolbar and toasts are drawn: each open panel
  is rasterized on the CPU (Inter, the connect screen's font, since the 5x7 bitmap font has no lower case) and drawn
  as a window-only layer, so captures never contain it. It opens at the pointer on right-click and beside the
  **…** button, highlights the row under the pointer, opens submenus beside their row (with a "heading for the
  submenu" triangle so a diagonal move does not switch submenus), flips or shifts at the window's edges, caps a
  long panel to the window and scrolls it (wheel, scroll strips, arrow keys), and closes on Escape, a click
  outside, a chosen item, a resize or losing focus. While it is open it takes the pointer and the keyboard, so
  nothing reaches the phone.
- `HANDSET_DRAWN_MENU=1` uses the drawn menu on macOS and Windows too, so it can be tried without a Linux desktop.
- The drawn menu's About entry sends `about`, which shows the version in a toast that opens the website; there is
  no About window to open.

## Consequences

- Every menu feature works on Linux, with no new dependency or system library.
- The drawn menu cannot leave the window: on a small window a submenu covers part of its parent, and a long list
  scrolls instead of growing past the window as a native menu would.
- It does not follow the desktop's theme, accessibility tools do not see it, and there is no type-to-search. It
  has the toolbar's dark look on every desktop.
- The content is in one place, so the native and drawn menus cannot drift apart; a test checks that every id the
  menu can send is one `App::on_menu` knows.

## Alternatives considered

- **A GTK menu through `muda`:** needs a GTK window (or a hidden one and its own main loop) next to winit, and GTK
  as a build and run-time dependency, which ADR 0030 removed.
- **A separate popup window per panel:** could reach outside the main window, but Wayland gives no way to place a
  normal window at a position, and winit has no popup (`xdg_popup`) windows.
- **egui for the menu:** egui is already a dependency for the editor, but bringing an egui context into the main
  window's renderer for one menu is more than the menu needs.
