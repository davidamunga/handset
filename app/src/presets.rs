//! Presets: a style saved under a name to be picked again with one click, or given on the command line
//! as `--style "Launch post"`. Handset ships a few; the ones you save are kept in
//! `~/.config/handset/presets`, one `name<TAB>style` per line (the style in the form `Style::parse` reads).

use crate::style::Style;
use std::path::{Path, PathBuf};

const MAX_NAME: usize = 32;

#[derive(Clone, PartialEq, Debug)]
pub struct Preset {
    pub name: String,
    pub style: Style,
}

/// The presets that come with Handset.
pub fn builtin() -> Vec<Preset> {
    [
        ("Clean white", "bg=color:#f5f5f7;padding=12;radius=6;shadow=40;aspect=auto"),
        ("Dark card", "bg=color:#18181b;padding=10;radius=6;shadow=70;aspect=auto"),
        ("Sunset 4:5", "bg=gradient:sunset;padding=12;radius=6;shadow=60;aspect=4:5"),
        ("Aurora 16:9", "bg=wallpaper:aurora;padding=10;radius=6;shadow=60;aspect=16:9"),
        ("Dusk square", "bg=gradient:dusk;padding=12;radius=6;shadow=60;aspect=1:1"),
    ]
    .into_iter()
    .filter_map(|(name, spec)| Some(Preset { name: name.to_string(), style: Style::parse(spec)? }))
    .collect()
}

/// A name that is safe to keep: trimmed, on one line, not empty, not longer than 32 characters.
pub fn clean_name(raw: &str) -> Option<String> {
    let one_line: String = raw.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let name: String = one_line.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NAME).collect();
    let name = name.trim().to_string();
    (!name.is_empty()).then_some(name)
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Save `preset`, replacing a look of the same name (ignoring case).
pub fn upsert(list: &mut Vec<Preset>, preset: Preset) {
    match list.iter_mut().find(|p| same(&p.name, &preset.name)) {
        Some(existing) => *existing = preset,
        None => list.push(preset),
    }
}

/// Forget the look called `name`; false when there was none.
pub fn remove(list: &mut Vec<Preset>, name: &str) -> bool {
    let before = list.len();
    list.retain(|p| !same(&p.name, name));
    list.len() != before
}

/// What `--style` means: a saved look's name (yours before the built-in ones), or else a style written out.
pub fn resolve(arg: &str, saved: &[Preset]) -> Option<Style> {
    let by_name = |list: &[Preset]| list.iter().find(|p| same(&p.name, arg)).map(|p| p.style.clone());
    by_name(saved).or_else(|| by_name(&builtin())).or_else(|| Style::parse(arg))
}

fn file(dir: &Path) -> PathBuf {
    dir.join("presets")
}

/// What the file was called before presets were called presets. It is read when there is no `presets` file, and
/// removed the next time presets are saved.
fn legacy_file(dir: &Path) -> PathBuf {
    dir.join("looks")
}

pub fn load() -> Vec<Preset> {
    load_from(&crate::settings::dir())
}

/// The saved presets. Lines that cannot be read are skipped, so one bad line does not lose the rest.
pub fn load_from(dir: &Path) -> Vec<Preset> {
    let text =
        std::fs::read_to_string(file(dir)).or_else(|_| std::fs::read_to_string(legacy_file(dir))).unwrap_or_default();
    let mut list = Vec::new();
    for line in text.lines() {
        let Some((name, spec)) = line.split_once('\t') else { continue };
        if let (Some(name), Some(style)) = (clean_name(name), Style::parse(spec)) {
            upsert(&mut list, Preset { name, style });
        }
    }
    list
}

pub fn save(list: &[Preset]) {
    let _ = save_to(&crate::settings::dir(), list);
}

pub fn save_to(dir: &Path, list: &[Preset]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text: String = list.iter().map(|p| format!("{}\t{}\n", p.name, p.style.to_spec())).collect();
    std::fs::write(file(dir), text)?;
    let _ = std::fs::remove_file(legacy_file(dir));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Aspect, Background};

    fn look(name: &str, padding: u32) -> Preset {
        Preset { name: name.to_string(), style: Style { padding, ..Style::default() } }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("handset-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn every_built_in_look_is_valid_and_has_its_own_name() {
        let list = builtin();
        assert_eq!(list.len(), 5, "a built-in look failed to parse");
        let mut names: Vec<_> = list.iter().map(|p| p.name.to_lowercase()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 5);
        let wide = list.iter().find(|p| p.name == "Aurora 16:9").unwrap();
        assert_eq!(wide.style.aspect, Aspect::Ratio(16, 9));
    }

    #[test]
    fn names_are_tidied_and_limited() {
        assert_eq!(clean_name("  Launch   post \n"), Some("Launch post".into()));
        assert_eq!(clean_name("a\tb"), Some("a b".into()), "a tab would break the file");
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name(""), None);
        assert_eq!(clean_name(&"x".repeat(80)).unwrap().chars().count(), MAX_NAME);
    }

    #[test]
    fn saving_a_name_again_replaces_it_ignoring_case() {
        let mut list = vec![look("Launch", 5)];
        upsert(&mut list, look("launch", 20));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].style.padding, 20);
        upsert(&mut list, look("Other", 1));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn removing_a_look_forgets_only_that_one() {
        let mut list = vec![look("A", 1), look("B", 2)];
        assert!(remove(&mut list, "a"));
        assert!(!remove(&mut list, "missing"));
        assert_eq!(list, vec![look("B", 2)]);
    }

    #[test]
    fn presets_survive_a_round_trip_through_the_file() {
        let dir = temp_dir("presets-round-trip");
        let list = vec![
            look("Launch post", 14),
            Preset {
                name: "Wide, dark".into(),
                style: Style {
                    background: Background::Color([1, 2, 3]),
                    aspect: Aspect::Ratio(16, 9),
                    ..Style::default()
                },
            },
        ];
        save_to(&dir, &list).unwrap();
        assert_eq!(load_from(&dir), list);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_or_damaged_file_gives_what_can_be_read() {
        let dir = temp_dir("presets-damaged");
        assert!(load_from(&dir).is_empty(), "no file is no presets");
        std::fs::create_dir_all(&dir).unwrap();
        let good = look("Good", 9).style.to_spec();
        std::fs::write(file(&dir), format!("no tab on this line\nBad\tbg=sparkles\n\t{good}\nGood\t{good}\n")).unwrap();
        let loaded = load_from(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Good");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn presets_saved_under_the_old_file_name_are_found_and_moved() {
        let dir = temp_dir("presets-legacy");
        std::fs::create_dir_all(&dir).unwrap();
        let old = look("Old", 7);
        std::fs::write(legacy_file(&dir), format!("{}\t{}\n", old.name, old.style.to_spec())).unwrap();
        assert_eq!(load_from(&dir), vec![old.clone()]);
        save_to(&dir, &[old.clone(), look("New", 8)]).unwrap();
        assert!(!legacy_file(&dir).exists(), "the old file is removed once presets are saved");
        assert_eq!(load_from(&dir).len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn style_arguments_take_a_name_or_a_written_out_style() {
        let saved = vec![look("Clean white", 33)];
        // your own look wins over a built-in of the same name
        assert_eq!(resolve("clean WHITE", &saved).unwrap().padding, 33);
        assert_eq!(resolve("Dark card", &saved).unwrap().padding, 10);
        assert_eq!(resolve("bg=gradient:ocean;padding=3", &saved).unwrap().padding, 3);
        assert_eq!(resolve("no such look", &saved), None);
    }
}
