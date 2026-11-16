//! Keyboard shortcuts for menu items, and the ones you changed.
//!
//! An action is a menu item's id (`shot`, `rec`, `tilt:next`, ...), so a shortcut runs exactly what choosing the
//! item does. Bindings live in the settings file as `key.<id>=cmd+shift+s`; `none` takes the default away and
//! `default` (or no line) keeps it. The window to change them is `handset --shortcuts` (`shortcuts_ui.rs`), its
//! own process like the editor, so the mirroring window re-reads the file when it gets focus.
//!
//! `cmd` is Command on a Mac and Ctrl elsewhere (both work on either, as the fixed shortcuts always did). To keep
//! typing into the phone working, a binding needs `cmd` or `alt`, unless it is an F-key.

use eframe::egui;
use std::collections::BTreeMap;
use std::sync::{OnceLock, RwLock};

/// One thing a shortcut can do.
pub struct Action {
    /// The menu item id it runs.
    pub id: &'static str,
    pub label: &'static str,
    /// The binding it has until you change it.
    pub default: Option<&'static str>,
}

const fn act(id: &'static str, label: &'static str, default: Option<&'static str>) -> Action {
    Action { id, label, default }
}

pub const ACTIONS: &[Action] = &[
    act("shot", "Take Screenshot", Some("cmd+s")),
    act("rec", "Start or Stop Recording", Some("cmd+e")),
    act("rotate", "Rotate Device", Some("cmd+r")),
    act("zoom:in", "Larger", Some("cmd+plus")),
    act("zoom:out", "Smaller", Some("cmd+minus")),
    act("zoom:reset", "Default Size", Some("cmd+0")),
    act("tilt:next", "Next Tilt", Some("cmd+t")),
    act("toolbar", "Show or Hide Toolbar", Some("cmd+shift+t")),
    act("touches", "Show or Hide Touches", Some("cmd+shift+o")),
    act("mute", "Mute or Unmute Playback", Some("cmd+shift+m")),
    act("sync", "Share Clipboard With Phone", None),
    act("win:new", "New Window", Some("cmd+n")),
    act("conn:another", "Connect Another Phone", None),
    act("conn:disconnect", "Disconnect", None),
    act("share:copy", "Copy Share Link", None),
    act("shortcuts", "Keyboard Shortcuts", Some("cmd+comma")),
];

/// A key with the modifiers held down for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combo {
    /// One lowercase character (`plus` is `+`), or `f1`..`f12`, `space`, `left`, `right`, `up`, `down`.
    pub key: String,
    /// Command or Ctrl.
    pub cmd: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Combo {
    /// `cmd+shift+s`, `alt+left`, `f5`. The words are case-insensitive; `ctrl`, `command` and `super` mean `cmd`,
    /// `option` means `alt`, and `plus`, `minus`, `comma` stand for the characters that would split the text.
    pub fn parse(spec: &str) -> Option<Self> {
        let mut c = Combo { key: String::new(), cmd: false, shift: false, alt: false };
        let mut parts = spec.split('+').map(str::trim).peekable();
        while let Some(p) = parts.next() {
            let p = p.to_ascii_lowercase();
            if parts.peek().is_some() {
                match p.as_str() {
                    "cmd" | "command" | "ctrl" | "control" | "super" => c.cmd = true,
                    "shift" => c.shift = true,
                    "alt" | "option" | "opt" => c.alt = true,
                    _ => return None,
                }
            } else {
                c.key = key_from_word(&p)?;
            }
        }
        c.allowed().then_some(c)
    }

    pub fn to_spec(&self) -> String {
        let mut out = String::new();
        for (on, word) in [(self.cmd, "cmd"), (self.alt, "alt"), (self.shift, "shift")] {
            if on {
                out.push_str(word);
                out.push('+');
            }
        }
        out.push_str(word_for_key(&self.key));
        out
    }

    /// Whether this can be a shortcut: a plain letter would be typed into the phone.
    pub fn allowed(&self) -> bool {
        let f_key = self.key.len() > 1 && self.key.starts_with('f');
        !self.key.is_empty() && (self.cmd || self.alt || f_key)
    }

    /// As a menu shows it: glyphs on a Mac (`⌥⇧⌘S`), words elsewhere (`Ctrl+Alt+Shift+S`).
    pub fn text(&self) -> String {
        if cfg!(target_os = "macos") {
            let mut out = String::new();
            for (on, glyph) in [(self.alt, "\u{2325}"), (self.shift, "\u{21e7}"), (self.cmd, "\u{2318}")] {
                if on {
                    out.push_str(glyph);
                }
            }
            out + &key_label(&self.key, true)
        } else {
            self.words()
        }
    }

    /// With the modifiers spelled out, for the shortcuts window.
    pub fn words(&self) -> String {
        let cmd = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
        let mut out = String::new();
        for (on, word) in [(self.cmd, cmd), (self.alt, "Alt"), (self.shift, "Shift")] {
            if on {
                out.push_str(word);
                out.push('+');
            }
        }
        out + &key_label(&self.key, false)
    }

    /// The key that was pressed and the modifiers held, from a window event. `None` for a key that cannot be
    /// bound (a modifier on its own, Enter, Escape...).
    pub fn from_event(event: &winit::event::KeyEvent, mods: winit::keyboard::ModifiersState) -> Option<Self> {
        use winit::keyboard::{Key, NamedKey};
        use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
        // without the modifiers, so Option+S is `s` and Shift+= is `=`
        let key = match event.key_without_modifiers() {
            Key::Character(c) => canonical(&c)?,
            Key::Named(NamedKey::Space) => "space".to_string(),
            Key::Named(NamedKey::ArrowLeft) => "left".to_string(),
            Key::Named(NamedKey::ArrowRight) => "right".to_string(),
            Key::Named(NamedKey::ArrowUp) => "up".to_string(),
            Key::Named(NamedKey::ArrowDown) => "down".to_string(),
            Key::Named(n) => f_key(n)?,
            _ => return None,
        };
        Some(Combo { key, cmd: mods.super_key() || mods.control_key(), shift: mods.shift_key(), alt: mods.alt_key() })
    }

    /// The same from the shortcuts window's input.
    pub fn from_egui(key: egui::Key, m: egui::Modifiers) -> Option<Self> {
        let name = key.symbol_or_name();
        let key = if name.chars().count() == 1 {
            canonical(name)?
        } else {
            match key {
                egui::Key::Space => "space".to_string(),
                egui::Key::ArrowLeft => "left".to_string(),
                egui::Key::ArrowRight => "right".to_string(),
                egui::Key::ArrowUp => "up".to_string(),
                egui::Key::ArrowDown => "down".to_string(),
                _ => {
                    let n = name.to_ascii_lowercase();
                    let digits = n.strip_prefix('f')?;
                    (digits.parse::<u8>().ok()? <= 12).then_some(n)?
                }
            }
        };
        Some(Combo { key, cmd: m.command || m.ctrl || m.mac_cmd, shift: m.shift, alt: m.alt })
    }

    /// Does a pressed combo trigger this binding? Shift does not matter for `+`, which many keyboards type
    /// with it.
    fn matches(&self, pressed: &Combo) -> bool {
        self.key == pressed.key
            && self.cmd == pressed.cmd
            && self.alt == pressed.alt
            && (self.shift == pressed.shift || self.key == "+")
    }
}

fn f_key(n: winit::keyboard::NamedKey) -> Option<String> {
    use winit::keyboard::NamedKey::*;
    let i = [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12].iter().position(|k| *k == n)?;
    Some(format!("f{}", i + 1))
}

/// One printable character, lowercase, with `=` folded into `+` (the same key on most layouts).
fn canonical(c: &str) -> Option<String> {
    let mut chars = c.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || ch.is_control() || ch.is_whitespace() {
        return None;
    }
    Some(if ch == '=' { "+".to_string() } else { ch.to_lowercase().collect() })
}

fn key_from_word(w: &str) -> Option<String> {
    Some(match w {
        "plus" | "equals" => "+".to_string(),
        "minus" => "-".to_string(),
        "comma" => ",".to_string(),
        "space" | "left" | "right" | "up" | "down" => w.to_string(),
        _ if w.len() > 1 && w.starts_with('f') => {
            let n: u8 = w[1..].parse().ok()?;
            (1..=12).contains(&n).then(|| w.to_string())?
        }
        _ => canonical(w)?,
    })
}

fn word_for_key(k: &str) -> &str {
    match k {
        "+" => "plus",
        "-" => "minus",
        "," => "comma",
        other => other,
    }
}

fn key_label(k: &str, glyphs: bool) -> String {
    match k {
        "space" => "Space".to_string(),
        "+" if !glyphs => "Plus".to_string(),
        "-" if !glyphs => "Minus".to_string(),
        "left" => if glyphs { "\u{2190}" } else { "Left" }.to_string(),
        "right" => if glyphs { "\u{2192}" } else { "Right" }.to_string(),
        "up" => if glyphs { "\u{2191}" } else { "Up" }.to_string(),
        "down" => if glyphs { "\u{2193}" } else { "Down" }.to_string(),
        _ => k.to_uppercase(),
    }
}

/// The bindings now: the defaults with what was changed on top.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shortcuts {
    /// `Some(None)` is an action whose default was taken away.
    changed: BTreeMap<String, Option<Combo>>,
}

fn setting_key(id: &str) -> String {
    format!("key.{id}")
}

impl Shortcuts {
    pub fn load(s: &crate::settings::Settings) -> Self {
        let mut me = Self::default();
        for a in ACTIONS {
            match s.get(&setting_key(a.id)) {
                Some(v) if v.eq_ignore_ascii_case("none") => {
                    me.changed.insert(a.id.to_string(), None);
                }
                Some(v) => {
                    if let Some(c) = Combo::parse(v) {
                        me.changed.insert(a.id.to_string(), Some(c));
                    }
                }
                None => {}
            }
        }
        me
    }

    /// What `id` is bound to.
    pub fn get(&self, id: &str) -> Option<Combo> {
        match self.changed.get(id) {
            Some(c) => c.clone(),
            None => ACTIONS.iter().find(|a| a.id == id)?.default.and_then(Combo::parse),
        }
    }

    /// The action a pressed key runs.
    pub fn lookup(&self, pressed: &Combo) -> Option<&'static str> {
        ACTIONS.iter().find(|a| self.get(a.id).is_some_and(|c| c.matches(pressed))).map(|a| a.id)
    }

    /// Bind `id` (or take its binding away with `None`). A key another action had is taken from it; that
    /// action's id is returned, so it can be said.
    pub fn set(&mut self, id: &str, combo: Option<Combo>) -> Option<&'static str> {
        let taken = combo.as_ref().and_then(|c| {
            ACTIONS.iter().find(|a| a.id != id && self.get(a.id).is_some_and(|o| o.matches(c) || c.matches(&o)))
        });
        if let Some(t) = taken {
            self.changed.insert(t.id.to_string(), None);
        }
        self.changed.insert(id.to_string(), combo);
        taken.map(|a| a.id)
    }

    /// Back to the default for `id`.
    pub fn reset(&mut self, id: &str) {
        self.changed.remove(id);
    }

    pub fn reset_all(&mut self) {
        self.changed.clear();
    }

    pub fn is_default(&self, id: &str) -> bool {
        !self.changed.contains_key(id)
    }

    /// The saved form of `id`: what goes after `key.<id>=`.
    pub fn saved_value(&self, id: &str) -> String {
        match self.changed.get(id) {
            None => "default".to_string(),
            Some(None) => "none".to_string(),
            Some(Some(c)) => c.to_spec(),
        }
    }

    /// Write every binding to the settings file.
    pub fn save(&self) {
        for a in ACTIONS {
            crate::settings::save(&setting_key(a.id), &self.saved_value(a.id));
        }
    }
}

fn store() -> &'static RwLock<Shortcuts> {
    static STORE: OnceLock<RwLock<Shortcuts>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

/// Read the bindings from the settings file; menus and key presses use these until the next call.
pub fn reload() {
    *store().write().unwrap() = Shortcuts::load(&crate::settings::Settings::load());
}

/// The action a pressed key runs, with the current bindings.
pub fn lookup(pressed: &Combo) -> Option<&'static str> {
    store().read().unwrap().lookup(pressed)
}

/// How a menu item shows its shortcut (none for an item without one).
pub fn text_for(id: &str) -> Option<String> {
    store().read().unwrap().get(id).map(|c| c.text())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> Combo {
        Combo::parse(s).unwrap()
    }

    #[test]
    fn a_binding_reads_and_writes_the_same_text() {
        for spec in ["cmd+s", "cmd+shift+t", "alt+left", "f5", "cmd+plus", "cmd+minus", "cmd+comma", "cmd+0"] {
            assert_eq!(c(spec).to_spec(), spec);
        }
        assert_eq!(c("Ctrl+Option+S").to_spec(), "cmd+alt+s");
        assert_eq!(c("cmd+=").to_spec(), "cmd+plus");
    }

    #[test]
    fn a_plain_letter_cannot_be_a_shortcut_but_an_f_key_can() {
        assert!(Combo::parse("s").is_none());
        assert!(Combo::parse("shift+s").is_none());
        assert!(Combo::parse("f5").is_some());
        assert!(Combo::parse("alt+s").is_some());
        assert!(Combo::parse("cmd+").is_none());
        assert!(Combo::parse("bogus+s").is_none());
        assert!(Combo::parse("f13").is_none());
    }

    #[test]
    fn every_default_is_valid_and_no_two_share_a_key() {
        let s = Shortcuts::default();
        let mut seen = Vec::new();
        for a in ACTIONS {
            if let Some(d) = a.default {
                assert!(Combo::parse(d).is_some(), "{} default {d}", a.id);
            }
            if let Some(b) = s.get(a.id) {
                assert!(!seen.contains(&b), "{} repeats {b:?}", a.id);
                seen.push(b);
            }
        }
    }

    #[test]
    fn the_defaults_keep_the_keys_that_always_worked() {
        let s = Shortcuts::default();
        assert_eq!(s.lookup(&c("cmd+s")), Some("shot"));
        assert_eq!(s.lookup(&c("cmd+e")), Some("rec"));
        assert_eq!(s.lookup(&c("cmd+r")), Some("rotate"));
        assert_eq!(s.lookup(&c("cmd+0")), Some("zoom:reset"));
        // + with or without Shift, and the = key that shares it
        assert_eq!(s.lookup(&c("cmd+plus")), Some("zoom:in"));
        assert_eq!(s.lookup(&c("cmd+shift+plus")), Some("zoom:in"));
        assert_eq!(s.lookup(&c("cmd+minus")), Some("zoom:out"));
        assert_eq!(s.lookup(&c("cmd+shift+s")), None);
        assert_eq!(s.lookup(&c("alt+s")), None);
    }

    #[test]
    fn a_new_binding_takes_the_key_from_its_old_owner() {
        let mut s = Shortcuts::default();
        assert_eq!(s.set("mute", Some(c("cmd+s"))), Some("shot"));
        assert_eq!(s.lookup(&c("cmd+s")), Some("mute"));
        assert_eq!(s.get("shot"), None);
        s.reset("shot");
        assert_eq!(s.get("shot"), Some(c("cmd+s")));
    }

    #[test]
    fn a_binding_can_be_taken_away_and_comes_back_on_reset() {
        let mut s = Shortcuts::default();
        s.set("shot", None);
        assert_eq!(s.lookup(&c("cmd+s")), None);
        assert_eq!(s.saved_value("shot"), "none");
        assert_eq!(s.saved_value("rec"), "default");
        s.reset_all();
        assert_eq!(s.lookup(&c("cmd+s")), Some("shot"));
    }

    #[test]
    fn what_was_saved_is_what_is_loaded() {
        let mut s = Shortcuts::default();
        s.set("shot", Some(c("alt+p")));
        s.set("rec", None);
        let mut text = String::new();
        for a in ACTIONS {
            text.push_str(&format!("{}={}\n", setting_key(a.id), s.saved_value(a.id)));
        }
        let loaded = Shortcuts::load(&crate::settings::Settings::from_text(&text));
        assert_eq!(loaded, s);
        // a line that cannot be read leaves the default
        let broken = Shortcuts::load(&crate::settings::Settings::from_text("key.shot=banana\n"));
        assert_eq!(broken.get("shot"), Some(c("cmd+s")));
    }

    #[test]
    fn every_action_is_a_menu_item_the_app_handles() {
        // ids that the app answers in `on_menu`; a new action must be added there too
        let src = include_str!("app.rs");
        for a in ACTIONS {
            assert!(src.contains(&format!("\"{}\"", a.id)), "{} is not handled in app.rs", a.id);
        }
    }
}
