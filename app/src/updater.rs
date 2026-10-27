//! Updating Handset in place, for a downloaded app (macOS), an unzipped release (Windows) or an unpacked tarball
//! (Linux) and nothing else (see `install.rs`; ADRs 0024 and 0033).
//!
//! The steps, each of which can fail without touching the installed app:
//! 1. download `SHA256SUMS`, its signature and the zip (or tarball) into a temporary folder;
//! 2. check the signature against the keys built into the app, and the zip's SHA-256 against the signed list;
//! 3. unpack, and check that what was unpacked is the version that was announced and starts;
//! 4. write a small helper script. When Handset quits, the helper replaces the app and starts the new one, and
//!    puts the old one back if anything goes wrong.
//!
//! Nothing is installed for a release that is not signed: [`Failure::Unsigned`] lets the app fall back to
//! opening the download page.

use crate::install::Source;
use crate::verify;
use ed25519_dalek::VerifyingKey;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const RELEASE_BASE: &str = "https://github.com/davidamunga/handset/releases/download";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

impl Os {
    /// The release file to download: a zip, or on Linux the tarball for this machine (`uname -m`, which is
    /// what Rust calls `x86_64` and `aarch64` too).
    pub fn zip_name(self, version: &str) -> String {
        match self {
            Self::Mac => format!("Handset-{version}-macos.zip"),
            Self::Windows => format!("Handset-{version}-windows.zip"),
            Self::Linux => format!("Handset-{version}-linux-{}.tar.gz", std::env::consts::ARCH),
        }
    }

    pub fn sums_name(self) -> &'static str {
        match self {
            Self::Mac => "SHA256SUMS",
            Self::Windows => "SHA256SUMS-windows",
            Self::Linux => "SHA256SUMS-linux",
        }
    }
}

/// Which system's update this install can have done in place, if any.
pub fn os_for(source: Source) -> Option<Os> {
    match source {
        Source::MacApp => Some(Os::Mac),
        Source::WindowsZip => Some(Os::Windows),
        Source::LinuxTarball => Some(Os::Linux),
        _ => None,
    }
}

/// What to download for a version.
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub version: String,
    pub os: Os,
    base: String,
}

impl Plan {
    /// `HANDSET_UPDATE_BASE` replaces the folder the files are fetched from (a mirror, or a local folder as
    /// `file:///...` for trying an update out). Signatures are checked all the same.
    pub fn new(version: &str, os: Os) -> Self {
        let base = std::env::var("HANDSET_UPDATE_BASE")
            .ok()
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| format!("{RELEASE_BASE}/v{version}"));
        Self { version: version.to_string(), os, base: base.trim_end_matches('/').to_string() }
    }

    pub fn zip_name(&self) -> String {
        self.os.zip_name(&self.version)
    }

    pub fn zip_url(&self) -> String {
        format!("{}/{}", self.base, self.zip_name())
    }

    pub fn sums_url(&self) -> String {
        format!("{}/{}", self.base, self.os.sums_name())
    }

    pub fn sig_url(&self) -> String {
        format!("{}.sig", self.sums_url())
    }
}

#[derive(Debug, PartialEq)]
pub enum Failure {
    /// The release has no signature (or no signed list): it is announced but not installed in place.
    Unsigned,
    /// Handset cannot write where it is installed (the folder named), for example `/Applications` for a
    /// person who is not an administrator: it cannot replace itself there.
    NotWritable(String),
    Other(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsigned => write!(f, "this release is not signed"),
            Self::NotWritable(dir) => write!(f, "Handset cannot write to {dir}"),
            Self::Other(e) => write!(f, "{e}"),
        }
    }
}

fn other<E: std::fmt::Display>(e: E) -> Failure {
    Failure::Other(e.to_string())
}

// ---- download ---------------------------------------------------------------------------------------

/// The last `Content-Length` in a block of response headers (the last, as redirects each have their own).
pub fn parse_content_length(headers: &str) -> Option<u64> {
    headers
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case("content-length").then(|| v.trim().parse::<u64>().ok())?
        })
        .next_back()
}

fn curl() -> Command {
    let mut c = crate::tools::command("curl");
    c.args(["-sS", "-L", "--fail", "--max-time", "900", "-A"]).arg(format!("handset/{}", crate::update::CURRENT));
    c
}

/// How big `url` is, when the server says.
fn size_of(url: &str) -> Option<u64> {
    let out = crate::adb::output_within(
        {
            let mut c = curl();
            c.arg("-I").arg(url);
            c
        },
        Duration::from_secs(20),
    )
    .ok()?;
    parse_content_length(&String::from_utf8_lossy(&out.stdout))
}

/// Download `url` to `dest`, calling `progress` with 0..=1 now and then when the size is known.
pub fn fetch(url: &str, dest: &Path, progress: &dyn Fn(f32)) -> Result<(), String> {
    let total = size_of(url).filter(|t| *t > 0);
    let mut cmd = curl();
    cmd.arg("-o").arg(dest).arg(url).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("cannot run curl: {e}"))?;
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => {
                if status.success() {
                    progress(1.0);
                    return Ok(());
                }
                let mut err = String::new();
                if let Some(mut s) = child.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut s, &mut err);
                }
                let _ = std::fs::remove_file(dest);
                return Err(format!("download failed: {}", err.trim()));
            }
            None => {
                if let (Some(total), Ok(meta)) = (total, std::fs::metadata(dest)) {
                    progress((meta.len() as f32 / total as f32).min(0.99));
                }
                std::thread::sleep(Duration::from_millis(120));
            }
        }
    }
}

// ---- check ------------------------------------------------------------------------------------------

/// Check the download in `dir` (the zip, `SHA256SUMS` and its signature) against `keys`.
pub fn verify_download(dir: &Path, plan: &Plan, keys: &[VerifyingKey]) -> Result<(), Failure> {
    let sums_name = plan.os.sums_name();
    let sums = std::fs::read(dir.join(sums_name)).map_err(other)?;
    let sig = std::fs::read(dir.join(format!("{sums_name}.sig"))).map_err(|_| Failure::Unsigned)?;
    if !verify::verify_with(keys, &sums, &sig) {
        return Err(Failure::Other(format!("the signature on {sums_name} does not verify")));
    }
    let text = String::from_utf8(sums).map_err(|_| other("the signed list is not text"))?;
    let zip = plan.zip_name();
    let want = verify::expected_hash(&text, &zip).ok_or_else(|| other(format!("{zip} is not in the signed list")))?;
    let got = verify::sha256_file(&dir.join(&zip)).map_err(other)?;
    if got != want {
        return Err(other(format!("{zip} does not match the signed list")));
    }
    Ok(())
}

// ---- unpack -----------------------------------------------------------------------------------------

/// Unpack `zip` into `dest` and return the new app: the `.app` bundle (macOS) or the folder that holds
/// `handset.exe` (Windows) or `handset` (Linux).
pub fn extract(zip: &Path, dest: &Path, os: Os) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut cmd = match os {
        // ditto keeps the code signature and extended attributes that a plain unzip drops
        Os::Mac => {
            let mut c = Command::new("/usr/bin/ditto");
            c.args(["-x", "-k"]).arg(zip).arg(dest);
            c
        }
        Os::Windows => {
            let mut c = crate::tools::command("tar");
            c.arg("-xf").arg(zip).arg("-C").arg(dest);
            c
        }
        Os::Linux => {
            let mut c = Command::new("tar");
            c.arg("-xzf").arg(zip).arg("-C").arg(dest);
            c
        }
    };
    let out = cmd.stdin(Stdio::null()).output().map_err(|e| format!("cannot unpack the download: {e}"))?;
    if !out.status.success() {
        return Err(format!("cannot unpack the download: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    locate(dest, os)
}

fn locate(dest: &Path, os: Os) -> Result<PathBuf, String> {
    let entries: Vec<PathBuf> =
        std::fs::read_dir(dest).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).collect();
    match os {
        Os::Mac => entries
            .into_iter()
            .find(|p| p.extension().is_some_and(|e| e == "app") && p.is_dir())
            .ok_or_else(|| "the download holds no app".to_string()),
        Os::Windows | Os::Linux => {
            let name = program_name(os);
            if dest.join(name).is_file() {
                return Ok(dest.to_path_buf());
            }
            entries.into_iter().find(|p| p.join(name).is_file()).ok_or_else(|| format!("the download holds no {name}"))
        }
    }
}

fn program_name(os: Os) -> &'static str {
    if os == Os::Windows { "handset.exe" } else { "handset" }
}

/// The program inside an unpacked app.
fn program(app: &Path, os: Os) -> PathBuf {
    match os {
        Os::Mac => app.join("Contents/MacOS/handset"),
        Os::Windows | Os::Linux => app.join(program_name(os)),
    }
}

/// `CFBundleShortVersionString` of a macOS app, read from its `Info.plist`.
pub fn bundle_version(app: &Path) -> Option<String> {
    let text = std::fs::read_to_string(app.join("Contents/Info.plist")).ok()?;
    let after = &text[text.find("<key>CFBundleShortVersionString</key>")?..];
    let start = after.find("<string>")? + "<string>".len();
    let end = after[start..].find("</string>")?;
    Some(after[start..start + end].trim().to_string())
}

/// Is what was unpacked really version `version`, and does it start? `codesign` also checks a macOS app's
/// signature seal (`false` in tests, whose stand-in apps are not signed).
pub fn check_package(app: &Path, os: Os, version: &str, codesign: bool) -> Result<(), String> {
    if os == Os::Mac {
        match bundle_version(app) {
            Some(v) if v == version => {}
            Some(v) => return Err(format!("the download is Handset {v}, not {version}")),
            None => return Err("the download has no version".into()),
        }
        if codesign {
            let out = Command::new("/usr/bin/codesign")
                .args(["--verify", "--deep", "--strict"])
                .arg(app)
                .output()
                .map_err(|e| format!("cannot check the signature seal: {e}"))?;
            if !out.status.success() {
                return Err(format!(
                    "the app's signature seal is broken: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
        }
    }
    let exe = program(app, os);
    if !exe.is_file() {
        return Err("the download has no program".into());
    }
    let out = Command::new(&exe)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("the new version does not start: {e}"))?;
    let said = String::from_utf8_lossy(&out.stdout);
    if !out.status.success() || said.trim() != format!("handset {version}") {
        return Err(format!("the new version says {:?}, not handset {version}", said.trim()));
    }
    Ok(())
}

// ---- replace ----------------------------------------------------------------------------------------

/// What the helper does once Handset has quit.
#[derive(Debug, Clone, PartialEq)]
pub struct Swap {
    pub os: Os,
    /// The process to wait for.
    pub pid: u32,
    /// The installed app (macOS: the `.app` bundle), its folder (Windows) or the program itself (Linux).
    pub current: PathBuf,
    /// The unpacked new app, or its folder.
    pub new: PathBuf,
    /// Where the helper writes what it did.
    pub log: PathBuf,
    /// The command that starts the new app: `open` on macOS, nothing (the program itself) on Linux; tests use
    /// `true` and `false`.
    pub relaunch: String,
}

/// `'...'` for a POSIX shell, whatever the text holds.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// `'...'` for PowerShell: single quotes inside are doubled.
pub fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The helper script: wait for Handset to quit, put the new app in place, start it, and put the old one back
/// if the swap or the start fails.
pub fn script(swap: &Swap) -> String {
    match swap.os {
        Os::Mac => mac_script(swap),
        Os::Windows => windows_script(swap),
        Os::Linux => linux_script(swap),
    }
}

fn mac_script(s: &Swap) -> String {
    format!(
        r#"#!/bin/sh
# Handset update helper: waits for Handset to quit, swaps the app, starts the new one.
exec >>{log} 2>&1
echo "update: waiting for $(date)"
pid={pid}
app={current}
new={new}
tmp="$app.update-new"
old="$app.update-old"
n=0
while kill -0 "$pid" 2>/dev/null && [ "$n" -lt 150 ]; do sleep 0.2; n=$((n + 1)); done
rm -rf "$tmp" "$old"
# copy next to the installed app, so that putting it in place is a rename on one volume
if ! ditto "$new" "$tmp"; then echo "update: could not copy the new app"; rm -rf "$tmp"; exit 1; fi
if ! mv "$app" "$old"; then echo "update: could not move the old app aside"; rm -rf "$tmp"; exit 1; fi
if ! mv "$tmp" "$app"; then
  echo "update: could not put the new app in place; restoring"
  mv "$old" "$app"; {relaunch} "$app"; exit 1
fi
xattr -dr com.apple.quarantine "$app" 2>/dev/null
if {relaunch} "$app"; then
  rm -rf "$old"
  echo "update: done"
else
  echo "update: the new app did not start; restoring"
  rm -rf "$app"; mv "$old" "$app"; {relaunch} "$app"; exit 1
fi
"#,
        log = sh_quote(&s.log.to_string_lossy()),
        pid = s.pid,
        current = sh_quote(&s.current.to_string_lossy()),
        new = sh_quote(&s.new.to_string_lossy()),
        relaunch = s.relaunch,
    )
}

/// Only the program is swapped, because an unpacked tarball may have been copied into a shared folder such as
/// `~/.local/bin`; the tarball's other files are refreshed only where they already sit next to it.
fn linux_script(s: &Swap) -> String {
    format!(
        r#"#!/bin/sh
# Handset update helper: waits for Handset to quit, swaps the program, starts the new one.
exec >>{log} 2>&1
echo "update: waiting for $(date)"
pid={pid}
app={current}
new={new}
dir=$(dirname "$app")
tmp="$app.update-new"
old="$app.update-old"
# a program that cannot run (a missing library, a newer glibc) exits at once; Handset itself keeps running
start() {{
  {relaunch} "$1" </dev/null >/dev/null 2>&1 &
  p=$!
  sleep 1
  if kill -0 "$p" 2>/dev/null; then return 0; fi
  wait "$p"
}}
n=0
while kill -0 "$pid" 2>/dev/null && [ "$n" -lt 150 ]; do sleep 0.2; n=$((n + 1)); done
rm -f "$tmp" "$old"
# copy next to the installed program, so that putting it in place is a rename on one file system
if ! cp "$new/handset" "$tmp" || ! chmod 755 "$tmp"; then echo "update: could not copy the new program"; rm -f "$tmp"; exit 1; fi
if ! mv "$app" "$old"; then echo "update: could not move the old program aside"; rm -f "$tmp"; exit 1; fi
if ! mv "$tmp" "$app"; then
  echo "update: could not put the new program in place; restoring"
  mv "$old" "$app"; start "$app"; exit 1
fi
if start "$app"; then
  rm -f "$old"
  for f in handset.desktop handset.png LICENSE NOTICE; do
    if [ -f "$dir/$f" ] && [ -f "$new/$f" ]; then cp "$new/$f" "$dir/$f"; fi
  done
  echo "update: done"
else
  echo "update: the new program did not start; restoring"
  rm -f "$app"; mv "$old" "$app"; start "$app"; exit 1
fi
"#,
        log = sh_quote(&s.log.to_string_lossy()),
        pid = s.pid,
        current = sh_quote(&s.current.to_string_lossy()),
        new = sh_quote(&s.new.to_string_lossy()),
        relaunch = s.relaunch,
    )
}

fn windows_script(s: &Swap) -> String {
    format!(
        r#"# Handset update helper: waits for Handset to quit, copies the new files over it, starts it again.
$ErrorActionPreference = 'Stop'
Start-Transcript -Path {log} -Append | Out-Null
$dir = {current}
$new = {new}
$exe = Join-Path $dir 'handset.exe'
$backup = Join-Path $dir '.update-backup'

# CreateProcess (no shell), so a program that cannot run fails here and not silently
function Start-Handset {{
  $psi = New-Object System.Diagnostics.ProcessStartInfo
  $psi.FileName = $exe
  $psi.WorkingDirectory = $dir
  $psi.UseShellExecute = $false
{arguments}  $p = [System.Diagnostics.Process]::Start($psi)
  if ($null -eq $p) {{ throw 'the program did not start' }}
  # a program that cannot run exits at once with an error; Handset itself keeps running
  if ($p.WaitForExit(3000) -and $p.ExitCode -ne 0) {{ throw ('the program exited with code ' + $p.ExitCode) }}
}}

function Restore-Old {{
  Write-Output 'update: restoring the old version'
  Copy-Item (Join-Path $backup 'handset.exe') $exe -Force
  try {{ Start-Handset }} catch {{ Write-Output ('update: the old version did not start: ' + $_.Exception.Message) }}
  exit 1
}}

try {{ Wait-Process -Id {pid} -Timeout 30 -ErrorAction SilentlyContinue }} catch {{ }}
Remove-Item -Recurse -Force $backup -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $backup | Out-Null
Copy-Item $exe (Join-Path $backup 'handset.exe') -Force

$copied = $false
for ($i = 0; $i -lt 15 -and -not $copied; $i++) {{
  try {{
    Copy-Item -Path (Join-Path $new '*') -Destination $dir -Recurse -Force
    $copied = $true
  }} catch {{
    Write-Output ('update: could not copy the new files yet: ' + $_.Exception.Message)
    Start-Sleep -Seconds 1
  }}
}}
if (-not $copied) {{ Restore-Old }}

$started = $false
try {{
  Start-Handset
  $started = $true
}} catch {{
  Write-Output ('update: the new version did not start: ' + $_.Exception.Message)
}}
if (-not $started) {{ Restore-Old }}

try {{ Remove-Item -Recurse -Force $backup }} catch {{ Write-Output ('update: could not remove the backup: ' + $_.Exception.Message) }}
Write-Output 'update: done'
"#,
        log = ps_quote(&s.log.to_string_lossy()),
        current = ps_quote(&s.current.to_string_lossy()),
        new = ps_quote(&s.new.to_string_lossy()),
        pid = s.pid,
        arguments = if s.relaunch.is_empty() {
            String::new()
        } else {
            format!("  $psi.Arguments = {}\n", ps_quote(&s.relaunch))
        },
    )
}

/// Write the helper next to `dir` and start it, detached, so that it outlives Handset.
pub fn spawn_helper(swap: &Swap, dir: &Path) -> Result<(), String> {
    let text = script(swap);
    match swap.os {
        Os::Mac | Os::Linux => spawn_sh(&text, dir),
        Os::Windows => spawn_windows(&text, dir),
    }
}

#[cfg(unix)]
fn spawn_sh(text: &str, dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::process::CommandExt;
    let path = dir.join("update.sh");
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    Command::new("/bin/sh")
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("cannot start the update helper: {e}"))
}

#[cfg(not(unix))]
fn spawn_sh(_text: &str, _dir: &Path) -> Result<(), String> {
    Err("not a macOS or Linux update".into())
}

#[cfg(windows)]
fn spawn_windows(text: &str, dir: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let path = dir.join("update.ps1");
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
    Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"])
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x0000_0008 | 0x0000_0200 | 0x0800_0000)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("cannot start the update helper: {e}"))
}

#[cfg(not(windows))]
fn spawn_windows(_text: &str, _dir: &Path) -> Result<(), String> {
    Err("not a Windows update".into())
}

/// The installed app that would be replaced: the `.app` bundle holding `exe` (macOS), the folder holding it
/// (Windows), or `exe` itself (Linux).
pub fn install_root(exe: &Path, os: Os) -> Option<PathBuf> {
    match os {
        Os::Windows => exe.parent().map(Path::to_path_buf),
        Os::Linux => exe.parent().map(|_| exe.to_path_buf()),
        Os::Mac => exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf),
    }
}

/// Can files be created next to the installed app? Replacing an app in `/Applications` needs that.
pub fn can_replace(current: &Path) -> bool {
    let Some(parent) = current.parent() else { return false };
    let probe = parent.join(".handset-update-write-test");
    let ok = std::fs::File::create(&probe).is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

// ---- tidying up -------------------------------------------------------------------------------------

/// The folders `prepare` leaves in `tmp` (`handset-update-<pid>`: the download, the unpacked app and the
/// helper's log) that are older than `max_age`, so they can be removed. A folder that is still in use is
/// recent, and the one for `current_pid` is never listed.
pub fn stale_work_dirs(tmp: &Path, current_pid: u32, max_age: Duration) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(tmp) else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let pid = name.strip_prefix("handset-update-")?.parse::<u32>().ok()?;
            let old = e.metadata().ok()?.modified().ok()?.elapsed().is_ok_and(|age| age >= max_age);
            (pid != current_pid && e.path().is_dir() && old).then(|| e.path())
        })
        .collect()
}

/// Remove old update folders from the temporary folder. Logs are kept for a week.
pub fn clean_stale() {
    for dir in stale_work_dirs(&std::env::temp_dir(), std::process::id(), Duration::from_secs(7 * 24 * 60 * 60)) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// ---- the whole thing --------------------------------------------------------------------------------

/// Everything up to, but not including, replacing the app: download, check, unpack, check again. `work` is a
/// folder to use for the files; on success the returned [`Swap`] says what the helper should do.
pub fn prepare(
    version: &str,
    source: Source,
    exe: &Path,
    work: &Path,
    keys: &[VerifyingKey],
    codesign: bool,
    progress: &dyn Fn(f32),
) -> Result<Swap, Failure> {
    let os = os_for(source).ok_or_else(|| other("this copy of Handset is updated another way"))?;
    let current = install_root(exe, os).ok_or_else(|| other("cannot tell where Handset is installed"))?;
    if !can_replace(&current) {
        return Err(Failure::NotWritable(current.parent().unwrap_or(&current).display().to_string()));
    }
    let plan = Plan::new(version, os);
    std::fs::create_dir_all(work).map_err(other)?;
    if let Err(e) = fetch(&plan.sums_url(), &work.join(os.sums_name()), &|_| {}) {
        // releases before Linux was one have no Linux list at all: send those to the download page
        return Err(if os == Os::Linux { Failure::Unsigned } else { other(e) });
    }
    if fetch(&plan.sig_url(), &work.join(format!("{}.sig", os.sums_name())), &|_| {}).is_err() {
        // no signature file at all: the release was made without the signing key
        return Err(Failure::Unsigned);
    }
    fetch(&plan.zip_url(), &work.join(plan.zip_name()), &|p| progress(p * 0.9)).map_err(other)?;
    verify_download(work, &plan, keys)?;
    progress(0.92);
    let new = extract(&work.join(plan.zip_name()), &work.join("new"), os).map_err(other)?;
    check_package(&new, os, version, codesign).map_err(other)?;
    progress(1.0);
    let relaunch = match os {
        Os::Mac => "open".to_string(),
        Os::Windows | Os::Linux => String::new(),
    };
    Ok(Swap { os, pid: std::process::id(), current, new, log: work.join("update.log"), relaunch })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_files_to_fetch_are_named_for_the_version_and_system() {
        let mac = Plan { version: "0.5.0".into(), os: Os::Mac, base: "https://x/v0.5.0".into() };
        assert_eq!(mac.zip_url(), "https://x/v0.5.0/Handset-0.5.0-macos.zip");
        assert_eq!(mac.sums_url(), "https://x/v0.5.0/SHA256SUMS");
        assert_eq!(mac.sig_url(), "https://x/v0.5.0/SHA256SUMS.sig");
        let win = Plan { version: "0.5.0".into(), os: Os::Windows, base: "https://x".into() };
        assert_eq!(win.zip_url(), "https://x/Handset-0.5.0-windows.zip");
        assert_eq!(win.sig_url(), "https://x/SHA256SUMS-windows.sig");
        let linux = Plan { version: "0.5.0".into(), os: Os::Linux, base: "https://x".into() };
        let arch = std::env::consts::ARCH;
        assert_eq!(linux.zip_url(), format!("https://x/Handset-0.5.0-linux-{arch}.tar.gz"));
        assert_eq!(linux.sums_url(), "https://x/SHA256SUMS-linux");
        assert_eq!(linux.sig_url(), "https://x/SHA256SUMS-linux.sig");
    }

    #[test]
    fn only_a_downloaded_app_an_unzipped_release_or_an_unpacked_tarball_updates_itself() {
        use crate::install::{ArchPackage, AurHelper};
        assert_eq!(os_for(Source::MacApp), Some(Os::Mac));
        assert_eq!(os_for(Source::WindowsZip), Some(Os::Windows));
        assert_eq!(os_for(Source::LinuxTarball), Some(Os::Linux));
        for s in [
            Source::Homebrew,
            Source::Scoop,
            Source::Winget,
            Source::Arch(ArchPackage::HandsetBin, Some(AurHelper::Yay)),
            Source::Arch(ArchPackage::Handset, None),
            Source::Cargo,
            Source::Nix,
            Source::Other,
        ] {
            assert_eq!(os_for(s), None, "{s:?}");
        }
    }

    #[test]
    fn the_size_is_the_last_content_length_after_redirects() {
        let h = "HTTP/2 302\r\ncontent-length: 0\r\nlocation: x\r\n\r\nHTTP/2 200\r\nContent-Length: 12345\r\n\r\n";
        assert_eq!(parse_content_length(h), Some(12345));
        assert_eq!(parse_content_length("HTTP/2 200\r\n"), None);
        assert_eq!(parse_content_length("content-length: soon"), None);
    }

    #[test]
    fn quoting_survives_spaces_and_quotes() {
        assert_eq!(sh_quote("/Applications/Handset.app"), "'/Applications/Handset.app'");
        assert_eq!(sh_quote("it's here"), r"'it'\''s here'");
        assert_eq!(sh_quote("a $b `c`"), "'a $b `c`'");
        assert_eq!(ps_quote(r"C:\Users\O'Brien\Handset"), r"'C:\Users\O''Brien\Handset'");
    }

    fn swap(os: Os) -> Swap {
        Swap {
            os,
            pid: 4242,
            current: PathBuf::from(match os {
                Os::Mac => "/Applications/Handset.app",
                Os::Windows => r"C:\Tools\Handset",
                Os::Linux => "/home/me/.local/bin/handset",
            }),
            new: PathBuf::from("/tmp/new"),
            log: PathBuf::from("/tmp/update.log"),
            relaunch: "open".into(),
        }
    }

    #[test]
    fn the_mac_helper_waits_swaps_and_can_roll_back() {
        let s = script(&swap(Os::Mac));
        assert!(s.starts_with("#!/bin/sh"));
        assert!(
            s.contains("pid=4242") && s.contains("app='/Applications/Handset.app'") && s.contains("new='/tmp/new'")
        );
        assert!(s.contains("kill -0"), "it waits for Handset to quit");
        let at = |needle: &str| s.find(needle).unwrap_or_else(|| panic!("missing {needle}"));
        assert!(
            at("ditto") < at("mv \"$app\" \"$old\"") && at("mv \"$app\" \"$old\"") < at("mv \"$tmp\" \"$app\""),
            "copy, move aside, put in place"
        );
        assert!(s.contains("mv \"$old\" \"$app\""), "it can put the old app back");
        assert!(s.contains("com.apple.quarantine"));
    }

    #[test]
    fn the_linux_helper_swaps_only_the_program_and_can_roll_back() {
        let s = script(&Swap { relaunch: String::new(), ..swap(Os::Linux) });
        assert!(s.starts_with("#!/bin/sh"));
        assert!(
            s.contains("pid=4242") && s.contains("app='/home/me/.local/bin/handset'") && s.contains("new='/tmp/new'")
        );
        assert!(s.contains("kill -0 \"$pid\""), "it waits for Handset to quit");
        let at = |needle: &str| s.find(needle).unwrap_or_else(|| panic!("missing {needle}"));
        assert!(
            at("cp \"$new/handset\" \"$tmp\"") < at("mv \"$app\" \"$old\"")
                && at("mv \"$app\" \"$old\"") < at("mv \"$tmp\" \"$app\""),
            "copy, move aside, put in place"
        );
        assert!(s.contains("mv \"$old\" \"$app\""), "it can put the old program back");
        assert!(s.contains("  \"$1\" </dev/null"), "with no relaunch command the program starts itself");
        assert!(!s.contains("rm -rf"), "nothing but the program's own files is removed");
    }

    #[test]
    fn the_windows_helper_waits_backs_up_copies_and_can_roll_back() {
        let s = script(&swap(Os::Windows));
        assert!(s.contains("Wait-Process -Id 4242"));
        assert!(s.contains(r"$dir = 'C:\Tools\Handset'"));
        let at = |needle: &str| s.find(needle).unwrap_or_else(|| panic!("missing {needle}"));
        assert!(at("$backup") < at("Copy-Item -Path (Join-Path $new '*')"), "the old program is saved before the copy");
        assert!(s.contains("restoring the old version"));
        assert!(s.contains("$psi.Arguments = 'open'"), "the relaunch arguments are passed on");
        let none = script(&Swap { relaunch: String::new(), ..swap(Os::Windows) });
        assert!(!none.contains("$psi.Arguments"), "no arguments by default");
        assert!(
            s.contains("UseShellExecute = $false"),
            "a program that cannot run fails instead of being handed to the shell"
        );
    }

    #[test]
    fn the_installed_app_is_found_from_the_program_path() {
        assert_eq!(
            install_root(Path::new("/Applications/Handset.app/Contents/MacOS/handset"), Os::Mac),
            Some(PathBuf::from("/Applications/Handset.app"))
        );
        assert_eq!(install_root(Path::new("/usr/local/bin/handset"), Os::Mac), None);
        assert_eq!(
            install_root(Path::new("/home/me/.local/bin/handset"), Os::Linux),
            Some(PathBuf::from("/home/me/.local/bin/handset"))
        );
        assert_eq!(
            install_root(Path::new("/Tools/Handset-0.4.0/handset.exe"), Os::Windows),
            Some(PathBuf::from("/Tools/Handset-0.4.0"))
        );
    }

    /// Held by every test that sets `HANDSET_UPDATE_BASE`, which the whole test process shares.
    #[cfg(unix)]
    static BASE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("handset-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_folder_that_cannot_be_written_is_not_replaceable() {
        let dir = temp_dir("writable");
        assert!(can_replace(&dir.join("Handset.app")));
        assert!(!can_replace(&dir.join("no/such/folder/Handset.app")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn only_old_update_folders_of_other_runs_are_cleaned_up() {
        let tmp = temp_dir("clean");
        for name in
            ["handset-update-111", "handset-update-222", "handset-update-333", "something-else", "handset-update-abc"]
        {
            std::fs::create_dir_all(tmp.join(name)).unwrap();
        }
        std::fs::write(tmp.join("handset-update-444"), "a file, not a folder").unwrap();
        let names = |max_age| {
            let mut v: Vec<String> = stale_work_dirs(&tmp, 333, max_age)
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect();
            v.sort();
            v
        };
        // everything just made is old enough when no age is asked for: of those, only folders named for a
        // pid and not the running copy's are listed
        assert_eq!(names(Duration::ZERO), vec!["handset-update-111", "handset-update-222"]);
        // ...and nothing is old enough when a day is asked for
        assert!(names(Duration::from_secs(24 * 60 * 60)).is_empty(), "a folder made just now is not stale");
        assert!(stale_work_dirs(&tmp.join("missing"), 1, Duration::ZERO).is_empty());
        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn the_bundle_version_is_read_from_the_plist() {
        let dir = temp_dir("plist");
        let app = dir.join("Handset.app");
        std::fs::create_dir_all(app.join("Contents")).unwrap();
        std::fs::write(
            app.join("Contents/Info.plist"),
            "<dict>\n  <key>CFBundleVersion</key><string>9</string>\n  <key>CFBundleShortVersionString</key><string> 1.2.3 </string>\n</dict>",
        )
        .unwrap();
        assert_eq!(bundle_version(&app).as_deref(), Some("1.2.3"));
        assert_eq!(bundle_version(&dir.join("nothing.app")), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    // ---- on macOS the whole path runs for real, in a temporary folder, against a local "release" ----------

    #[cfg(target_os = "macos")]
    mod mac {
        use super::*;
        use ed25519_dalek::{Signer, SigningKey};
        use std::os::unix::fs::PermissionsExt;

        const VERSION: &str = "9.9.9";

        /// A stand-in app: `Handset.app` whose program prints its version and leaves a marker.
        fn make_app(parent: &Path, version: &str, marker: &str) -> PathBuf {
            let app = parent.join("Handset.app");
            std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
            std::fs::write(
                app.join("Contents/Info.plist"),
                format!("<dict><key>CFBundleShortVersionString</key><string>{version}</string></dict>"),
            )
            .unwrap();
            let exe = app.join("Contents/MacOS/handset");
            std::fs::write(&exe, format!("#!/bin/sh\n[ \"$1\" = --version ] && echo \"handset {version}\"\n")).unwrap();
            std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::write(app.join("Contents/marker.txt"), marker).unwrap();
            app
        }

        fn zip_app(app: &Path, zip: &Path) {
            assert!(
                Command::new("/usr/bin/ditto")
                    .args(["-c", "-k", "--keepParent"])
                    .arg(app)
                    .arg(zip)
                    .status()
                    .unwrap()
                    .success()
            );
        }

        /// A local release folder: the zip, SHA256SUMS and (when `key` is given) its signature.
        fn make_release(dir: &Path, app: &Path, key: Option<&SigningKey>) {
            std::fs::create_dir_all(dir).unwrap();
            let zip = dir.join(Os::Mac.zip_name(VERSION));
            zip_app(app, &zip);
            let sums = format!("{}  {}\n", verify::sha256_file(&zip).unwrap(), Os::Mac.zip_name(VERSION));
            std::fs::write(dir.join("SHA256SUMS"), &sums).unwrap();
            if let Some(k) = key {
                std::fs::write(dir.join("SHA256SUMS.sig"), k.sign(sums.as_bytes()).to_bytes()).unwrap();
            }
        }

        fn key(seed: u8) -> SigningKey {
            SigningKey::from_bytes(&[seed; 32])
        }

        /// `prepare` against a local release, with `HANDSET_UPDATE_BASE` pointing at it. Tests that set it run
        /// one at a time (the variable is shared).
        fn prepare_from(
            release: &Path,
            install: &Path,
            work: &Path,
            keys: &[VerifyingKey],
            progress: &dyn Fn(f32),
        ) -> Result<Swap, Failure> {
            let _guard = BASE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            // SAFETY: the lock above serialises every test that touches this variable
            unsafe { std::env::set_var("HANDSET_UPDATE_BASE", format!("file://{}", release.display())) };
            let exe = install.join("Handset.app/Contents/MacOS/handset");
            let result = prepare(VERSION, Source::MacApp, &exe, work, keys, false, progress);
            unsafe { std::env::remove_var("HANDSET_UPDATE_BASE") };
            result
        }

        fn run_helper(swap: &Swap) {
            // wait for no one: the pid of a process that has already ended
            let mut done = swap.clone();
            done.pid = 999_999;
            // next to the log, so that tests running side by side never share a script
            let script = swap.log.parent().unwrap().join("update.sh");
            std::fs::write(&script, super::super::script(&done)).unwrap();
            let _ = Command::new("/bin/sh").arg(&script).status();
        }

        #[test]
        fn a_signed_release_is_downloaded_checked_unpacked_and_swapped_in() {
            let root = temp_dir("mac-e2e");
            let (release, install, work) = (root.join("release"), root.join("Applications"), root.join("work"));
            std::fs::create_dir_all(&install).unwrap();
            let old = make_app(&install, "1.0.0", "old");
            let staging = root.join("staging");
            std::fs::create_dir_all(&staging).unwrap();
            let new_app = make_app(&staging, VERSION, "new");
            let k = key(11);
            make_release(&release, &new_app, Some(&k));

            let progress = std::sync::Mutex::new(Vec::new());
            let swap =
                prepare_from(&release, &install, &work, &[k.verifying_key()], &|p| progress.lock().unwrap().push(p))
                    .expect("the update is prepared");
            assert_eq!(swap.current, old);
            assert!(swap.new.ends_with("Handset.app") && swap.new.join("Contents/marker.txt").is_file());
            let seen = progress.lock().unwrap().clone();
            assert!(seen.last().is_some_and(|p| (*p - 1.0).abs() < 1e-6), "progress ends at 1: {seen:?}");
            assert!(seen.windows(2).all(|w| w[0] <= w[1] + 1e-6), "progress never goes back: {seen:?}");
            assert_eq!(
                std::fs::read_to_string(old.join("Contents/marker.txt")).unwrap(),
                "old",
                "nothing is replaced yet"
            );

            let mut swap = swap;
            swap.relaunch = "true".into();
            run_helper(&swap);
            assert_eq!(
                std::fs::read_to_string(old.join("Contents/marker.txt")).unwrap(),
                "new",
                "the new app is in place"
            );
            assert!(
                !install.join("Handset.app.update-old").exists() && !install.join("Handset.app.update-new").exists(),
                "no leftovers"
            );
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn a_failed_start_puts_the_old_app_back() {
            let root = temp_dir("mac-rollback");
            let (install, staging) = (root.join("Applications"), root.join("staging"));
            std::fs::create_dir_all(&install).unwrap();
            std::fs::create_dir_all(&staging).unwrap();
            let old = make_app(&install, "1.0.0", "old");
            let new_app = make_app(&staging, VERSION, "new");
            let swap = Swap {
                os: Os::Mac,
                pid: 999_999,
                current: old.clone(),
                new: new_app,
                log: root.join("update.log"),
                relaunch: "false".into(), // the new app "does not start"
            };
            run_helper(&swap);
            assert_eq!(std::fs::read_to_string(old.join("Contents/marker.txt")).unwrap(), "old", "the old app is back");
            let log = std::fs::read_to_string(root.join("update.log")).unwrap_or_default();
            assert!(log.contains("restoring"), "the helper says what it did: {log}");
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn an_unsigned_release_is_not_installed() {
            let root = temp_dir("mac-unsigned");
            let (release, install, work) = (root.join("release"), root.join("Applications"), root.join("work"));
            std::fs::create_dir_all(&install).unwrap();
            make_app(&install, "1.0.0", "old");
            let staging = root.join("staging");
            std::fs::create_dir_all(&staging).unwrap();
            make_release(&release, &make_app(&staging, VERSION, "new"), None);
            let e = prepare_from(&release, &install, &work, &[key(1).verifying_key()], &|_| {}).unwrap_err();
            assert_eq!(e, Failure::Unsigned);
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn a_release_signed_by_someone_else_or_tampered_with_is_refused() {
            let root = temp_dir("mac-bad");
            let (release, install) = (root.join("release"), root.join("Applications"));
            std::fs::create_dir_all(&install).unwrap();
            make_app(&install, "1.0.0", "old");
            let staging = root.join("staging");
            std::fs::create_dir_all(&staging).unwrap();
            make_release(&release, &make_app(&staging, VERSION, "new"), Some(&key(2)));

            let e = prepare_from(&release, &install, &root.join("w1"), &[key(3).verifying_key()], &|_| {}).unwrap_err();
            assert!(matches!(&e, Failure::Other(m) if m.contains("does not verify")), "{e}");

            // the zip changes after the list was signed
            std::fs::write(release.join(Os::Mac.zip_name(VERSION)), b"something else").unwrap();
            let e = prepare_from(&release, &install, &root.join("w2"), &[key(2).verifying_key()], &|_| {}).unwrap_err();
            assert!(matches!(&e, Failure::Other(m) if m.contains("does not match the signed list")), "{e}");
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn a_download_that_is_another_version_than_announced_is_refused() {
            let root = temp_dir("mac-version");
            let (release, install) = (root.join("release"), root.join("Applications"));
            std::fs::create_dir_all(&install).unwrap();
            make_app(&install, "1.0.0", "old");
            let staging = root.join("staging");
            std::fs::create_dir_all(&staging).unwrap();
            // signed and intact, but it is 1.0.0 inside: an old release offered as a new one
            let k = key(4);
            make_release(&release, &make_app(&staging, "1.0.0", "new"), Some(&k));
            let e = prepare_from(&release, &install, &root.join("work"), &[k.verifying_key()], &|_| {}).unwrap_err();
            assert!(matches!(&e, Failure::Other(m) if m.contains("not 9.9.9")), "{e}");
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn a_folder_that_cannot_be_written_stops_before_downloading_anything() {
            let root = temp_dir("mac-readonly");
            let exe = root.join("missing/Handset.app/Contents/MacOS/handset");
            let e = prepare(VERSION, Source::MacApp, &exe, &root.join("work"), &[], false, &|_| {}).unwrap_err();
            assert!(matches!(&e, Failure::NotWritable(d) if d.contains("missing")), "{e:?}");
            assert!(!root.join("work").exists(), "nothing was downloaded");
            let _ = std::fs::remove_dir_all(root);
        }

        /// Manual check with the real thing: a packaged `Handset.app` zipped into a release folder, signed with the
        /// real key by `scripts/update-key.sh`, installed over a copy of itself with the real code-signature check.
        /// Run: HANDSET_REAL_RELEASE=<folder with the zip, SHA256SUMS and .sig> HANDSET_REAL_VERSION=<its version>
        /// cargo test real_release -- --ignored
        #[test]
        #[ignore = "needs a packaged, signed release (see the doc comment)"]
        fn real_release_installs() {
            let (Some(release), Some(version)) =
                (std::env::var_os("HANDSET_REAL_RELEASE"), std::env::var("HANDSET_REAL_VERSION").ok())
            else {
                panic!("set HANDSET_REAL_RELEASE and HANDSET_REAL_VERSION");
            };
            let root = temp_dir("mac-real");
            let (install, work) = (root.join("Applications"), root.join("work"));
            std::fs::create_dir_all(&install).unwrap();
            make_app(&install, "0.0.1", "old");
            let exe = install.join("Handset.app/Contents/MacOS/handset");
            let _g = BASE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            unsafe { std::env::set_var("HANDSET_UPDATE_BASE", format!("file://{}", PathBuf::from(release).display())) };
            let swap = prepare(&version, Source::MacApp, &exe, &work, &verify::trusted_keys(), true, &|_| {})
                .expect("prepared");
            unsafe { std::env::remove_var("HANDSET_UPDATE_BASE") };
            let mut swap = swap;
            swap.relaunch = "true".into();
            run_helper(&swap);
            let out = Command::new(&exe).arg("--version").output().unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), format!("handset {version}"));
            let _ = std::fs::remove_dir_all(root);
        }
    }

    // ---- the Linux path is plain sh and tar, so it runs for real on macOS too ------------------------------

    #[cfg(unix)]
    mod linux {
        use super::*;
        use ed25519_dalek::{Signer, SigningKey};
        use std::os::unix::fs::PermissionsExt;

        const VERSION: &str = "9.9.9";

        /// A stand-in program that prints its version; its text says which one it is.
        fn make_program(path: &Path, version: &str) {
            std::fs::write(path, format!("#!/bin/sh\n[ \"$1\" = --version ] && echo \"handset {version}\"\n")).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        /// A local release folder holding the tarball (laid out as `scripts/package-linux.sh` makes it),
        /// `SHA256SUMS-linux` and, when `key` is given, its signature.
        fn make_release(dir: &Path, version: &str, key: Option<&SigningKey>) {
            let name = format!("Handset-{VERSION}-linux-{}", std::env::consts::ARCH);
            let stage = dir.join("stage");
            std::fs::create_dir_all(stage.join(&name)).unwrap();
            make_program(&stage.join(&name).join("handset"), version);
            std::fs::write(stage.join(&name).join("handset.desktop"), "[Desktop Entry]\nName=Handset new\n").unwrap();
            std::fs::write(stage.join(&name).join("LICENSE"), "licence").unwrap();
            let tarball = Os::Linux.zip_name(VERSION);
            let made = Command::new("tar")
                .arg("-czf")
                .arg(dir.join(&tarball))
                .arg("-C")
                .arg(&stage)
                .arg(&name)
                .status()
                .unwrap();
            assert!(made.success());
            std::fs::remove_dir_all(&stage).unwrap();
            let sums = format!("{}  {tarball}\n", verify::sha256_file(&dir.join(&tarball)).unwrap());
            std::fs::write(dir.join("SHA256SUMS-linux"), &sums).unwrap();
            if let Some(k) = key {
                std::fs::write(dir.join("SHA256SUMS-linux.sig"), k.sign(sums.as_bytes()).to_bytes()).unwrap();
            }
        }

        fn prepare_from(release: &Path, exe: &Path, work: &Path, keys: &[VerifyingKey]) -> Result<Swap, Failure> {
            let _guard = BASE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            // SAFETY: the lock above serialises every test that touches this variable
            unsafe { std::env::set_var("HANDSET_UPDATE_BASE", format!("file://{}", release.display())) };
            let result = prepare(VERSION, Source::LinuxTarball, exe, work, keys, false, &|_| {});
            unsafe { std::env::remove_var("HANDSET_UPDATE_BASE") };
            result
        }

        fn run_helper(swap: &Swap) {
            let mut done = swap.clone();
            done.pid = 999_999;
            let script = swap.log.parent().unwrap().join("update.sh");
            std::fs::write(&script, super::super::script(&done)).unwrap();
            let _ = Command::new("/bin/sh").arg(&script).status();
        }

        #[test]
        fn a_signed_tarball_is_checked_unpacked_and_only_the_program_swapped_in() {
            let root = temp_dir("linux-e2e");
            let (release, bin, work) = (root.join("release"), root.join("local-bin"), root.join("work"));
            std::fs::create_dir_all(&bin).unwrap();
            let exe = bin.join("handset");
            make_program(&exe, "1.0.0");
            // someone else's program in the same folder, and the desktop entry kept next to Handset
            std::fs::write(bin.join("other-tool"), "mine").unwrap();
            std::fs::write(bin.join("handset.desktop"), "old entry").unwrap();
            let k = SigningKey::from_bytes(&[21; 32]);
            make_release(&release, VERSION, Some(&k));

            let mut swap = prepare_from(&release, &exe, &work, &[k.verifying_key()]).expect("the update is prepared");
            assert_eq!(swap.current, exe);
            assert!(swap.new.join("handset").is_file());
            assert!(std::fs::read_to_string(&exe).unwrap().contains("1.0.0"), "nothing is replaced yet");

            swap.relaunch = "true".into();
            run_helper(&swap);
            assert!(std::fs::read_to_string(&exe).unwrap().contains(VERSION), "the new program is in place");
            assert!(std::fs::metadata(&exe).unwrap().permissions().mode() & 0o111 != 0, "and can run");
            assert_eq!(
                std::fs::read_to_string(bin.join("handset.desktop")).unwrap(),
                "[Desktop Entry]\nName=Handset new\n"
            );
            assert_eq!(std::fs::read_to_string(bin.join("other-tool")).unwrap(), "mine", "other files are left alone");
            assert!(!bin.join("LICENSE").exists(), "files that were not there are not added");
            assert!(
                !bin.join("handset.update-old").exists() && !bin.join("handset.update-new").exists(),
                "no leftovers"
            );
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn a_new_program_that_does_not_start_is_replaced_by_the_old_one_again() {
            let root = temp_dir("linux-rollback");
            let (bin, new) = (root.join("bin"), root.join("new"));
            std::fs::create_dir_all(&bin).unwrap();
            std::fs::create_dir_all(&new).unwrap();
            make_program(&bin.join("handset"), "1.0.0");
            make_program(&new.join("handset"), VERSION);
            let swap = Swap {
                os: Os::Linux,
                pid: 999_999,
                current: bin.join("handset"),
                new,
                log: root.join("update.log"),
                relaunch: "false".into(),
            };
            run_helper(&swap);
            assert!(std::fs::read_to_string(bin.join("handset")).unwrap().contains("1.0.0"), "the old program is back");
            assert!(!bin.join("handset.update-old").exists());
            let log = std::fs::read_to_string(root.join("update.log")).unwrap_or_default();
            assert!(log.contains("restoring"), "the helper says what it did: {log}");
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn an_unsigned_tarball_or_a_release_without_one_is_not_installed() {
            let root = temp_dir("linux-unsigned");
            let (release, bin) = (root.join("release"), root.join("bin"));
            std::fs::create_dir_all(&bin).unwrap();
            let exe = bin.join("handset");
            make_program(&exe, "1.0.0");
            let k = SigningKey::from_bytes(&[22; 32]);
            make_release(&release, VERSION, None);
            let e = prepare_from(&release, &exe, &root.join("w1"), &[k.verifying_key()]).unwrap_err();
            assert_eq!(e, Failure::Unsigned);
            // an older release, from before Linux had a tarball: no list at all
            let e = prepare_from(&root.join("empty"), &exe, &root.join("w2"), &[k.verifying_key()]).unwrap_err();
            assert_eq!(e, Failure::Unsigned);
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn a_tampered_or_mislabelled_tarball_is_refused() {
            let root = temp_dir("linux-bad");
            let (release, bin) = (root.join("release"), root.join("bin"));
            std::fs::create_dir_all(&bin).unwrap();
            let exe = bin.join("handset");
            make_program(&exe, "1.0.0");
            let k = SigningKey::from_bytes(&[23; 32]);
            // signed and intact, but the program inside is 1.0.0
            make_release(&release, "1.0.0", Some(&k));
            let e = prepare_from(&release, &exe, &root.join("w1"), &[k.verifying_key()]).unwrap_err();
            assert!(matches!(&e, Failure::Other(m) if m.contains("not handset 9.9.9")), "{e}");
            std::fs::write(release.join(Os::Linux.zip_name(VERSION)), b"something else").unwrap();
            let e = prepare_from(&release, &exe, &root.join("w2"), &[k.verifying_key()]).unwrap_err();
            assert!(matches!(&e, Failure::Other(m) if m.contains("does not match the signed list")), "{e}");
            assert!(std::fs::read_to_string(&exe).unwrap().contains("1.0.0"), "the installed program is untouched");
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn a_folder_that_cannot_be_written_stops_before_downloading_anything() {
            let root = temp_dir("linux-readonly");
            let exe = root.join("missing/handset");
            let e = prepare(VERSION, Source::LinuxTarball, &exe, &root.join("work"), &[], false, &|_| {}).unwrap_err();
            assert!(matches!(&e, Failure::NotWritable(d) if d.ends_with("missing")), "{e:?}");
            assert!(!root.join("work").exists(), "nothing was downloaded");
            let _ = std::fs::remove_dir_all(root);
        }
    }

    // ---- on Windows the helper runs for real too, with small system programs standing in for handset.exe ----

    #[cfg(windows)]
    mod windows {
        use super::*;

        fn system_exe(name: &str) -> PathBuf {
            PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into()))
                .join("System32")
                .join(name)
        }

        fn run_helper(swap: &Swap) -> bool {
            let mut done = swap.clone();
            done.pid = 4_000_000; // no such process: nothing to wait for
            // next to the log, so that tests running side by side never share a script
            let script = swap.log.parent().unwrap().join("update.ps1");
            std::fs::write(&script, super::super::script(&done)).unwrap();
            Command::new("powershell")
                .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                .arg(&script)
                .status()
                .unwrap()
                .success()
        }

        #[test]
        fn the_helper_copies_the_new_files_over_the_old_ones_and_starts_the_new_program() {
            let root = temp_dir("win-swap");
            let (current, new) = (root.join("Handset"), root.join("new"));
            std::fs::create_dir_all(&current).unwrap();
            std::fs::create_dir_all(&new).unwrap();
            std::fs::copy(system_exe("whoami.exe"), current.join("handset.exe")).unwrap();
            std::fs::write(current.join("keep-me.txt"), "mine").unwrap();
            std::fs::copy(system_exe("hostname.exe"), new.join("handset.exe")).unwrap();
            std::fs::write(new.join("added.txt"), "new").unwrap();
            let swap = Swap {
                os: Os::Windows,
                pid: 0,
                current: current.clone(),
                new,
                log: root.join("update.log"),
                relaunch: String::new(),
            };
            assert!(run_helper(&swap), "the helper succeeds");
            assert_eq!(
                std::fs::read(current.join("handset.exe")).unwrap(),
                std::fs::read(system_exe("hostname.exe")).unwrap(),
                "the program is the new one"
            );
            assert_eq!(std::fs::read_to_string(current.join("added.txt")).unwrap(), "new");
            assert_eq!(
                std::fs::read_to_string(current.join("keep-me.txt")).unwrap(),
                "mine",
                "files the update does not carry are kept"
            );
            assert!(!current.join(".update-backup").exists(), "no backup is left behind");
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn a_new_program_that_exits_with_an_error_is_replaced_by_the_old_one_again() {
            let root = temp_dir("win-rollback");
            let (current, new) = (root.join("Handset"), root.join("new"));
            std::fs::create_dir_all(&current).unwrap();
            std::fs::create_dir_all(&new).unwrap();
            std::fs::copy(system_exe("whoami.exe"), current.join("handset.exe")).unwrap();
            // a real program that fails at once: `cmd /c exit 3` exits with code 3
            std::fs::copy(system_exe("cmd.exe"), new.join("handset.exe")).unwrap();
            let swap = Swap {
                os: Os::Windows,
                pid: 0,
                current: current.clone(),
                new,
                log: root.join("update.log"),
                relaunch: "/c exit 3".into(),
            };
            assert!(!run_helper(&swap), "the helper reports the failure");
            assert_eq!(
                std::fs::read(current.join("handset.exe")).unwrap(),
                std::fs::read(system_exe("whoami.exe")).unwrap(),
                "the old program is back"
            );
            let _ = std::fs::remove_dir_all(root);
        }

        #[test]
        fn a_zip_is_unpacked_and_its_folder_found() {
            let root = temp_dir("win-zip");
            let folder = root.join("pack/Handset-9.9.9");
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join("handset.exe"), "x").unwrap();
            let zip = root.join("Handset-9.9.9-windows.zip");
            let made = Command::new("tar")
                .args(["-a", "-c", "-f"])
                .arg(&zip)
                .arg("-C")
                .arg(root.join("pack"))
                .arg("Handset-9.9.9")
                .status()
                .unwrap();
            assert!(made.success(), "tar can make a zip");
            let out = extract(&zip, &root.join("out"), Os::Windows).unwrap();
            assert!(out.ends_with("Handset-9.9.9") && out.join("handset.exe").is_file(), "{out:?}");
            let _ = std::fs::remove_dir_all(root);
        }
    }
}
