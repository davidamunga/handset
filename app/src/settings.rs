//! Preferences kept between runs in `~/.config/handset/settings`, one `key=value` per line. They are what a
//! person changed in the menus or on the toolbar; command-line options still win for the run they are given.
//!
//! Keys: `frame` (a built-in name, a path, or `none`), `toolbar`, `clipboard` (copy captures to the
//! clipboard), `normalize` (recorded audio), `sync-clipboard`, `mute`, `format` (`mp4` or `gif`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Settings(BTreeMap<String, String>);

fn dir() -> PathBuf {
    crate::tools::home().join(".config/handset")
}

impl Settings {
    pub fn load() -> Self {
        Self::load_from(&dir())
    }

    fn load_from(dir: &Path) -> Self {
        Self(std::fs::read_to_string(dir.join("settings")).map(|t| parse(&t)).unwrap_or_default())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// `on`/`off` (also `true`/`false`, `yes`/`no`, `1`/`0`).
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key)?.to_ascii_lowercase().as_str() {
            "on" | "true" | "yes" | "1" => Some(true),
            "off" | "false" | "no" | "0" => Some(false),
            _ => None,
        }
    }
}

/// Remember one setting. Other saved settings are kept; errors are ignored, since this is a convenience.
pub fn save(key: &str, value: &str) {
    let _ = save_in(&dir(), key, value);
}

pub fn save_bool(key: &str, on: bool) {
    save(key, if on { "on" } else { "off" });
}

fn save_in(dir: &Path, key: &str, value: &str) -> std::io::Result<()> {
    let mut settings = Settings::load_from(dir);
    settings.0.insert(key.to_string(), value.trim().to_string());
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("settings"), render(&settings.0))
}

fn parse(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            let (k, v) = (k.trim(), v.trim());
            (!k.is_empty() && !v.is_empty()).then(|| (k.to_string(), v.to_string()))
        })
        .collect()
}

fn render(map: &BTreeMap<String, String>) -> String {
    let mut out = String::from("# Handset settings, written when you change something in the menu. Edit freely.\n");
    for (k, v) in map {
        out.push_str(&format!("{k}={}\n", v.replace('\n', " ")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_values_and_skips_comments_and_blank_lines() {
        let m = parse("# a comment\nframe = hazel\n\ntoolbar=off\nbroken line\nempty=\n=novalue\n");
        assert_eq!(m.get("frame").map(String::as_str), Some("hazel"));
        assert_eq!(m.get("toolbar").map(String::as_str), Some("off"));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn booleans_accept_the_usual_words() {
        let s = Settings(parse("a=on\nb=Off\nc=true\nd=no\ne=maybe\n"));
        assert_eq!(s.get_bool("a"), Some(true));
        assert_eq!(s.get_bool("b"), Some(false));
        assert_eq!(s.get_bool("c"), Some(true));
        assert_eq!(s.get_bool("d"), Some(false));
        assert_eq!(s.get_bool("e"), None);
        assert_eq!(s.get_bool("missing"), None);
    }

    #[test]
    fn saving_one_setting_keeps_the_others() {
        let dir = std::env::temp_dir().join(format!("handset-settings-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(Settings::load_from(&dir).get("frame").is_none());
        save_in(&dir, "frame", "porcelain").unwrap();
        save_in(&dir, "toolbar", "off").unwrap();
        save_in(&dir, "frame", "hazel").unwrap(); // replaced, not duplicated
        let s = Settings::load_from(&dir);
        assert_eq!(s.get("frame"), Some("hazel"));
        assert_eq!(s.get_bool("toolbar"), Some(false));
        let text = std::fs::read_to_string(dir.join("settings")).unwrap();
        assert_eq!(text.matches("frame=").count(), 1);
        assert!(text.starts_with('#'));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
