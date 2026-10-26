//! Push and start the scrcpy server, then open its video and control sockets.
//!
//! Wire format reference: scrcpy `doc/develop.md` and the server's
//! `Streamer.java` / `DesktopConnection.java`.

use crate::adb;
use std::io::{self, Read};
use std::net::TcpStream;
use std::process::{Child, Stdio};
use std::thread::sleep;
use std::time::Duration;

const SERVER_BIN: &[u8] = include_bytes!("../assets/scrcpy-server");
/// Must match the embedded server exactly.
pub const SERVER_VERSION: &str = "5.0.1";
/// Video codec id sent by the server: "h264".
const CODEC_H264: u32 = 0x6832_3634;
/// Where the jar goes for listing apps, apart from the one a session runs.
const APPS_PATH: &str = "/data/local/tmp/handset-apps.jar";
const DEVICE_PATH: &str = "/data/local/tmp/handset-server.jar";

pub struct Options {
    pub max_size: u32,
    pub max_fps: u32,
    pub bit_rate: u32,
    pub audio: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_size: 1920, max_fps: 60, bit_rate: 8_000_000, audio: false }
    }
}

pub struct Session {
    serial: String,
    port: String,
    child: Child,
    pub video: TcpStream,
    pub audio: Option<TcpStream>,
    pub control: TcpStream,
    pub device_name: String,
}

impl Session {
    /// The adb serial this session mirrors.
    pub fn serial(&self) -> &str {
        &self.serial
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = adb::run(&self.serial, &["forward", "--remove", &format!("tcp:{}", self.port)]);
    }
}

fn connect_with_retry(port: &str) -> io::Result<TcpStream> {
    // In "forward" mode adb accepts the TCP connection even before the server
    // listens; the server then sends one dummy byte on the first socket.
    // A failed read means the server isn't ready yet, so retry.
    for _ in 0..100 {
        if let Ok(mut s) = TcpStream::connect(format!("127.0.0.1:{port}")) {
            let mut b = [0u8; 1];
            if s.read_exact(&mut b).is_ok() {
                return Ok(s);
            }
        }
        sleep(Duration::from_millis(100));
    }
    Err(io::Error::other("timed out waiting for the scrcpy server"))
}

/// Ask the phone for its installed apps (this can take several seconds).
pub fn list_apps(serial: &str) -> io::Result<Vec<crate::apps::AppEntry>> {
    // a separate jar: pushing over the one a running session uses makes that server abort
    let local = std::env::temp_dir().join(format!("handset-server-apps-{}.jar", std::process::id()));
    std::fs::write(&local, SERVER_BIN)?;
    let pushed = adb::run(serial, &["push", local.to_str().unwrap(), APPS_PATH]);
    let _ = std::fs::remove_file(&local);
    pushed?;
    let mut cmd = adb::command(serial);
    cmd.args([
        "shell".to_string(),
        format!("CLASSPATH={APPS_PATH}"),
        "app_process".into(),
        "/".into(),
        "com.genymobile.scrcpy.Server".into(),
        SERVER_VERSION.into(),
        "list_apps=true".into(),
        "log_level=info".into(),
    ]);
    let out = adb::output_within(cmd, Duration::from_secs(90));
    let _ = adb::run(serial, &["shell", "rm", "-f", APPS_PATH]);
    let out = out?;
    let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let apps = crate::apps::for_menu(crate::apps::parse_list(&text));
    if apps.is_empty() {
        let why = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("the server printed nothing");
        return Err(io::Error::other(format!("no apps in the server's answer ({why})")));
    }
    Ok(apps)
}

pub fn start(serial: &str, opts: &Options) -> io::Result<Session> {
    let local = std::env::temp_dir().join("handset-server.jar");
    std::fs::write(&local, SERVER_BIN)?;
    adb::run(serial, &["push", local.to_str().unwrap(), DEVICE_PATH])?;

    let scid = std::process::id() & 0x7fff_ffff;
    let port = adb::run(serial, &["forward", "tcp:0", &format!("localabstract:scrcpy_{scid:08x}")])?;

    let child = adb::command(serial)
        .args([
            "shell".to_string(),
            format!("CLASSPATH={DEVICE_PATH}"),
            "app_process".into(),
            "/".into(),
            "com.genymobile.scrcpy.Server".into(),
            SERVER_VERSION.into(),
            format!("scid={scid:08x}"),
            "log_level=info".into(),
            "tunnel_forward=true".into(),
            format!("audio={}", opts.audio),
            "audio_codec=raw".into(),
            "control=true".into(),
            "video_codec=h264".into(),
            format!("max_size={}", opts.max_size),
            format!("max_fps={}", opts.max_fps),
            format!("video_bit_rate={}", opts.bit_rate),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    // From here on `Drop` cleans up the child and the port forward.
    let mut session_parts = (child, port.clone());
    let video = match connect_with_retry(&port) {
        Ok(v) => v,
        Err(e) => {
            let _ = session_parts.0.kill();
            return Err(e);
        }
    };
    // the server accepts the sockets in order: video, audio, control
    let audio = if opts.audio { Some(TcpStream::connect(format!("127.0.0.1:{port}"))?) } else { None };
    let control = TcpStream::connect(format!("127.0.0.1:{port}"))?;
    control.set_nodelay(true)?;

    let mut video = video;
    video.set_nodelay(true)?;
    let mut name = [0u8; 64];
    video.read_exact(&mut name)?;
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    let device_name = String::from_utf8_lossy(&name[..end]).into_owned();
    let mut codec = [0u8; 4];
    video.read_exact(&mut codec)?;
    let codec_id = u32::from_be_bytes(codec);
    if codec_id != CODEC_H264 {
        return Err(io::Error::other(format!("unsupported video codec 0x{codec_id:08x} (expected H.264)")));
    }

    Ok(Session { serial: serial.to_string(), port, child: session_parts.0, video, audio, control, device_name })
}

#[derive(Debug)]
pub enum Packet {
    /// New video session (first frame, or after rotation): frame size.
    Session { width: u32, height: u32 },
    /// SPS/PPS.
    Config(Vec<u8>),
    #[allow(dead_code)] // pts and key are kept for upcoming A/V sync work
    Frame { pts_us: u64, key: bool, data: Vec<u8> },
}

const FLAG_SESSION: u64 = 1 << 63;
const FLAG_CONFIG: u64 = 1 << 62;
const FLAG_KEY: u64 = 1 << 61;

pub fn read_packet(video: &mut impl Read) -> io::Result<Packet> {
    let mut h = [0u8; 12];
    video.read_exact(&mut h)?;
    let first = u64::from_be_bytes(h[..8].try_into().unwrap());
    if first & FLAG_SESSION != 0 {
        let width = u32::from_be_bytes(h[4..8].try_into().unwrap());
        let height = u32::from_be_bytes(h[8..12].try_into().unwrap());
        return Ok(Packet::Session { width, height });
    }
    let size = u32::from_be_bytes(h[8..12].try_into().unwrap()) as usize;
    let mut data = vec![0u8; size];
    video.read_exact(&mut data)?;
    if first & FLAG_CONFIG != 0 {
        Ok(Packet::Config(data))
    } else {
        Ok(Packet::Frame { pts_us: first & !(FLAG_KEY | FLAG_CONFIG | FLAG_SESSION), key: first & FLAG_KEY != 0, data })
    }
}
