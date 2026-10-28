//! The right-click menu. Its content is built once as plain data (`build`); macOS and Windows show it as a
//! native menu whose selections come back as `UserEvent::Menu(id)` through the event-loop proxy, and Linux
//! draws it in the window (`popup.rs`).

use crate::stream::UserEvent;
use winit::event_loop::EventLoopProxy;
use winit::window::Window;

/// What the menu needs to show its current state.
pub struct State {
    /// An iOS device is mirrored: view only, so the phone's own controls are left out.
    pub ios: bool,
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
    /// How the phone is turned in the window.
    pub tilt: crate::tilt::Tilt,
    /// Look for newer releases by itself, and the newer version once one was found.
    pub update_auto: bool,
    pub update: Option<String>,
    /// The command that updates a package-manager install, when this is one.
    pub update_command: Option<&'static str>,
    /// This copy can install the update itself.
    pub update_install: bool,
    pub clipboard: bool,
    /// Each new capture also gets a styled copy.
    pub auto_style: bool,
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
#[cfg(not(target_os = "macos"))]
const MOD: &str = "Ctrl+";
#[cfg(target_os = "macos")]
const FILE_MANAGER: &str = "Finder";
#[cfg(windows)]
const FILE_MANAGER: &str = "Explorer";
#[cfg(not(any(target_os = "macos", windows)))]
const FILE_MANAGER: &str = "Folder";

/// One line of the menu, as plain data: the native menu and the drawn one are both built from it.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Item(Item),
    Submenu {
        label: String,
        items: Vec<Entry>,
    },
    Separator,
    /// The About window on macOS and Windows; the drawn menu sends `about` instead.
    About,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// What a selection sends to `App::on_menu`; `None` for a line that only informs.
    pub id: Option<String>,
    pub label: String,
    /// The key that goes with the platform's modifier, shown after the label.
    pub shortcut: Option<&'static str>,
    pub enabled: bool,
    /// `Some` for an item with a check mark, ticked or not.
    pub check: Option<bool>,
}

impl Item {
    /// The label with its shortcut, as the native menu shows it.
    #[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
    pub fn full_label(&self) -> String {
        match self.shortcut {
            Some(key) => format!("{}    {MOD}{key}", self.label),
            None => self.label.clone(),
        }
    }

    /// The shortcut as it is written, modifier included.
    pub fn shortcut_text(&self) -> Option<String> {
        self.shortcut.map(|key| format!("{MOD}{key}"))
    }
}

fn item(id: impl Into<String>, label: impl Into<String>) -> Item {
    Item { id: Some(id.into()), label: label.into(), shortcut: None, enabled: true, check: None }
}

/// A line that cannot be chosen.
fn note(label: impl Into<String>) -> Item {
    Item { id: None, label: label.into(), shortcut: None, enabled: false, check: None }
}

impl Item {
    fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    fn checked(mut self, on: bool) -> Self {
        self.check = Some(on);
        self
    }
    fn key(mut self, key: &'static str) -> Self {
        self.shortcut = Some(key);
        self
    }
}

impl From<Item> for Entry {
    fn from(i: Item) -> Self {
        Entry::Item(i)
    }
}

fn sub(label: impl Into<String>, items: Vec<Entry>) -> Entry {
    Entry::Submenu { label: label.into(), items }
}

/// The whole menu for this state.
pub fn build(s: &State) -> Vec<Entry> {
    let mut frame = vec![item("frame:none", "None").checked(s.frame.is_none() && !s.custom_frame).into()];
    // one submenu per device type (the part of the name before the colour), the original three on top
    let titled = |name: &str| -> String {
        name.chars().enumerate().map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c }).collect()
    };
    let choice = |name: &str, label: String| -> Entry {
        item(format!("frame:{name}"), label).checked(s.frame.as_deref() == Some(name)).into()
    };
    let mut groups: Vec<(String, Vec<Entry>)> = Vec::new();
    for name in &s.frames {
        match name.split_once('-') {
            None => frame.push(choice(name, titled(name))),
            Some((device, colour)) => {
                let idx = match groups.iter().position(|(d, _)| d == device) {
                    Some(i) => i,
                    None => {
                        groups.push((device.to_string(), Vec::new()));
                        groups.len() - 1
                    }
                };
                groups[idx].1.push(choice(name, titled(colour)));
            }
        }
    }
    if !groups.is_empty() {
        frame.push(Entry::Separator);
    }
    for (device, items) in groups {
        frame.push(sub(titled(&device), items));
    }
    if s.custom_frame {
        frame.push(note("Custom (from --frame)").checked(true).into());
    }

    let record = vec![
        item("fmt:mp4", "MP4").checked(!s.gif).into(),
        item("fmt:gif", "GIF (no audio)").checked(s.gif).into(),
        Entry::Separator,
        item("norm", "Normalize audio").enabled(s.audio).checked(s.normalize).into(),
        item("clip", "Copy captures to clipboard").checked(s.clipboard).into(),
        item("style", "Save a styled copy of each capture").checked(s.auto_style).into(),
    ];

    let mut recent = Vec::new();
    if s.recent.is_empty() {
        recent.push(note("No captures yet").into());
    }
    for path in &s.recent {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let p = path.to_string_lossy();
        let mut items = vec![item(format!("recent:open:{p}"), "Open").into()];
        if matches!(path.extension().and_then(|e| e.to_str()), Some("png" | "mp4" | "gif")) {
            items.push(item(format!("recent:edit:{p}"), "Edit Style...").into());
            items.push(item(format!("recent:style:{p}"), "Save Styled Copy").into());
        }
        items.push(item(format!("recent:reveal:{p}"), format!("Show in {FILE_MANAGER}")).into());
        items.push(item(format!("recent:copy:{p}"), "Copy").into());
        recent.push(sub(name, items));
    }

    let phone = vec![
        item("conn:wifi", "Switch to Wi-Fi").enabled(s.mirroring && s.usb).into(),
        item("conn:another", "Connect Another Phone\u{2026}").into(),
        item("conn:disconnect", "Disconnect").enabled(s.mirroring).into(),
    ];
    let mut open_app = Vec::new();
    if s.apps.is_empty() {
        open_app.push(note(if s.apps_loading { "Loading apps\u{2026}" } else { "No apps found" }).into());
    }
    let app = |a: &crate::apps::AppEntry| -> Entry { item(format!("app:{}", a.package), &a.name).into() };
    open_app.extend(s.apps.iter().filter(|a| !a.system).take(80).map(app));
    if s.apps.iter().any(|a| a.system) {
        open_app.push(Entry::Separator);
        open_app.push(sub("System Apps", s.apps.iter().filter(|a| a.system).map(app).collect()));
    }

    let tilt = crate::tilt::Tilt::PRESETS
        .iter()
        .map(|(name, t)| item(format!("tilt:{}", t.to_spec()), *name).checked(s.tilt == *t).into())
        .collect();

    let mut menu = Vec::new();
    if !s.ios {
        menu.push(sub("Phone", phone));
    }
    menu.push(sub("Frame", frame));
    menu.push(sub("Tilt", tilt));
    if !s.ios {
        menu.push(sub("Open App", open_app));
    }
    menu.push(Entry::Separator);
    menu.push(item("shot", "Take Screenshot").key("S").into());
    menu.push(item("rec", format!("{} Recording", if s.recording { "Stop" } else { "Start" })).key("E").into());
    menu.push(sub("Recent Captures", recent));
    menu.push(sub("Recording", record));
    menu.push(Entry::Separator);
    if s.audio {
        menu.push(item("mute", "Mute playback").checked(s.muted).into());
    } else if s.ios {
        menu.push(item("mute", "Audio off (start with --audio)").enabled(false).into());
    } else {
        menu.push(item("audio:on", "Turn On Audio").into());
    }
    if !s.ios {
        menu.push(item("rotate", "Rotate Device").key("R").into());
        menu.push(item("sync", "Share Clipboard With Phone").checked(s.sync).into());
    }
    menu.push(item("toolbar", "Show Toolbar").checked(s.toolbar).into());
    if !s.ios {
        menu.push(item("touches", "Show Touches").checked(s.touches).into());
    }
    menu.push(item("zoom:in", "Larger").key("+").into());
    menu.push(item("zoom:out", "Smaller").key("-").into());
    menu.push(item("zoom:reset", "Default Size").key("0").into());
    menu.push(Entry::Separator);
    if let Some(v) = &s.update {
        if s.update_install {
            menu.push(item("update-install", format!("Install Handset {v} and Restart")).into());
        }
        let open = match s.update_command {
            Some(cmd) => format!("Copy \"{cmd}\" to Update to {v}"),
            None => format!("Download Handset {v}..."),
        };
        menu.push(item("update-open", open).into());
        menu.push(item("update-notes", format!("What's New in {v}...")).into());
        menu.push(item("update-skip", format!("Skip {v}")).into());
        menu.push(item("update-later", "Remind Me in 3 Days").into());
        menu.push(Entry::Separator);
    }
    menu.push(item("update", "Check for Updates...").into());
    menu.push(item("update-auto", "Check for Updates on Start").checked(s.update_auto).into());
    menu.push(Entry::Separator);
    menu.push(Entry::About);
    menu.push(item("quit", "Quit").into());
    menu
}

/// Draw the menu ourselves: always on Linux, where muda needs a GTK window winit does not have, and on
/// macOS and Windows when `HANDSET_DRAWN_MENU=1` asks for it.
pub fn drawn() -> bool {
    cfg!(not(any(target_os = "macos", windows))) || std::env::var_os("HANDSET_DRAWN_MENU").is_some_and(|v| v == "1")
}

#[cfg(any(target_os = "macos", windows))]
pub fn install(proxy: EventLoopProxy<UserEvent>) {
    muda::MenuEvent::set_event_handler(Some(move |e: muda::MenuEvent| {
        let _ = proxy.send_event(UserEvent::Menu(e.id.0));
    }));
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn install(_proxy: EventLoopProxy<UserEvent>) {}

/// Show the native menu at the pointer.
#[cfg(any(target_os = "macos", windows))]
pub fn show(window: &Window, s: &State) {
    use muda::ContextMenu;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = window.window_handle() else { return };
    #[cfg(target_os = "macos")]
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return };
    #[cfg(windows)]
    let RawWindowHandle::Win32(h) = handle.as_raw() else { return };

    let menu = muda::Menu::new();
    for e in &build(s) {
        let _ = menu.append(native(e).as_ref());
    }
    unsafe {
        #[cfg(target_os = "macos")]
        menu.show_context_menu_for_nsview(h.ns_view.as_ptr() as *const std::ffi::c_void, None);
        #[cfg(windows)]
        menu.show_context_menu_for_hwnd(h.hwnd.get(), None);
    }
}

/// The native item for one entry, with everything under it.
#[cfg(any(target_os = "macos", windows))]
fn native(e: &Entry) -> Box<dyn muda::IsMenuItem> {
    use muda::{CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu};
    match e {
        Entry::Separator => Box::new(PredefinedMenuItem::separator()),
        Entry::About => Box::new(PredefinedMenuItem::about(Some("About Handset"), Some(about()))),
        Entry::Submenu { label, items } => {
            let s = Submenu::new(label, true);
            for e in items {
                let _ = s.append(native(e).as_ref());
            }
            Box::new(s)
        }
        Entry::Item(i) => {
            let label = i.full_label();
            match (&i.id, i.check) {
                (Some(id), Some(c)) => Box::new(CheckMenuItem::with_id(id.as_str(), label, i.enabled, c, None)),
                (None, Some(c)) => Box::new(CheckMenuItem::new(label, i.enabled, c, None)),
                (Some(id), None) => Box::new(MenuItem::with_id(id.as_str(), label, i.enabled, None)),
                (None, None) => Box::new(MenuItem::new(label, i.enabled, None)),
            }
        }
    }
}

/// What the About window says: who made Handset, what it stands on, and under which licence.
#[cfg(any(target_os = "macos", windows))]
fn about() -> muda::AboutMetadata {
    muda::AboutMetadata {
        name: Some("Handset".into()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        authors: Some(vec!["David Amunga (@davidamunga_)".into()]),
        comments: Some("Your phone, on your desktop.".into()),
        copyright: Some("\u{a9} 2026 David Amunga".into()),
        license: Some("Apache-2.0".into()),
        website: Some("https://github.com/davidamunga/handset".into()),
        website_label: Some("github.com/davidamunga/handset".into()),
        credits: Some(
            "Made by David Amunga (@davidamunga_).\n\nBuilt on the scrcpy server by Genymobile and Romain Vimont \
             (Apache-2.0). Set in Inter by the Inter Project Authors (SIL Open Font License). Not affiliated with \
             or endorsed by Genymobile."
                .into(),
        ),
        ..Default::default()
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn show(_window: &Window, _s: &State) {}
