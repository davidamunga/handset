# 0029. Support Linux by building it in CI first, and use the desktop's own helpers

- Status: accepted
- Date: 2026-10-27

## Context

Most of Handset is cross-platform already (wgpu, winit, egui, cpal, openh264, and `adb` and `ffmpeg` as
subprocesses). The platform-specific parts (opening files, the clipboard, prompts) had no-op stubs for anything
that is not macOS or Windows, and the menu library (`muda`) was an unconditional dependency, pulling in GTK and
xdo although the menu is only built for macOS and Windows. Nobody has a Linux desktop to test on yet.

## Decision

- Add a Linux job to CI (fmt, clippy with `-D warnings`, tests, release build on Ubuntu 24.04), so the code cannot
  stop compiling there while the rest is worked out.
- Fill the stubs the way the macOS and Windows code does, by running the system's own programs, so there is no new
  dependency or system library: `xdg-open` to open things, `wl-copy`/`wl-paste`, `xclip` or `xsel` for the
  clipboard, `zenity` or `kdialog` for prompts. A missing helper means the feature quietly does nothing.
- Make `muda` a macOS and Windows dependency only. Linux then needs GTK (for `rfd`'s file dialogs) and ALSA
  (for `cpal`), and nothing else at build time.
- Call it experimental in the README until it has run on a real desktop. No Linux release file, update path or
  right-click menu yet.

## Consequences

- Linux contributors can build and get working copy and open features, with the tools named in the README.
- The transparent, borderless frame, the GPU path and USB permissions are unverified on real systems.
- Each helper differs by desktop; a missing one is silent, so a future `--doctor` check could name it.

## Alternatives considered

- **The `arboard` crate for the clipboard:** no helper to install, but a heavier dependency, an X11 selection owner
  to keep alive, and no file clipboard. Revisit if the helpers prove unreliable.
- **A GTK menu through `muda`:** a Linux menu is wanted, but it needs a GTK window handle that winit does not give
  us; left for later.
