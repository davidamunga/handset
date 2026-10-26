//! Looking for a newer release on GitHub. Package managers update their own installs; this is for
//! people who downloaded a zip. It asks GitHub's public API (through `curl`, which macOS and
//! Windows 10 and later ship) at most once a day, sends nothing but a User-Agent, and can be
//! switched off in the menu.

use std::time::Duration;

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const LATEST: &str = "https://api.github.com/repos/davidamunga/handset/releases/latest";
pub const RELEASES: &str = "https://github.com/davidamunga/handset/releases/latest";
/// The running version. `HANDSET_UPDATE_CURRENT` pretends to be another one, to see the update notice.
pub fn current() -> String {
    std::env::var("HANDSET_UPDATE_CURRENT").unwrap_or_else(|_| CURRENT.to_string())
}

/// How long to wait before the next automatic check.
const EVERY: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    /// Without the leading `v`.
    pub version: String,
    pub url: String,
}

/// Ask GitHub for the latest release.
pub fn latest() -> Result<Release, String> {
    let mut cmd = crate::tools::command("curl");
    cmd.args(["-sS", "-L", "--max-time", "10", "-H", "Accept: application/vnd.github+json", "-A"])
        .arg(format!("handset/{CURRENT}"))
        .arg(LATEST);
    let out = crate::adb::output_within(cmd, Duration::from_secs(15)).map_err(|e| e.to_string())?;
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

/// Is an automatic check due, given when the last one succeeded (Unix seconds)?
pub fn due(last_checked: Option<u64>, now: u64) -> bool {
    last_checked.is_none_or(|t| now.saturating_sub(t) >= EVERY)
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
    fn a_check_is_due_once_a_day() {
        let day = 24 * 60 * 60;
        assert!(due(None, 1_000_000));
        assert!(!due(Some(1_000_000), 1_000_000 + day - 1));
        assert!(due(Some(1_000_000), 1_000_000 + day));
        assert!(!due(Some(2_000_000), 1_000_000), "a clock set back does not trigger a check");
    }

    #[test]
    fn this_build_has_a_version_that_parses() {
        assert!(parse_version(CURRENT).is_some());
    }
}
