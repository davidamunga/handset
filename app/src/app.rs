//! Window, input handling and the glue between stream, renderer and control.

use crate::adb;
use crate::audio;
use crate::capture::{self, Recorder};
use crate::connect::{self, Action, LandingEvent, Qr, Status, View};
use crate::control::{Control, Touch, keycode};
use crate::frame::{self, Bezel};
use crate::menu;
use crate::mirror::Mirror;
use crate::overlay;
use crate::render::Renderer;
use crate::server;
use crate::stream::{Slot, UserEvent};
use crate::toolbar::{self, Button};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Cursor, CursorIcon, Window, WindowId};

/// Everything `App` needs from `main`.
pub struct Init {
    pub bezel: Option<Bezel>,
    pub title: String,
    pub slot: Arc<Mutex<Slot>>,
    pub control: Control,
    pub cap: capture::Settings,
    pub audio_sink: audio::RecSink,
    pub with_audio: bool,
    pub frame_spec: Option<String>,
    /// The running session, or `None` to open on the connect screen and wait for a phone.
    pub mirror: Option<Mirror>,
    pub wait: Option<Wait>,
    pub proxy: EventLoopProxy<UserEvent>,
    pub opts: server::Options,
    pub mute: bool,
}

/// What the connect screen is waiting for.
pub struct Wait {
    /// A specific phone, when one was named on the command line.
    pub serial: Option<String>,
    /// Switch the phone to Wi-Fi once it is found (`--wifi` with nothing plugged in yet).
    pub wifi: bool,
}

/// The connect screen's state while waiting.
struct Waiting {
    want: Option<String>,
    wifi: bool,
    status: Status,
    tick: u32,
    tick_at: Instant,
    /// A phone is ready: start mirroring it at this time (after the "connecting" screen has been seen).
    starting: Option<(String, Instant)>,
    /// A phone that failed to start, and when: not retried for a while.
    failed: Option<(String, Instant)>,
    view: View,
    /// Index of the button under the cursor, and the buttons of the picture currently shown.
    hover: Option<usize>,
    buttons: Vec<connect::Button>,
    /// Size of that picture in pixels.
    img: (u32, u32),
    /// Phones announcing themselves on the network.
    nearby: Vec<crate::adb::Service>,
    /// A USB phone that is ready but is not being started because the Wi-Fi view is open.
    usb: Option<String>,
    /// The pairing code on screen, whether a phone has scanned it, and the phone it paired with.
    qr: Option<Qr>,
    pairing: bool,
    paired_ip: Option<(String, Instant)>,
    /// A connection is being made in the background; further connect clicks are ignored.
    busy: bool,
    /// A line of progress or trouble for the Wi-Fi and QR views.
    note: Option<String>,
    /// After a Wi-Fi phone went away: its address, and when to try connecting to it next.
    retry_addr: Option<String>,
    next_retry: Instant,
    /// Mirroring was ended on purpose: a ready phone is offered, not started.
    paused: bool,
}

impl Waiting {
    fn new(want: Option<String>, wifi: bool, status: Status) -> Self {
        Self {
            want,
            wifi,
            status,
            tick: 0,
            tick_at: Instant::now(),
            starting: None,
            failed: None,
            view: View::Home,
            hover: None,
            buttons: Vec::new(),
            img: (0, 0),
            nearby: Vec::new(),
            usb: None,
            qr: None,
            pairing: false,
            paired_ip: None,
            busy: false,
            note: None,
            retry_addr: None,
            next_retry: Instant::now() + RECONNECT_EVERY,
            paused: false,
        }
    }
}

/// How often to try `adb connect` again after a Wi-Fi phone went away.
const RECONNECT_EVERY: Duration = Duration::from_secs(4);
/// How long to leave a phone alone after mirroring it failed.
const RETRY_AFTER: Duration = Duration::from_secs(10);
/// Height of the connect screen's picture; the width follows the screen's aspect.
const CONNECT_HEIGHT: u32 = 1560;

pub struct App {
    pub bezel: Option<Bezel>,
    pub title: String,
    pub slot: Arc<Mutex<Slot>>,
    pub control: Control,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    cursor: (f64, f64),
    touching: bool,
    mods: ModifiersState,
    device: (u16, u16),
    debug: bool,
    cap: capture::Settings,
    recorder: Option<Recorder>,
    next_tick: Instant,
    rec_started: Instant,
    audio_sink: audio::RecSink,
    with_audio: bool,
    frame_spec: Option<String>,
    /// `--record-for`: record this long from the first frame, then quit.
    pub auto_record: Option<Duration>,
    auto_stop_at: Option<Instant>,
    hover: Option<Button>,
    toolbar_key: Option<(u32, Option<Button>, bool)>,
    toast: Option<Toast>,
    /// What the renderer's overlay currently shows (text, dot colour).
    overlay_key: Option<(String, Option<[u8; 3]>)>,
    overlay_opacity: f32,
    mirror: Option<Mirror>,
    waiting: Option<Waiting>,
    proxy: EventLoopProxy<UserEvent>,
    opts: server::Options,
    mute: bool,
}

/// A short message pill. With a `path` it is clickable and reveals that file.
struct Toast {
    text: String,
    since: Instant,
    ttl: Duration,
    path: Option<std::path::PathBuf>,
}

const TOAST: Duration = Duration::from_millis(1800);
/// Capture toasts stay long enough to be clicked.
const TOAST_SAVED: Duration = Duration::from_millis(4500);
const TOAST_FADE: Duration = Duration::from_millis(400);
/// Cursor position in window space (0..1) and the device pixel under it, if any.
type Located = ((f32, f32), Option<(i32, i32)>);
/// What the overlay should show: text, optional dot colour, opacity.
type OverlayWant = (String, Option<[u8; 3]>, f32);

const REC_RED: [u8; 3] = [235, 60, 60];
const REC_DIM: [u8; 3] = [110, 35, 35];

impl App {
    pub fn new(init: Init) -> Self {
        Self {
            bezel: init.bezel,
            title: init.title,
            slot: init.slot,
            control: init.control,
            window: None,
            renderer: None,
            cursor: (0.0, 0.0),
            touching: false,
            mods: ModifiersState::empty(),
            device: (0, 0),
            debug: std::env::var_os("HANDSET_DEBUG").is_some(),
            cap: init.cap,
            recorder: None,
            next_tick: Instant::now(),
            rec_started: Instant::now(),
            audio_sink: init.audio_sink,
            with_audio: init.with_audio,
            frame_spec: init.frame_spec,
            auto_record: None,
            auto_stop_at: None,
            hover: None,
            toolbar_key: None,
            toast: None,
            overlay_key: None,
            overlay_opacity: 1.0,
            mirror: init.mirror,
            waiting: init.wait.map(|w| Waiting::new(w.serial, w.wifi, Status::Looking)),
            proxy: init.proxy,
            opts: init.opts,
            mute: init.mute,
        }
    }

    /// Draw the landing screen into the phone's screen area.
    fn draw_connect(&mut self) {
        let (Some(wt), Some(r), Some(w)) = (self.waiting.as_mut(), self.renderer.as_mut(), self.window.as_ref()) else {
            return;
        };
        let aspect = r.content_aspect().unwrap_or(9.0 / 20.0);
        let (pw, ph) = (((CONNECT_HEIGHT as f32 * aspect).round() as u32) & !1, CONNECT_HEIGHT);
        let screen = {
            let model = connect::Model {
                status: &wt.status,
                view: wt.view,
                tick: wt.tick,
                hover: wt.hover,
                nearby: &wt.nearby,
                usb: wt.usb.as_deref(),
                qr: wt.qr.as_ref(),
                note: wt.note.as_deref(),
            };
            connect::render(pw, ph, &model)
        };
        wt.buttons = screen.buttons;
        wt.img = (pw, ph);
        if self.debug {
            eprintln!(
                "landing picture {pw}x{ph}, buttons {:?}",
                wt.buttons.iter().map(|b| (b.rect, b.label.clone())).collect::<Vec<_>>()
            );
        }
        r.upload(&connect::to_yuv(&screen.rgb, pw, ph));
        w.request_redraw();
    }

    /// The landing screen button under the cursor.
    fn landing_hit(&self) -> Option<usize> {
        let (wt, r, w) = (self.waiting.as_ref()?, self.renderer.as_ref()?, self.window.as_ref()?);
        let s = w.inner_size();
        let p = ((self.cursor.0 / s.width as f64) as f32, (self.cursor.1 / s.height as f64) as f32);
        let v = r.video_rect;
        if p.0 < v[0] || p.0 >= v[0] + v[2] || p.1 < v[1] || p.1 >= v[1] + v[3] {
            return None;
        }
        let ix = ((p.0 - v[0]) / v[2] * wt.img.0 as f32) as u32;
        let iy = ((p.1 - v[1]) / v[3] * wt.img.1 as f32) as u32;
        wt.buttons.iter().position(|b| {
            ix >= b.rect[0] && ix < b.rect[0] + b.rect[2] && iy >= b.rect[1] && iy < b.rect[1] + b.rect[3]
        })
    }

    /// New `adb devices` result while waiting: update what the screen says, or start mirroring.
    fn on_adb(&mut self, result: Result<Vec<(String, String)>, String>) {
        let Some(wt) = self.waiting.as_mut() else { return };
        if wt.starting.is_some() {
            return;
        }
        let mut usb = None;
        let next = match result {
            Err(_) => Status::AdbMissing,
            Ok(list) => match connect::pick(&list, wt.want.as_deref()) {
                // with the Wi-Fi view open, a USB phone is offered for switching instead of started
                Some(serial) if wt.paused || (wt.view == View::Wifi && !connect::is_wireless(&serial)) => {
                    usb = Some(serial);
                    wt.status.clone()
                }
                Some(serial) => {
                    let recently_failed =
                        wt.failed.as_ref().is_some_and(|(s, at)| *s == serial && at.elapsed() < RETRY_AFTER);
                    if recently_failed {
                        return;
                    }
                    wt.starting = Some((serial.clone(), Instant::now() + Duration::from_millis(300)));
                    Status::Connecting(serial)
                }
                None => {
                    wt.failed = None; // the phone went away; a replug gets a fresh try
                    let found = connect::status_from(&list, wt.want.as_deref());
                    if found == Status::Looking && matches!(wt.status, Status::Lost(_)) {
                        wt.status.clone()
                    } else {
                        found
                    }
                }
            },
        };
        let changed = next != wt.status || wt.usb != usb;
        wt.usb = usb;
        wt.status = next;
        if changed {
            self.draw_connect();
        }
    }

    /// Something started from the landing screen finished.
    fn on_landing(&mut self, event: LandingEvent) {
        let proxy = self.proxy.clone();
        let Some(wt) = self.waiting.as_mut() else { return };
        // first line of an error, short enough for the screen
        let brief = |e: &str| e.lines().next().unwrap_or_default().chars().take(90).collect::<String>();
        let mut redraw = true;
        match event {
            LandingEvent::Nearby(list) => {
                // a phone that scanned our code announces a pairing service named after it
                if let Some(qr) = wt.qr.as_ref().filter(|_| !wt.pairing)
                    && let Some(p) =
                        list.iter().find(|s| s.kind == crate::adb::ServiceKind::Pairing && s.name == qr.name)
                {
                    wt.pairing = true;
                    wt.note = Some("Pairing...".into());
                    connect::pair_async(proxy.clone(), p.addr.clone(), qr.password.clone());
                }
                // adb usually connects to a paired phone by itself; if it has not after a few seconds, do it
                if let Some((ip, at)) = wt.paired_ip.as_ref().filter(|(_, at)| at.elapsed() > Duration::from_secs(4))
                    && !wt.busy
                    && let Some(c) = list
                        .iter()
                        .find(|s| s.kind == crate::adb::ServiceKind::Connect && s.addr.starts_with(&format!("{ip}:")))
                {
                    let _ = at;
                    wt.busy = true;
                    connect::connect_async(proxy.clone(), c.addr.clone());
                }
                redraw = list != wt.nearby;
                wt.nearby = list;
            }
            LandingEvent::Paired(Ok(ip)) => {
                wt.paired_ip = Some((ip, Instant::now()));
                wt.note = Some("Paired. Connecting...".into());
            }
            LandingEvent::Paired(Err(e)) => {
                // the phone's scan window has closed: show a fresh code to try again
                wt.pairing = false;
                wt.note = Some(format!("Pairing failed: {}", brief(&e)));
                wt.qr = Qr::new().ok();
            }
            LandingEvent::Connected(Ok(serial)) => {
                wt.busy = false;
                wt.note = None;
                wt.wifi = false;
                wt.starting = Some((serial.clone(), Instant::now() + Duration::from_millis(300)));
                wt.status = Status::Connecting(serial);
            }
            LandingEvent::Connected(Err(e)) => {
                wt.busy = false;
                wt.note = Some(brief(&e));
            }
            LandingEvent::Typed(Some(addr)) => {
                wt.busy = true;
                wt.note = Some(format!("Connecting to {}...", crate::adb::normalize_addr(&addr)));
                connect::connect_async(proxy, addr);
            }
            LandingEvent::Typed(None) => redraw = false,
        }
        if redraw {
            self.draw_connect();
        }
    }

    /// A landing screen button was clicked.
    fn on_action(&mut self, action: Action) {
        let proxy = self.proxy.clone();
        let Some(wt) = self.waiting.as_mut() else { return };
        match action {
            Action::Open(View::Wifi) => {
                wt.view = View::Wifi;
                wt.note = None;
                connect::want_mdns(true);
            }
            Action::Open(View::Qr) => {
                wt.view = View::Qr;
                wt.note = None;
                wt.pairing = false;
                wt.paired_ip = None;
                wt.qr = Qr::new().ok();
                connect::want_mdns(true);
            }
            Action::Open(View::Home) | Action::Back => {
                wt.view = View::Home;
                wt.note = None;
                wt.qr = None;
                connect::want_mdns(false);
            }
            Action::Connect(addr) if !wt.busy => {
                wt.busy = true;
                wt.note = Some(format!("Connecting to {addr}..."));
                connect::connect_async(proxy, addr);
            }
            Action::TypeAddress if !wt.busy => connect::ask_address_async(proxy),
            Action::Mirror(serial) => {
                wt.paused = false;
                wt.starting = Some((serial.clone(), Instant::now() + Duration::from_millis(300)));
                wt.status = Status::Connecting(serial);
            }
            Action::SwitchToWifi(usb) if !wt.busy => {
                wt.busy = true;
                wt.note = Some("Switching to Wi-Fi...".into());
                connect::wifi_async(proxy, usb);
            }
            _ => {}
        }
        wt.hover = None;
        self.draw_connect();
    }

    /// Stop mirroring and return the frame to its idle state. Returns the serial that was mirrored.
    fn end_session(&mut self) -> Option<String> {
        let mirror = self.mirror.take()?;
        let serial = mirror.session.serial().to_string();
        self.stop_recording();
        drop(mirror); // kills the server and removes the adb forward
        self.control = Control::disconnected();
        self.with_audio = false;
        self.touching = false;
        self.device = (0, 0);
        self.slot.lock().unwrap().fresh = false;
        self.title = "Handset".to_string();
        if let Some(w) = &self.window {
            w.set_title("Handset");
        }
        // the connect screen is portrait: turn the frame back if the phone was in landscape
        if let (Some(r), Some(w)) = (self.renderer.as_mut(), self.window.as_ref())
            && r.landscape()
        {
            let new_h = (r.bezel_rect()[2] * w.inner_size().width as f32).round() as u32;
            r.set_landscape(false);
            if let Some(a) = r.window_aspect() {
                let _ = w.request_inner_size(winit::dpi::PhysicalSize::new((new_h as f32 * a).round() as u32, new_h));
            }
        }
        Some(serial)
    }

    /// The stream ended. Quit if there is nothing to recover; otherwise go back to the connect screen and
    /// wait for the phone to return, retrying the connection for a phone that was on Wi-Fi.
    fn on_disconnected(&mut self, el: &ActiveEventLoop) {
        if self.mirror.is_none() || self.auto_record.is_some() {
            // nothing to recover, or `--record-for`, which ends with its session
            el.exit();
            return;
        }
        let Some(serial) = self.end_session() else { return };
        eprintln!("connection to {serial} lost; waiting for it to come back");
        let mut wt = Waiting::new(None, false, Status::Lost(serial.clone()));
        wt.retry_addr = connect::retry_addr(&serial);
        self.waiting = Some(wt);
        connect::spawn_poll(self.proxy.clone());
        self.draw_connect();
    }

    /// The Phone menu: end mirroring on purpose and open the connect screen, on `view`, without
    /// restarting by itself. Returns the serial that was mirrored.
    fn leave_session(&mut self, view: View) -> Option<String> {
        let serial = self.end_session()?;
        let mut wt = Waiting::new(None, false, Status::Disconnected);
        wt.paused = true;
        self.waiting = Some(wt);
        connect::spawn_poll(self.proxy.clone());
        match view {
            View::Home => self.draw_connect(),
            other => self.on_action(Action::Open(other)),
        }
        Some(serial)
    }

    /// Animate the connect screen and start mirroring once its "connecting" frame has been shown.
    /// Returns when this wants to run again.
    fn tick_connect(&mut self) -> Option<Instant> {
        let now = Instant::now();
        let wt = self.waiting.as_mut()?;
        let mut redraw = false;
        if now.duration_since(wt.tick_at) >= Duration::from_millis(500) {
            wt.tick += 1;
            wt.tick_at = now;
            redraw = true;
        }
        let start = wt.starting.as_ref().filter(|(_, at)| now >= *at).map(|(s, _)| s.clone());
        if let Some(addr) = wt.retry_addr.clone().filter(|_| !wt.busy && wt.starting.is_none() && now >= wt.next_retry)
        {
            wt.busy = true;
            wt.next_retry = now + RECONNECT_EVERY;
            connect::connect_async(self.proxy.clone(), addr);
        }
        let next = wt.tick_at + Duration::from_millis(500);
        if redraw {
            self.draw_connect();
        }
        if let Some(serial) = start {
            self.begin_mirroring(serial);
        }
        self.waiting.as_ref().map(|_| next)
    }

    fn begin_mirroring(&mut self, usb_serial: String) {
        let Some(wt) = self.waiting.as_mut() else { return };
        wt.starting = None;
        let mut serial = usb_serial.clone();
        if wt.wifi {
            match adb::wifi(&usb_serial) {
                Ok(s) => {
                    eprintln!("on Wi-Fi as {s}: you can unplug the cable");
                    serial = s;
                }
                Err(e) => eprintln!("could not switch to Wi-Fi ({e}); mirroring over USB"),
            }
        }
        match Mirror::start(
            &serial,
            &self.opts,
            self.mute,
            self.proxy.clone(),
            self.slot.clone(),
            self.audio_sink.clone(),
        ) {
            Ok((mirror, control)) => {
                self.with_audio = mirror.with_audio();
                self.control = control;
                if let Some(w) = &self.window {
                    w.set_title(&mirror.session.device_name);
                }
                self.title = mirror.session.device_name.clone();
                self.mirror = Some(mirror);
                self.waiting = None;
                connect::stop_poll();
                connect::want_mdns(false);
                if let Some(w) = &self.window {
                    w.set_cursor(Cursor::Icon(CursorIcon::Default));
                }
            }
            Err(e) => {
                eprintln!("{e}");
                if let Some(wt) = self.waiting.as_mut() {
                    wt.failed = Some((usb_serial, Instant::now()));
                    wt.status = Status::Failed(e);
                }
                self.draw_connect();
            }
        }
    }

    /// Cursor in window space (0..1), plus the device pixel under it if it is
    /// on the video.
    fn locate(&self) -> Option<Located> {
        let (w, r) = (self.window.as_ref()?, self.renderer.as_ref()?);
        let s = w.inner_size();
        let p = ((self.cursor.0 / s.width as f64) as f32, (self.cursor.1 / s.height as f64) as f32);
        let v = r.video_rect;
        let (dw, dh) = self.device;
        let inside = dw > 0 && p.0 >= v[0] && p.0 < v[0] + v[2] && p.1 >= v[1] && p.1 < v[1] + v[3];
        let dev = inside.then(|| (((p.0 - v[0]) / v[2] * dw as f32) as i32, ((p.1 - v[1]) / v[3] * dh as f32) as i32));
        Some((p, dev))
    }

    /// Re-rasterize the toolbar when its size, hover or recording state changed.
    fn refresh_toolbar(&mut self) {
        let (Some(r), Some(w)) = (self.renderer.as_mut(), self.window.as_ref()) else { return };
        if !r.toolbar_visible() {
            self.toolbar_key = None;
            return;
        }
        let wh = w.inner_size().height;
        let key = (wh, self.hover, self.recorder.is_some());
        if self.toolbar_key != Some(key) {
            r.set_toolbar_image(Some(&toolbar::raster(wh as f32, self.hover, self.recorder.is_some())));
            self.toolbar_key = Some(key);
            w.request_redraw();
        }
    }

    fn toolbar_button(&self) -> Option<Button> {
        let (r, w) = (self.renderer.as_ref()?, self.window.as_ref()?);
        if !r.toolbar_visible() {
            return None;
        }
        let s = w.inner_size();
        toolbar::button_at(s.width as f32, s.height as f32, self.cursor.0 as f32, self.cursor.1 as f32)
    }

    fn on_button(&mut self, el: &ActiveEventLoop, b: Button) {
        if self.debug {
            eprintln!("toolbar: {b:?}");
        }
        match b {
            Button::Screenshot => self.on_menu(el, "shot"),
            Button::Record => self.on_menu(el, "rec"),
            Button::Rotate => self.on_menu(el, "rotate"),
            Button::More => self.show_menu(),
            Button::Quit => el.exit(),
            Button::NextFrame => {
                // None -> each built-in frame -> back to None
                let names: Vec<&str> = frame::builtin_names().collect();
                let next = match self.frame_spec.as_deref().and_then(|s| names.iter().position(|n| *n == s)) {
                    None if self.frame_spec.is_none() => names.first().map(|n| n.to_string()),
                    None => names.first().map(|n| n.to_string()), // a custom frame: restart the cycle
                    Some(i) => names.get(i + 1).map(|n| n.to_string()),
                };
                self.set_frame(next);
            }
        }
    }

    /// Window size for a given height, following the current layout.
    fn size_for_height(&self, height: u32) -> Option<winit::dpi::PhysicalSize<u32>> {
        let aspect = self.renderer.as_ref()?.window_aspect()?;
        Some(winit::dpi::PhysicalSize::new((height as f32 * aspect).round() as u32, height))
    }

    fn show_menu(&mut self) {
        let Some(w) = self.window.clone() else { return };
        let builtin = |s: &str| frame::builtin_names().any(|n| n == s);
        let state = menu::State {
            frames: frame::builtin_names().collect(),
            frame: self.frame_spec.clone().filter(|s| builtin(s)),
            custom_frame: self.frame_spec.as_deref().is_some_and(|s| !builtin(s)),
            recording: self.recorder.is_some(),
            gif: self.cap.format == capture::Format::Gif,
            normalize: self.cap.normalize,
            audio: self.with_audio,
            muted: audio::is_muted(),
            toolbar: self.renderer.as_ref().is_some_and(|r| r.toolbar_visible()),
            clipboard: self.cap.clipboard,
            mirroring: self.mirror.is_some(),
            usb: self.mirror.as_ref().is_some_and(|m| !connect::is_wireless(m.session.serial())),
            recent: capture::recent_captures(5),
        };
        menu::show(&w, &state);
    }

    /// Switch the frame while running (`None` = no frame).
    fn set_frame(&mut self, spec: Option<String>) {
        let bezel = match spec.as_deref().map(frame::load).transpose() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("cannot load frame: {e}");
                self.show_toast("FRAME FAILED");
                return;
            }
        };
        let (Some(r), Some(w)) = (self.renderer.as_mut(), self.window.as_ref()) else { return };
        r.replace_bezel(bezel.as_ref());
        // a framed window is borderless and transparent, an unframed one is a normal window
        w.set_decorations(bezel.is_none());
        w.set_transparent(bezel.is_some());
        self.bezel = bezel.map(|mut b| {
            b.rgba = Vec::new(); // already on the GPU
            b
        });
        self.frame_spec = spec;
        // keep the height, follow the new aspect ratio
        let height = w.inner_size().height;
        if let Some(size) = self.size_for_height(height) {
            let _ = self.window.as_ref().unwrap().request_inner_size(size);
        }
        self.toolbar_key = None;
        self.window.as_ref().unwrap().request_redraw();
    }

    fn on_menu(&mut self, el: &ActiveEventLoop, id: &str) {
        match id {
            "shot" => self.screenshot(),
            "rec" => {
                if self.recorder.is_some() {
                    self.stop_recording();
                } else {
                    self.start_recording();
                }
            }
            "fmt:mp4" => self.cap.format = capture::Format::Mp4,
            "fmt:gif" => self.cap.format = capture::Format::Gif,
            "norm" => self.cap.normalize = !self.cap.normalize,
            "clip" => self.cap.clipboard = !self.cap.clipboard,
            "toolbar" => {
                if let Some(r) = self.renderer.as_mut() {
                    let on = !r.toolbar_visible();
                    r.set_toolbar_visible(on);
                }
                self.toolbar_key = None;
                if let Some(h) = self.window.as_ref().map(|w| w.inner_size().height)
                    && let Some(size) = self.size_for_height(h)
                {
                    let _ = self.window.as_ref().unwrap().request_inner_size(size);
                }
            }
            "mute" => audio::set_muted(!audio::is_muted()),
            "conn:disconnect" => {
                self.leave_session(View::Home);
            }
            "conn:another" => {
                self.leave_session(View::Wifi);
            }
            "conn:wifi" => {
                if let Some(serial) = self.leave_session(View::Wifi) {
                    self.on_action(Action::SwitchToWifi(serial));
                }
            }
            "rotate" => {
                let _ = self.control.rotate_device();
            }
            "quit" => el.exit(),
            "frame:none" => self.set_frame(None),
            other if other.starts_with("recent:") => {
                // recent:<action>:<absolute path>
                let rest = &other["recent:".len()..];
                if let Some((action, path)) = rest.split_once(':') {
                    let path = std::path::PathBuf::from(path);
                    match action {
                        "open" => capture::open_file(&path),
                        "reveal" => capture::reveal(&path),
                        "copy" => {
                            let ok = capture::copy_to_clipboard(&path);
                            self.show_toast(if ok { "COPIED" } else { "COPY FAILED" });
                        }
                        _ => {}
                    }
                }
            }
            other => {
                if let Some(name) = other.strip_prefix("frame:") {
                    self.set_frame(Some(name.to_string()));
                }
            }
        }
    }

    fn show_toast(&mut self, text: &str) {
        self.toast = Some(Toast { text: text.to_string(), since: Instant::now(), ttl: TOAST, path: None });
        self.refresh_overlay();
    }

    /// Announce a saved capture; clicking the toast shows it in Finder. The
    /// capture is also copied to the clipboard unless that is switched off.
    fn show_saved(&mut self, what: &str, path: std::path::PathBuf) {
        let copied = self.cap.clipboard && capture::copy_to_clipboard(&path);
        if self.cap.clipboard {
            eprintln!("clipboard: {}", if copied { "copied" } else { "copy failed" });
        }
        let text = format!("{what} {} - CLICK TO SHOW", if copied { "COPIED" } else { "SAVED" });
        self.toast = Some(Toast { text, since: Instant::now(), ttl: TOAST_SAVED, path: Some(path) });
        self.refresh_overlay();
    }

    /// Is the cursor on a clickable capture toast?
    fn toast_hit(&self) -> bool {
        self.toast.as_ref().is_some_and(|t| t.path.is_some())
            && self.renderer.as_ref().is_some_and(|r| r.overlay_contains(self.cursor.0 as f32, self.cursor.1 as f32))
    }

    /// Bring the window-only overlay (toast, else REC indicator) up to date.
    /// Returns true while something is animating and needs regular redraws.
    fn refresh_overlay(&mut self) -> bool {
        let now = Instant::now();
        if self.toast.as_ref().is_some_and(|t| now.duration_since(t.since) >= t.ttl) {
            self.toast = None;
        }
        let desired: Option<OverlayWant> = if let Some(t) = &self.toast {
            let left = t.ttl.saturating_sub(now.duration_since(t.since));
            let opacity = (left.as_secs_f32() / TOAST_FADE.as_secs_f32()).min(1.0);
            Some((t.text.clone(), None, opacity))
        } else if self.recorder.is_some() {
            let e = now.duration_since(self.rec_started);
            let dot = if (e.as_millis() / 500).is_multiple_of(2) { REC_RED } else { REC_DIM };
            let secs = e.as_secs();
            Some((format!("REC {}:{:02}", secs / 60, secs % 60), Some(dot), 1.0))
        } else {
            None
        };

        let (Some(r), Some(w)) = (self.renderer.as_mut(), self.window.as_ref()) else { return false };
        match desired {
            None => {
                if self.overlay_key.take().is_some() {
                    r.set_overlay(None);
                    w.request_redraw();
                }
                false
            }
            Some((text, dot, opacity)) => {
                let key = Some((text, dot));
                let mut dirty = false;
                if self.overlay_key != key {
                    let mut s = ((w.scale_factor() * 1.5).round() as u32).max(1);
                    let (text, dot) = key.clone().unwrap();
                    // shrink long messages to fit the window
                    let max_w = (w.inner_size().width as f32 * 0.92) as u32;
                    let mut pill = overlay::pill(&text, dot, s);
                    while pill.width > max_w && s > 1 {
                        s -= 1;
                        pill = overlay::pill(&text, dot, s);
                    }
                    r.set_overlay(Some(&pill));
                    self.overlay_key = key;
                    dirty = true;
                }
                if (opacity - self.overlay_opacity).abs() > 0.004 || dirty {
                    self.overlay_opacity = opacity;
                    r.set_overlay_opacity(opacity);
                    w.request_redraw();
                }
                true
            }
        }
    }

    fn screenshot(&mut self) {
        let Some(r) = self.renderer.as_mut() else { return };
        let Some((w, h)) = r.capture_size(self.cap.rec_size.max(1800)) else { return };
        match r
            .capture((w, h), None)
            .ok_or_else(|| "no video yet".to_string())
            .and_then(|rgba| capture::save_png(&rgba, w, h))
        {
            Ok(p) => {
                eprintln!("screenshot saved: {}", p.display());
                self.show_saved("SCREENSHOT", p);
            }
            Err(e) => {
                eprintln!("screenshot failed: {e}");
                self.show_toast("SCREENSHOT FAILED");
            }
        }
    }

    fn start_recording(&mut self) {
        let Some(r) = self.renderer.as_ref() else { return };
        let Some(size) = r.capture_size(self.cap.rec_size) else {
            eprintln!("cannot record: no video yet");
            self.show_toast("NO VIDEO YET");
            return;
        };
        match Recorder::start(size, &self.cap, "handset", self.with_audio) {
            Ok(rec) => {
                eprintln!(
                    "recording to {} ({}x{}) - press the shortcut again to stop",
                    rec.path.display(),
                    size.0,
                    size.1
                );
                *self.audio_sink.lock().unwrap() = rec.audio_sender();
                self.recorder = Some(rec);
                self.next_tick = Instant::now();
                self.rec_started = Instant::now();
                self.refresh_overlay();
            }
            Err(e) => {
                eprintln!("cannot record: {e}");
                self.show_toast("RECORDING FAILED");
            }
        }
    }

    fn stop_recording(&mut self) {
        if let Some(rec) = self.recorder.take() {
            // stop feeding audio first, so the recorder's audio channel can close
            *self.audio_sink.lock().unwrap() = None;
            let path = rec.finish();
            eprintln!("recording saved: {}", path.display());
            self.show_saved("RECORDING", path);
        }
    }

    fn on_bezel(&self, p: (f32, f32)) -> bool {
        let (Some(r), Some(b)) = (self.renderer.as_ref(), self.bezel.as_ref()) else { return false };
        let br = r.bezel_rect();
        let (fx, fy) = ((p.0 - br[0]) / br[2], (p.1 - br[1]) / br[3]);
        // the bezel image is drawn rotated 90 degrees clockwise in landscape
        if r.landscape() { b.opaque_at(fy, 1.0 - fx) } else { b.opaque_at(fx, fy) }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let aspect = (self.bezel.as_ref().map_or(9.0 / 20.0, |b| b.aspect()) + toolbar::SIDE_FRAC) as f64;
        let height = 820.0;
        let mut attrs = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(LogicalSize::new(height * aspect, height))
            .with_min_inner_size(LogicalSize::new(200.0 * aspect, 200.0));
        if self.bezel.is_some() {
            attrs = attrs.with_transparent(true).with_decorations(false);
        }
        let window = Arc::new(el.create_window(attrs).expect("create window"));
        match Renderer::new(window.clone(), self.bezel.as_ref()) {
            Ok(r) => self.renderer = Some(r),
            Err(e) => {
                eprintln!("renderer: {e}");
                el.exit();
                return;
            }
        }
        // The CPU copy of the bezel is on the GPU now; drop the big RGBA buffer.
        if let Some(b) = self.bezel.as_mut() {
            b.rgba = Vec::new();
        }
        self.window = Some(window);
        if self.waiting.is_some() {
            // HANDSET_LANDING=wifi|qr opens that view first (for looking at it without clicking)
            match std::env::var("HANDSET_LANDING").as_deref() {
                Ok("wifi") => self.on_action(Action::Open(View::Wifi)),
                Ok("qr") => self.on_action(Action::Open(View::Qr)),
                _ => self.draw_connect(),
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.auto_stop_at.is_some_and(|t| Instant::now() >= t) {
            self.auto_stop_at = None;
            self.auto_record = None;
            self.stop_recording();
            el.exit();
            return;
        }
        let connect_wake = self.tick_connect();
        self.refresh_toolbar();
        let animating = self.refresh_overlay();
        let Some(rec) = &self.recorder else {
            let mut next = animating.then(|| Instant::now() + Duration::from_millis(33));
            if let Some(t) = connect_wake {
                next = Some(next.map_or(t, |n| n.min(t)));
            }
            el.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
            return;
        };
        let period = Duration::from_micros(1_000_000 / self.cap.fps.max(1) as u64);
        let now = Instant::now();
        if now >= self.next_tick {
            self.next_tick = (self.next_tick + period).max(now.checked_sub(period).unwrap_or(now));
            let size = rec.size;
            let bg = Some(self.cap.rec_bg);
            if let Some(r) = self.renderer.as_mut() {
                if r.capture_size(self.cap.rec_size) != Some(size) {
                    // the phone rotated: continue in a new file with the new size
                    self.stop_recording();
                    self.start_recording();
                } else if let Some(rgba) = r.capture(size, bg) {
                    self.recorder.as_ref().unwrap().push(rgba);
                }
            }
        }
        el.set_control_flow(ControlFlow::WaitUntil(self.next_tick));
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.stop_recording();
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Disconnected => self.on_disconnected(el),
            UserEvent::Failed(msg) => {
                eprintln!("handset: {msg}");
                el.exit();
            }
            UserEvent::Menu(id) => self.on_menu(el, &id),
            UserEvent::Adb(result) => self.on_adb(result),
            UserEvent::Landing(event) => self.on_landing(event),
            UserEvent::Frame => {
                let (Some(r), Some(w)) = (self.renderer.as_mut(), self.window.as_ref()) else { return };
                let mut s = self.slot.lock().unwrap();
                if s.fresh {
                    r.upload(&s.yuv);
                    self.device = (s.yuv.width as u16, s.yuv.height as u16);
                    s.fresh = false;
                }
                drop(s);

                let landscape = self.device.0 > self.device.1;
                if landscape != r.landscape() {
                    // turn the phone: its current width becomes the new height
                    let s = w.inner_size();
                    let new_h = (r.bezel_rect()[2] * s.width as f32).round() as u32;
                    r.set_landscape(landscape);
                    let size = r
                        .window_aspect()
                        .map(|a| winit::dpi::PhysicalSize::new((new_h as f32 * a).round() as u32, new_h));
                    if let Some(size) = size {
                        let _ = w.request_inner_size(size);
                    }
                    if self.debug {
                        eprintln!("orientation: {}", if landscape { "landscape" } else { "portrait" });
                    }
                }
                w.request_redraw();
                // `--record-for`: start once the first frame is on screen
                if let (Some(d), None) = (self.auto_record, self.auto_stop_at) {
                    self.start_recording();
                    if self.recorder.is_some() {
                        self.auto_stop_at = Some(Instant::now() + d);
                    }
                }
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(s) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(s.width, s.height);
                }
                self.refresh_toolbar();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(r) = self.renderer.as_mut() {
                    r.render();
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                let hover = self.toolbar_button();
                if hover != self.hover {
                    self.hover = hover;
                    self.refresh_toolbar();
                }
                if self.waiting.is_some() {
                    let over = self.landing_hit();
                    if over != self.waiting.as_ref().and_then(|wt| wt.hover) {
                        if let Some(wt) = self.waiting.as_mut() {
                            wt.hover = over;
                        }
                        if let Some(w) = &self.window {
                            w.set_cursor(Cursor::Icon(if over.is_some() {
                                CursorIcon::Pointer
                            } else {
                                CursorIcon::Default
                            }));
                        }
                        self.draw_connect();
                    }
                }
                if self.touching
                    && let Some((_, Some((x, y)))) = self.locate()
                {
                    if self.debug {
                        eprintln!("move -> device ({x}, {y})");
                    }
                    let _ = self.control.touch(Touch::Move, x, y, self.device.0, self.device.1);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } => {
                if self.touching {
                    self.touching = false;
                    let _ = self.control.touch(Touch::Up, 0, 0, self.device.0, self.device.1);
                }
                self.show_menu();
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.toast_hit() =>
            {
                if let Some(t) = self.toast.take()
                    && let Some(path) = t.path
                {
                    capture::reveal(&path);
                }
                self.refresh_overlay();
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.toolbar_button().is_some() =>
            {
                if let Some(b) = self.toolbar_button() {
                    self.on_button(el, b);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.waiting.is_some() =>
            {
                if let Some(i) = self.landing_hit() {
                    let action = self.waiting.as_ref().map(|wt| wt.buttons[i].action.clone());
                    if let Some(a) = action {
                        self.on_action(a);
                    }
                } else if let Some((p, _)) = self.locate()
                    && self.on_bezel(p)
                    && let Some(w) = &self.window
                {
                    let _ = w.drag_window();
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                let Some((p, dev)) = self.locate() else { return };
                if self.debug {
                    eprintln!("click {state:?} at {p:?} -> device {dev:?} (device size {:?})", self.device);
                }
                match (state, dev) {
                    (ElementState::Pressed, Some((x, y))) => {
                        self.touching = true;
                        let _ = self.control.touch(Touch::Down, x, y, self.device.0, self.device.1);
                    }
                    (ElementState::Pressed, None) => {
                        if self.on_bezel(p)
                            && let Some(w) = &self.window
                        {
                            let _ = w.drag_window();
                        }
                    }
                    (ElementState::Released, _) if self.touching => {
                        self.touching = false;
                        // release at the last known position inside the screen
                        let (x, y) = dev.unwrap_or((0, 0));
                        let _ = self.control.touch(Touch::Up, x, y, self.device.0, self.device.1);
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32 / 40.0, p.y as f32 / 40.0),
                };
                if dx == 0.0 && dy == 0.0 {
                    // trackpad phase events carry no movement
                } else if self.mods.super_key() || self.mods.control_key() {
                    // Cmd/Ctrl + scroll resizes the window
                    if let Some(w) = &self.window {
                        let s = w.inner_size();
                        let k = 1.0 + (dy as f64 * 0.04);
                        let _ = w.request_inner_size(winit::dpi::PhysicalSize::new(
                            (s.width as f64 * k).max(100.0),
                            (s.height as f64 * k).max(100.0),
                        ));
                    }
                } else if let Some((_, Some((x, y)))) = self.locate() {
                    if self.debug {
                        eprintln!("scroll ({dx}, {dy}) at device ({x}, {y})");
                    }
                    let _ = self.control.scroll(x, y, self.device.0, self.device.1, dx, dy);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if self.debug {
                    eprintln!("key {:?} text {:?}", event.logical_key, event.text);
                }
                let code = match &event.logical_key {
                    Key::Named(NamedKey::Escape) => Some(keycode::BACK),
                    Key::Named(NamedKey::Home) => Some(keycode::HOME),
                    Key::Named(NamedKey::End) => Some(keycode::APP_SWITCH),
                    Key::Named(NamedKey::Enter) => Some(keycode::ENTER),
                    Key::Named(NamedKey::Backspace) => Some(keycode::DEL),
                    Key::Named(NamedKey::Tab) => Some(keycode::TAB),
                    Key::Named(NamedKey::ArrowUp) => Some(keycode::DPAD_UP),
                    Key::Named(NamedKey::ArrowDown) => Some(keycode::DPAD_DOWN),
                    Key::Named(NamedKey::ArrowLeft) => Some(keycode::DPAD_LEFT),
                    Key::Named(NamedKey::ArrowRight) => Some(keycode::DPAD_RIGHT),
                    _ => None,
                };
                let cmd = self.mods.super_key() || self.mods.control_key();
                let ch = |k: &str| matches!(&event.logical_key, Key::Character(c) if c.eq_ignore_ascii_case(k));
                if cmd && ch("r") {
                    let _ = self.control.rotate_device();
                } else if cmd && ch("s") {
                    self.screenshot();
                } else if cmd && ch("e") {
                    if self.recorder.is_some() {
                        self.stop_recording();
                    } else {
                        self.start_recording();
                    }
                } else if let Some(c) = code {
                    let _ = self.control.key(c);
                } else if !self.mods.super_key()
                    && !self.mods.control_key()
                    && let Some(t) = &event.text
                {
                    let _ = self.control.text(t);
                }
            }
            _ => {}
        }
    }
}
