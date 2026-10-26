# 0019. Support Windows

- Status: accepted
- Date: 2026-10-26

## Context

Handset was macOS-only, with platform code behind `cfg(target_os = "macos")` and stubs elsewhere. The
renderer, decoder, audio and protocol code are portable already, so Windows needed the pieces that were
stubbed: the menu, the clipboard, file actions, dialogs and tool lookup. Testing on a real Windows laptop
(Intel HD 620) also showed two platform problems: wgpu's Vulkan backend failed to create a device
("Not enough memory left"), and a transparent window rendered black or white.

## Decision

We will support Windows with native or dependency-free code where possible.

- **Backend.** wgpu uses Direct3D 12 unless `WGPU_BACKEND` is set. The swapchain is a DirectComposition
  visual (`DxgiFromVisual`), because the default HWND swapchain only supports opaque output, and the window is
  created without a redirection bitmap, which would otherwise show through as white.
- **Clipboard.** Text, PNG, DIB and file lists use the Win32 clipboard directly. PowerShell is too slow
  (seconds) to call on the UI thread, so it is kept only for rare dialogs (address prompt, error box).
- **Menu.** `muda` shows the same context menu through the window's HWND, with Ctrl in place of the Command key.
- **Processes.** Helper programs start with `CREATE_NO_WINDOW`, and the release build uses the GUI
  subsystem, attaching to the parent console so terminal output still works.
- **Tools.** `adb` and `ffmpeg` are looked up on `PATH`, the Android SDK, winget, scoop, Chocolatey and
  next to the exe.
- **Icon.** The window icon is set at runtime. The file icon is a compiled resource (`handset.res`) handed to
  the linker with `cargo rustc ... -- -C link-arg=...`, because build scripts are blocked on some machines.

## Alternatives considered

- **A build script (`winres`) for the icon.** Blocked by Application Control on the test machine.
- **Editing the exe after the build.** The modified exe was refused by the same policy.
- **Global `rustflags` for the icon.** It also applies to every dependency's build script and invalidates
  their cached builds.
- **Vulkan by default.** Faster on some GPUs, but failed on the Intel driver used for testing.
- **PowerShell for all clipboard work.** Froze the app for several seconds after each capture.

## Consequences

- The Dock icon states stay macOS-only.
- Windows releases are an unsigned zip (`scripts/package-windows.ps1`); SmartScreen will warn until it is signed.
- Plain `cargo build` works but leaves the `.exe` without its file icon.
- CI builds and tests on Windows too, but only with `cargo test`; behaviour needing a phone is checked by hand.
