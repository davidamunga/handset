//! Native right-click menu (macOS). Selections come back as
//! `UserEvent::Menu(id)` through the event-loop proxy.

use crate::stream::UserEvent;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

/// What the menu needs to show its current state.
pub struct State {
    pub frames: Vec<&'static str>,
    /// Current frame: a built-in name, or `None` for no frame.
    pub frame: Option<String>,
    pub custom_frame: bool,
    pub recording: bool,
    pub gif: bool,
    pub normalize: bool,
    pub audio: bool,
    pub muted: bool,
    pub toolbar: bool,
    pub clipboard: bool,
    /// A phone is being mirrored.
    pub mirroring: bool,
    /// ...and it is connected by USB, so it can be switched to Wi-Fi.
    pub usb: bool,
    /// Newest captures, for the Recent Captures submenu.
    pub recent: Vec<std::path::PathBuf>,
}

#[cfg(target_os = "macos")]
pub fn install(proxy: EventLoopProxy<UserEvent>) {
    muda::MenuEvent::set_event_handler(Some(move |e: muda::MenuEvent| {
        let _ = proxy.send_event(UserEvent::Menu(e.id.0));
    }));
}

#[cfg(not(target_os = "macos"))]
pub fn install(_proxy: EventLoopProxy<UserEvent>) {}

#[cfg(target_os = "macos")]
pub fn show(window: &Window, s: &State) {
    use muda::{CheckMenuItem, ContextMenu, Menu, MenuItem, PredefinedMenuItem, Submenu};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return };

    let menu = Menu::new();

    let frame = Submenu::new("Frame", true);
    let _ =
        frame.append(&CheckMenuItem::with_id("frame:none", "None", true, s.frame.is_none() && !s.custom_frame, None));
    for name in &s.frames {
        let title: String =
            name.chars().enumerate().map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c }).collect();
        let on = s.frame.as_deref() == Some(*name);
        let _ = frame.append(&CheckMenuItem::with_id(format!("frame:{name}"), title, true, on, None));
    }
    if s.custom_frame {
        let _ = frame.append(&CheckMenuItem::new("Custom (from --frame)", false, true, None));
    }

    let record = Submenu::new("Recording", true);
    let _ = record.append(&CheckMenuItem::with_id("fmt:mp4", "MP4", true, !s.gif, None));
    let _ = record.append(&CheckMenuItem::with_id("fmt:gif", "GIF (no audio)", true, s.gif, None));
    let _ = record.append(&PredefinedMenuItem::separator());
    let _ = record.append(&CheckMenuItem::with_id("norm", "Normalize audio", s.audio, s.normalize, None));
    let _ = record.append(&CheckMenuItem::with_id("clip", "Copy captures to clipboard", true, s.clipboard, None));

    let recent = Submenu::new("Recent Captures", true);
    if s.recent.is_empty() {
        let _ = recent.append(&MenuItem::new("No captures yet", false, None));
    }
    for path in &s.recent {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let item = Submenu::new(name, true);
        let p = path.to_string_lossy();
        let _ = item.append(&MenuItem::with_id(format!("recent:open:{p}"), "Open", true, None));
        let _ = item.append(&MenuItem::with_id(format!("recent:reveal:{p}"), "Show in Finder", true, None));
        let _ = item.append(&MenuItem::with_id(format!("recent:copy:{p}"), "Copy", true, None));
        let _ = recent.append(&item);
    }

    let phone = Submenu::new("Phone", true);
    let _ = phone.append(&MenuItem::with_id("conn:wifi", "Switch to Wi-Fi", s.mirroring && s.usb, None));
    let _ = phone.append(&MenuItem::with_id("conn:another", "Connect Another Phone\u{2026}", true, None));
    let _ = phone.append(&MenuItem::with_id("conn:disconnect", "Disconnect", s.mirroring, None));

    let audio_label = if s.audio { "Mute playback" } else { "Audio off (start with --audio)" };
    let _ = menu.append(&phone);
    let _ = menu.append(&frame);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id("shot", "Take Screenshot    \u{2318}S", true, None));
    let rec_label = if s.recording { "Stop Recording    \u{2318}E" } else { "Start Recording    \u{2318}E" };
    let _ = menu.append(&MenuItem::with_id("rec", rec_label, true, None));
    let _ = menu.append(&recent);
    let _ = menu.append(&record);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&CheckMenuItem::with_id("mute", audio_label, s.audio, s.audio && s.muted, None));
    let _ = menu.append(&MenuItem::with_id("rotate", "Rotate Device    \u{2318}R", true, None));
    let _ = menu.append(&CheckMenuItem::with_id("toolbar", "Show Toolbar", true, s.toolbar, None));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id("quit", "Quit", true, None));

    unsafe {
        menu.show_context_menu_for_nsview(h.ns_view.as_ptr() as *const std::ffi::c_void, None);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn show(_window: &Window, _s: &State) {}
