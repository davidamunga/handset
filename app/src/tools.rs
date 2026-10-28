//! Locate helper programs (`adb`, `ffmpeg`). An app started from Finder gets
//! a bare PATH without Homebrew, so well-known install locations are tried too.

use std::path::PathBuf;

/// Find `name`: `$NAME` override first, then PATH, then common install places.
/// Falls back to the bare name so the OS error message names the program.
pub fn find(name: &str) -> PathBuf {
    if let Some(p) = std::env::var_os(name.to_ascii_uppercase()) {
        return p.into();
    }
    let home = home();
    let mut dirs: Vec<PathBuf> =
        std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    dirs.extend(
        ["Library/Android/sdk/platform-tools", "Android/Sdk/platform-tools", ".local/bin"].map(|r| home.join(r)),
    );
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            dirs.push(local.join("Android/Sdk/platform-tools"));
            dirs.push(local.join("Microsoft/WinGet/Links"));
        }
        // winget unpacks portable tools (platform-tools, ffmpeg) into per-package folders
        if let Some(pkgs) = std::env::var_os("LOCALAPPDATA")
            .map(|l| PathBuf::from(l).join("Microsoft/WinGet/Packages"))
            .and_then(|p| std::fs::read_dir(p).ok())
        {
            for pkg in pkgs.flatten().map(|e| e.path()) {
                dirs.push(pkg.join("platform-tools"));
                for sub in std::fs::read_dir(&pkg).into_iter().flatten().flatten() {
                    dirs.push(sub.path().join("bin"));
                }
            }
        }
        dirs.push(home.join("scoop/shims"));
        dirs.push(PathBuf::from("C:/ProgramData/chocolatey/bin"));
        // next to the exe, so a release zip can bundle platform-tools
        if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)) {
            dirs.push(dir.join("platform-tools"));
            dirs.push(dir);
        }
    }
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    dirs.into_iter().map(|d| d.join(&file)).find(|p| p.is_file()).unwrap_or_else(|| name.into())
}

/// Open a web page in the default browser.
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(windows)]
    let _ = command("cmd").args(["/C", "start", ""]).arg(url).spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

/// The user's home directory (`HOME`, or `USERPROFILE` on Windows).
pub fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

/// A command that does not flash a console window when the app has none (Windows).
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    #[allow(unused_mut)]
    let mut c = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

/// Run a PowerShell snippet, optionally feeding `stdin`; returns stdout on success.
#[cfg(windows)]
pub fn powershell(script: &str, stdin: Option<&str>) -> Option<String> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let _ = pipe.write_all(text.as_bytes());
    }
    let out = child.wait_with_output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Attach to the parent's console so `eprintln!` works when started from a terminal (the release
/// build is a GUI-subsystem exe, which has none of its own).
#[cfg(windows)]
pub fn attach_console() {
    unsafe extern "system" {
        fn AttachConsole(pid: u32) -> i32;
    }
    // SAFETY: plain Win32 call; failing because there is no parent console is fine
    unsafe { AttachConsole(u32::MAX) };
}

/// Print an error and quit. When there is no terminal to read it (launched
/// from Finder), show it in a dialog instead.
pub fn die(msg: &str) -> ! {
    use std::io::IsTerminal;
    eprintln!("{msg}");
    #[cfg(not(any(target_os = "macos", windows)))]
    if !std::io::stderr().is_terminal() {
        linux::error_dialog(msg);
    }
    #[cfg(windows)]
    if !std::io::stderr().is_terminal() {
        let text = msg.replace('\'', "''");
        let _ = powershell(
            &format!(
                "Add-Type -AssemblyName System.Windows.Forms; \
                 [void][System.Windows.Forms.MessageBox]::Show('{text}', 'Handset', 'OK', 'Error')"
            ),
            None,
        );
    }
    #[cfg(target_os = "macos")]
    if !std::io::stderr().is_terminal() {
        let text = msg.replace('\\', "\\\\").replace('"', "\\\"");
        let _ = std::process::Command::new("osascript")
            .args(["-e", &format!("display alert \"Handset\" message \"{text}\" as critical")])
            .output();
    }
    std::process::exit(1)
}

/// Arguments to use when none were given: `~/.config/handset/args` if it
/// exists, otherwise `--audio` when run as an app bundle.
pub fn default_args() -> Vec<String> {
    let home = home();
    if let Ok(text) = std::fs::read_to_string(home.join(".config/handset/args")) {
        return text.split_whitespace().map(String::from).collect();
    }
    let in_bundle = std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS"));
    if in_bundle { vec!["--audio".to_string()] } else { Vec::new() }
}

/// Helpers for Linux desktops, where the same job is done by different programs depending on the desktop and
/// whether it runs Wayland or X11. Each tries what is installed and gives up quietly if there is nothing.
#[cfg(not(any(target_os = "macos", windows)))]
pub mod linux {
    use std::io::Write;
    use std::process::{Command, Stdio};

    fn wayland() -> bool {
        std::env::var_os("WAYLAND_DISPLAY").is_some()
    }

    /// The clipboard commands to try, best first: `wl-clipboard` on Wayland, then `xclip` and `xsel` (which also
    /// work through XWayland).
    fn copy_commands(mime: Option<&str>) -> Vec<Command> {
        let mut list = Vec::new();
        if wayland() {
            let mut c = Command::new("wl-copy");
            if let Some(m) = mime {
                c.args(["--type", m]);
            }
            list.push(c);
        }
        let mut c = Command::new("xclip");
        c.args(["-selection", "clipboard"]);
        if let Some(m) = mime {
            c.args(["-t", m]);
        }
        list.push(c);
        if mime.is_none() {
            let mut c = Command::new("xsel");
            c.args(["--clipboard", "--input"]);
            list.push(c);
        }
        list
    }

    /// Put `data` on the clipboard, as `mime` or as plain text. Returns whether a program took it.
    pub fn copy(data: &[u8], mime: Option<&str>) -> bool {
        for mut cmd in copy_commands(mime) {
            // wl-copy and xclip stay running to serve the clipboard, so we hand over the data and let go
            let Ok(mut child) = cmd.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
                continue;
            };
            let wrote = child.stdin.take().is_some_and(|mut pipe| pipe.write_all(data).is_ok());
            // both hand the clipboard to a background process and exit at once; waiting also reaps the child
            if wrote && child.wait().is_ok_and(|s| s.success()) {
                return true;
            }
        }
        false
    }

    /// The clipboard as text.
    pub fn paste() -> Option<String> {
        let mut list = Vec::new();
        if wayland() {
            let mut c = Command::new("wl-paste");
            c.arg("--no-newline");
            list.push(c);
        }
        let mut c = Command::new("xclip");
        c.args(["-selection", "clipboard", "-o"]);
        list.push(c);
        let mut c = Command::new("xsel");
        c.args(["--clipboard", "--output"]);
        list.push(c);
        list.into_iter().find_map(|mut c| {
            let out = c.stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
            out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
        })
    }

    /// Show an error in a window when there is no terminal to print it in.
    pub fn error_dialog(msg: &str) {
        let shown = Command::new("zenity")
            .args(["--error", "--title=Handset", "--no-markup"])
            .arg(format!("--text={msg}"))
            .status();
        if shown.is_err() {
            let _ = Command::new("kdialog").args(["--title", "Handset", "--error", msg]).status();
        }
    }
}
