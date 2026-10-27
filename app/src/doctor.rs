//! `handset --doctor`: check the tools and the connected phones, and say what to fix.

use crate::adb;
use crate::connect::is_wireless;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub level: Level,
    pub title: String,
    pub hint: Option<String>,
}

impl Check {
    /// The title without a trailing `(path)`, which is too long for the connect screen.
    pub fn short_title(&self) -> &str {
        match self.title.rsplit_once(" (") {
            Some((head, tail)) if tail.contains(['/', '\\']) => head,
            _ => &self.title,
        }
    }
}

fn check(level: Level, title: impl Into<String>, hint: Option<&str>) -> Check {
    Check { level, title: title.into(), hint: hint.map(str::to_string) }
}

/// The oldest Android version (API level) the scrcpy server runs on.
const MIN_SDK: u32 = 21;

#[derive(Debug, Default, PartialEq)]
pub struct Props {
    pub model: String,
    pub release: String,
    pub sdk: u32,
    pub awake: Option<bool>,
}

pub fn props(serial: &str) -> Props {
    let get = |k: &str| adb::run(serial, &["shell", "getprop", k]).unwrap_or_default();
    let awake = adb::run(serial, &["shell", "dumpsys", "power"]).ok().and_then(|t| wakefulness(&t));
    Props {
        model: get("ro.product.model"),
        release: get("ro.build.version.release"),
        sdk: get("ro.build.version.sdk").parse().unwrap_or(0),
        awake,
    }
}

/// Is the screen on, from `dumpsys power` (`mWakefulness=Awake|Asleep|Dozing`).
fn wakefulness(dumpsys: &str) -> Option<bool> {
    let line = dumpsys.lines().find(|l| l.contains("mWakefulness="))?;
    Some(line.trim().ends_with("Awake"))
}

pub fn no_phone(found_on_wifi: bool) -> Check {
    if found_on_wifi {
        check(
            Level::Warn,
            "no phone connected, but one is announcing itself on Wi-Fi",
            Some("run `handset --last`, or pick it on the connect screen (Wi-Fi), then connect"),
        )
    } else {
        check(
            Level::Fail,
            "no phone connected",
            Some(
                "plug it in with a data cable (some cables only charge), unlock it, and turn on USB debugging in \
                 Settings > Developer options; or pair over Wi-Fi with `handset --pair <ip:port> <code>`",
            ),
        )
    }
}

pub fn device_checks(serial: &str, state: &str, p: &Props) -> Vec<Check> {
    let how = if is_wireless(serial) { "Wi-Fi" } else { "USB" };
    let mut out = Vec::new();
    match state {
        "device" => {
            let name = if p.model.is_empty() { serial.to_string() } else { format!("{} ({serial})", p.model) };
            out.push(check(Level::Ok, format!("{name} is connected over {how}"), None));
            if p.sdk != 0 && p.sdk < MIN_SDK {
                out.push(check(
                    Level::Fail,
                    format!("Android {} is too old (needs Android 5.0 or later)", p.release),
                    None,
                ));
            } else if p.sdk != 0 {
                out.push(check(Level::Ok, format!("Android {} (API {})", p.release, p.sdk), None));
            }
            if p.awake == Some(false) {
                out.push(check(
                    Level::Warn,
                    "the screen is off, so the mirror will be black",
                    Some("press the phone's power button, or the Power button on the toolbar"),
                ));
            }
        }
        "unauthorized" => out.push(check(
            Level::Fail,
            format!("{serial} has not authorised this computer"),
            Some(
                "unlock the phone and accept \"Allow USB debugging?\". If nothing appears, unplug it, run \
                 `adb kill-server`, plug it in again, or use Developer options > Revoke USB debugging authorisations",
            ),
        )),
        "no permissions" => out.push(check(
            Level::Fail,
            format!("{serial} is plugged in but this user may not use it"),
            Some(
                "on Linux this is missing udev rules: install `android-udev-rules` (or your distro's `android-tools` \
                 package), or add a rule for the phone's vendor id, replug it, and run `adb kill-server`",
            ),
        )),
        "offline" => out.push(check(
            Level::Fail,
            format!("{serial} is offline"),
            Some("unplug it and plug it in again, or run `adb kill-server`; over Wi-Fi, connect again"),
        )),
        other => out.push(check(
            Level::Fail,
            format!("{serial} is in the \"{other}\" state"),
            Some("run `adb devices` and look up that state"),
        )),
    }
    out
}

pub fn render(checks: &[Check]) -> String {
    let mut s = String::new();
    for c in checks {
        let mark = match c.level {
            Level::Ok => "[ok]",
            Level::Warn => "[!!]",
            Level::Fail => "[xx]",
        };
        s.push_str(&format!("{mark} {}\n", c.title));
        if let Some(h) = &c.hint {
            s.push_str(&format!("     -> {h}\n"));
        }
    }
    let fails = checks.iter().filter(|c| c.level == Level::Fail).count();
    let warns = checks.iter().filter(|c| c.level == Level::Warn).count();
    s.push_str(&match (fails, warns) {
        (0, 0) => "\nAll good.\n".to_string(),
        (0, w) => format!("\nNo problems that stop mirroring ({w} to look at).\n"),
        (f, _) => format!("\n{f} problem{} to fix.\n", if f == 1 { "" } else { "s" }),
    });
    s
}

fn tools() -> Vec<Check> {
    let adb_path = crate::tools::find("adb");
    let version = crate::tools::command(&adb_path).arg("version").output().ok().filter(|o| o.status.success());
    let mut out = vec![match version {
        Some(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            let line = text.lines().next().unwrap_or("adb").trim();
            check(Level::Ok, format!("{line} ({})", adb_path.display()), None)
        }
        None => check(
            Level::Fail,
            "adb was not found",
            Some(
                "install Android platform-tools (`brew install android-platform-tools`, or winget install Google.PlatformTools), or set the ADB environment variable to its path",
            ),
        ),
    }];
    let ffmpeg = crate::tools::find("ffmpeg");
    let found = crate::tools::command(&ffmpeg).arg("-version").output().is_ok_and(|o| o.status.success());
    out.push(if found {
        check(Level::Ok, format!("ffmpeg ({})", ffmpeg.display()), None)
    } else {
        check(
            Level::Warn,
            "ffmpeg was not found, so recording is unavailable",
            Some("install it (`brew install ffmpeg`, or winget install Gyan.FFmpeg), or set the FFMPEG environment variable"),
        )
    });
    out
}

/// Is `name` an executable on `PATH`?
#[cfg(not(any(target_os = "macos", windows)))]
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

/// The optional desktop programs Handset runs on Linux. `have` says whether one is installed; `wayland` picks
/// which clipboard program matters. Each missing one only turns a feature off, so these are warnings.
#[cfg(any(test, not(any(target_os = "macos", windows))))]
fn helper_checks(have: &dyn Fn(&str) -> bool, wayland: bool) -> Vec<Check> {
    let mut out = Vec::new();
    let any = |names: &[&str]| names.iter().any(|n| have(n));
    if !have("xdg-open") {
        out.push(check(
            Level::Warn,
            "xdg-open was not found, so links and captures cannot be opened from Handset",
            Some("install xdg-utils"),
        ));
    }
    let (clipboard, hint) = if wayland {
        (any(&["wl-copy"]) && any(&["wl-paste"]), "install wl-clipboard (Wayland session)")
    } else {
        (any(&["xclip", "xsel"]), "install xclip (or xsel) (X11 session)")
    };
    if !clipboard {
        out.push(check(
            Level::Warn,
            "no clipboard program was found, so clipboard sync and \"copy capture\" are off",
            Some(hint),
        ));
    }
    if !any(&["zenity", "kdialog"]) {
        out.push(check(
            Level::Warn,
            "neither zenity nor kdialog was found, so \"Type an address\", \"Pair with a code\" and error dialogs are off",
            Some("install zenity (GNOME and others) or kdialog (KDE)"),
        ));
    }
    out
}

/// Run every check (slow: it talks to adb and the phones).
pub fn checks() -> Vec<Check> {
    let mut checks = tools();
    #[cfg(not(any(target_os = "macos", windows)))]
    checks.extend(helper_checks(&on_path, std::env::var_os("WAYLAND_DISPLAY").is_some()));
    if checks[0].level == Level::Ok {
        match adb::list() {
            Err(e) => checks.push(check(Level::Fail, format!("could not run `adb devices`: {e}"), None)),
            Ok(list) if list.is_empty() => {
                let wifi = adb::mdns_services().is_ok_and(|s| s.iter().any(|s| s.kind == adb::ServiceKind::Connect));
                checks.push(no_phone(wifi));
            }
            Ok(list) => {
                for (serial, state) in list {
                    let p = if state == "device" { props(&serial) } else { Props::default() };
                    checks.extend(device_checks(&serial, &state, &p));
                }
            }
        }
    }
    checks.push(check(Level::Ok, format!("embedded scrcpy server {}", crate::server::SERVER_VERSION), None));
    checks
}

/// Run every check, print the report and return the process exit code.
pub fn run() -> i32 {
    let checks = checks();
    print!("{}", render(&checks));
    i32::from(checks.iter().any(|c| c.level == Level::Fail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(sdk: u32, awake: Option<bool>) -> Props {
        Props { model: "Pixel 7".into(), release: "14".into(), sdk, awake }
    }

    #[test]
    fn a_ready_phone_has_no_problems() {
        let c = device_checks("ABC123", "device", &props(34, Some(true)));
        assert!(c.iter().all(|c| c.level == Level::Ok));
        assert!(c[0].title.contains("Pixel 7") && c[0].title.contains("USB"));
    }

    #[test]
    fn wifi_phones_are_named_as_such() {
        let c = device_checks("192.168.1.5:5555", "device", &props(34, None));
        assert!(c[0].title.contains("Wi-Fi"));
    }

    #[test]
    fn old_android_and_a_sleeping_screen_are_called_out() {
        let c = device_checks("X", "device", &props(19, Some(false)));
        assert!(c.iter().any(|c| c.level == Level::Fail && c.title.contains("too old")));
        assert!(c.iter().any(|c| c.level == Level::Warn && c.title.contains("screen is off")));
    }

    #[test]
    fn unauthorized_and_offline_phones_say_what_to_do() {
        let u = device_checks("X", "unauthorized", &Props::default());
        assert_eq!(u[0].level, Level::Fail);
        assert!(u[0].hint.as_deref().unwrap().contains("Allow USB debugging"));
        let o = device_checks("X", "offline", &Props::default());
        assert!(o[0].hint.as_deref().unwrap().contains("kill-server"));
    }

    #[test]
    fn no_phone_depends_on_whether_one_is_on_wifi() {
        assert_eq!(no_phone(false).level, Level::Fail);
        assert_eq!(no_phone(true).level, Level::Warn);
    }

    #[test]
    fn a_phone_without_permission_points_at_udev_rules() {
        let c = device_checks("X", "no permissions", &Props::default());
        assert_eq!(c[0].level, Level::Fail);
        assert!(c[0].hint.as_deref().unwrap().contains("udev"));
    }

    #[test]
    fn missing_linux_helpers_are_warnings_that_name_what_to_install() {
        let none = helper_checks(&|_| false, false);
        assert_eq!(none.len(), 3);
        assert!(none.iter().all(|c| c.level == Level::Warn));
        assert!(none.iter().any(|c| c.hint.as_deref().is_some_and(|h| h.contains("xclip"))));
        // on Wayland the clipboard needs both halves of wl-clipboard
        let wl = helper_checks(&|n| ["xdg-open", "wl-copy", "zenity"].contains(&n), true);
        assert_eq!(wl.len(), 1);
        assert!(wl[0].hint.as_deref().unwrap().contains("wl-clipboard"));
        assert!(helper_checks(&|n| n != "kdialog" && n != "xsel", false).is_empty());
    }

    #[test]
    fn short_titles_drop_paths_only() {
        let c = |t: &str| check(Level::Ok, t, None);
        assert_eq!(
            c("Android Debug Bridge version 1.0.41 (/opt/x/adb)").short_title(),
            "Android Debug Bridge version 1.0.41"
        );
        assert_eq!(c("adb (C:\\tools\\adb.exe)").short_title(), "adb");
        assert_eq!(c("Android 14 (API 34)").short_title(), "Android 14 (API 34)");
    }

    #[test]
    fn wakefulness_comes_from_dumpsys() {
        assert_eq!(wakefulness("  mWakefulness=Awake\n"), Some(true));
        assert_eq!(wakefulness("  mWakefulness=Asleep\n"), Some(false));
        assert_eq!(wakefulness("nothing here"), None);
    }

    #[test]
    fn the_report_summarises_by_severity() {
        let ok = check(Level::Ok, "fine", None);
        let warn = check(Level::Warn, "hm", Some("do this"));
        let fail = check(Level::Fail, "bad", None);
        assert!(render(std::slice::from_ref(&ok)).contains("All good"));
        let r = render(&[ok.clone(), warn]);
        assert!(r.contains("[!!] hm") && r.contains("-> do this") && r.contains("No problems that stop"));
        assert!(render(&[ok, fail]).contains("1 problem to fix"));
    }
}
