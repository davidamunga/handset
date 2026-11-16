//! Agent control: a program drives the phone through a small local socket instead of the window.
//!
//! Off unless asked for (`--ctl`, or `ctl=on` in the settings). A running Handset listens on the loopback
//! address and writes where, and the secret needed to talk to it, to `~/.config/handset/ctl` (readable only
//! by you). `handset ctl <command>` reads that file and sends one request.
//!
//! One request per connection, one line: `<token> <command> [arguments]`. The answer is `ok <n>` and `n`
//! bytes of text (or a PNG for `screenshot`), or `err <message>`. Coordinates are pixels of the picture
//! `screenshot` returns, so what the agent sees is what it taps. See `docs/agents.md`.

use crate::control::{Control, Touch, keycode};
use crate::render::Yuv;
use crate::stream::Slot;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A request line this long is nonsense.
const MAX_LINE: u64 = 2048;
/// The most text one `type` may carry (the phone's server refuses more).
const MAX_TEXT: usize = 300;
/// The longest a swipe or a press may take.
const MAX_MS: u64 = 10_000;
/// The pill shows while the last request is this recent.
const ACTIVE_FOR: Duration = Duration::from_secs(4);
/// The finger the agent touches with, so it is never mistaken for the mouse or a Share viewer.
const POINTER: u64 = 0xA6E27;

#[derive(Debug, PartialEq)]
pub enum Command {
    Info,
    Screenshot,
    Tap { x: i32, y: i32 },
    Hold { x: i32, y: i32, ms: u64 },
    Swipe { from: (i32, i32), to: (i32, i32), ms: u64 },
    Scroll { x: i32, y: i32, h: f32, v: f32 },
    Text(String),
    Key(u32),
    Rotate,
    Open(String),
    Notifications,
    Collapse,
}

pub const HELP: &str = "\
commands:
  info                       the phone's name and screen size
  screenshot <file|->        the screen as a PNG (- for standard output)
  tap <x> <y>
  hold <x> <y> [ms]          a long press (default 600 ms)
  swipe <x1> <y1> <x2> <y2> [ms]
  scroll <x> <y> <h> <v>     in notches, -16 to 16; v > 0 scrolls up
  type <text>                at most 300 characters
  key <name>                 back home recents enter del tab power volup voldown up down left right
  rotate
  open <package>             or ?<app name>, which must match exactly one app
  notifications | collapse   pull the shade down, close it
Coordinates are pixels of the screenshot.";

fn int(s: &str, what: &str) -> Result<i32, String> {
    s.parse().map_err(|_| format!("{what} must be a whole number, not {s:?}"))
}

fn ms(s: Option<&&str>, default: u64) -> Result<u64, String> {
    let Some(s) = s else { return Ok(default) };
    let v: u64 = s.parse().map_err(|_| format!("milliseconds must be a whole number, not {s:?}"))?;
    Ok(v.clamp(20, MAX_MS))
}

pub fn parse(line: &str) -> Result<Command, String> {
    let line = line.trim_end_matches(['\r', '\n']);
    let (name, rest) = line.split_once(' ').unwrap_or((line, ""));
    let a: Vec<&str> = rest.split_whitespace().collect();
    let need = |n: usize| {
        if a.len() == n { Ok(()) } else { Err(format!("{name} takes {n} numbers (see `handset ctl help`)")) }
    };
    Ok(match name {
        "info" => Command::Info,
        "screenshot" => Command::Screenshot,
        "rotate" => Command::Rotate,
        "notifications" => Command::Notifications,
        "collapse" => Command::Collapse,
        "tap" => {
            need(2)?;
            Command::Tap { x: int(a[0], "x")?, y: int(a[1], "y")? }
        }
        "hold" => {
            if !(2..=3).contains(&a.len()) {
                return Err("hold takes x y and optionally milliseconds".into());
            }
            Command::Hold { x: int(a[0], "x")?, y: int(a[1], "y")?, ms: ms(a.get(2), 600)? }
        }
        "swipe" => {
            if !(4..=5).contains(&a.len()) {
                return Err("swipe takes x1 y1 x2 y2 and optionally milliseconds".into());
            }
            Command::Swipe {
                from: (int(a[0], "x1")?, int(a[1], "y1")?),
                to: (int(a[2], "x2")?, int(a[3], "y2")?),
                ms: ms(a.get(4), 300)?,
            }
        }
        "scroll" => {
            need(4)?;
            let n = |s: &str| s.parse::<f32>().ok().filter(|v| v.is_finite()).ok_or(format!("{s:?} is not a number"));
            Command::Scroll {
                x: int(a[0], "x")?,
                y: int(a[1], "y")?,
                h: n(a[2])?.clamp(-16.0, 16.0),
                v: n(a[3])?.clamp(-16.0, 16.0),
            }
        }
        "type" => {
            if rest.chars().any(char::is_control) {
                return Err("text may not contain control characters or line breaks".into());
            }
            if rest.is_empty() {
                return Err("type needs some text".into());
            }
            if rest.chars().count() > MAX_TEXT {
                return Err(format!("text is longer than {MAX_TEXT} characters; send it in pieces"));
            }
            Command::Text(rest.to_string())
        }
        "key" => Command::Key(match rest.trim() {
            "back" => keycode::BACK,
            "home" => keycode::HOME,
            "recents" => keycode::APP_SWITCH,
            "enter" => keycode::ENTER,
            "del" => keycode::DEL,
            "tab" => keycode::TAB,
            "power" => keycode::POWER,
            "volup" => keycode::VOLUME_UP,
            "voldown" => keycode::VOLUME_DOWN,
            "up" => keycode::DPAD_UP,
            "down" => keycode::DPAD_DOWN,
            "left" => keycode::DPAD_LEFT,
            "right" => keycode::DPAD_RIGHT,
            other => return Err(format!("unknown key {other:?} (see `handset ctl help`)")),
        }),
        "open" => {
            let app = rest.trim();
            if app.is_empty() || app.len() > 255 || app.chars().any(char::is_control) {
                return Err("open needs a package name, or ?<app name>".into());
            }
            Command::Open(app.to_string())
        }
        "" => return Err("empty request".into()),
        other => return Err(format!("unknown command {other:?} (see `handset ctl help`)")),
    })
}

/// What the phone being mirrored lets us do. An iPhone has no control, only a picture.
pub struct Target {
    pub control: Option<Control>,
    pub name: String,
    pub serial: String,
}

struct Shared {
    token: String,
    slot: Arc<Mutex<Slot>>,
    target: Mutex<Option<Target>>,
    last: Mutex<Option<Instant>>,
    stop: AtomicBool,
    notify: Box<dyn Fn() + Send + Sync>,
}

pub struct Server {
    shared: Arc<Shared>,
    addr: SocketAddr,
    file: PathBuf,
}

impl Server {
    /// Listen on the loopback address and publish where in `~/.config/handset/ctl`.
    pub fn start(slot: Arc<Mutex<Slot>>, notify: impl Fn() + Send + Sync + 'static) -> std::io::Result<Server> {
        Self::start_in(&crate::settings::dir(), slot, notify)
    }

    fn start_in(
        dir: &Path,
        slot: Arc<Mutex<Slot>>,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> std::io::Result<Server> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let addr = listener.local_addr()?;
        let shared = Arc::new(Shared {
            token: random_hex(16),
            slot,
            target: Mutex::new(None),
            last: Mutex::new(None),
            stop: AtomicBool::new(false),
            notify: Box::new(notify),
        });
        let file = dir.join("ctl");
        write_private(&file, &format!("port={}\ntoken={}\npid={}\n", addr.port(), shared.token, std::process::id()))?;
        let s = shared.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                if s.stop.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(conn) = conn {
                    handle(&s, conn);
                }
            }
        });
        Ok(Server { shared, addr, file })
    }

    /// The phone now being mirrored, or none.
    pub fn set_target(&self, target: Option<Target>) {
        *self.shared.target.lock().unwrap() = target;
    }

    /// Something has used the socket a moment ago: the window says so.
    pub fn active(&self) -> bool {
        self.shared.last.lock().unwrap().is_some_and(|t| t.elapsed() < ACTIVE_FOR)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200)); // wakes the accept loop
        let _ = std::fs::remove_file(&self.file);
    }
}

fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)?.write_all(text.as_bytes())
}

/// A secret nobody can guess: the system's randomness, or, where there is none to read, what the standard
/// library seeds its hash maps with (which also comes from the system).
fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut buf)).is_err() {
        use std::hash::{BuildHasher, Hasher};
        for (i, chunk) in buf.chunks_mut(8).enumerate() {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_usize(i);
            h.write_u32(std::process::id());
            chunk.copy_from_slice(&h.finish().to_le_bytes()[..chunk.len()]);
        }
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Same answer in the same time whatever the mismatch.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0, |d, (x, y)| d | (x ^ y)) == 0
}

fn handle(s: &Shared, conn: TcpStream) {
    let _ = conn.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = conn.set_write_timeout(Some(Duration::from_secs(10)));
    let mut line = Vec::new();
    let mut reader = BufReader::new(&conn);
    if reader.by_ref().take(MAX_LINE).read_until(b'\n', &mut line).is_err() || line.last() != Some(&b'\n') {
        return;
    }
    let reply = match std::str::from_utf8(&line) {
        Ok(text) => match text.trim_end_matches(['\r', '\n']).split_once(' ') {
            Some((token, request)) if same(token, &s.token) => {
                *s.last.lock().unwrap() = Some(Instant::now());
                (s.notify)();
                parse(request).and_then(|c| run(s, c))
            }
            // do not say whether the secret or the request was wrong
            _ => Err("not allowed".to_string()),
        },
        Err(_) => Err("request is not text".to_string()),
    };
    let mut out = &conn;
    let _ = match reply {
        Ok(body) => out.write_all(format!("ok {}\n", body.len()).as_bytes()).and_then(|_| out.write_all(&body)),
        Err(msg) => out.write_all(format!("err {}\n", msg.replace('\n', " ")).as_bytes()),
    };
}

/// The phone, its size and a copy of its controls, taken so no lock is held while a gesture plays.
struct Handle {
    control: Option<Control>,
    name: String,
    serial: String,
    size: (u16, u16),
}

fn handle_of(s: &Shared) -> Result<Handle, String> {
    let guard = s.target.lock().unwrap();
    let t = guard.as_ref().ok_or("no phone is being mirrored")?;
    let size = {
        let slot = s.slot.lock().unwrap();
        (slot.yuv.width.min(u16::MAX as u32) as u16, slot.yuv.height.min(u16::MAX as u32) as u16)
    };
    Ok(Handle {
        control: t.control.as_ref().and_then(Control::try_clone),
        name: t.name.clone(),
        serial: t.serial.clone(),
        size,
    })
}

fn run(s: &Shared, cmd: Command) -> Result<Vec<u8>, String> {
    let mut h = handle_of(s)?;
    let (w, ht) = h.size;
    let inside = |x: i32, y: i32| {
        if w == 0 {
            Err("no picture yet".to_string())
        } else if x < 0 || y < 0 || x >= w as i32 || y >= ht as i32 {
            Err(format!("({x}, {y}) is outside the screen, which is {w}x{ht}"))
        } else {
            Ok(())
        }
    };
    let text = |t: String| Ok(t.into_bytes());
    match cmd {
        Command::Info => text(format!(
            "device {}\nserial {}\nplatform {}\nwidth {w}\nheight {ht}\ncontrol {}\n",
            h.name,
            h.serial,
            if h.control.is_some() { "android" } else { "ios" },
            if h.control.is_some() { "yes" } else { "no" }
        )),
        Command::Screenshot => {
            let slot = s.slot.lock().unwrap();
            png(&slot.yuv).map_err(|e| format!("screenshot failed: {e}"))
        }
        other => {
            let c = h.control.as_mut().ok_or("this phone cannot be controlled (an iPhone or iPad is view only)")?;
            let io = |e: std::io::Error| format!("the phone did not take it: {e}");
            match other {
                Command::Tap { x, y } => {
                    inside(x, y)?;
                    press(c, x, y, Duration::from_millis(40), w, ht).map_err(io)?;
                }
                Command::Hold { x, y, ms } => {
                    inside(x, y)?;
                    press(c, x, y, Duration::from_millis(ms), w, ht).map_err(io)?;
                }
                Command::Swipe { from, to, ms } => {
                    inside(from.0, from.1)?;
                    inside(to.0, to.1)?;
                    swipe(c, from, to, Duration::from_millis(ms), w, ht).map_err(io)?;
                }
                Command::Scroll { x, y, h: dh, v } => {
                    inside(x, y)?;
                    c.scroll(x, y, w, ht, dh, v).map_err(io)?;
                }
                Command::Text(t) => c.text(&t).map_err(io)?,
                Command::Key(k) => c.key(k).map_err(io)?,
                Command::Rotate => c.rotate_device().map_err(io)?,
                Command::Open(app) => c.start_app(&app).map_err(io)?,
                Command::Notifications => c.expand_notifications().map_err(io)?,
                Command::Collapse => c.collapse_panels().map_err(io)?,
                Command::Info | Command::Screenshot => unreachable!(),
            }
            Ok(Vec::new())
        }
    }
}

fn press(c: &mut Control, x: i32, y: i32, held: Duration, w: u16, h: u16) -> std::io::Result<()> {
    c.touch_as(POINTER, Touch::Down, x, y, w, h)?;
    std::thread::sleep(held);
    c.touch_as(POINTER, Touch::Up, x, y, w, h)
}

/// A finger that goes from one point to another in `over`, and is lifted even if the phone stops listening.
fn swipe(c: &mut Control, from: (i32, i32), to: (i32, i32), over: Duration, w: u16, h: u16) -> std::io::Result<()> {
    let steps = (over.as_millis() / 16).clamp(2, 600) as i32;
    c.touch_as(POINTER, Touch::Down, from.0, from.1, w, h)?;
    let mut at = from;
    let mut result = Ok(());
    for i in 1..=steps {
        std::thread::sleep(over / steps as u32);
        at = (from.0 + (to.0 - from.0) * i / steps, from.1 + (to.1 - from.1) * i / steps);
        if let Err(e) = c.touch_as(POINTER, Touch::Move, at.0, at.1, w, h) {
            result = Err(e);
            break;
        }
    }
    let up = c.touch_as(POINTER, Touch::Up, at.0, at.1, w, h);
    result.and(up)
}

/// The decoded picture as RGBA, the way the shader reads it: BT.709, limited range.
fn rgba(y: &Yuv) -> Vec<u8> {
    let (w, h) = (y.width as usize, y.height as usize);
    let mut out = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        for col in 0..w {
            let l = y.y.get(row * y.stride_y as usize + col).copied().unwrap_or(16) as f32;
            let at = (row / 2) * y.stride_uv as usize + col / 2;
            let cb = y.u.get(at).copied().unwrap_or(128) as f32;
            let cr = y.v.get(at).copied().unwrap_or(128) as f32;
            let l = (l - 16.0) * (255.0 / 219.0);
            let (u, v) = ((cb - 128.0) * (255.0 / 224.0), (cr - 128.0) * (255.0 / 224.0));
            for c in [l + 1.5748 * v, l - 0.1873 * u - 0.4681 * v, l + 1.8556 * u] {
                out.push(c.clamp(0.0, 255.0).round() as u8);
            }
            out.push(255);
        }
    }
    out
}

fn png(y: &Yuv) -> Result<Vec<u8>, String> {
    if y.width == 0 || y.height == 0 {
        return Err("no picture yet".into());
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, y.width, y.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(&rgba(y)).map_err(|e| e.to_string())?;
    w.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

// ---- the `handset ctl` command ----

/// `handset ctl <command>`: returns the exit code.
pub fn client(args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "help" || args[0] == "-h" || args[0] == "--help" {
        println!(
            "usage: handset ctl <command>\n{HELP}\n\nStart Handset with --ctl first (or put ctl=on in ~/.config/handset/settings)."
        );
        return if args.is_empty() { 2 } else { 0 };
    }
    let (request, save) = if args[0] == "screenshot" {
        match args.get(1) {
            Some(to) if args.len() == 2 => ("screenshot".to_string(), Some(to.clone())),
            _ => {
                eprintln!("handset ctl screenshot needs a file name, or - for standard output");
                return 2;
            }
        }
    } else {
        (args.join(" "), None)
    };
    match send(&crate::settings::dir().join("ctl"), &request) {
        Ok(body) => match save {
            Some(to) if to == "-" => std::io::stdout().write_all(&body).map_or(1, |_| 0),
            Some(to) => match std::fs::write(&to, &body) {
                Ok(()) => {
                    println!("{to}");
                    0
                }
                Err(e) => {
                    eprintln!("{to}: {e}");
                    1
                }
            },
            None => {
                print!("{}", String::from_utf8_lossy(&body));
                0
            }
        },
        Err(e) => {
            eprintln!("handset ctl: {e}");
            1
        }
    }
}

fn send(file: &Path, request: &str) -> Result<Vec<u8>, String> {
    let text = std::fs::read_to_string(file).map_err(|_| {
        "Handset is not taking commands. Start it with `handset --ctl`, or put ctl=on in ~/.config/handset/settings."
            .to_string()
    })?;
    let field = |k: &str| text.lines().find_map(|l| l.strip_prefix(k)?.strip_prefix('=')).map(str::trim);
    let (Some(port), Some(token)) = (field("port").and_then(|p| p.parse::<u16>().ok()), field("token")) else {
        return Err(format!("{} is not readable; restart Handset", file.display()));
    };
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut conn = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .map_err(|_| "Handset is not running (a stale file was left); start it with `handset --ctl`".to_string())?;
    // a swipe takes up to ten seconds
    conn.set_read_timeout(Some(Duration::from_secs(30))).map_err(|e| e.to_string())?;
    conn.write_all(format!("{token} {request}\n").as_bytes()).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(conn);
    let mut head = String::new();
    reader.read_line(&mut head).map_err(|e| e.to_string())?;
    let head = head.trim_end();
    if let Some(n) = head.strip_prefix("ok ") {
        let n: usize = n.parse().map_err(|_| "garbled answer".to_string())?;
        let mut body = vec![0u8; n];
        reader.read_exact(&mut body).map_err(|e| e.to_string())?;
        Ok(body)
    } else {
        Err(head.strip_prefix("err ").unwrap_or("garbled answer").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_read_strictly() {
        assert_eq!(parse("tap 10 20"), Ok(Command::Tap { x: 10, y: 20 }));
        assert_eq!(parse("hold 1 2"), Ok(Command::Hold { x: 1, y: 2, ms: 600 }));
        assert_eq!(parse("hold 1 2 99999"), Ok(Command::Hold { x: 1, y: 2, ms: MAX_MS }));
        assert_eq!(parse("swipe 1 2 3 4"), Ok(Command::Swipe { from: (1, 2), to: (3, 4), ms: 300 }));
        assert_eq!(parse("key back"), Ok(Command::Key(keycode::BACK)));
        assert_eq!(parse("type hello, world"), Ok(Command::Text("hello, world".into())));
        assert_eq!(parse("scroll 5 5 0 -40"), Ok(Command::Scroll { x: 5, y: 5, h: 0.0, v: -16.0 }));
        for bad in [
            "",
            "tap",
            "tap 1",
            "tap 1 2 3",
            "tap a b",
            "tap 1.5 2",
            "swipe 1 2 3",
            "key power2",
            "type",
            "open",
            "fly",
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        assert!(parse("type a\u{7}b").is_err());
        assert!(parse(&format!("type {}", "x".repeat(MAX_TEXT + 1))).is_err());
        assert!(parse(&format!("type {}", "x".repeat(MAX_TEXT))).is_ok());
    }

    #[test]
    fn the_secret_is_compared_whole() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("abc", "ab"));
        assert!(!same("", "a"));
        assert_ne!(random_hex(16), random_hex(16));
        assert_eq!(random_hex(16).len(), 32);
    }

    fn picture(w: u32, h: u32, luma: u8) -> Yuv {
        Yuv {
            width: w,
            height: h,
            y: vec![luma; (w * h) as usize],
            u: vec![128; (w * h / 4) as usize],
            v: vec![128; (w * h / 4) as usize],
            stride_y: w,
            stride_uv: w / 2,
        }
    }

    #[test]
    fn limited_range_black_and_white_come_out_black_and_white() {
        assert_eq!(&rgba(&picture(2, 2, 16))[..4], &[0, 0, 0, 255]);
        assert_eq!(&rgba(&picture(2, 2, 235))[..4], &[255, 255, 255, 255]);
        assert!(png(&Yuv::default()).is_err());
    }

    /// A phone that is just a socket we can read from.
    fn fake_phone() -> (Control, TcpStream) {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let near = TcpStream::connect(l.local_addr().unwrap()).unwrap();
        let (far, _) = l.accept().unwrap();
        far.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        (Control::new(near), far)
    }

    fn started(tag: &str) -> (Server, PathBuf, Arc<Mutex<Slot>>) {
        let dir = std::env::temp_dir().join(format!("handset-ctl-test-{}-{tag}", std::process::id()));
        let slot = Arc::new(Mutex::new(Slot { yuv: picture(8, 16, 235), ..Default::default() }));
        let server = Server::start_in(&dir, slot.clone(), || {}).unwrap();
        (server, dir, slot)
    }

    #[test]
    fn a_tap_reaches_the_phone_and_a_screenshot_is_a_png() {
        let (server, dir, _) = started("tap");
        let (control, mut phone) = fake_phone();
        server.set_target(Some(Target { control: Some(control), name: "Test".into(), serial: "S1".into() }));
        let file = dir.join("ctl");

        assert!(send(&file, "tap 4 8").unwrap().is_empty());
        let mut got = [0u8; 64];
        phone.read_exact(&mut got).unwrap(); // a down and an up, 32 bytes each
        assert_eq!((got[0], got[1]), (2, Touch::Down as u8));
        assert_eq!((got[32], got[33]), (2, Touch::Up as u8));
        assert_eq!(&got[2..10], &POINTER.to_be_bytes());

        let info = String::from_utf8(send(&file, "info").unwrap()).unwrap();
        assert!(info.contains("device Test") && info.contains("width 8") && info.contains("height 16"), "{info}");

        let shot = send(&file, "screenshot").unwrap();
        assert_eq!(&shot[1..4], b"PNG");

        assert!(send(&file, "tap 8 0").unwrap_err().contains("outside the screen, which is 8x16"));
        assert!(send(&file, "tap 1").is_err());
        drop(server);
        assert!(!file.exists(), "the file goes with the server");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_swipe_lifts_the_finger_where_it_ends() {
        let (server, dir, _) = started("swipe");
        let (control, mut phone) = fake_phone();
        server.set_target(Some(Target { control: Some(control), name: "T".into(), serial: "S".into() }));
        send(&dir.join("ctl"), "swipe 1 2 7 14 50").unwrap();
        let mut all = Vec::new();
        phone.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        let _ = phone.read_to_end(&mut all);
        let kinds: Vec<u8> = all.chunks(32).map(|m| m[1]).collect();
        assert_eq!(kinds.first(), Some(&(Touch::Down as u8)));
        assert_eq!(kinds.last(), Some(&(Touch::Up as u8)));
        assert!(kinds[1..kinds.len() - 1].iter().all(|k| *k == Touch::Move as u8) && kinds.len() > 3);
        let up = all.chunks(32).last().unwrap();
        assert_eq!(
            (i32::from_be_bytes(up[10..14].try_into().unwrap()), i32::from_be_bytes(up[14..18].try_into().unwrap())),
            (7, 14)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wrong_secret_gets_nothing_and_an_iphone_cannot_be_tapped() {
        let (server, dir, _) = started("auth");
        server.set_target(Some(Target { control: None, name: "iPhone".into(), serial: "u".into() }));
        let file = dir.join("ctl");
        let forged = std::fs::read_to_string(&file).unwrap().replace("token=", "token=0");
        let other = dir.join("forged");
        std::fs::write(&other, forged).unwrap();
        assert_eq!(send(&other, "info").unwrap_err(), "not allowed");
        assert!(send(&file, "tap 1 1").unwrap_err().contains("view only"));
        assert!(String::from_utf8(send(&file, "info").unwrap()).unwrap().contains("platform ios"));
        assert!(send(&file, "screenshot").is_ok());
        server.set_target(None);
        assert!(send(&file, "info").unwrap_err().contains("no phone"));
        assert!(server.active(), "a request makes the window say so");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn the_secret_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let (_server, dir, _) = started("mode");
        let mode = std::fs::metadata(dir.join("ctl")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
