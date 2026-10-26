//! Files dropped on the window: an APK is installed, anything else is copied to the phone's Downloads.

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

const LIMIT: Duration = Duration::from_secs(600);
const DOWNLOADS: &str = "/sdcard/Download";

/// adb handles one transfer at a time, and a drop of several files arrives as several events.
static BUSY: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq)]
pub enum Kind {
    Install,
    Push,
}

pub fn kind(path: &Path) -> Kind {
    match path.extension().and_then(|e| e.to_str()) {
        Some(e) if e.eq_ignore_ascii_case("apk") => Kind::Install,
        _ => Kind::Push,
    }
}

pub fn name(path: &Path) -> String {
    path.file_name().map_or_else(|| "FILE".into(), |n| n.to_string_lossy().into_owned())
}

/// Install or copy one file; the result is the sentence to show, or why it failed.
pub fn send(serial: &str, path: &Path) -> Result<String, String> {
    let _one_at_a_time = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    if !path.is_file() {
        return Err("NOT A FILE".into());
    }
    let name = name(path);
    let mut cmd = crate::adb::command(serial);
    let what = match kind(path) {
        Kind::Install => {
            cmd.args(["install", "-r"]).arg(path);
            "INSTALLED"
        }
        Kind::Push => {
            cmd.arg("push").arg(path).arg(format!("{DOWNLOADS}/{name}"));
            "COPIED TO DOWNLOADS"
        }
    };
    let out = crate::adb::output_within(cmd, LIMIT).map_err(|e| e.to_string())?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() && !text.contains("Failure") { Ok(format!("{what} {name}")) } else { Err(failure(&text)) }
}

/// The reason from adb's output: `Failure [INSTALL_FAILED_...]` or `adb: error: ...`.
fn failure(text: &str) -> String {
    let line = text
        .lines()
        .rev()
        .find(|l| l.contains("Failure") || l.contains("error"))
        .unwrap_or_else(|| text.lines().last().unwrap_or("FAILED"));
    let reason =
        line.split_once('[').map_or(line, |(_, r)| r.trim_end_matches(']')).trim_start_matches("adb: error:").trim();
    reason.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apks_install_and_everything_else_is_copied() {
        assert_eq!(kind(Path::new("/tmp/app.apk")), Kind::Install);
        assert_eq!(kind(Path::new("/tmp/APP.APK")), Kind::Install);
        assert_eq!(kind(Path::new("/tmp/photo.png")), Kind::Push);
        assert_eq!(kind(Path::new("/tmp/noextension")), Kind::Push);
        assert_eq!(kind(Path::new("/tmp/app.apk.zip")), Kind::Push);
    }

    #[test]
    fn failure_reasons_come_from_adbs_output() {
        let out =
            "Performing Streamed Install\nadb: failed to install a.apk: Failure [INSTALL_FAILED_VERSION_DOWNGRADE]\n";
        assert_eq!(failure(out), "INSTALL_FAILED_VERSION_DOWNGRADE");
        assert_eq!(
            failure("adb: error: failed to copy 'x' to '/sdcard/Download/x': remote couldn't create file\n"),
            "failed to copy 'x' to '/sdcard/Download/x': remote couldn't create file"
        );
        assert_eq!(failure(""), "FAILED");
    }
}
