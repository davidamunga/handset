//! How this copy of Handset was installed, which decides how it should be updated: a package manager updates
//! its own installs (and would be confused by a copy swapped behind its back), a downloaded app, zip or tarball
//! can be updated by Handset itself.

use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// A Homebrew cask (`brew install --cask davidamunga/tap/handset`).
    Homebrew,
    /// Scoop (`scoop install handset/handset`).
    Scoop,
    /// winget (`winget install davidamunga.Handset`).
    Winget,
    /// A downloaded `Handset.app` on macOS.
    MacApp,
    /// An unzipped `Handset-X.Y.Z-windows.zip`.
    WindowsZip,
    /// An Arch package that owns `/usr/bin/handset`, and the AUR helper on `PATH` if there is one.
    Arch(ArchPackage, Option<AurHelper>),
    /// `cargo install`, in `~/.cargo/bin`.
    Cargo,
    /// The Nix package, in `/nix/store`: whatever Nix setup installed it updates it.
    Nix,
    /// An unpacked `Handset-X.Y.Z-linux-<arch>.tar.gz`: the binary next to the tarball's `handset.desktop`, or
    /// copied to `~/.local/bin`.
    LinuxTarball,
    /// A development build or anything else: there is nothing to update in place.
    Other,
}

/// Which of the Arch packages is installed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArchPackage {
    /// `handset`, built from the release tag.
    Handset,
    /// `handset-bin`, the release tarball repackaged.
    HandsetBin,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AurHelper {
    Yay,
    Paru,
}

impl Source {
    /// The command that updates an install managed by a package manager.
    pub fn upgrade_command(self) -> Option<&'static str> {
        use {ArchPackage::*, AurHelper::*};
        match self {
            Self::Homebrew => Some("brew upgrade --cask handset"),
            Self::Scoop => Some("scoop update handset"),
            Self::Winget => Some("winget upgrade davidamunga.Handset"),
            Self::Arch(Handset, Some(Yay)) => Some("yay -S handset"),
            Self::Arch(Handset, Some(Paru)) => Some("paru -S handset"),
            Self::Arch(HandsetBin, Some(Yay)) => Some("yay -S handset-bin"),
            Self::Arch(HandsetBin, Some(Paru)) => Some("paru -S handset-bin"),
            // without a helper, the AUR's own steps: clone the package and build it
            Self::Arch(Handset, None) => {
                Some("git clone https://aur.archlinux.org/handset.git && cd handset && makepkg -si")
            }
            Self::Arch(HandsetBin, None) => {
                Some("git clone https://aur.archlinux.org/handset-bin.git && cd handset-bin && makepkg -si")
            }
            Self::Cargo => Some("cargo install --git https://github.com/davidamunga/handset --locked handset --force"),
            _ => None,
        }
    }
}

/// Where a Homebrew cask keeps its record of an install, on Apple Silicon and on Intel.
const CASKROOMS: [&str; 2] = ["/opt/homebrew/Caskroom/handset", "/usr/local/Caskroom/handset"];

/// Where both Arch packages put the program.
const ARCH_BIN: &str = "/usr/bin/handset";

/// What the update banner says and does for `source`: a package-manager install gets the command to run (a click
/// copies it), anything else a link to the download.
#[derive(Debug, PartialEq)]
pub struct Notice {
    pub text: String,
    pub url: Option<String>,
    pub copy: Option<String>,
    /// A click installs the update in place (a downloaded app that can replace itself).
    pub install: bool,
}

pub fn notice(source: Source, version: &str, url: &str) -> Notice {
    match source.upgrade_command() {
        Some(cmd) => Notice {
            text: format!("HANDSET {version} IS OUT - CLICK TO COPY: {}", cmd.to_uppercase()),
            url: None,
            copy: Some(cmd.to_string()),
            install: false,
        },
        None if crate::updater::os_for(source).is_some() => Notice {
            text: format!("HANDSET {version} IS OUT - CLICK TO INSTALL"),
            url: None,
            copy: None,
            install: true,
        },
        None => Notice {
            text: if source == Source::Nix {
                format!("HANDSET {version} IS OUT - UPDATE IT THROUGH NIX - CLICK TO VIEW")
            } else {
                format!("HANDSET {version} IS OUT - CLICK TO VIEW")
            },
            url: Some(url.to_string()),
            copy: None,
            install: false,
        },
    }
}

/// What [`detect`] needs to know besides the program's path, found out by [`current`].
#[derive(Clone, Copy, Default, Debug)]
pub struct Facts {
    /// Homebrew has a record of the cask.
    pub in_caskroom: bool,
    /// The Arch package that owns `/usr/bin/handset`, by `pacman -Qqo`.
    pub arch_package: Option<ArchPackage>,
    pub aur_helper: Option<AurHelper>,
    /// The tarball's `handset.desktop` is next to the program.
    pub beside_desktop_entry: bool,
    /// Running on Linux, where an unpacked tarball can be in `~/.local/bin`.
    pub linux: bool,
}

/// The install source of the running program.
pub fn current() -> Source {
    let exe = std::env::current_exe().unwrap_or_default();
    let linux = cfg!(target_os = "linux");
    let arch_package = if linux && exe == Path::new(ARCH_BIN) { arch_owner() } else { None };
    let on_path = |name: &str| crate::tools::find(name).is_file();
    let facts = Facts {
        in_caskroom: CASKROOMS.iter().any(|p| Path::new(p).is_dir()),
        arch_package,
        aur_helper: arch_package.and_then(|_| {
            if on_path("yay") {
                Some(AurHelper::Yay)
            } else if on_path("paru") {
                Some(AurHelper::Paru)
            } else {
                None
            }
        }),
        beside_desktop_entry: linux && exe.with_file_name("handset.desktop").is_file(),
        linux,
    };
    detect(&exe, &facts)
}

/// The Arch package that owns `/usr/bin/handset`, when pacman is there to ask.
fn arch_owner() -> Option<ArchPackage> {
    let pacman = crate::tools::find("pacman");
    if !pacman.is_file() {
        return None;
    }
    let mut cmd = crate::tools::command(&pacman);
    cmd.args(["-Qqo", ARCH_BIN]);
    let out = crate::adb::output_within(cmd, std::time::Duration::from_secs(5)).ok()?;
    out.status.success().then(|| parse_arch_owner(&String::from_utf8_lossy(&out.stdout)))?
}

/// `pacman -Qqo`'s answer: the name of the package that owns the file. Another package that happens to own
/// `/usr/bin/handset` is not ours.
pub fn parse_arch_owner(out: &str) -> Option<ArchPackage> {
    match out.lines().next()?.trim() {
        "handset" => Some(ArchPackage::Handset),
        "handset-bin" => Some(ArchPackage::HandsetBin),
        _ => None,
    }
}

/// Work out the install source from the program's path and `facts`. The Caskroom is what tells a Homebrew
/// install of `/Applications/Handset.app` from a downloaded one; pacman, what tells a package's
/// `/usr/bin/handset` from a copy put there by hand.
pub fn detect(exe: &Path, facts: &Facts) -> Source {
    // one spelling for both kinds of separator, and no case, so the rules read the same on every system
    let p = exe.to_string_lossy().replace('\\', "/").to_lowercase();
    if p.contains("/scoop/apps/handset/") {
        Source::Scoop
    } else if p.contains("/winget/packages/") || p.contains("/microsoft/winget/") {
        Source::Winget
    } else if p.ends_with("/.cargo/bin/handset") || p.ends_with("/.cargo/bin/handset.exe") {
        Source::Cargo
    } else if p.ends_with("/handset.exe") {
        Source::WindowsZip
    } else if p.contains(".app/contents/macos/") {
        if facts.in_caskroom { Source::Homebrew } else { Source::MacApp }
    } else if p.starts_with("/nix/store/") {
        Source::Nix
    } else if p == ARCH_BIN
        && let Some(pkg) = facts.arch_package
    {
        Source::Arch(pkg, facts.aur_helper)
    } else if p.ends_with("/handset")
        && (facts.beside_desktop_entry || (facts.linux && p.ends_with("/.local/bin/handset")))
    {
        Source::LinuxTarball
    } else {
        Source::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(path: &str, caskroom: bool) -> Source {
        detect(Path::new(path), &Facts { in_caskroom: caskroom, ..Facts::default() })
    }

    fn linux(path: &str, facts: Facts) -> Source {
        detect(Path::new(path), &Facts { linux: true, ..facts })
    }

    #[test]
    fn a_downloaded_mac_app_is_told_from_a_homebrew_one_by_the_caskroom() {
        let exe = "/Applications/Handset.app/Contents/MacOS/handset";
        assert_eq!(src(exe, false), Source::MacApp);
        assert_eq!(src(exe, true), Source::Homebrew);
        assert_eq!(src("/Users/me/Applications/Handset.app/Contents/MacOS/handset", false), Source::MacApp);
    }

    #[test]
    fn scoop_and_winget_installs_are_found_by_their_folders() {
        assert_eq!(src(r"C:\Users\me\scoop\apps\handset\0.4.0\handset.exe", false), Source::Scoop);
        assert_eq!(src(r"C:\Users\me\scoop\apps\handset\current\handset.exe", false), Source::Scoop);
        assert_eq!(
            src(r"C:\Users\me\AppData\Local\Microsoft\WinGet\Packages\davidamunga.Handset_x\handset.exe", false),
            Source::Winget
        );
    }

    #[test]
    fn an_unzipped_windows_release_is_a_zip_install() {
        assert_eq!(src(r"C:\Tools\Handset-0.4.0\handset.exe", false), Source::WindowsZip);
        assert_eq!(src(r"D:\handset\HANDSET.EXE", false), Source::WindowsZip);
    }

    #[test]
    fn cargo_install_is_found_in_cargos_bin_folder_on_every_system() {
        assert_eq!(src("/home/me/.cargo/bin/handset", false), Source::Cargo);
        assert_eq!(src("/Users/me/.cargo/bin/handset", false), Source::Cargo);
        // checked before the plain handset.exe rule, so a cargo install is never swapped like an unzipped release
        assert_eq!(src(r"C:\Users\me\.cargo\bin\handset.exe", false), Source::Cargo);
        assert_eq!(
            linux("/home/me/.cargo/bin/handset", Facts { beside_desktop_entry: true, ..Facts::default() }),
            Source::Cargo
        );
    }

    #[test]
    fn development_builds_are_other() {
        assert_eq!(src("/Users/me/Work/handset/app/target/release/handset", false), Source::Other);
        assert_eq!(linux("/home/me/handset/app/target/release/handset", Facts::default()), Source::Other);
        assert_eq!(linux("/home/me/.cargo/target/release/handset", Facts::default()), Source::Other);
        assert_eq!(linux("/home/me/.cargo/bin/handset-old", Facts::default()), Source::Other);
        assert_eq!(src("", false), Source::Other);
    }

    #[test]
    fn a_scoop_folder_wins_over_the_exe_name() {
        // the scoop rule is checked before the plain handset.exe rule
        assert_eq!(src("C:/scoop/apps/handset/current/handset.exe", true), Source::Scoop);
    }

    #[test]
    fn an_arch_package_is_the_usr_bin_program_that_pacman_says_is_ours() {
        let pkg = |arch_package, aur_helper| Facts { arch_package, aur_helper, ..Facts::default() };
        assert_eq!(
            linux("/usr/bin/handset", pkg(Some(ArchPackage::HandsetBin), Some(AurHelper::Yay))),
            Source::Arch(ArchPackage::HandsetBin, Some(AurHelper::Yay))
        );
        assert_eq!(
            linux("/usr/bin/handset", pkg(Some(ArchPackage::Handset), None)),
            Source::Arch(ArchPackage::Handset, None)
        );
        // no package owns it (no pacman, or a copy put there by hand)
        assert_eq!(linux("/usr/bin/handset", Facts::default()), Source::Other);
        assert_eq!(linux("/usr/local/bin/handset", Facts::default()), Source::Other);
        // only /usr/bin is asked about, so another path is never an Arch install
        assert_eq!(linux("/usr/local/bin/handset", pkg(Some(ArchPackage::HandsetBin), None)), Source::Other);
    }

    #[test]
    fn pacmans_answer_names_one_of_our_packages_or_none() {
        assert_eq!(parse_arch_owner("handset\n"), Some(ArchPackage::Handset));
        assert_eq!(parse_arch_owner("handset-bin\n"), Some(ArchPackage::HandsetBin));
        assert_eq!(parse_arch_owner("handset-git\n"), None);
        assert_eq!(parse_arch_owner("someone-elses-handset\n"), None);
        assert_eq!(parse_arch_owner(""), None);
    }

    #[test]
    fn the_nix_package_is_found_in_the_store() {
        assert_eq!(linux("/nix/store/abc123-handset-0.8.1/bin/handset", Facts::default()), Source::Nix);
        // a store path wins even with a desktop entry next to the program
        let beside = Facts { beside_desktop_entry: true, ..Facts::default() };
        assert_eq!(linux("/nix/store/abc123-handset-0.8.1/bin/handset", beside), Source::Nix);
        assert_eq!(linux("/home/me/nix/store/handset", Facts::default()), Source::Other);
    }

    #[test]
    fn an_unpacked_tarball_is_found_by_its_desktop_entry_or_in_local_bin() {
        let beside = Facts { beside_desktop_entry: true, ..Facts::default() };
        assert_eq!(linux("/home/me/Apps/Handset-0.8.1-linux-x86_64/handset", beside), Source::LinuxTarball);
        assert_eq!(linux("/opt/handset/handset", beside), Source::LinuxTarball);
        assert_eq!(linux("/home/me/.local/bin/handset", Facts::default()), Source::LinuxTarball);
        // without the desktop entry, a folder of its own is not enough
        assert_eq!(linux("/home/me/Apps/Handset-0.8.1-linux-x86_64/handset", Facts::default()), Source::Other);
        // ~/.local/bin means the tarball only on Linux
        assert_eq!(src("/Users/me/.local/bin/handset", false), Source::Other);
        // the desktop entry next to something that is not the program
        assert_eq!(linux("/home/me/Apps/handset-1/run.sh", beside), Source::Other);
    }

    #[test]
    fn a_package_manager_install_is_told_the_command_and_others_get_a_link() {
        let n = notice(Source::Homebrew, "0.5.0", "https://example.com/r");
        assert_eq!(n.copy.as_deref(), Some("brew upgrade --cask handset"));
        assert_eq!(n.url, None);
        assert!(n.text.contains("0.5.0") && n.text.contains("BREW UPGRADE --CASK HANDSET"), "{}", n.text);
        assert!(!n.install);
        // a downloaded app, an unzipped release or an unpacked tarball installs the update itself
        for s in [Source::MacApp, Source::WindowsZip, Source::LinuxTarball] {
            let n = notice(s, "0.5.0", "https://example.com/r");
            assert!(n.install && n.url.is_none() && n.copy.is_none(), "{s:?}");
            assert!(n.text.contains("CLICK TO INSTALL"), "{}", n.text);
        }
        let n = notice(Source::Other, "0.5.0", "https://example.com/r");
        assert_eq!((n.url.as_deref(), n.copy, n.install), (Some("https://example.com/r"), None, false));
        assert!(n.text.contains("CLICK TO VIEW"));
        assert_eq!(notice(Source::Scoop, "1.0.0", "u").copy.as_deref(), Some("scoop update handset"));
        assert!(notice(Source::Other, "1.0.0", "u").url.is_some());
    }

    #[test]
    fn arch_and_cargo_installs_copy_their_command() {
        let n = notice(Source::Arch(ArchPackage::HandsetBin, Some(AurHelper::Paru)), "0.9.0", "u");
        assert_eq!(n.copy.as_deref(), Some("paru -S handset-bin"));
        assert_eq!(n.text, "HANDSET 0.9.0 IS OUT - CLICK TO COPY: PARU -S HANDSET-BIN");
        assert!(!n.install && n.url.is_none());
        let n = notice(Source::Cargo, "0.9.0", "u");
        assert_eq!(
            n.copy.as_deref(),
            Some("cargo install --git https://github.com/davidamunga/handset --locked handset --force")
        );
        assert!(!n.install && n.url.is_none());
    }

    #[test]
    fn a_nix_install_is_told_to_update_through_nix_and_gets_the_page() {
        let n = notice(Source::Nix, "0.9.0", "https://example.com/r");
        assert_eq!((n.url.as_deref(), n.copy, n.install), (Some("https://example.com/r"), None, false));
        // the banner puts the version on the first line and the rest on the next
        assert_eq!(n.text.replacen(" - ", "\n", 1), "HANDSET 0.9.0 IS OUT\nUPDATE IT THROUGH NIX - CLICK TO VIEW");
    }

    #[test]
    fn package_managers_have_an_upgrade_command() {
        use {ArchPackage::*, AurHelper::*};
        assert_eq!(Source::Homebrew.upgrade_command(), Some("brew upgrade --cask handset"));
        assert_eq!(Source::Scoop.upgrade_command(), Some("scoop update handset"));
        assert_eq!(Source::Winget.upgrade_command(), Some("winget upgrade davidamunga.Handset"));
        assert_eq!(Source::Arch(Handset, Some(Yay)).upgrade_command(), Some("yay -S handset"));
        assert_eq!(Source::Arch(HandsetBin, Some(Yay)).upgrade_command(), Some("yay -S handset-bin"));
        assert_eq!(Source::Arch(Handset, Some(Paru)).upgrade_command(), Some("paru -S handset"));
        assert_eq!(
            Source::Arch(HandsetBin, None).upgrade_command(),
            Some("git clone https://aur.archlinux.org/handset-bin.git && cd handset-bin && makepkg -si")
        );
        assert_eq!(
            Source::Arch(Handset, None).upgrade_command(),
            Some("git clone https://aur.archlinux.org/handset.git && cd handset && makepkg -si")
        );
        for s in [Source::MacApp, Source::WindowsZip, Source::LinuxTarball, Source::Nix, Source::Other] {
            assert_eq!(s.upgrade_command(), None, "{s:?} is not managed by a package manager");
        }
    }
}
