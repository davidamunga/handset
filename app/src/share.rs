//! Share: the phone's H.264 stream forwarded, untouched, to a browser on the same network over WebRTC.
//! The host decides about every viewer before anything is sent, and whether that viewer may also touch the
//! phone (see ADR 0039).
//!
//! A small HTTP server hands out the viewer page and answers each viewer's WebRTC offer. Every viewer then gets
//! a thread and a UDP socket of its own: the packets come from the `Tap`, `str0m` packetizes them, nothing is
//! decoded or encoded here.

use crate::control::Touch;
use crate::frame::Art;
use crate::share_audio::{self, Packet as Sound};
use crate::share_input::{self, Gate, Input as Command};
use crate::tap::{Tap, Unit};
use std::collections::HashMap;
use std::io::{self, ErrorKind, Read};
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
use std::sync::mpsc::{Receiver, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use str0m::change::SdpOffer;
use str0m::channel::ChannelId;
use str0m::format::Codec;
use str0m::media::{Frequency, MediaKind, MediaTime, Mid};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc, RtcConfig, RtcError};

const VIEWER_PAGE: &str = include_str!("../assets/share-viewer.html");
const PREFERRED_PORT: u16 = 8099;
const MAX_VIEWERS: usize = 4;
/// Offers waiting for the host to answer. Each one is a thread and a dialog in a queue.
const MAX_WAITING: usize = 3;
/// A refused address is turned away without asking again for this long.
const DENIED_FOR: Duration = Duration::from_secs(30);
/// Browsers ask for a keyframe often and each one is large: at most this many.
const KEYFRAME_EVERY: Duration = Duration::from_millis(700);
/// A viewer that sends nothing for this long is gone: browsers answer every second or so, and a closed tab
/// does not tell us.
const SILENCE: Duration = Duration::from_secs(10);
/// Packets of sound a viewer may fall behind by (20 ms each).
const SOUND_BACKLOG: usize = 50;
const MAX_OFFER: u64 = 64 * 1024;

/// What the host lets a viewer do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Access {
    Deny,
    Watch,
    Control,
}

/// Asks the host what `ip` may do. Blocks until they answer.
pub type Approve = Arc<dyn Fn(IpAddr) -> Access + Send + Sync>;
type Notify = Arc<dyn Fn() + Send + Sync>;

/// Lets a keyframe request through at most once per interval, for all viewers together.
struct Limiter {
    last: Mutex<Option<Instant>>,
    every: Duration,
}

impl Limiter {
    fn new(every: Duration) -> Self {
        Self { last: Mutex::new(None), every }
    }

    fn due(&self, now: Instant) -> bool {
        let mut last = self.last.lock().unwrap();
        if last.is_some_and(|t| now.duration_since(t) < self.every) {
            return false;
        }
        *last = Some(now);
        true
    }
}

/// What a viewer needs to draw the phone the way the host sees it.
struct Look {
    device: String,
    frame: Option<Arc<Art>>,
    /// Changes with the frame, so a viewer knows to fetch it again.
    version: u64,
}

/// Text for a JSON string.
fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The message that tells a viewer how to draw the phone: `look,` and then JSON.
fn look_message(look: &Look) -> String {
    let frame = match &look.frame {
        Some(art) => format!("{{\"hole\":[{},{},{},{}]}}", art.hole[0], art.hole[1], art.hole[2], art.hole[3]),
        None => "null".into(),
    };
    format!("look,{{\"device\":{},\"frame\":{frame},\"v\":{}}}", json_string(&look.device), look.version)
}

struct Inner {
    look: Mutex<Look>,
    tap: Arc<Tap>,
    control: Mutex<crate::control::Control>,
    keyframes: Limiter,
    approve: Approve,
    /// Called when the number of viewers changes.
    changed: Notify,
    stop: Arc<AtomicBool>,
    /// Where the encoded sound goes, one channel per viewer.
    sound: share_audio::Listeners,
    viewers: AtomicUsize,
    /// ...and how many of them may also touch the phone.
    controllers: AtomicUsize,
    /// The pointer ids viewers touch with, so two fingers are never the same finger.
    slots: Mutex<[bool; MAX_VIEWERS]>,
    waiting: AtomicUsize,
    /// One question to the host at a time.
    asking: Mutex<()>,
    denied: Mutex<HashMap<IpAddr, Instant>>,
}

impl Inner {
    fn request_keyframe(&self) {
        if self.keyframes.due(Instant::now()) {
            let _ = self.control.lock().unwrap().request_keyframe();
        }
    }
}

/// A running share. Dropping it stops the server and ends every viewer.
pub struct Share {
    inner: Arc<Inner>,
    server: Arc<tiny_http::Server>,
    url: String,
}

impl Share {
    pub fn start(
        tap: Arc<Tap>,
        control: crate::control::Control,
        device: String,
        approve: Approve,
        changed: Notify,
    ) -> io::Result<Share> {
        let server = tiny_http::Server::http(("0.0.0.0", PREFERRED_PORT))
            .or_else(|_| tiny_http::Server::http(("0.0.0.0", 0)))
            .map_err(|e| io::Error::other(e.to_string()))?;
        let port = server.server_addr().to_ip().map(|a| a.port()).unwrap_or(PREFERRED_PORT);
        let server = Arc::new(server);
        let inner = Arc::new(Inner {
            look: Mutex::new(Look { device, frame: None, version: 0 }),
            tap,
            control: Mutex::new(control),
            keyframes: Limiter::new(KEYFRAME_EVERY),
            approve,
            changed,
            stop: Arc::new(AtomicBool::new(false)),
            sound: Arc::default(),
            viewers: AtomicUsize::new(0),
            controllers: AtomicUsize::new(0),
            slots: Mutex::new([false; MAX_VIEWERS]),
            waiting: AtomicUsize::new(0),
            asking: Mutex::new(()),
            denied: Mutex::new(HashMap::new()),
        });
        share_audio::spawn(inner.tap.subscribe_audio(), inner.sound.clone(), inner.stop.clone());
        {
            let (inner, server) = (inner.clone(), server.clone());
            std::thread::spawn(move || serve(&inner, &server));
        }
        Ok(Share { inner, server, url: format!("http://{}:{port}/", local_ip()) })
    }

    /// Where a viewer on this network opens the page.
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn viewers(&self) -> usize {
        self.inner.viewers.load(SeqCst)
    }

    /// The frame the host has chosen, or none. Viewers draw the phone in it.
    pub fn set_frame(&self, art: Option<Art>) {
        let mut look = self.inner.look.lock().unwrap();
        look.frame = art.map(Arc::new);
        look.version += 1;
    }

    pub fn controllers(&self) -> usize {
        self.inner.controllers.load(SeqCst)
    }
}

impl Drop for Share {
    fn drop(&mut self) {
        self.inner.stop.store(true, SeqCst);
        self.server.unblock();
    }
}

fn serve(inner: &Arc<Inner>, server: &tiny_http::Server) {
    for req in server.incoming_requests() {
        if inner.stop.load(SeqCst) {
            return;
        }
        let path = req.url().split('?').next().unwrap_or("").to_string();
        match (req.method(), path.as_str()) {
            (tiny_http::Method::Get, "/") => {
                let html = "Content-Type: text/html; charset=utf-8".parse::<tiny_http::Header>().unwrap();
                let _ = req.respond(tiny_http::Response::from_string(VIEWER_PAGE).with_header(html));
            }
            (tiny_http::Method::Get, "/styles.json") => {
                let json = "Content-Type: application/json".parse::<tiny_http::Header>().unwrap();
                let _ = req.respond(tiny_http::Response::from_string(crate::style::palette_json()).with_header(json));
            }
            (tiny_http::Method::Get, "/frame.png") => {
                let art = inner.look.lock().unwrap().frame.clone();
                let _ = match art {
                    Some(art) => {
                        let png = "Content-Type: image/png".parse::<tiny_http::Header>().unwrap();
                        let fresh = "Cache-Control: no-store".parse::<tiny_http::Header>().unwrap();
                        req.respond(tiny_http::Response::from_data(art.png.clone()).with_header(png).with_header(fresh))
                    }
                    None => req.respond(text("no frame", 404)),
                };
            }
            (tiny_http::Method::Post, "/offer") => {
                if inner.waiting.fetch_add(1, SeqCst) >= MAX_WAITING {
                    inner.waiting.fetch_sub(1, SeqCst);
                    let _ = req.respond(text("too many people waiting", 429));
                    continue;
                }
                let inner = inner.clone();
                std::thread::spawn(move || {
                    offer(&inner, req);
                    inner.waiting.fetch_sub(1, SeqCst);
                });
            }
            _ => {
                let _ = req.respond(text("not found", 404));
            }
        }
    }
}

fn text(body: &str, status: u16) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string(body).with_status_code(status)
}

/// Answer one viewer's offer: check it, ask the host, then start its thread.
fn offer(inner: &Arc<Inner>, mut req: tiny_http::Request) {
    let Some(peer) = req.remote_addr().map(|a| a.ip()) else {
        return drop(req.respond(text("unknown address", 400)));
    };
    let mut body = String::new();
    if req.as_reader().take(MAX_OFFER).read_to_string(&mut body).is_err() {
        return drop(req.respond(text("unreadable offer", 400)));
    }
    // a malformed offer is turned away before it can put a question to the host
    let Ok(sdp) = SdpOffer::from_sdp_string(&body) else {
        return drop(req.respond(text("not a WebRTC offer", 400)));
    };
    if inner.viewers.load(SeqCst) >= MAX_VIEWERS {
        return drop(req.respond(text("too many viewers", 503)));
    }
    let answer = {
        let _one_at_a_time = inner.asking.lock().unwrap();
        if inner.stop.load(SeqCst) {
            return drop(req.respond(text("sharing stopped", 503)));
        }
        let recently_denied = inner.denied.lock().unwrap().get(&peer).is_some_and(|t| t.elapsed() < DENIED_FOR);
        let access = if recently_denied { Access::Deny } else { (inner.approve)(peer) };
        if access == Access::Deny {
            inner.denied.lock().unwrap().insert(peer, Instant::now());
            return drop(req.respond(text("not allowed", 403)));
        }
        accept(inner, sdp, peer, access)
    };
    match answer {
        Ok(sdp) => drop(req.respond(tiny_http::Response::from_string(sdp))),
        Err(e) => {
            eprintln!("share: could not answer {peer}: {e}");
            drop(req.respond(text(&e, 400)));
        }
    }
}

fn accept(inner: &Arc<Inner>, offer: SdpOffer, peer: IpAddr, access: Access) -> Result<String, String> {
    let mut rtc = RtcConfig::new().build(Instant::now());
    // the address this computer has on the network the viewer is on
    let socket = UdpSocket::bind((local_toward(peer), 0)).map_err(|e| e.to_string())?;
    let addr = socket.local_addr().map_err(|e| e.to_string())?;
    rtc.add_local_candidate(Candidate::host(addr, "udp").map_err(|e| e.to_string())?);
    let answer = rtc.sdp_api().accept_offer(offer).map_err(|e| e.to_string())?;

    let (sound_tx, sound) = sync_channel(SOUND_BACKLOG);
    inner.sound.lock().unwrap().push(sound_tx);
    let link = Link { frames: inner.tap.subscribe(), sound, access, slot: 0 };
    let slot = {
        let mut slots = inner.slots.lock().unwrap();
        let free = slots.iter().position(|taken| !taken).ok_or("too many viewers")?;
        slots[free] = true;
        free
    };
    inner.viewers.fetch_add(1, SeqCst);
    if access == Access::Control {
        inner.controllers.fetch_add(1, SeqCst);
    }
    (inner.changed)();
    // a joining viewer needs a keyframe now, not at the encoder's next interval
    inner.request_keyframe();
    let inner = inner.clone();
    std::thread::spawn(move || {
        eprintln!("share: {peer} joined ({access:?})");
        if let Err(e) = run(&inner, rtc, &socket, Link { slot, ..link }) {
            eprintln!("share: {peer} ended: {e:?}");
        }
        inner.slots.lock().unwrap()[slot] = false;
        inner.viewers.fetch_sub(1, SeqCst);
        if access == Access::Control {
            inner.controllers.fetch_sub(1, SeqCst);
        }
        (inner.changed)();
        eprintln!("share: {peer} left");
    });
    Ok(answer.to_sdp_string())
}

/// What one viewer is sent, and what it may do.
struct Link {
    frames: Receiver<Arc<Unit>>,
    sound: Receiver<Arc<Sound>>,
    access: Access,
    /// The pointer id its touches use.
    slot: usize,
}

/// One viewer's connection, until it goes away or sharing stops.
fn run(inner: &Inner, rtc: Rtc, socket: &UdpSocket, link: Link) -> Result<(), RtcError> {
    let slot = link.slot;
    let mut gate = Gate::default();
    let result = serve_viewer(inner, rtc, socket, link, &mut gate);
    // a viewer that leaves mid-touch must not leave the phone held
    if let Some((x, y)) = gate.release() {
        apply(inner, slot, Command::Touch(Touch::Up, x, y));
    }
    result
}

fn serve_viewer(inner: &Inner, mut rtc: Rtc, socket: &UdpSocket, link: Link, gate: &mut Gate) -> Result<(), RtcError> {
    let Link { frames, sound, access, slot } = link;
    let mut buf = vec![0u8; 2000];
    let mut mid: Option<Mid> = None;
    let mut voice: Option<Mid> = None;
    let mut need_key = true;
    // the channel the host talks to the page on, and the version of the look the page has been told
    let mut channel: Option<ChannelId> = None;
    let mut told: Option<u64> = None;
    let mut last_kick = Instant::now();
    let mut heard = Instant::now();
    while !inner.stop.load(SeqCst) && heard.elapsed() < SILENCE {
        let timeout = match rtc.poll_output()? {
            Output::Timeout(t) => t,
            Output::Transmit(t) => {
                socket.send_to(&t.contents, t.destination)?;
                continue;
            }
            Output::Event(e) => {
                match e {
                    Event::IceConnectionStateChange(IceConnectionState::Disconnected) => return Ok(()),
                    Event::MediaAdded(m) => match m.kind {
                        MediaKind::Video => mid = Some(m.mid),
                        MediaKind::Audio => voice = Some(m.mid),
                    },
                    Event::Connected => {
                        // the keyframe asked for at join time went out before the link was up
                        need_key = true;
                        inner.request_keyframe();
                        last_kick = Instant::now();
                    }
                    Event::KeyframeRequest(_) => inner.request_keyframe(),
                    // the page turns its sound button and its controls on when it hears these
                    Event::ChannelOpen(id, _) => {
                        channel = Some(id);
                        if let Some(mut c) = rtc.channel(id) {
                            if inner.tap.has_audio() {
                                let _ = c.write(false, b"audio");
                            }
                            // every viewer is told which it is, so the page can say so
                            let verdict: &[u8] = if access == Access::Control { b"control" } else { b"view" };
                            let _ = c.write(false, verdict);
                        }
                    }
                    Event::ChannelData(d) if access == Access::Control && !d.binary => {
                        if let Some(command) = std::str::from_utf8(&d.data).ok().and_then(share_input::parse)
                            && gate.allow(&command)
                        {
                            apply(inner, slot, command);
                        }
                    }
                    _ => {}
                }
                continue;
            }
        };

        if let Some(mut c) = channel.and_then(|id| rtc.channel(id)) {
            let look = inner.look.lock().unwrap();
            if told != Some(look.version) {
                told = Some(look.version);
                let _ = c.write(false, look_message(&look).as_bytes());
            }
        }

        if need_key && mid.is_some() && last_kick.elapsed() > Duration::from_secs(1) {
            inner.request_keyframe();
            last_kick = Instant::now();
        }

        while let Ok(unit) = frames.try_recv() {
            let Some(mid) = mid else { continue };
            if need_key && !unit.key {
                continue;
            }
            let Some(writer) = rtc.writer(mid) else { continue };
            // the browser offers several H.264 profiles; it decodes by what the SPS says, so any mode 1 entry will do
            let Some(pt) = writer
                .payload_params()
                .find(|p| p.spec().codec == Codec::H264 && p.spec().format.packetization_mode == Some(1))
                .map(|p| p.pt())
            else {
                continue;
            };
            let pts = MediaTime::from_90khz(unit.pts_us * 9 / 100);
            if writer.write(pt, Instant::now(), pts, unit.data.clone()).is_ok() {
                need_key = false;
            }
        }

        while let Ok(packet) = sound.try_recv() {
            let Some(writer) = voice.and_then(|m| rtc.writer(m)) else { continue };
            let Some(pt) = writer.payload_params().find(|p| p.spec().codec == Codec::Opus).map(|p| p.pt()) else {
                continue;
            };
            let at = MediaTime::new(packet.pts_us * 48 / 1000, Frequency::FORTY_EIGHT_KHZ);
            let _ = writer.write(pt, Instant::now(), at, packet.data.clone());
        }

        let wait = timeout.saturating_duration_since(Instant::now()).min(Duration::from_millis(5));
        if wait.is_zero() {
            rtc.handle_input(Input::Timeout(Instant::now()))?;
            continue;
        }
        socket.set_read_timeout(Some(wait))?;
        buf.resize(2000, 0);
        let input = match socket.recv_from(&mut buf) {
            Ok((n, source)) => {
                heard = Instant::now();
                buf.truncate(n);
                Input::Receive(
                    Instant::now(),
                    Receive {
                        proto: Protocol::Udp,
                        source,
                        destination: socket.local_addr()?,
                        contents: buf.as_slice().try_into()?,
                    },
                )
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Input::Timeout(Instant::now()),
            Err(e) => return Err(e.into()),
        };
        rtc.handle_input(input)?;
    }
    Ok(())
}

/// Do what a viewer asked, on the phone.
fn apply(inner: &Inner, slot: usize, command: Command) {
    let (w, h) = inner.tap.size();
    if w == 0 || h == 0 {
        return;
    }
    let pixel = |fraction: f32, extent: u16| (fraction * f32::from(extent - 1)).round() as i32;
    let mut control = inner.control.lock().unwrap();
    let _ = match command {
        Command::Touch(action, x, y) => control.touch_as(slot as u64, action, pixel(x, w), pixel(y, h), w, h),
        Command::Scroll { x, y, h: dx, v: dy } => control.scroll(pixel(x, w), pixel(y, h), w, h, dx, dy),
        Command::Key(code) => control.key(code),
        Command::Text(text) => control.text(&text),
        Command::Rotate => control.rotate_device(),
    };
}

/// The address this computer would use to reach `peer`. Connecting a UDP socket sends nothing.
fn local_toward(peer: IpAddr) -> IpAddr {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect(SocketAddr::new(peer, 9)).and_then(|_| s.local_addr()))
        .map(|a| a.ip())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]))
}

/// The address other computers on the network reach this one at.
fn local_ip() -> IpAddr {
    local_toward(IpAddr::from([192, 0, 2, 1]))
}

/// What the host is told before letting a viewer in: what that person gets to see and do.
fn question(device: &str, ip: IpAddr, sound: bool) -> String {
    let hear = if sound { " and hear everything it plays" } else { "" };
    format!(
        "{ip} wants to open {device}.\n\nThey will see the phone's screen{hear}, including notifications, \
         messages and anything typed on it. With control they can also tap, swipe, type and press Back and Home, \
         which can reach anything the phone can do.\n\nOnly allow someone you know."
    )
}

/// The host's question: what may this address do with the phone?
pub fn ask_host(device: &str, ip: IpAddr, sound: bool) -> Access {
    use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
    let answer = MessageDialog::new()
        .set_level(MessageLevel::Info)
        .set_title("Handset Share")
        .set_description(question(device, ip, sound))
        .set_buttons(MessageButtons::YesNoCancelCustom(
            "Allow and Control".into(),
            "Watch Only".into(),
            "Don't Allow".into(),
        ))
        .show();
    match answer {
        MessageDialogResult::Custom(s) if s == "Allow and Control" => Access::Control,
        MessageDialogResult::Custom(s) if s == "Watch Only" => Access::Watch,
        _ => Access::Deny,
    }
}

/// Is this computer on a private network (a home or office one) rather than directly on the internet?
/// A cafe's Wi-Fi is private too, so this cannot tell it apart: it only catches the plainly public case.
pub fn network_is_private() -> bool {
    is_private(local_ip())
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            // 100.64.0.0/10 is shared address space: carrier NAT, and private overlays such as Tailscale
            a.is_private()
                || a.is_link_local()
                || a.is_loopback()
                || (a.octets()[0] == 100 && a.octets()[1] & 0xc0 == 64)
        }
        IpAddr::V6(a) => {
            a.is_loopback() || (a.segments()[0] & 0xfe00 == 0xfc00) || (a.segments()[0] & 0xffc0 == 0xfe80)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn the_host_is_told_what_the_viewer_can_see_and_do() {
        let ip = IpAddr::from([192, 168, 1, 9]);
        let with = question("Pixel", ip, true);
        assert!(with.contains("192.168.1.9") && with.contains("Pixel") && with.contains("hear"));
        assert!(with.contains("notifications") && with.contains("control"));
        assert!(!question("Pixel", ip, false).contains("hear"));
    }

    #[test]
    fn only_private_addresses_count_as_a_private_network() {
        for private in ["10.1.2.3", "172.16.0.5", "192.168.1.71", "169.254.3.4", "127.0.0.1", "100.100.1.1", "fd00::1"]
        {
            assert!(is_private(private.parse().unwrap()), "{private}");
        }
        for public in ["8.8.8.8", "172.32.0.1", "100.128.0.1", "203.0.113.9", "2001:db8::1"] {
            assert!(!is_private(public.parse().unwrap()), "{public}");
        }
    }
    use std::net::TcpStream;

    #[test]
    fn keyframe_requests_are_spaced_out() {
        let l = Limiter::new(Duration::from_millis(700));
        let t = Instant::now();
        assert!(l.due(t));
        assert!(!l.due(t + Duration::from_millis(100)));
        assert!(!l.due(t + Duration::from_millis(699)));
        assert!(l.due(t + Duration::from_millis(700)));
    }

    fn share(approve: Approve) -> (Share, SocketAddr) {
        let share = Share::start(
            Arc::new(Tap::default()),
            crate::control::Control::disconnected(),
            "Pixel".into(),
            approve,
            Arc::new(|| {}),
        )
        .unwrap();
        let port = share.url().trim_end_matches('/').rsplit(':').next().unwrap().parse().unwrap();
        (share, SocketAddr::new(IpAddr::from([127, 0, 0, 1]), port))
    }

    fn http(addr: SocketAddr, request: &str, body: &str) -> String {
        let mut s = TcpStream::connect(addr).unwrap();
        write!(s, "{request} HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
            .unwrap();
        let mut out = Vec::new();
        s.read_to_end(&mut out).unwrap();
        String::from_utf8_lossy(&out).into_owned()
    }

    #[test]
    fn the_look_message_carries_the_device_and_the_screen_of_the_frame() {
        let mut look = Look { device: "Pixel \"7\"\n".into(), frame: None, version: 3 };
        assert_eq!(look_message(&look), "look,{\"device\":\"Pixel \\\"7\\\" \",\"frame\":null,\"v\":3}");
        look.frame = Some(Arc::new(Art { png: Vec::new(), hole: [0.1, 0.2, 0.5, 0.25] }));
        assert!(look_message(&look).contains("\"frame\":{\"hole\":[0.1,0.2,0.5,0.25]}"));
    }

    #[test]
    fn viewers_get_the_hosts_frame_and_the_editors_backgrounds() {
        let (share, addr) = share(Arc::new(|_| Access::Watch));
        assert!(http(addr, "GET /frame.png", "").starts_with("HTTP/1.1 404"));
        share.set_frame(crate::frame::art("obsidian").ok());
        let png = http(addr, "GET /frame.png", "");
        assert!(png.starts_with("HTTP/1.1 200") && png.contains("image/png"));
        let styles = http(addr, "GET /styles.json", "");
        for g in crate::style::GRADIENTS {
            assert!(styles.contains(g.name), "{}", g.name);
        }
        assert!(styles.contains("Aurora 16:9") && styles.contains("\"maxPadding\":40"));
    }

    #[test]
    fn the_page_asks_for_what_the_host_serves_and_reads_what_it_sends() {
        for needed in [
            "/styles.json",
            "/frame.png",
            "'look,'",
            "e.data === 'audio'",
            "e.data === 'control'",
            "e.data === 'view'",
            "github.com/davidamunga/handset",
            "data-save",
        ] {
            assert!(VIEWER_PAGE.contains(needed), "the page does not use {needed}");
        }
    }

    #[test]
    fn serves_the_viewer_page_and_nothing_else() {
        let (_share, addr) = share(Arc::new(|_| Access::Watch));
        assert!(http(addr, "GET /", "").contains("<title>Handset Share</title>"));
        assert!(http(addr, "GET /secret", "").starts_with("HTTP/1.1 404"));
    }

    #[test]
    fn a_malformed_offer_is_refused_without_asking_the_host() {
        let asked = Arc::new(AtomicBool::new(false));
        let a = asked.clone();
        let (_share, addr) = share(Arc::new(move |_| {
            a.store(true, SeqCst);
            Access::Watch
        }));
        assert!(http(addr, "POST /offer", "not sdp").starts_with("HTTP/1.1 400"));
        assert!(!asked.load(SeqCst));
    }

    /// What a browser's offer looks like to the host: one receive-only video line.
    fn browser_offer() -> String {
        use str0m::change::SdpApi;
        use str0m::media::{Direction, MediaKind};
        let mut rtc = RtcConfig::new().build(Instant::now());
        let mut api: SdpApi = rtc.sdp_api();
        api.add_media(MediaKind::Video, Direction::RecvOnly, None, None, None);
        let (offer, _) = api.apply().unwrap();
        offer.to_sdp_string()
    }

    #[test]
    fn a_viewer_the_host_refuses_is_turned_away_and_not_asked_again_at_once() {
        let asked = Arc::new(AtomicUsize::new(0));
        let a = asked.clone();
        let (share, addr) = share(Arc::new(move |_| {
            a.fetch_add(1, SeqCst);
            Access::Deny
        }));
        let offer = browser_offer();
        assert!(http(addr, "POST /offer", &offer).starts_with("HTTP/1.1 403"));
        assert!(http(addr, "POST /offer", &offer).starts_with("HTTP/1.1 403"));
        assert_eq!(asked.load(SeqCst), 1);
        assert_eq!(share.viewers(), 0);
    }

    #[test]
    fn a_viewer_the_host_allows_gets_an_answer_and_is_counted() {
        let (share, addr) = share(Arc::new(|_| Access::Watch));
        let reply = http(addr, "POST /offer", &browser_offer());
        assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
        assert!(reply.contains("m=video"));
        assert_eq!(share.viewers(), 1);
        assert_eq!(share.inner.tap.subscribers(), 1);
    }
}
