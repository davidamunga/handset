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
    /// Show where the phone is touched, in the window and in captures.
    pub touches: bool,
    pub clipboard: bool,
    /// A phone is being mirrored.
    pub mirroring: bool,
    /// ...and it is connected by USB, so it can be switched to Wi-Fi.
    pub usb: bool,
    /// Clipboard text is shared with the phone.
    pub sync: bool,
    /// The phone's apps (empty until listed) and whether the list is still being fetched.
    pub apps: Vec<crate::apps::AppEntry>,
    pub apps_loading: bool,
    /// Newest captures, for the Recent Captures submenu.
    pub recent: Vec<std::path::PathBuf>,
}

#[cfg(target_os = "macos")]
const MOD: &str = "\u{2318}";
#[cfg(windows)]
const MOD: &str = "Ctrl+";
#[cfg(target_os = "macos")]
const FILE_MANAGER: &str = "Finder";
#[cfg(windows)]
const FILE_MANAGER: &str = "Explorer";

#[cfg(any(target_os = "macos", windows))]
pub fn install(proxy: EventLoopProxy<UserEvent>) {
    muda::MenuEvent::set_event_handler(Some(move |e: muda::MenuEvent| {
        let _ = proxy.send_event(UserEvent::Menu(e.id.0));
    }));
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn install(_proxy: EventLoopProxy<UserEvent>) {}

#[cfg(any(target_os = "macos", windows))]
pub fn show(window: &Window, s: &State) {
    use muda::{CheckMenuItem, ContextMenu, Menu, MenuItem, PredefinedMenuItem, Submenu};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = window.window_handle() else { return };
    #[cfg(target_os = "macos")]
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return };
    #[cfg(windows)]
    let RawWindowHandle::Win32(h) = handle.as_raw() else { return };

    let menu = Menu::new();

    let frame = Submenu::new("Frame", true);
    let _ =
        frame.append(&CheckMenuItem::with_id("frame:none", "None", true, s.frame.is_none() && !s.custom_frame, None));
    // one submenu per device type (the part of the name before the colour), the original three on top
    let titled = |name: &str| -> String {
        name.chars().enumerate().map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c }).collect()
    };
    let item = |name: &str, label: String| {
        CheckMenuItem::with_id(format!("frame:{name}"), label, true, s.frame.as_deref() == Some(name), None)
    };
    let mut groups: Vec<(String, Submenu)> = Vec::new();
    for name in &s.frames {
        match name.split_once('-') {
            None => {
                let _ = frame.append(&item(name, titled(name)));
            }
            Some((device, colour)) => {
                let idx = match groups.iter().position(|(d, _)| d == device) {
                    Some(i) => i,
                    None => {
                        groups.push((device.to_string(), Submenu::new(titled(device), true)));
                        groups.len() - 1
                    }
                };
                let _ = groups[idx].1.append(&item(name, titled(colour)));
            }
        }
    }
    if !groups.is_empty() {
        let _ = frame.append(&PredefinedMenuItem::separator());
    }
    for (_, sub) in &groups {
        let _ = frame.append(sub);
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
        let _ = item.append(&MenuItem::with_id(
            format!("recent:reveal:{p}"),
            format!("Show in {FILE_MANAGER}"),
            true,
            None,
        ));
        let _ = item.append(&MenuItem::with_id(format!("recent:copy:{p}"), "Copy", true, None));
        let _ = recent.append(&item);
    }

    let phone = Submenu::new("Phone", true);
    let _ = phone.append(&MenuItem::with_id("conn:wifi", "Switch to Wi-Fi", s.mirroring && s.usb, None));
    let _ = phone.append(&MenuItem::with_id("conn:another", "Connect Another Phone\u{2026}", true, None));
    let _ = phone.append(&MenuItem::with_id("conn:disconnect", "Disconnect", s.mirroring, None));
    let open_app = Submenu::new("Open App", true);
    if s.apps.is_empty() {
        let label = if s.apps_loading { "Loading apps\u{2026}" } else { "No apps found" };
        let _ = open_app.append(&MenuItem::new(label, false, None));
    }
    for app in s.apps.iter().filter(|a| !a.system).take(80) {
        let _ = open_app.append(&MenuItem::with_id(format!("app:{}", app.package), &app.name, true, None));
    }
    if s.apps.iter().any(|a| a.system) {
        let system = Submenu::new("System Apps", true);
        for app in s.apps.iter().filter(|a| a.system) {
            let _ = system.append(&MenuItem::with_id(format!("app:{}", app.package), &app.name, true, None));
        }
        let _ = open_app.append(&PredefinedMenuItem::separator());
        let _ = open_app.append(&system);
    }

    let audio_label = if s.audio { "Mute playback" } else { "Audio off (start with --audio)" };
    let _ = menu.append(&phone);
    let _ = menu.append(&frame);
    let _ = menu.append(&open_app);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id("shot", format!("Take Screenshot    {MOD}S"), true, None));
    let rec_label = format!("{} Recording    {MOD}E", if s.recording { "Stop" } else { "Start" });
    let _ = menu.append(&MenuItem::with_id("rec", &rec_label, true, None));
    let _ = menu.append(&recent);
    let _ = menu.append(&record);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&CheckMenuItem::with_id("mute", audio_label, s.audio, s.audio && s.muted, None));
    let _ = menu.append(&MenuItem::with_id("rotate", format!("Rotate Device    {MOD}R"), true, None));
    let _ = menu.append(&CheckMenuItem::with_id("sync", "Share Clipboard With Phone", true, s.sync, None));
    let _ = menu.append(&CheckMenuItem::with_id("toolbar", "Show Toolbar", true, s.toolbar, None));
    let _ = menu.append(&CheckMenuItem::with_id("touches", "Show Touches", true, s.touches, None));
    let _ = menu.append(&MenuItem::with_id("zoom:in", format!("Larger    {MOD}+"), true, None));
    let _ = menu.append(&MenuItem::with_id("zoom:out", format!("Smaller    {MOD}-"), true, None));
    let _ = menu.append(&MenuItem::with_id("zoom:reset", format!("Default Size    {MOD}0"), true, None));
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id("quit", "Quit", true, None));

    unsafe {
        #[cfg(target_os = "macos")]
        menu.show_context_menu_for_nsview(h.ns_view.as_ptr() as *const std::ffi::c_void, None);
        #[cfg(windows)]
        menu.show_context_menu_for_hwnd(h.hwnd.get(), None);
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn show(_window: &Window, _s: &State) {}
