//! Framed screenshots (PNG) and recordings (ffmpeg sidecar).

use std::io::{BufWriter, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Sender, SyncSender, channel, sync_channel};
use std::thread::JoinHandle;

#[derive(Clone, Copy, PartialEq)]
pub enum Format {
    Mp4,
    Gif,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mp4" => Some(Self::Mp4),
            "gif" => Some(Self::Gif),
            _ => None,
        }
    }
    fn ext(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Gif => "gif",
        }
    }
}

pub struct Settings {
    pub format: Format,
    /// Longest side of recordings, in pixels.
    pub rec_size: u32,
    pub rec_bg: [f32; 3],
    pub fps: u32,
    /// Extra gain applied to recorded audio, in dB.
    pub gain_db: f32,
    /// Normalize recorded audio loudness (EBU R128, about -16 LUFS).
    pub normalize: bool,
    /// Copy each new screenshot / recording to the clipboard.
    pub clipboard: bool,
    /// Look applied to captures (see `style.rs`); used for new captures when `auto_style` is on.
    pub style: crate::style::Style,
    /// Save a styled copy next to each new screenshot and recording.
    pub auto_style: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: Format::Mp4,
            rec_size: 1280,
            rec_bg: [0.063, 0.063, 0.078],
            fps: 30,
            gain_db: 0.0,
            normalize: true,
            clipboard: true,
            style: crate::style::Style::default(),
            auto_style: false,
        }
    }
}

/// `#rrggbb` -> linear-free 0..1 components (the surface is not sRGB-encoded).
pub fn parse_color(s: &str) -> Option<[f32; 3]> {
    let h = s.strip_prefix('#').unwrap_or(s);
    if h.len() != 6 {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|v| v as f32 / 255.0);
    Some([c(0)?, c(2)?, c(4)?])
}

#[cfg(windows)]
fn local_timestamp() -> Option<String> {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        dow: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        ms: u16,
    }
    unsafe extern "system" {
        fn GetLocalTime(t: *mut SystemTime);
    }
    let mut t = SystemTime::default();
    // SAFETY: GetLocalTime fills the struct, which matches the Win32 SYSTEMTIME layout
    unsafe { GetLocalTime(&mut t) };
    Some(format!("{:04}{:02}{:02}-{:02}{:02}{:02}", t.year, t.month, t.day, t.hour, t.minute, t.second))
}

#[cfg(not(windows))]
fn local_timestamp() -> Option<String> {
    Command::new("date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn timestamp() -> String {
    local_timestamp().unwrap_or_else(|| {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        secs.to_string()
    })
}

fn dir_path(candidates: &[&str]) -> PathBuf {
    let home = crate::tools::home();
    let base = candidates.iter().map(|c| home.join(c)).find(|p| p.is_dir()).unwrap_or(home);
    base.join("handset")
}

fn out_dir(candidates: &[&str]) -> PathBuf {
    let dir = dir_path(candidates);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Remove the outline files (see `restyle::matte_path`) of recordings that have been deleted.
pub fn prune_mattes() {
    let Ok(rd) = std::fs::read_dir(dir_path(&["Movies", "Videos"])) else { return };
    for path in rd.flatten().map(|e| e.path()) {
        if path.extension().and_then(|x| x.to_str()) == Some("matte")
            && !path.with_extension("mp4").exists()
            && !path.with_extension("gif").exists()
        {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// The newest screenshots and recordings (from both output folders), newest first.
pub fn recent_captures(n: usize) -> Vec<PathBuf> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for dir in [dir_path(&["Pictures"]), dir_path(&["Movies", "Videos"])] {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for e in rd.flatten() {
            let path = e.path();
            let ok = path.extension().and_then(|x| x.to_str()).is_some_and(|x| matches!(x, "png" | "mp4" | "gif"));
            if let (true, Ok(m)) = (ok, e.metadata().and_then(|m| m.modified())) {
                files.push((m, path));
            }
        }
    }
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files.into_iter().take(n).map(|f| f.1).collect()
}

/// Show the file in Finder (macOS).
pub fn reveal(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(windows)]
    let _ = Command::new("explorer").arg(format!("/select,{}", path.display())).spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        // no portable "select this file": open the folder that holds it
        let _ = Command::new("xdg-open").arg(path.parent().unwrap_or(path)).spawn();
    }
}

/// Open the file in its default app.
pub fn open_file(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(path).spawn();
    #[cfg(windows)]
    let _ = crate::tools::command("cmd").args(["/C", "start", ""]).arg(path).spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = Command::new("xdg-open").arg(path).spawn();
}

/// Put the capture on the clipboard: a PNG as an image, anything else as a
/// file reference (pastes as an attachment in Finder, Slack, Mail...).
pub fn copy_to_clipboard(path: &std::path::Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        let p = path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
        let script = if path.extension().is_some_and(|e| e == "png") {
            format!("set the clipboard to (read (POSIX file \"{p}\") as \u{ab}class PNGf\u{bb})")
        } else {
            format!("set the clipboard to (POSIX file \"{p}\")")
        };
        Command::new("osascript").args(["-e", &script]).output().is_ok_and(|o| o.status.success())
    }
    #[cfg(windows)]
    {
        crate::clipboard::win::copy_capture(path)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let Ok(path) = path.canonicalize() else { return false };
        if path.extension().is_some_and(|e| e == "png") {
            std::fs::read(&path).is_ok_and(|png| crate::tools::linux::copy(&png, Some("image/png")))
        } else {
            let (data, mime) = file_clip(&path, &default_file_manager());
            crate::tools::linux::copy(data.as_bytes(), Some(mime))
        }
    }
}

/// The clipboard payload that makes a file paste into the file manager. Nautilus, Nemo and Caja read their own
/// type; the others (Dolphin, Thunar, ...) read a URI list. `file_manager` is its `.desktop` name.
#[cfg(any(test, not(any(target_os = "macos", windows))))]
fn file_clip(path: &std::path::Path, file_manager: &str) -> (String, &'static str) {
    let uri = format!("file://{}", path.display());
    let fm = file_manager.to_ascii_lowercase();
    if ["nautilus", "nemo", "caja"].iter().any(|n| fm.contains(n)) {
        (format!("copy\n{uri}"), "x-special/gnome-copied-files")
    } else {
        (format!("{uri}\r\n"), "text/uri-list")
    }
}

/// The `.desktop` name of the app that opens folders, empty if it cannot be found.
#[cfg(not(any(target_os = "macos", windows)))]
fn default_file_manager() -> String {
    Command::new("xdg-mime")
        .args(["query", "default", "inode/directory"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// A path that does not exist yet, `<stem>[-N].<ext>`.
pub(crate) fn unique(dir: PathBuf, stem: &str, ext: &str) -> PathBuf {
    let mut p = dir.join(format!("{stem}.{ext}"));
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{stem}-{n}.{ext}"));
        n += 1;
    }
    p
}

pub fn save_png(rgba: &[u8], w: u32, h: u32) -> Result<PathBuf, String> {
    let path = unique(out_dir(&["Pictures"]), &format!("handset-{}", timestamp()), "png");
    let file = std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(path)
}

/// ffmpeg audio filter chain for recordings, or `None` when the audio is left as captured.
///
/// Gain comes first, then loudness normalisation (which also limits peaks). `loudnorm` has been seen to
/// emit NaN now and then, which makes the AAC encoder drop the whole audio track, so a guard that turns
/// NaN and infinity into silence follows it.
fn audio_filters(settings: &Settings) -> Option<String> {
    const NAN_GUARD: &str =
        "aeval='if(isnan(val(0))+isinf(val(0)),0,val(0))|if(isnan(val(1))+isinf(val(1)),0,val(1))':c=same";
    let mut filters = Vec::new();
    if settings.gain_db != 0.0 {
        filters.push(format!("volume={}dB", settings.gain_db));
    }
    if settings.normalize {
        filters.push("loudnorm=I=-16:TP=-1.5:LRA=11".to_string());
        filters.push(NAN_GUARD.to_string());
    }
    (!filters.is_empty()).then(|| filters.join(","))
}

/// Pipes raw RGBA frames to an `ffmpeg` child. Frames are dropped, not
/// queued without bound, when ffmpeg cannot keep up.
pub struct Recorder {
    pub path: PathBuf,
    pub size: (u32, u32),
    tx: Option<SyncSender<Vec<u8>>>,
    writer: Option<JoinHandle<()>>,
    audio_tx: Option<Sender<Vec<u8>>>,
    audio_writer: Option<JoinHandle<()>>,
    child: Child,
}

impl Recorder {
    pub fn start(size: (u32, u32), settings: &Settings, stem_hint: &str, with_audio: bool) -> Result<Self, String> {
        let dir = out_dir(&["Movies", "Videos"]);
        let path = unique(dir, &format!("{stem_hint}-{}", timestamp()), settings.format.ext());
        let (w, h) = size;

        let mut cmd = crate::tools::command(crate::tools::find("ffmpeg"));
        cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba"]).args([
            "-s",
            &format!("{w}x{h}"),
            "-framerate",
            &settings.fps.to_string(),
            "-i",
            "-",
        ]);

        // Audio goes over a local socket that ffmpeg connects to as a second input.
        let listener = if with_audio && settings.format == Format::Mp4 {
            Some(TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?)
        } else {
            None
        };
        if let Some(l) = &listener {
            let port = l.local_addr().map_err(|e| e.to_string())?.port();
            cmd.args([
                "-f",
                "s16le",
                "-ar",
                &crate::audio::SAMPLE_RATE.to_string(),
                "-ac",
                &crate::audio::CHANNELS.to_string(),
            ])
            .args(["-i", &format!("tcp://127.0.0.1:{port}")]);
        } else {
            cmd.arg("-an");
        }
        match settings.format {
            Format::Mp4 => {
                if listener.is_some() {
                    // -shortest: the audio filter pads the end, so stop with the video
                    cmd.args([
                        "-map",
                        "0:v",
                        "-map",
                        "1:a",
                        "-c:a",
                        "aac",
                        "-b:a",
                        "160k",
                        "-ar",
                        "48000",
                        "-shortest",
                    ]);
                    if let Some(chain) = audio_filters(settings) {
                        cmd.args(["-af", &chain]);
                    }
                }
                cmd.args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p"])
                    .args(["-movflags", "+faststart"]);
            }
            Format::Gif => {
                let gw = w.min(480);
                cmd.args([
                    "-vf",
                    &format!("fps=15,scale={gw}:-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse"),
                ]);
            }
        }
        cmd.arg(&path).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::inherit());

        let mut child = cmd.spawn().map_err(|e| format!("cannot run ffmpeg (is it installed?): {e}"))?;
        let mut stdin = child.stdin.take().ok_or("no ffmpeg stdin")?;
        let (tx, rx) = sync_channel::<Vec<u8>>(3);
        let writer = std::thread::spawn(move || {
            while let Ok(frame) = rx.recv() {
                if stdin.write_all(&frame).is_err() {
                    break;
                }
            }
            // dropping stdin closes the pipe and lets ffmpeg finish the file
        });

        // Everything captured since the recording started is queued (the
        // channel is unbounded) until ffmpeg connects, so audio and video
        // start together.
        let (audio_tx, audio_writer) = match listener {
            Some(l) => {
                let (atx, arx) = channel::<Vec<u8>>();
                let handle = std::thread::spawn(move || {
                    let _ = l.set_nonblocking(true);
                    let start = std::time::Instant::now();
                    let mut sock = loop {
                        match l.accept() {
                            Ok((s, _)) => break s,
                            Err(_) if start.elapsed() < std::time::Duration::from_secs(15) => {
                                std::thread::sleep(std::time::Duration::from_millis(5));
                            }
                            Err(_) => return,
                        }
                    };
                    let _ = sock.set_nonblocking(false);
                    while let Ok(chunk) = arx.recv() {
                        if sock.write_all(&chunk).is_err() {
                            break;
                        }
                    }
                });
                (Some(atx), Some(handle))
            }
            None => (None, None),
        };
        Ok(Self { path, size, tx: Some(tx), writer: Some(writer), audio_tx, audio_writer, child })
    }

    /// A sender for the device's PCM audio, when this recording has audio.
    pub fn audio_sender(&self) -> Option<Sender<Vec<u8>>> {
        self.audio_tx.clone()
    }

    pub fn push(&self, rgba: Vec<u8>) {
        if let Some(tx) = &self.tx {
            let _ = tx.try_send(rgba);
        }
    }

    pub fn finish(mut self) -> PathBuf {
        self.tx.take();
        self.audio_tx.take();
        if let Some(w) = self.writer.take() {
            let _ = w.join();
        }
        if let Some(w) = self.audio_writer.take() {
            let _ = w.join();
        }
        let _ = self.child.wait();
        self.path.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn touch(path: &std::path::Path, age_secs: u64) {
        let f = std::fs::File::create(path).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(age_secs)).unwrap();
    }

    #[test]
    fn recent_captures_are_newest_first_and_only_media() {
        let home = std::env::temp_dir().join(format!("handset-test-{}", std::process::id()));
        let (pics, movs) = (home.join("Pictures/handset"), home.join("Movies/handset"));
        std::fs::create_dir_all(&pics).unwrap();
        std::fs::create_dir_all(&movs).unwrap();
        touch(&pics.join("old.png"), 500);
        touch(&movs.join("newest.mp4"), 10);
        touch(&pics.join("middle.png"), 100);
        touch(&movs.join("notes.txt"), 1); // not a capture
        touch(&movs.join("clip.gif"), 300);
        // SAFETY: this is the only test that touches the environment
        unsafe { std::env::set_var("HOME", &home) };

        let names: Vec<String> =
            recent_captures(3).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["newest.mp4", "middle.png", "clip.gif"]);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn file_clip_matches_the_file_manager() {
        let p = std::path::Path::new("/home/me/Videos/handset/a.mp4");
        let (data, mime) = file_clip(p, "org.gnome.Nautilus.desktop");
        assert_eq!(
            (data.as_str(), mime),
            ("copy\nfile:///home/me/Videos/handset/a.mp4", "x-special/gnome-copied-files")
        );
        assert_eq!(file_clip(p, "nemo.desktop").1, "x-special/gnome-copied-files");
        let (data, mime) = file_clip(p, "org.kde.dolphin.desktop");
        assert_eq!((data.as_str(), mime), ("file:///home/me/Videos/handset/a.mp4\r\n", "text/uri-list"));
        assert_eq!(file_clip(p, "").1, "text/uri-list");
    }

    #[test]
    fn audio_filter_chain() {
        let mut s = Settings::default();
        let chain = audio_filters(&s).unwrap();
        assert!(chain.starts_with("loudnorm="), "{chain}");
        assert!(chain.contains("isnan"), "normalised audio is guarded against NaN: {chain}");

        s.gain_db = 6.0;
        assert!(audio_filters(&s).unwrap().starts_with("volume=6dB,loudnorm="));

        s.normalize = false;
        assert_eq!(audio_filters(&s).unwrap(), "volume=6dB");

        s.gain_db = 0.0;
        assert_eq!(audio_filters(&s), None);
    }

    /// Changes the real clipboard, so it only runs when asked for:
    /// `cargo test -- --ignored clipboard`
    #[test]
    #[ignore]
    #[cfg(target_os = "macos")]
    fn clipboard_copy_of_png_and_file() {
        let dir = std::env::temp_dir().join(format!("handset-clip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png_path = dir.join("shot.png");
        {
            let mut enc = png::Encoder::new(std::fs::File::create(&png_path).unwrap(), 2, 2);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&[255u8; 16]).unwrap();
        }
        let mp4_path = dir.join("clip.mp4");
        std::fs::write(&mp4_path, b"not really a video").unwrap();

        let info = || {
            String::from_utf8_lossy(&Command::new("osascript").args(["-e", "clipboard info"]).output().unwrap().stdout)
                .into_owned()
        };
        assert!(copy_to_clipboard(&png_path), "png copy failed");
        let after_png = info();
        assert!(after_png.contains("PNGf"), "png clipboard: {after_png}");
        assert!(copy_to_clipboard(&mp4_path), "file copy failed");
        let after_file = info();
        assert!(after_file.contains("furl"), "file clipboard: {after_file}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
