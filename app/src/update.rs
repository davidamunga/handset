//! Looking for a newer release on GitHub. Package managers update their own installs; this is for
//! people who downloaded a zip. It asks GitHub's public API (through `curl`, which macOS and
//! Windows 10 and later ship) once on every start, sends nothing but a User-Agent, and can be
//! switched off in the menu.

use std::time::Duration;

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const LATEST: &str = "https://api.github.com/repos/davidamunga/handset/releases/latest";
pub const RELEASES: &str = "https://github.com/davidamunga/handset/releases/latest";
/// The running version. `HANDSET_UPDATE_CURRENT` pretends to be another one, to see the update notice.
pub fn current() -> String {
    std::env::var("HANDSET_UPDATE_CURRENT").unwrap_or_else(|_| CURRENT.to_string())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// Without the leading `v`.
    pub version: String,
    pub url: String,
}

/// Ask GitHub for the latest release.
pub fn latest() -> Result<Release, String> {
    let mut cmd = crate::tools::command("curl");
    cmd.args(["-sS", "-L", "--max-time", "5", "-H", "Accept: application/vnd.github+json", "-A"])
        .arg(format!("handset/{CURRENT}"))
        .arg(LATEST);
    let out = crate::adb::output_within(cmd, Duration::from_secs(8)).map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    parse_release(&String::from_utf8_lossy(&out.stdout))
}

fn parse_release(json: &str) -> Result<Release, String> {
    let tag = field(json, "tag_name").ok_or("no release found")?;
    let version = tag.trim_start_matches('v').to_string();
    parse_version(&version).ok_or_else(|| format!("unexpected version {tag}"))?;
    let url = field(json, "html_url").unwrap_or_else(|| RELEASES.to_string());
    Ok(Release { version, url })
}

/// The string value of `"name": "..."` in a JSON document (the first one; enough for GitHub's reply).
fn field(json: &str, name: &str) -> Option<String> {
    let at = json.find(&format!("\"{name}\""))? + name.len() + 2;
    let rest = json[at..].trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => out.push(chars.next()?),
            c => out.push(c),
        }
    }
    None
}

fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let mut parts = v.trim_start_matches('v').split(['.', '-', '+']).map(|p| p.parse::<u32>());
    Some((parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?))
}

/// Is `latest` a higher version than `current`?
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Should this start look for a newer release? Every start does when automatic checking is on, except a
/// scripted recording, which has nobody to tell.
pub fn check_on_start(auto: bool, unattended: bool) -> bool {
    auto && !unattended
}

/// The version Handset ran the last time, when this run is a newer one: the person has just updated. `None` for
/// the first run that records a version, for the same version and for an older one (a downgrade says nothing).
pub fn updated_from(previous: Option<&str>, current: &str) -> Option<String> {
    let previous = previous?;
    is_newer(current, previous).then(|| previous.to_string())
}

/// How long "Remind me later" waits.
pub const SNOOZE: u64 = 3 * 24 * 60 * 60;

/// Should the person hear about `latest`? A manual check always says so when there is something newer. An
/// automatic one stays quiet about a version they chose to skip, while a "remind me later" is running, and
/// about a version it already announced in this run.
pub fn announce(
    latest: &str,
    current: &str,
    skipped: Option<&str>,
    remind_after: Option<u64>,
    now: u64,
    told: Option<&str>,
    manual: bool,
) -> bool {
    if !is_newer(latest, current) {
        return false;
    }
    manual || (skipped != Some(latest) && told != Some(latest) && remind_after.is_none_or(|t| now >= t))
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.4", "0.2.0"));
        assert!(!is_newer("nonsense", "0.2.0"));
    }

    #[test]
    fn a_release_is_read_from_githubs_reply() {
        let json = r#"{"url":"https://api.github.com/x","html_url":"https://github.com/davidamunga/handset/releases/tag/v0.3.0",
            "tag_name": "v0.3.0","name":"v0.3.0","body":"notes with \"quotes\""}"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.version, "0.3.0");
        assert_eq!(r.url, "https://github.com/davidamunga/handset/releases/tag/v0.3.0");
        assert!(parse_release(r#"{"message":"Not Found"}"#).is_err());
        assert!(parse_release(r#"{"tag_name":"nightly"}"#).is_err());
    }

    #[test]
    fn a_newer_version_is_announced_unless_skipped_or_snoozed() {
        let go = |skipped, remind, manual| announce("0.5.0", "0.4.0", skipped, remind, 1_000, None, manual);
        assert!(go(None, None, false));
        assert!(!go(Some("0.5.0"), None, false), "a skipped version stays quiet");
        assert!(go(Some("0.4.9"), None, false), "skipping an older version does not hide a newer one");
        assert!(!go(None, Some(2_000), false), "a reminder still running stays quiet");
        assert!(go(None, Some(1_000), false), "and speaks up when its time comes");
        assert!(go(None, Some(900), false));
    }

    #[test]
    fn asking_yourself_always_answers() {
        assert!(announce("0.5.0", "0.4.0", Some("0.5.0"), Some(9_999), 1_000, Some("0.5.0"), true));
        assert!(!announce("0.4.0", "0.4.0", None, None, 1_000, None, true), "nothing newer is never announced");
        assert!(!announce("0.3.0", "0.4.0", None, None, 1_000, None, true));
    }

    #[test]
    fn a_newer_run_than_the_last_one_means_it_was_updated() {
        assert_eq!(updated_from(Some("0.6.0"), "0.6.1").as_deref(), Some("0.6.0"));
        assert_eq!(updated_from(Some("0.5.9"), "0.6.0").as_deref(), Some("0.5.9"));
        assert_eq!(updated_from(Some("0.9.0"), "0.10.0").as_deref(), Some("0.9.0"), "compared as numbers, not text");
        assert_eq!(updated_from(None, "0.6.1"), None, "the first run that records a version says nothing");
        assert_eq!(updated_from(Some("0.6.1"), "0.6.1"), None, "the same version");
        assert_eq!(updated_from(Some("0.7.0"), "0.6.1"), None, "an older version is not an update");
        assert_eq!(updated_from(Some("garbled"), "0.6.1"), None);
    }

    #[test]
    fn snoozing_waits_three_days() {
        assert_eq!(SNOOZE, 259_200);
    }

    #[test]
    fn a_version_is_announced_once_per_run() {
        assert!(!announce("0.5.0", "0.4.0", None, None, 1_000, Some("0.5.0"), false));
        assert!(announce("0.6.0", "0.4.0", None, None, 1_000, Some("0.5.0"), false), "a newer one is still news");
    }

    #[test]
    fn every_start_checks_unless_off_or_unattended() {
        assert!(check_on_start(true, false));
        assert!(!check_on_start(false, false), "switched off");
        assert!(!check_on_start(true, true), "a scripted recording");
    }

    #[test]
    fn this_build_has_a_version_that_parses() {
        assert!(parse_version(CURRENT).is_some());
    }
}
