//! How this copy of Handset was installed, which decides how it should be updated: a package manager updates
//! its own installs (and would be confused by a copy swapped behind its back), a downloaded app or zip can be
//! updated by Handset itself.

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
    /// `cargo install`, a development build or anything else: there is nothing to update in place.
    Other,
}

impl Source {
    /// The command that updates an install managed by a package manager.
    pub fn upgrade_command(self) -> Option<&'static str> {
        match self {
            Self::Homebrew => Some("brew upgrade --cask handset"),
            Self::Scoop => Some("scoop update handset"),
            Self::Winget => Some("winget upgrade davidamunga.Handset"),
            _ => None,
        }
    }
}

/// Where a Homebrew cask keeps its record of an install, on Apple Silicon and on Intel.
const CASKROOMS: [&str; 2] = ["/opt/homebrew/Caskroom/handset", "/usr/local/Caskroom/handset"];

/// What the update toast says and does for `source`: a package-manager install gets the command to run (a click
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
            text: format!("UPDATE {version} - CLICK TO COPY: {}", cmd.to_uppercase()),
            url: None,
            copy: Some(cmd.to_string()),
            install: false,
        },
        None if crate::updater::os_for(source).is_some() => {
            Notice { text: format!("UPDATE {version} - CLICK TO INSTALL"), url: None, copy: None, install: true }
        }
        None => Notice {
            text: format!("UPDATE {version} AVAILABLE - CLICK TO OPEN"),
            url: Some(url.to_string()),
            copy: None,
            install: false,
        },
    }
}

/// The install source of the running program.
pub fn current() -> Source {
    let exe = std::env::current_exe().unwrap_or_default();
    detect(&exe, CASKROOMS.iter().any(|p| Path::new(p).is_dir()))
}

/// Work out the install source from the program's path. `in_caskroom` says whether Homebrew has a record of
/// the cask, which is what tells a Homebrew install of `/Applications/Handset.app` from a downloaded one.
pub fn detect(exe: &Path, in_caskroom: bool) -> Source {
    // one spelling for both kinds of separator, and no case, so the rules read the same on every system
    let p = exe.to_string_lossy().replace('\\', "/").to_lowercase();
    if p.contains("/scoop/apps/handset/") {
        Source::Scoop
    } else if p.contains("/winget/packages/") || p.contains("/microsoft/winget/") {
        Source::Winget
    } else if p.ends_with("/handset.exe") {
        Source::WindowsZip
    } else if p.contains(".app/contents/macos/") {
        if in_caskroom { Source::Homebrew } else { Source::MacApp }
    } else {
        Source::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(path: &str, caskroom: bool) -> Source {
        detect(Path::new(path), caskroom)
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
    fn cargo_and_development_builds_are_other() {
        assert_eq!(src("/Users/me/.cargo/bin/handset", false), Source::Other);
        assert_eq!(src("/Users/me/Work/handset/app/target/release/handset", false), Source::Other);
        assert_eq!(src("", false), Source::Other);
    }

    #[test]
    fn a_scoop_folder_wins_over_the_exe_name() {
        // the scoop rule is checked before the plain handset.exe rule
        assert_eq!(src("C:/scoop/apps/handset/current/handset.exe", true), Source::Scoop);
    }

    #[test]
    fn a_package_manager_install_is_told_the_command_and_others_get_a_link() {
        let n = notice(Source::Homebrew, "0.5.0", "https://example.com/r");
        assert_eq!(n.copy.as_deref(), Some("brew upgrade --cask handset"));
        assert_eq!(n.url, None);
        assert!(n.text.contains("0.5.0") && n.text.contains("BREW UPGRADE --CASK HANDSET"), "{}", n.text);
        assert!(!n.install);
        // a downloaded app or an unzipped release installs the update itself
        for s in [Source::MacApp, Source::WindowsZip] {
            let n = notice(s, "0.5.0", "https://example.com/r");
            assert!(n.install && n.url.is_none() && n.copy.is_none(), "{s:?}");
            assert!(n.text.contains("CLICK TO INSTALL"), "{}", n.text);
        }
        let n = notice(Source::Other, "0.5.0", "https://example.com/r");
        assert_eq!((n.url.as_deref(), n.copy, n.install), (Some("https://example.com/r"), None, false));
        assert!(n.text.contains("CLICK TO OPEN"));
        assert_eq!(notice(Source::Scoop, "1.0.0", "u").copy.as_deref(), Some("scoop update handset"));
        assert!(notice(Source::Other, "1.0.0", "u").url.is_some());
    }

    #[test]
    fn package_managers_have_an_upgrade_command() {
        assert_eq!(Source::Homebrew.upgrade_command(), Some("brew upgrade --cask handset"));
        assert_eq!(Source::Scoop.upgrade_command(), Some("scoop update handset"));
        assert_eq!(Source::Winget.upgrade_command(), Some("winget upgrade davidamunga.Handset"));
        for s in [Source::MacApp, Source::WindowsZip, Source::Other] {
            assert_eq!(s.upgrade_command(), None, "{s:?} is not managed by a package manager");
        }
    }
}
