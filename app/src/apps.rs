//! The apps installed on the phone, as the scrcpy server lists them (`list_apps=true`), for the
//! "Open App" menu.

#[derive(Debug, Clone, PartialEq)]
pub struct AppEntry {
    pub name: String,
    pub package: String,
    pub system: bool,
}

/// Parse the server's list:
///
/// ```text
/// [server] INFO: List of apps:
///  * Calculator                    com.android.calculator2
///  - Some app with a very long name that does not fit the column
///                                  com.example.long
/// ```
///
/// `*` marks system apps and `-` the rest. A name longer than the 30-character column is followed by its
/// package on the next line.
pub fn parse_list(text: &str) -> Vec<AppEntry> {
    let mut out = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((system, rest)) =
            line.strip_prefix(" * ").map(|r| (true, r)).or_else(|| line.strip_prefix(" - ").map(|r| (false, r)))
        else {
            continue;
        };
        let rest = rest.trim_end();
        // the package is the last word, if the line carries one
        let (name, package) = match rest.rsplit_once(char::is_whitespace) {
            Some((name, pkg)) if is_package(pkg) => (name.trim().to_string(), pkg.to_string()),
            _ => {
                // long name: the package is alone on the next line
                let Some(next) = lines.peek().map(|l| l.trim()).filter(|p| is_package(p)) else { continue };
                let package = next.to_string();
                lines.next();
                (rest.trim().to_string(), package)
            }
        };
        if !name.is_empty() {
            out.push(AppEntry { name, package, system });
        }
    }
    out
}

/// A Java package name: dotted words, no spaces.
fn is_package(s: &str) -> bool {
    s.contains('.')
        && !s.starts_with('.')
        && !s.ends_with('.')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

/// Apps for the menu: the ones you installed first, by name, then the system ones.
pub fn for_menu(mut apps: Vec<AppEntry>) -> Vec<AppEntry> {
    apps.sort_by(|a, b| a.system.cmp(&b.system).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = "[server] INFO: List of apps:\n \
        * Calculator                    com.android.calculator2\n \
        * Camera                        com.android.camera\n \
        - YouTube                       com.google.android.youtube\n \
        - A very long application name!\n                                 com.example.long_name\n \
        - Mpesa                         com.safaricom.mpesa\n";

    #[test]
    fn parses_names_packages_and_the_system_flag() {
        let apps = parse_list(LIST);
        assert_eq!(apps.len(), 5);
        assert_eq!(
            apps[0],
            AppEntry { name: "Calculator".into(), package: "com.android.calculator2".into(), system: true }
        );
        assert_eq!(apps[2].package, "com.google.android.youtube");
        assert!(!apps[2].system);
    }

    #[test]
    fn a_long_name_takes_its_package_from_the_next_line() {
        let apps = parse_list(LIST);
        let long = apps.iter().find(|a| a.package == "com.example.long_name").unwrap();
        assert_eq!(long.name, "A very long application name!");
        // the neighbours are not disturbed
        assert_eq!(apps[4].name, "Mpesa");
    }

    #[test]
    fn ignores_noise() {
        assert!(parse_list("").is_empty());
        assert!(
            parse_list("[server] INFO: Processing Android apps... (this may take some time)\nsomething else\n")
                .is_empty()
        );
    }

    #[test]
    fn menu_order_is_your_apps_first_then_by_name() {
        let names: Vec<_> = for_menu(parse_list(LIST)).into_iter().map(|a| a.name).collect();
        assert_eq!(names, ["A very long application name!", "Mpesa", "YouTube", "Calculator", "Camera"]);
    }
}
