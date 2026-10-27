//! Checks that the styling guide and the README still say what the code does: the names of the backgrounds,
//! the ranges, the defaults, the example looks, the options and the pictures they link to.

use crate::style::{ASPECTS, Aspect, Background, GRADIENTS, MAX_PADDING, MAX_RADIUS, Style, WALLPAPERS};
use std::path::Path;

const GUIDE: &str = include_str!("../../docs/styling.md");
const README: &str = include_str!("../../README.md");
const MAIN: &str = include_str!("main.rs");
const UPDATES: &str = include_str!("../../docs/updates.md");

fn docs_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs")
}

#[test]
fn every_gradient_and_wallpaper_is_named_in_the_guide() {
    for g in GRADIENTS {
        assert!(GUIDE.contains(&format!("`{}`", g.name)), "gradient {} is not in docs/styling.md", g.name);
    }
    for w in WALLPAPERS {
        assert!(GUIDE.contains(&format!("`{}`", w.name)), "wallpaper {} is not in docs/styling.md", w.name);
    }
}

#[test]
fn every_built_in_look_is_named_in_the_guide() {
    for p in crate::presets::builtin() {
        assert!(GUIDE.contains(&format!("`{}`", p.name)), "look {} is not in docs/styling.md", p.name);
    }
}

#[test]
fn the_trim_limit_in_the_guide_is_the_real_one() {
    assert_eq!(crate::trim::MIN_LEN, 0.2);
    assert!(GUIDE.contains("shorter than 0.2 seconds"));
}

#[test]
fn every_aspect_ratio_is_named_in_the_guide() {
    for a in ASPECTS.iter().filter(|a| **a != Aspect::Auto) {
        assert!(GUIDE.contains(&a.label()), "aspect {} is not in docs/styling.md", a.label());
    }
}

#[test]
fn the_ranges_in_the_guide_are_the_real_ones() {
    assert!(GUIDE.contains(&format!("0 to {MAX_PADDING} percent")), "padding range");
    assert!(GUIDE.contains(&format!("0 to {MAX_RADIUS} percent")), "radius range");
    assert!(GUIDE.contains("| `padding` | 0 to 40 |") && MAX_PADDING == 40);
    assert!(GUIDE.contains("| `radius` | 0 to 25 |") && MAX_RADIUS == 25);
    assert!(GUIDE.contains("| `shadow` | 0 to 100 |"));
}

#[test]
fn the_defaults_in_the_guide_are_the_real_ones() {
    let d = Style::default();
    assert_eq!(d.background, Background::Gradient(0));
    assert_eq!(GRADIENTS[0].name, "sunset");
    assert_eq!((d.padding, d.radius, d.shadow, d.aspect), (10, 6, 60, Aspect::Auto));
    assert!(GUIDE.contains("(`gradient:sunset`, padding 10, radius 6, shadow 60, aspect `auto`)"));
}

#[test]
fn the_example_looks_in_the_guide_can_be_read() {
    let specs: Vec<&str> = GUIDE.lines().filter(|l| l.starts_with("bg=")).collect();
    assert!(!specs.is_empty(), "the guide shows a written-out look");
    for spec in specs {
        assert!(Style::parse(spec).is_some(), "cannot read the example look {spec:?}");
    }
    // the written-out looks inside the command examples too
    for line in GUIDE.lines().filter(|l| l.starts_with("handset ")) {
        if let Some(spec) = line.split('"').nth(1).filter(|s| s.starts_with("bg=")) {
            assert!(Style::parse(spec).is_some(), "cannot read {spec:?} in {line:?}");
        }
    }
}

#[test]
fn the_options_are_in_the_readme_and_in_the_usage_text() {
    for flag in ["--style", "--style-captures", "--restyle", "--edit"] {
        assert!(README.contains(&format!("`{flag}")), "{flag} is not in the README options");
        assert!(MAIN.contains(flag), "{flag} is not handled in main.rs");
    }
}

#[test]
fn the_pictures_the_docs_show_exist() {
    for doc in [GUIDE, README] {
        for line in doc.lines().filter(|l| l.contains("<img src=\"")) {
            let src = line.split("src=\"").nth(1).and_then(|r| r.split('"').next()).unwrap();
            let path =
                if doc.as_ptr() == GUIDE.as_ptr() { docs_dir().join(src) } else { docs_dir().join("..").join(src) };
            assert!(path.is_file(), "{src} is linked but missing");
        }
    }
}

#[test]
fn the_guide_is_linked_from_the_readme_and_points_at_the_adr() {
    assert!(README.contains("docs/styling.md"));
    assert!(GUIDE.contains("adr/0022-style-captures-with-a-shared-compositor.md"));
    assert!(docs_dir().join("adr/0022-style-captures-with-a-shared-compositor.md").is_file());
}

#[test]
fn the_update_guide_gives_the_real_commands_for_each_package_manager() {
    use crate::install::Source;
    use crate::install::{ArchPackage, AurHelper};
    for s in [Source::Homebrew, Source::Scoop, Source::Winget, Source::Cargo] {
        let cmd = s.upgrade_command().unwrap();
        assert!(UPDATES.contains(&format!("copies `{cmd}`")), "docs/updates.md does not give {cmd}");
        assert!(
            UPDATES.contains(&format!("CLICK TO COPY: {}", cmd.to_uppercase())),
            "the toast for {cmd} is not shown"
        );
        assert!(README.contains(cmd), "the README does not give {cmd}");
    }
    for s in [Source::Arch(ArchPackage::HandsetBin, Some(AurHelper::Yay)), Source::Arch(ArchPackage::HandsetBin, None)]
    {
        let cmd = s.upgrade_command().unwrap();
        assert!(UPDATES.contains(&format!("copies `{cmd}`")), "docs/updates.md does not give {cmd}");
    }
}

#[test]
fn the_update_guide_matches_the_snooze_the_settings_and_the_environment() {
    assert_eq!(crate::update::SNOOZE, 3 * 24 * 60 * 60);
    assert!(UPDATES.contains("Remind Me in 3 Days") && UPDATES.contains("three days"));
    assert!(UPDATES.contains("`update-check=off`"));
    for name in ["HANDSET_UPDATE_BASE", "HANDSET_UPDATE_CURRENT"] {
        assert!(UPDATES.contains(name), "{name} is not in docs/updates.md");
        assert!(
            include_str!("updater.rs").contains(name) || include_str!("update.rs").contains(name),
            "{name} is documented but not used"
        );
    }
}

#[test]
fn the_update_guide_names_the_files_the_updater_really_downloads() {
    use crate::updater::Os;
    assert!(UPDATES.contains(&format!("`{}`", Os::Mac.zip_name("X"))));
    assert!(UPDATES.contains(&format!("`{}`", Os::Windows.zip_name("X"))));
    assert!(UPDATES.contains(&format!("`{}`", Os::Windows.sums_name())));
    assert!(UPDATES.contains("handset-update-<id>/update.log"));
    assert!(MAIN.contains("--verify-release") && UPDATES.contains("--verify-release"));
    assert!(README.contains("docs/updates.md"));
}
