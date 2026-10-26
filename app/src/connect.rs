//! The landing screen. While no phone is ready it is drawn inside the phone frame, as if it were the
//! phone's own screen. It says what Handset sees (nothing yet, a phone waiting for permission, a stale
//! connection), what to do about it, and offers the other ways in: Wi-Fi (phones found on the network, a
//! typed address, switching a USB phone) and QR code pairing. It goes away by itself once a phone is
//! ready. The picture is rasterized on the CPU with the overlay's bitmap font, with clickable buttons
//! whose rectangles are returned alongside it, and handed to the renderer like a video frame.

use crate::adb;
use crate::recent::Recent;
use crate::render::Yuv;
use crate::stream::UserEvent;
use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use qrcode::{Color, QrCode};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use winit::event_loop::EventLoopProxy;

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// Nothing to talk to yet.
    Looking,
    /// A phone is plugged in but has not allowed this Mac.
    Unauthorized(String),
    Offline(String),
    Permissions(String),
    AdbMissing,
    /// A phone is ready and mirroring is starting.
    Connecting(String),
    /// Starting failed; the text says why.
    Failed(String),
    /// A phone that was mirrored went away; Handset is waiting for it to come back.
    Lost(String),
    /// Mirroring was ended on purpose: nothing starts until a phone is chosen.
    Disconnected,
}

/// The phone to mirror: the one named `want` if given, else the first in the `device` state.
pub fn pick(list: &[(String, String)], want: Option<&str>) -> Option<String> {
    list.iter()
        .find(|(serial, state)| state == "device" && want.is_none_or(|w| w == serial))
        .map(|(serial, _)| serial.clone())
}

/// Is this adb serial a phone reached over the network (`ip:port` or an mDNS name)?
pub fn is_wireless(serial: &str) -> bool {
    serial.contains(':') || serial.contains("_adb-tls")
}

/// The address to keep reconnecting to after a phone mirrored over Wi-Fi went away. Phones found by
/// their mDNS name come back through adb's own auto-connect, so they need no retry.
pub fn retry_addr(serial: &str) -> Option<String> {
    (serial.contains(':') && !serial.contains("_adb-tls")).then(|| serial.to_string())
}

/// What to tell the person when no phone in `list` is ready.
pub fn status_from(list: &[(String, String)], want: Option<&str>) -> Status {
    let relevant = |serial: &String| want.is_none_or(|w| w == serial);
    let find =
        |pred: &dyn Fn(&str) -> bool| list.iter().find(|(s, st)| relevant(s) && pred(st)).map(|(s, _)| s.clone());
    if let Some(s) = find(&|st| st == "unauthorized") {
        Status::Unauthorized(s)
    } else if let Some(s) = find(&|st| st == "offline") {
        Status::Offline(s)
    } else if let Some(s) = find(&|st| st.contains("permissions")) {
        Status::Permissions(s)
    } else {
        Status::Looking
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    Home,
    Wifi,
    Qr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Open(View),
    Back,
    /// Connect to `ip:port`.
    Connect(String),
    TypeAddress,
    /// Switch this USB phone to adb over Wi-Fi.
    SwitchToWifi(String),
    /// Mirror this ready phone (when mirroring was ended on purpose).
    Mirror(String),
}

pub struct Button {
    /// x, y, width, height in the picture's pixels.
    pub rect: [u32; 4],
    pub label: String,
    pub action: Action,
}

/// Results of work done off the main thread.
#[derive(Debug)]
pub enum LandingEvent {
    /// Phones announcing themselves on the network.
    Nearby(Vec<adb::Service>),
    /// `adb pair` finished: the phone's IP, or why not.
    Paired(Result<String, String>),
    /// `adb connect` or the Wi-Fi switch finished: the serial to mirror, or why not.
    Connected(Result<String, String>),
    /// The address typed into the prompt, or `None` if it was cancelled.
    Typed(Option<String>),
}

static STOP: AtomicBool = AtomicBool::new(false);
/// Which poll thread is current: starting a new one retires any older one that has not noticed `STOP` yet.
static POLL: AtomicU64 = AtomicU64::new(0);
static MDNS: AtomicBool = AtomicBool::new(false);

/// Ask adb for the device list every second or so and send it to the app, until `stop_poll`. While the
/// Wi-Fi or QR views are open it also asks which phones are announcing themselves on the network.
pub fn spawn_poll(proxy: EventLoopProxy<UserEvent>) {
    STOP.store(false, Ordering::SeqCst);
    let id = POLL.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        while !STOP.load(Ordering::SeqCst) && POLL.load(Ordering::SeqCst) == id {
            let result = adb::list().map_err(|e| e.to_string());
            if proxy.send_event(UserEvent::Adb(result)).is_err() {
                break;
            }
            if MDNS.load(Ordering::SeqCst)
                && let Ok(services) = adb::mdns_services()
                && proxy.send_event(UserEvent::Landing(LandingEvent::Nearby(services))).is_err()
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(1200));
        }
    });
}

pub fn stop_poll() {
    STOP.store(true, Ordering::SeqCst);
}

/// Turn the network lookup on or off (only the Wi-Fi and QR views need it).
pub fn want_mdns(on: bool) {
    MDNS.store(on, Ordering::SeqCst);
}

fn send(proxy: &EventLoopProxy<UserEvent>, e: LandingEvent) {
    let _ = proxy.send_event(UserEvent::Landing(e));
}

pub fn connect_async(proxy: EventLoopProxy<UserEvent>, addr: String) {
    std::thread::spawn(move || send(&proxy, LandingEvent::Connected(adb::connect(&addr).map_err(|e| e.to_string()))));
}

pub fn wifi_async(proxy: EventLoopProxy<UserEvent>, usb_serial: String) {
    std::thread::spawn(move || {
        send(&proxy, LandingEvent::Connected(adb::wifi(&usb_serial).map_err(|e| e.to_string())))
    });
}

pub fn pair_async(proxy: EventLoopProxy<UserEvent>, addr: String, password: String) {
    std::thread::spawn(move || {
        let ip = addr.split(':').next().unwrap_or_default().to_string();
        send(&proxy, LandingEvent::Paired(adb::pair(&addr, &password).map(|_| ip).map_err(|e| e.to_string())));
    });
}

/// Ask for a phone address with a native prompt (the landing screen has no text field).
pub fn ask_address_async(proxy: EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || {
        let typed = ask_address();
        send(&proxy, LandingEvent::Typed(typed));
    });
}

#[cfg(target_os = "macos")]
fn ask_address() -> Option<String> {
    let script = "text returned of (display dialog \"Phone address, as ip or ip:port (shown under Wireless debugging):\" \
                  default answer \"192.168.\" buttons {\"Cancel\", \"Connect\"} default button \"Connect\" with title \"Handset\")";
    let out = std::process::Command::new("osascript").args(["-e", script]).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(windows)]
fn ask_address() -> Option<String> {
    let script = "Add-Type -AssemblyName Microsoft.VisualBasic; \
                  [Console]::OutputEncoding=[Text.Encoding]::UTF8; \
                  [Microsoft.VisualBasic.Interaction]::InputBox('Phone address, as ip or ip:port (shown under Wireless debugging):', 'Handset', '192.168.')";
    crate::tools::powershell(script, None).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(not(any(target_os = "macos", windows)))]
fn ask_address() -> Option<String> {
    None
}

/// A QR code that pairs a phone: scanning it on the phone (Wireless debugging > Pair device with QR code)
/// makes the phone announce a pairing service called `name`, which `password` unlocks.
pub struct Qr {
    pub name: String,
    pub password: String,
    pub width: usize,
    pub dark: Vec<bool>,
}

impl Qr {
    pub fn new() -> Result<Qr, String> {
        let name = format!("handset-{}", random_alnum(6).to_lowercase());
        let password = random_alnum(10);
        let code = QrCode::new(payload(&name, &password).as_bytes()).map_err(|e| e.to_string())?;
        let dark = code.to_colors().into_iter().map(|c| c == Color::Dark).collect();
        Ok(Qr { name, password, width: code.width(), dark })
    }
}

/// What the phone's QR scanner reads.
fn payload(name: &str, password: &str) -> String {
    format!("WIFI:T:ADB;S:{name};P:{password};;")
}

fn random_alnum(n: usize) -> String {
    use std::io::Read;
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789";
    let mut bytes = vec![0u8; n];
    if std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut bytes)).is_err() {
        // no system randomness: fall back to the clock, which is enough for a one-off pairing code
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (t >> (i * 5 % 100)) as u8 ^ (i as u8).wrapping_mul(37);
        }
    }
    bytes.iter().map(|b| CHARS[*b as usize % CHARS.len()] as char).collect()
}

// Handset's colours, taken from the app icon: its green, its teal, and the pale mint of its plate.
const BG: [u8; 3] = [8, 24, 22];
const SURFACE: [u8; 3] = [14, 54, 50];
const SURFACE_HOVER: [u8; 3] = [7, 112, 99];
const GREEN: [u8; 3] = [48, 221, 129];
const TEXT: [u8; 3] = [217, 245, 229];
const MUTED: [u8; 3] = [128, 166, 156];
const AMBER: [u8; 3] = [255, 190, 60];
const RED: [u8; 3] = [255, 110, 100];

static REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
static SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
static LOGO_PNG: &[u8] = include_bytes!("../packaging/icon/handset-icon-512.png");

/// How a run of text looks: weight, size in design units, colour.
#[derive(Clone, Copy)]
struct Style(Weight, f32, [u8; 3]);

#[derive(Clone, Copy, PartialEq)]
enum Weight {
    Regular,
    Semibold,
}

fn font(weight: Weight) -> &'static FontRef<'static> {
    static FONTS: OnceLock<[FontRef<'static>; 2]> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| {
        [
            FontRef::try_from_slice(REGULAR).expect("bundled Inter Regular"),
            FontRef::try_from_slice(SEMIBOLD).expect("bundled Inter SemiBold"),
        ]
    });
    &fonts[weight as usize]
}

/// The app icon, decoded once: width, height, straight-alpha RGBA.
fn logo() -> &'static (u32, u32, Vec<u8>) {
    static LOGO: OnceLock<(u32, u32, Vec<u8>)> = OnceLock::new();
    LOGO.get_or_init(|| {
        let mut reader = png::Decoder::new(std::io::Cursor::new(LOGO_PNG)).read_info().expect("bundled icon");
        let mut buf = vec![0; reader.output_buffer_size().expect("icon size")];
        let info = reader.next_frame(&mut buf).expect("bundled icon pixels");
        buf.truncate(info.buffer_size());
        (info.width, info.height, buf)
    })
}

/// Width of `text` in pixels at `px` size.
fn measure(weight: Weight, px: f32, text: &str) -> f32 {
    let scaled = font(weight).as_scaled(PxScale::from(px));
    let mut width = 0.0;
    let mut prev = None;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(p) = prev {
            width += scaled.kern(p, id);
        }
        width += scaled.h_advance(id);
        prev = Some(id);
    }
    width
}

/// Break `text` into lines no wider than `max_w` pixels, on spaces.
fn wrap(weight: Weight, px: f32, max_w: f32, text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if !line.is_empty() && measure(weight, px, &candidate) > max_w {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

struct Canvas {
    w: u32,
    h: u32,
    px: Vec<u8>,
    /// Pixels per design unit: the picture is designed at 776 wide.
    u: f32,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        let mut px = Vec::with_capacity((w * h * 3) as usize);
        for _ in 0..w * h {
            px.extend_from_slice(&BG);
        }
        Self { w, h, px, u: w as f32 / 776.0 }
    }

    /// Design units to pixels.
    fn d(&self, v: f32) -> f32 {
        v * self.u
    }

    /// Text sizes are a little larger than the layout units: the screen is small on a desktop.
    fn px(&self, size: f32) -> f32 {
        size * self.u * 1.18
    }

    /// Blend `c` over the pixel at (x, y) with coverage `a` (0 to 1).
    fn blend(&mut self, x: i64, y: i64, c: [u8; 3], a: f32) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 || a <= 0.0 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 3) as usize;
        let a = a.min(1.0);
        for (dst, src) in self.px[i..i + 3].iter_mut().zip(c) {
            *dst = (*dst as f32 * (1.0 - a) + src as f32 * a).round() as u8;
        }
    }

    /// Draw one line of text with its top at `y`; returns its width.
    fn text(&mut self, x: f32, y: f32, weight: Weight, size: f32, c: [u8; 3], text: &str) -> f32 {
        let px = self.px(size);
        let f = font(weight);
        let scaled = f.as_scaled(PxScale::from(px));
        let baseline = y + scaled.ascent();
        let mut caret = x;
        let mut prev = None;
        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(p) = prev {
                caret += scaled.kern(p, id);
            }
            let glyph = id.with_scale_and_position(PxScale::from(px), point(caret, baseline));
            if let Some(outline) = f.outline_glyph(glyph) {
                let b = outline.px_bounds();
                outline.draw(|gx, gy, cov| self.blend(b.min.x as i64 + gx as i64, b.min.y as i64 + gy as i64, c, cov));
            }
            caret += scaled.h_advance(id);
            prev = Some(id);
        }
        caret - x
    }

    /// Line height for `size`.
    fn line(&self, size: f32) -> f32 {
        self.px(size) * 1.38
    }

    /// Text centred on `cx`.
    fn text_centered(&mut self, cx: f32, y: f32, weight: Weight, size: f32, c: [u8; 3], text: &str) {
        let w = measure(weight, self.px(size), text);
        self.text(cx - w / 2.0, y, weight, size, c, text);
    }

    /// A wrapped paragraph; returns the y below it.
    fn para(&mut self, x: f32, y: f32, max_w: f32, style: Style, text: &str) -> f32 {
        let Style(weight, size, colour) = style;
        let mut y = y;
        for line in wrap(weight, self.px(size), max_w, text) {
            self.text(x, y, weight, size, colour, &line);
            y += self.line(size);
        }
        y
    }

    fn disc(&mut self, cx: f32, cy: f32, r: f32, c: [u8; 3]) {
        for y in (cy - r - 1.0).floor() as i64..=(cy + r + 1.0).ceil() as i64 {
            for x in (cx - r - 1.0).floor() as i64..=(cx + r + 1.0).ceil() as i64 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                self.blend(x, y, c, r - d + 0.5);
            }
        }
    }

    /// A rounded rectangle, filled, with an optional border drawn inside it.
    fn round_rect(&mut self, rect: [f32; 4], radius: f32, fill: Option<[u8; 3]>, border: Option<([u8; 3], f32)>) {
        let [x, y, w, h] = rect;
        let r = radius.min(w / 2.0).min(h / 2.0);
        for py in y.floor() as i64..(y + h).ceil() as i64 {
            for px in x.floor() as i64..(x + w).ceil() as i64 {
                // distance outside the rounded shape (negative inside)
                let qx = ((px as f32 + 0.5 - (x + w / 2.0)).abs() - (w / 2.0 - r)).max(0.0);
                let qy = ((py as f32 + 0.5 - (y + h / 2.0)).abs() - (h / 2.0 - r)).max(0.0);
                let dist = (qx * qx + qy * qy).sqrt() - r;
                let inside = (0.5 - dist).clamp(0.0, 1.0);
                if let Some(f) = fill {
                    self.blend(px, py, f, inside);
                }
                if let Some((bc, bw)) = border {
                    let ring = inside.min((dist + bw + 0.5).clamp(0.0, 1.0));
                    self.blend(px, py, bc, ring);
                }
            }
        }
    }

    /// The app icon, `size` design units wide, with its top-left at (x, y).
    fn logo(&mut self, x: f32, y: f32, size: f32) {
        let (lw, lh, rgba) = logo();
        let side = self.d(size).round().max(1.0) as u32;
        for ty in 0..side {
            for tx in 0..side {
                // average the source pixels under this target pixel, weighted by alpha
                let (x0, x1) = (tx * lw / side, ((tx + 1) * lw / side).max(tx * lw / side + 1));
                let (y0, y1) = (ty * lh / side, ((ty + 1) * lh / side).max(ty * lh / side + 1));
                let (mut r, mut g, mut b, mut a, mut n) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
                for sy in y0..y1.min(*lh) {
                    for sx in x0..x1.min(*lw) {
                        let i = ((sy * lw + sx) * 4) as usize;
                        let al = rgba[i + 3] as f32 / 255.0;
                        r += rgba[i] as f32 * al;
                        g += rgba[i + 1] as f32 * al;
                        b += rgba[i + 2] as f32 * al;
                        a += al;
                        n += 1.0;
                    }
                }
                if a > 0.0 {
                    let c = [(r / a).round() as u8, (g / a).round() as u8, (b / a).round() as u8];
                    self.blend(x as i64 + tx as i64, y as i64 + ty as i64, c, a / n);
                }
            }
        }
    }

    /// A pill button with a centred label that shrinks until it fits.
    fn button(&mut self, rect: [u32; 4], label: &str, hover: bool, quiet: bool) {
        let r = [rect[0] as f32, rect[1] as f32, rect[2] as f32, rect[3] as f32];
        let bw = self.d(3.0).max(1.0);
        if quiet {
            self.round_rect(r, r[3] / 2.0, hover.then_some(SURFACE), Some((if hover { GREEN } else { MUTED }, bw)));
        } else {
            let fill = if hover { SURFACE_HOVER } else { SURFACE };
            self.round_rect(r, r[3] / 2.0, Some(fill), Some((GREEN, bw)));
        }
        let mut size = 38.0;
        while size > 20.0 && measure(Weight::Semibold, self.px(size), label) > r[2] - self.d(56.0) {
            size -= 2.0;
        }
        let px = self.px(size);
        let top = r[1] + (r[3] - self.line(size) / 1.0) / 2.0 + self.d(size * 0.04);
        let _ = px;
        self.text_centered(r[0] + r[2] / 2.0, top, Weight::Semibold, size, TEXT, label);
    }
}

struct Words {
    /// Status line, its colour, and whether it ends in animated dots.
    status: String,
    colour: [u8; 3],
    animate: bool,
    /// Paragraphs, or numbered steps when `numbered`.
    body: Vec<String>,
    numbered: bool,
    /// Smaller print, in grey.
    hint: String,
    /// Which phone this is about, if any.
    phone: Option<String>,
}

fn words(status: &Status) -> Words {
    let w = |status: &str, colour, animate, body: &[&str], numbered, hint: &str, phone: Option<&String>| Words {
        status: status.to_string(),
        colour,
        animate,
        body: body.iter().map(|s| s.to_string()).collect(),
        numbered,
        hint: hint.to_string(),
        phone: phone.cloned(),
    };
    match status {
        Status::Looking => w(
            "Looking for a phone",
            AMBER,
            true,
            &["Plug it in with a USB cable", "Turn on USB debugging in Developer options", "Tap Allow on the phone"],
            true,
            "",
            None,
        ),
        Status::Unauthorized(s) => w(
            "Allow this Mac on the phone",
            AMBER,
            false,
            &["Unlock the phone and tap Allow on the \"Allow USB debugging?\" prompt."],
            false,
            "No prompt? Unplug the cable, turn USB debugging off and on, and plug it back in.",
            Some(s),
        ),
        Status::Offline(s) => w(
            "The phone is offline",
            RED,
            false,
            &["Unplug and replug the cable."],
            false,
            "If it stays offline, run adb kill-server and try again.",
            Some(s),
        ),
        Status::Permissions(s) => w(
            "No access to the phone",
            RED,
            false,
            &["adb reports a permissions problem.", "Try another cable or USB port."],
            false,
            "",
            Some(s),
        ),
        Status::AdbMissing => w(
            "adb is missing",
            RED,
            false,
            &["Handset needs Android platform-tools."],
            false,
            "Install it with: brew install --cask android-platform-tools",
            None,
        ),
        Status::Connecting(s) => w("Connecting", GREEN, true, &["Starting mirroring."], false, "", Some(s)),
        Status::Disconnected => {
            w("Disconnected", MUTED, false, &["Plug a phone in, or connect another way."], false, "", None)
        }
        Status::Lost(s) => {
            let body = if retry_addr(s).is_some() || s.contains("_adb-tls") {
                "Reconnecting. The phone and this Mac need to be on the same Wi-Fi."
            } else {
                "Plug the phone back in."
            };
            w("Connection lost", AMBER, true, &[body], false, "Mirroring starts again by itself.", Some(s))
        }
        Status::Failed(msg) => {
            let short: String = msg.chars().take(140).collect();
            w("Could not start mirroring", RED, false, &[&short], false, "Replug the phone to try again.", None)
        }
    }
}

/// Everything the screen shows.
pub struct Model<'a> {
    pub status: &'a Status,
    pub view: View,
    /// Animation step, one per half second.
    pub tick: u32,
    /// The button under the cursor.
    pub hover: Option<usize>,
    /// Phones announcing themselves that can be connected to.
    pub nearby: &'a [adb::Service],
    /// A USB phone that is ready but waiting in the Wi-Fi view, to offer to switch it.
    pub usb: Option<&'a str>,
    pub qr: Option<&'a Qr>,
    /// Phones connected to over Wi-Fi before, newest first.
    pub recent: &'a [Recent],
    /// A line of progress or trouble, shown in the Wi-Fi and QR views.
    pub note: Option<&'a str>,
}

pub struct Screen {
    pub rgb: Vec<u8>,
    pub buttons: Vec<Button>,
}

/// The screen as RGB, `w` by `h` pixels, and where its buttons are.
pub fn render(w: u32, h: u32, m: &Model) -> Screen {
    let mut c = Canvas::new(w, h);
    let u = c.u;
    let margin = 56.0 * u;
    let content_w = w as f32 - 2.0 * margin;
    let (btn_h, gap) = (100.0 * u, 24.0 * u);
    let mut buttons: Vec<Button> = Vec::new();
    let dots = ".".repeat((m.tick % 4) as usize);

    // header: the icon large on the home screen, small beside the title elsewhere
    let mut y;
    if m.view == View::Home {
        c.logo((776.0 - 190.0) / 2.0 * 1.0, 86.0, 190.0);
        y = 300.0 * u;
        c.text_centered(w as f32 / 2.0, y, Weight::Semibold, 54.0, TEXT, "Connect a phone");
        y += c.line(54.0) + 14.0 * u;
    } else {
        c.logo(56.0, 70.0, 92.0);
        let title = if m.view == View::Wifi { "Phones on Wi-Fi" } else { "Scan to pair" };
        c.text(
            margin + 92.0 * u + 24.0 * u,
            70.0 * u + (92.0 * u - c.line(46.0)) / 2.0,
            Weight::Semibold,
            46.0,
            TEXT,
            title,
        );
        y = 200.0 * u;
    }

    // bottom buttons are stacked upwards from the bottom edge; `bottom` is the top of the stack
    let mut bottom = h as f32 - 70.0 * u;
    let mut stack = |label: &str, action: Action, buttons: &mut Vec<Button>| {
        bottom -= btn_h;
        buttons.push(Button {
            rect: [margin as u32, bottom as u32, content_w as u32, btn_h as u32],
            label: label.to_string(),
            action,
        });
        bottom -= gap;
    };

    match m.view {
        View::Home => {
            stack("QR code", Action::Open(View::Qr), &mut buttons);
            stack("Wi-Fi", Action::Open(View::Wifi), &mut buttons);
            if let Some(phone) = m.usb {
                stack(&format!("Mirror {phone}"), Action::Mirror(phone.to_string()), &mut buttons);
            }
            if let Some(last) = m.recent.first() {
                stack(&format!("Reconnect to {}", last.name), Action::Connect(last.addr.clone()), &mut buttons);
            }
            let words = words(m.status);
            let lit = !words.animate || m.tick.is_multiple_of(2);
            let dot = if lit { words.colour } else { [words.colour[0] / 3, words.colour[1] / 3, words.colour[2] / 3] };
            let text_x = margin + 38.0 * u;
            let status = if words.animate { format!("{}{dots}", words.status) } else { words.status.clone() };
            c.disc(margin + 11.0 * u, y + c.line(36.0) / 2.0, 10.0 * u, dot);
            y = c.para(text_x, y, content_w - 38.0 * u, Style(Weight::Semibold, 36.0, words.colour), &status)
                + 26.0 * u;
            for (i, item) in words.body.iter().enumerate() {
                if words.numbered {
                    let top = y;
                    let tx = margin + 66.0 * u;
                    let lines = wrap(Weight::Regular, c.px(32.0), content_w - 66.0 * u, item).len().max(1) as f32;
                    let row = (lines * c.line(32.0)).max(48.0 * u);
                    c.disc(margin + 24.0 * u, top + 24.0 * u, 24.0 * u, SURFACE);
                    c.text_centered(
                        margin + 24.0 * u,
                        top + 24.0 * u - c.line(28.0) / 2.0,
                        Weight::Semibold,
                        28.0,
                        GREEN,
                        &(i + 1).to_string(),
                    );
                    c.para(
                        tx,
                        top + 24.0 * u - c.line(32.0) / 2.0,
                        content_w - 66.0 * u,
                        Style(Weight::Regular, 32.0, TEXT),
                        item,
                    );
                    y += row + 22.0 * u;
                } else {
                    y = c.para(margin, y, content_w, Style(Weight::Regular, 32.0, TEXT), item) + 10.0 * u;
                }
            }
            if let Some(phone) = &words.phone {
                y = c.para(
                    margin,
                    y + 6.0 * u,
                    content_w,
                    Style(Weight::Regular, 28.0, MUTED),
                    &format!("Phone {phone}"),
                );
            }
            // small print only while it clears the buttons and their lead-in
            let room = bottom - 70.0 * u;
            for line in wrap(Weight::Regular, c.px(28.0), content_w, &words.hint) {
                if y + c.line(28.0) > room {
                    break;
                }
                c.text(margin, y + 6.0 * u, Weight::Regular, 28.0, MUTED, &line);
                y += c.line(28.0);
            }
            c.text_centered(
                w as f32 / 2.0,
                bottom - 14.0 * u - c.line(28.0),
                Weight::Regular,
                28.0,
                MUTED,
                "No cable? Connect another way",
            );
        }
        View::Wifi => {
            stack("Back", Action::Back, &mut buttons);
            if let Some(usb) = m.usb.filter(|u| !is_wireless(u)) {
                stack(&format!("Switch {usb} to Wi-Fi"), Action::SwitchToWifi(usb.to_string()), &mut buttons);
            }
            stack("Type an address", Action::TypeAddress, &mut buttons);
            y = c.para(margin, y, content_w, Style(Weight::Regular, 32.0, MUTED), "Tap a paired phone to connect")
                + 22.0 * u;
            let connectable: Vec<&adb::Service> =
                m.nearby.iter().filter(|n| n.kind == adb::ServiceKind::Connect).take(2).collect();
            let recents: Vec<&Recent> = m
                .recent
                .iter()
                .filter(|r| !connectable.iter().any(|n| n.addr == r.addr))
                .take(3usize.saturating_sub(connectable.len()))
                .collect();
            if connectable.is_empty() && recents.is_empty() {
                c.text(margin, y, Weight::Semibold, 34.0, AMBER, &format!("None found yet{dots}"));
                y += c.line(34.0) + 14.0 * u;
                c.para(
                    margin,
                    y,
                    content_w,
                    Style(Weight::Regular, 30.0, MUTED),
                    "Turn on Wireless debugging in Developer options. A new phone needs pairing once: use the QR code.",
                );
            }
            for n in connectable {
                buttons.push(Button {
                    rect: [margin as u32, y as u32, content_w as u32, btn_h as u32],
                    label: n.addr.clone(),
                    action: Action::Connect(n.addr.clone()),
                });
                y += btn_h + gap;
            }
            for r in recents {
                buttons.push(Button {
                    rect: [margin as u32, y as u32, content_w as u32, btn_h as u32],
                    label: format!("{}  {}", r.name, r.addr),
                    action: Action::Connect(r.addr.clone()),
                });
                y += btn_h + gap;
            }
            // notes sit just above the buttons: progress or trouble first, then the USB hint above it
            let mut ny = bottom;
            if let Some(note) = m.note {
                let n = wrap(Weight::Semibold, c.px(30.0), content_w, note).len().min(3) as f32;
                ny -= n * c.line(30.0) + 10.0 * u;
                c.para(margin, ny, content_w, Style(Weight::Semibold, 30.0, AMBER), note);
            }
            if m.usb.is_some_and(|u| !is_wireless(u)) {
                ny -= c.line(30.0) + 10.0 * u;
                c.text(margin, ny, Weight::Regular, 30.0, MUTED, "A USB phone is plugged in.");
            }
        }
        View::Qr => {
            stack("Back", Action::Back, &mut buttons);
            if let Some(qr) = m.qr {
                let quiet = 3;
                let modules = qr.width as u32 + 2 * quiet;
                // leave room below for the steps and the status
                let side = content_w.min(bottom - y - 330.0 * u).max(modules as f32);
                let px = (side / modules as f32).floor().max(1.0);
                let size = px * modules as f32;
                let x0 = (w as f32 - size) / 2.0;
                c.round_rect([x0, y, size, size], 28.0 * u, Some([255, 255, 255]), None);
                for my in 0..qr.width {
                    for mx in 0..qr.width {
                        if qr.dark[my * qr.width + mx] {
                            let (rx, ry) = (x0 + (mx as u32 + quiet) as f32 * px, y + (my as u32 + quiet) as f32 * px);
                            for yy in ry as u32..(ry + px) as u32 {
                                for xx in rx as u32..(rx + px) as u32 {
                                    c.blend(xx as i64, yy as i64, [0, 0, 0], 1.0);
                                }
                            }
                        }
                    }
                }
                y += size + 30.0 * u;
            } else {
                c.text(margin, y, Weight::Semibold, 34.0, RED, "Could not make a QR code");
                y += c.line(34.0) * 2.0;
            }
            y = c.para(
                margin,
                y,
                content_w,
                Style(Weight::Regular, 30.0, MUTED),
                "On the phone: Developer options, Wireless debugging, Pair device with QR code.",
            ) + 14.0 * u;
            let (text, colour) = match m.note {
                Some(n) => (n.to_string(), AMBER),
                None => (format!("Waiting for the phone to scan{dots}"), TEXT),
            };
            c.para(margin, y, content_w, Style(Weight::Semibold, 32.0, colour), &text);
        }
    }

    // read top to bottom, and indexed that way for hover
    buttons.sort_by_key(|b| b.rect[1]);
    for (i, b) in buttons.iter().enumerate() {
        c.button(b.rect, &b.label, m.hover == Some(i), matches!(b.action, Action::Back));
    }
    Screen { rgb: c.px, buttons }
}

/// RGB to I420 the way the renderer reads it: BT.709, limited range, chroma averaged over 2x2.
pub fn to_yuv(rgb: &[u8], w: u32, h: u32) -> Yuv {
    let (w, h) = (w & !1, h & !1);
    let src = |x: u32, y: u32| {
        let i = ((y * w + x) * 3) as usize;
        [rgb[i] as f32, rgb[i + 1] as f32, rgb[i + 2] as f32]
    };
    let mut yuv = Yuv { width: w, height: h, stride_y: w, stride_uv: w / 2, ..Default::default() };
    yuv.y = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let [r, g, b] = src(x, y);
            let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            yuv.y.push((16.0 + luma * 219.0 / 255.0).round().clamp(16.0, 235.0) as u8);
        }
    }
    for y in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let mut avg = [0.0f32; 3];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let p = src(x + dx, y + dy);
                for k in 0..3 {
                    avg[k] += p[k] / 4.0;
                }
            }
            let [r, g, b] = avg;
            let cb = -0.1146 * r - 0.3854 * g + 0.5 * b;
            let cr = 0.5 * r - 0.4542 * g - 0.0458 * b;
            yuv.u.push((128.0 + cb * 224.0 / 255.0).round().clamp(16.0, 240.0) as u8);
            yuv.v.push((128.0 + cr * 224.0 / 255.0).round().clamp(16.0, 240.0) as u8);
        }
    }
    yuv
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    fn svc(kind: adb::ServiceKind, addr: &str) -> adb::Service {
        adb::Service { name: "adb-X".into(), kind, addr: addr.into() }
    }

    #[test]
    fn picks_a_ready_phone() {
        let list = l(&[("A", "unauthorized"), ("B", "device"), ("C", "device")]);
        assert_eq!(pick(&list, None), Some("B".into()));
        assert_eq!(pick(&list, Some("C")), Some("C".into()));
        assert_eq!(pick(&list, Some("A")), None);
        assert_eq!(pick(&[], None), None);
    }

    #[test]
    fn retries_only_phones_with_an_address() {
        assert_eq!(retry_addr("192.168.0.20:5555"), Some("192.168.0.20:5555".into()));
        assert_eq!(retry_addr("adb-W82202003817-MPn68n._adb-tls-connect._tcp"), None);
        assert_eq!(retry_addr("W82202003817"), None);
    }

    #[test]
    fn a_paused_screen_offers_the_ready_phone() {
        let (w, h) = (776u32, 1560u32);
        let status = Status::Disconnected;
        let home = render(w, h, &model(&status, View::Home, &[], Some("W82202003817"), None, None));
        let mirror: Vec<_> = home.buttons.iter().filter(|b| matches!(b.action, Action::Mirror(_))).collect();
        assert_eq!(mirror.len(), 1);
        assert_eq!(mirror[0].action, Action::Mirror("W82202003817".into()));
        // nothing ready: no mirror button
        let none = render(w, h, &model(&status, View::Home, &[], None, None, None));
        assert!(!none.buttons.iter().any(|b| matches!(b.action, Action::Mirror(_))));
        // a phone already on Wi-Fi is not offered for switching to Wi-Fi
        let wifi = render(w, h, &model(&status, View::Wifi, &[], Some("192.168.0.20:5555"), None, None));
        assert!(!wifi.buttons.iter().any(|b| matches!(b.action, Action::SwitchToWifi(_))));
        let usb = render(w, h, &model(&status, View::Wifi, &[], Some("W82202003817"), None, None));
        assert!(usb.buttons.iter().any(|b| matches!(b.action, Action::SwitchToWifi(_))));
    }

    #[test]
    fn tells_wireless_from_usb() {
        assert!(is_wireless("192.168.0.20:5555"));
        assert!(is_wireless("adb-W82202003817-MPn68n._adb-tls-connect._tcp"));
        assert!(!is_wireless("W82202003817"));
    }

    #[test]
    fn describes_what_is_wrong() {
        assert_eq!(status_from(&[], None), Status::Looking);
        assert_eq!(status_from(&l(&[("A", "unauthorized")]), None), Status::Unauthorized("A".into()));
        assert_eq!(status_from(&l(&[("A", "offline")]), None), Status::Offline("A".into()));
        assert_eq!(status_from(&l(&[("A", "no permissions (x)")]), None), Status::Permissions("A".into()));
        // a phone the person did not ask for is not relevant
        assert_eq!(status_from(&l(&[("A", "unauthorized")]), Some("B")), Status::Looking);
    }

    #[test]
    fn wraps_to_a_pixel_width() {
        let text = "Turn on USB debugging in Developer options";
        let narrow = wrap(Weight::Regular, 32.0, 200.0, text);
        assert!(narrow.len() > 1);
        assert!(narrow.iter().all(|l| measure(Weight::Regular, 32.0, l) <= 200.0 || !l.contains(' ')));
        assert_eq!(narrow.join(" "), text);
        assert_eq!(wrap(Weight::Regular, 32.0, 10_000.0, text), vec![text]);
        assert!(wrap(Weight::Regular, 32.0, 100.0, "").is_empty());
        // bold text is wider than regular at the same size
        assert!(measure(Weight::Semibold, 32.0, "Connect") > measure(Weight::Regular, 32.0, "Connect"));
    }

    #[test]
    fn the_icon_and_fonts_load() {
        let (w, h, rgba) = logo();
        assert_eq!((*w, *h), (512, 512));
        assert_eq!(rgba.len(), 512 * 512 * 4);
        assert!(measure(Weight::Regular, 30.0, "Wi-Fi") > 30.0);
    }

    #[test]
    fn white_converts_to_limited_range_white() {
        let (w, h) = (4, 4);
        let y = to_yuv(&vec![255u8; (w * h * 3) as usize], w, h);
        assert!(y.y.iter().all(|&v| v == 235));
        assert!(y.u.iter().all(|&v| v == 128) && y.v.iter().all(|&v| v == 128));
        assert_eq!((y.u.len(), y.stride_uv), (4, 2));
    }

    #[test]
    fn the_qr_payload_is_what_android_scans() {
        assert_eq!(payload("handset-abc123", "Secret42"), "WIFI:T:ADB;S:handset-abc123;P:Secret42;;");
        let qr = Qr::new().unwrap();
        assert!(qr.name.starts_with("handset-") && qr.name.len() == 14);
        assert_eq!(qr.password.len(), 10);
        assert_eq!(qr.dark.len(), qr.width * qr.width);
        assert!(qr.dark.iter().any(|&d| d) && qr.dark.iter().any(|&d| !d));
        // a second code is different
        assert_ne!(Qr::new().unwrap().password, qr.password);
    }

    fn model<'a>(
        status: &'a Status,
        view: View,
        nearby: &'a [adb::Service],
        usb: Option<&'a str>,
        qr: Option<&'a Qr>,
        note: Option<&'a str>,
    ) -> Model<'a> {
        Model { status, view, tick: 1, hover: None, nearby, usb, qr, recent: &[], note }
    }

    #[test]
    fn buttons_stay_inside_the_picture_and_do_not_overlap() {
        let (w, h) = (720u32, 1560u32);
        let qr = Qr::new().unwrap();
        let nearby = [
            svc(adb::ServiceKind::Connect, "192.168.0.20:41789"),
            svc(adb::ServiceKind::Connect, "192.168.0.31:40000"),
            svc(adb::ServiceKind::Pairing, "192.168.0.9:1"),
        ];
        let looking = Status::Looking;
        for m in [
            model(&looking, View::Home, &[], None, None, None),
            model(&looking, View::Wifi, &[], None, None, None),
            model(&looking, View::Wifi, &nearby, Some("W82202003817"), None, Some("Connecting to 192.168.0.20...")),
            model(&looking, View::Qr, &[], None, Some(&qr), None),
            model(&looking, View::Qr, &[], None, None, Some("PAIRING...")),
        ] {
            let screen = render(w, h, &m);
            assert_eq!(screen.rgb.len(), (w * h * 3) as usize);
            assert!(!screen.buttons.is_empty());
            for (i, b) in screen.buttons.iter().enumerate() {
                let [x, y, bw, bh] = b.rect;
                assert!(x + bw <= w && y + bh <= h, "{:?} outside", b.action);
                for o in &screen.buttons[i + 1..] {
                    let [ox, oy, ow, oh] = o.rect;
                    let apart = y + bh <= oy || oy + oh <= y || x + bw <= ox || ox + ow <= x;
                    assert!(apart, "{:?} overlaps {:?}", b.action, o.action);
                }
            }
        }
        // the Wi-Fi view lists the connectable phones, not the pairing entry
        let screen = render(w, h, &model(&looking, View::Wifi, &nearby, None, None, None));
        let connects: Vec<_> = screen.buttons.iter().filter(|b| matches!(b.action, Action::Connect(_))).collect();
        assert_eq!(connects.len(), 2);
    }

    #[test]
    fn recent_phones_are_offered_on_home_and_in_the_wifi_list() {
        let (w, h) = (776u32, 1560u32);
        let recent = [
            Recent { addr: "192.168.0.20:5555".into(), name: "UNIWA W888".into(), at: 2 },
            Recent { addr: "192.168.0.31:5555".into(), name: "Pixel".into(), at: 1 },
        ];
        let looking = Status::Looking;
        let mut m = model(&looking, View::Home, &[], None, None, None);
        m.recent = &recent;
        let home = render(w, h, &m);
        let reconnect: Vec<_> = home.buttons.iter().filter(|b| b.label.starts_with("Reconnect to")).collect();
        assert_eq!(reconnect.len(), 1);
        assert_eq!(reconnect[0].action, Action::Connect("192.168.0.20:5555".into()));

        // a recent phone that is also announcing itself is listed once
        let nearby = [svc(adb::ServiceKind::Connect, "192.168.0.20:5555")];
        let mut m = model(&looking, View::Wifi, &nearby, None, None, None);
        m.recent = &recent;
        let wifi = render(w, h, &m);
        let connects: Vec<_> = wifi.buttons.iter().filter(|b| matches!(b.action, Action::Connect(_))).collect();
        assert_eq!(connects.len(), 2);
        assert!(connects.iter().any(|b| b.label.contains("Pixel")));
        // and a phone only known from the recent list is not reported as "none found"
        let mut m = model(&looking, View::Wifi, &[], None, None, None);
        m.recent = &recent;
        assert_eq!(render(w, h, &m).buttons.iter().filter(|b| matches!(b.action, Action::Connect(_))).count(), 2);
    }

    /// Writes screens to `$HANDSET_TEST_OUT/connect-<n>.png` (if set) so they can be looked at.
    #[test]
    fn renders_every_screen() {
        let qr = Qr::new().unwrap();
        let nearby = [svc(adb::ServiceKind::Connect, "192.168.1.23:41789")];
        let looking = Status::Looking;
        let unauth = Status::Unauthorized("R58M12ABCDE".into());
        let failed = Status::Failed("Could not start mirroring: adb push: no such file".into());
        let lost = Status::Lost("192.168.0.20:5555".into());
        let disconnected = Status::Disconnected;
        let screens = [
            model(&disconnected, View::Home, &[], Some("W82202003817"), None, None),
            model(&lost, View::Home, &[], None, None, None),
            model(&looking, View::Home, &[], None, None, None),
            model(&unauth, View::Home, &[], None, None, None),
            model(&failed, View::Home, &[], None, None, None),
            model(&looking, View::Wifi, &[], None, None, None),
            model(&looking, View::Wifi, &nearby, Some("R58M12ABCDE"), None, Some("Connecting to 192.168.1.23...")),
            model(&looking, View::Qr, &[], None, Some(&qr), None),
        ];
        for (n, m) in screens.iter().enumerate() {
            let (w, h) = (702, 1560); // the aspect of the frame's screen
            let screen = render(w, h, m);
            if let Some(dir) = std::env::var_os("HANDSET_TEST_OUT") {
                let path = std::path::Path::new(&dir).join(format!("connect-{n}.png"));
                let mut enc = png::Encoder::new(std::fs::File::create(path).unwrap(), w, h);
                enc.set_color(png::ColorType::Rgb);
                enc.set_depth(png::BitDepth::Eight);
                enc.write_header().unwrap().write_image_data(&screen.rgb).unwrap();
            }
        }
    }
}
