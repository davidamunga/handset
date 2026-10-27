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
use crate::restyle;
use crate::server;
use crate::settings;
use crate::stream::{Slot, UserEvent};
use crate::toolbar::{self, Button};
use crate::transfer;
use crate::update;
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
    /// The version this run replaced, when Handset has just been updated.
    pub updated_from: Option<String>,
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
    pub sync: Arc<Mutex<crate::clipboard::Sync>>,
    /// `--start-app`: open this app once the phone is mirrored.
    pub start_app: Option<String>,
    /// Show the toolbar at start (a saved setting).
    pub toolbar: bool,
    /// Show touch indicators at start (a saved setting or `--show-touches`).
    pub touches: bool,
    /// Check for a newer release now and then (a saved setting, off with `--no-update-check`).
    pub update_check: bool,
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
    /// The doctor's results, `None` while it runs.
    doctor: Option<Vec<crate::doctor::Check>>,
    /// Phones connected to over Wi-Fi before, newest first.
    recent: Vec<crate::recent::Recent>,
    /// After a Wi-Fi phone went away: its address, and when to try connecting to it next.
    retry_addr: Option<String>,
    next_retry: Instant,
    /// Mirroring was ended on purpose: a ready phone is offered, not started.
    paused: bool,
    /// An iOS device that is plugged in (macOS): its capture id and name. Offered, never started by itself.
    iphone: Option<(String, String)>,
    /// The address or pairing code being typed, which takes the keyboard while it is open.
    field: Option<connect::Field>,
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
            doctor: None,
            retry_addr: None,
            next_retry: Instant::now() + RECONNECT_EVERY,
            paused: false,
            iphone: None,
            field: None,
            recent: crate::recent::load(),
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
    /// A resize by dragging the frame: the edge or corner held and the opposite point that stays put (screen pixels).
    resizing: Option<(winit::window::ResizeDirection, (f64, f64))>,
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
    styling: Vec<std::thread::JoinHandle<()>>,
    auto_stop_at: Option<Instant>,
    hover: Option<Button>,
    /// The window is fully covered or minimised: nothing is drawn until it shows again.
    occluded: bool,
    toolbar_key: Option<(u32, Option<Button>, bool, bool, bool)>,
    toast: Option<Toast>,
    /// What the renderer's overlay currently shows (text, dot colour).
    overlay_key: Option<(String, Option<[u8; 3]>)>,
    overlay_opacity: f32,
    /// The overlay's text as laid out (wrapped), whether it has a dot, and its font size: what a click hit.
    overlay_layout: Option<(String, bool, u32)>,
    /// The version the update banner is shown for, until it is dismissed or acted on.
    banner: Option<String>,
    /// What the banner currently shows (text, width) and where its close cross begins, in its pixels.
    banner_key: Option<(String, u32)>,
    banner_close_x: u32,
    /// How long the screen has been black, and the message about it while it is.
    black: crate::black::BlackWatch,
    hint: Option<String>,
    notifications_open: bool,
    /// Whether the toolbar starts visible.
    toolbar_start: bool,
    touches_start: bool,
    /// Look for newer releases by itself (a saved setting), and the newer release once one is found.
    update_auto: bool,
    update: Option<update::Release>,
    /// The version this run has already announced with a toast.
    update_told: Option<String>,
    install: crate::install::Source,
    /// An update is being downloaded or prepared.
    updating: bool,
    /// Set after an update until the first toast has been shown.
    just_updated: Option<String>,
    /// Window height at start (physical pixels), what Default Size returns to.
    home_height: u32,
    mirror: Option<Mirror>,
    waiting: Option<Waiting>,
    proxy: EventLoopProxy<UserEvent>,
    opts: server::Options,
    mute: bool,
    sync: Arc<Mutex<crate::clipboard::Sync>>,
    start_app: Option<String>,
    /// The phone's apps, and whether they are still being listed.
    apps: Vec<crate::apps::AppEntry>,
    apps_loading: bool,
}

/// A short message pill. With a `path` it is clickable and reveals that file; with a `url` it opens the page.
struct Toast {
    text: String,
    since: Instant,
    ttl: Duration,
    path: Option<std::path::PathBuf>,
    url: Option<String>,
    /// The character where the EDIT part of the text begins (a capture that can be edited).
    edit_at: Option<usize>,
}

const TOAST: Duration = Duration::from_millis(1800);
/// Capture toasts stay long enough to be clicked.
const TOAST_SAVED: Duration = Duration::from_millis(4500);
const TOAST_FADE: Duration = Duration::from_millis(400);
/// Cursor position in window space (0..1) and the device pixel under it, if any.
type Located = ((f32, f32), Option<(i32, i32)>);
/// What the overlay should show: text, optional dot colour, opacity.
type OverlayWant = (String, Option<[u8; 3]>, f32);

const AMBER_DOT: [u8; 3] = [255, 190, 60];
const BLACK_HINT: &str = "BLACK SCREEN: PHONE ASLEEP OR APP BLOCKS CAPTURE";
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
            resizing: None,
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
            styling: Vec::new(),
            auto_stop_at: None,
            hover: None,
            occluded: false,
            toolbar_key: None,
            toast: None,
            overlay_key: None,
            overlay_opacity: 1.0,
            overlay_layout: None,
            banner: None,
            banner_key: None,
            banner_close_x: 0,
            black: Default::default(),
            hint: None,
            notifications_open: false,
            toolbar_start: init.toolbar,
            touches_start: init.touches,
            update_auto: init.update_check,
            just_updated: init.updated_from,
            update: None,
            update_told: None,
            install: crate::install::current(),
            updating: false,
            home_height: 0,
            mirror: init.mirror,
            waiting: init.wait.map(|w| Waiting::new(w.serial, w.wifi, Status::Looking)),
            proxy: init.proxy,
            opts: init.opts,
            mute: init.mute,
            sync: init.sync,
            start_app: init.start_app,
            apps: Vec::new(),
            apps_loading: false,
        }
    }

    /// Fetch the phone's app list in the background, for the Open App menu.
    fn list_apps(&mut self, serial: String) {
        self.apps.clear();
        self.apps_loading = true;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = crate::server::list_apps(&serial).map_err(|e| e.to_string());
            let _ = proxy.send_event(UserEvent::Apps(result));
        });
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
                recent: &wt.recent,
                note: wt.note.as_deref(),
                doctor: wt.doctor.as_deref(),
                ios: cfg!(target_os = "macos"),
                iphone: wt.iphone.as_ref().map(|(uid, name)| (uid.as_str(), name.as_str())),
                field: wt.field.as_ref(),
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
        // drawn like a phone frame: uploaded only when the window draws (`upload_latest`)
        let mut slot = self.slot.lock().unwrap();
        slot.yuv = connect::to_yuv(&screen.rgb, pw, ph);
        slot.fresh = true;
        drop(slot);
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
        // an iOS device is looked for on the same beat as adb (and must be, on the main thread)
        #[cfg(target_os = "macos")]
        let iphone = crate::ios::connected().into_iter().next().map(|d| (d.uid, d.name));
        #[cfg(not(target_os = "macos"))]
        let iphone = None;
        let changed = next != wt.status || wt.usb != usb || wt.iphone != iphone;
        wt.usb = usb;
        wt.iphone = iphone;
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
                // nothing answered the code for a while: say what usually gets in the way
                if wt.view == View::Qr
                    && wt.note.is_none()
                    && !wt.pairing
                    && wt.qr.as_ref().is_some_and(|q| q.at.elapsed() > connect::QR_HINT_AFTER)
                {
                    wt.note = Some(connect::QR_HINT.into());
                    redraw = true;
                }
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
            LandingEvent::Typed(addr) => {
                wt.field = None;
                wt.busy = true;
                wt.note = Some(format!("Connecting to {}...", crate::adb::normalize_addr(&addr)));
                connect::connect_async(proxy, addr);
            }
            LandingEvent::Code(code) => {
                // the phone's own pairing screen, not the one our QR code made
                let ours = wt.qr.as_ref().map(|q| q.name.as_str());
                match wt
                    .nearby
                    .iter()
                    .find(|s| s.kind == crate::adb::ServiceKind::Pairing && Some(s.name.as_str()) != ours)
                {
                    Some(p) => {
                        wt.field = None;
                        wt.pairing = true;
                        wt.note = Some("Pairing...".into());
                        connect::pair_async(proxy, p.addr.clone(), code);
                    }
                    None => {
                        wt.note =
                            Some("No phone is showing a pairing code. Open Pair device with pairing code on it.".into())
                    }
                }
            }
            LandingEvent::Doctor(checks) => wt.doctor = Some(checks),
        }
        if redraw {
            self.draw_connect();
        }
    }

    /// A key pressed while the connect screen's text field is open goes to the field. Returns whether it did.
    fn on_field_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        let cmd = self.mods.super_key() || self.mods.control_key();
        let Some(field) = self.waiting.as_mut().and_then(|wt| wt.field.as_mut()) else { return false };
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => self.on_action(Action::Cancel),
            Key::Named(NamedKey::Enter) => self.on_action(Action::Submit),
            Key::Named(NamedKey::Backspace) => {
                field.backspace();
                self.draw_connect();
            }
            Key::Character(c) if cmd && c.eq_ignore_ascii_case("v") => {
                if let Some(clip) = crate::clipboard::read_mac() {
                    field.paste(&clip);
                    self.draw_connect();
                }
            }
            // other shortcuts (zoom, for one) keep working while the field is open
            _ if cmd => return false,
            _ => {
                if let Some(t) = &event.text {
                    field.insert(t);
                    self.draw_connect();
                }
            }
        }
        true
    }

    /// A landing screen button was clicked.
    fn on_action(&mut self, action: Action) {
        if let Action::MirrorIos(uid) = &action {
            #[cfg(target_os = "macos")]
            self.begin_ios(uid.clone());
            #[cfg(not(target_os = "macos"))]
            let _ = uid;
            return;
        }
        let proxy = self.proxy.clone();
        let Some(wt) = self.waiting.as_mut() else { return };
        match action {
            Action::Open(View::Wifi) => {
                wt.view = View::Wifi;
                wt.field = None;
                wt.note = None;
                connect::want_mdns(true);
            }
            Action::Open(View::Qr) => {
                wt.view = View::Qr;
                wt.field = None;
                wt.note = None;
                wt.pairing = false;
                wt.paired_ip = None;
                wt.qr = Qr::new().ok();
                connect::want_mdns(true);
            }
            Action::Open(View::Doctor) | Action::RunChecks => {
                wt.view = View::Doctor;
                wt.note = None;
                wt.doctor = None;
                connect::doctor_async(proxy);
            }
            Action::Open(View::Home) | Action::Back => {
                wt.view = View::Home;
                wt.field = None;
                wt.note = None;
                wt.qr = None;
                connect::want_mdns(false);
            }
            Action::Connect(addr) if !wt.busy => {
                wt.busy = true;
                wt.note = Some(format!("Connecting to {addr}..."));
                connect::connect_async(proxy, addr);
            }
            Action::TypeAddress if !wt.busy => wt.field = Some(connect::Field::new(connect::FieldKind::Address)),
            Action::PairWithCode if !wt.busy && !wt.pairing => {
                wt.field = Some(connect::Field::new(connect::FieldKind::Code));
            }
            Action::Submit => {
                // the field stays open (with what was typed) until connecting or pairing actually starts
                if let Some(event) = wt.field.as_ref().and_then(|f| f.submit()) {
                    wt.hover = None;
                    self.on_landing(event);
                }
                return;
            }
            Action::Cancel => wt.field = None,
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
        let serial = mirror.serial().to_string();
        self.stop_recording();
        drop(mirror); // kills the server and removes the adb forward
        self.control = Control::disconnected();
        self.notifications_open = false;
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
        // the stream of a session ended on purpose (Phone menu) reports its end late: the connect screen is up
        if self.mirror.is_none() && self.waiting.is_some() && self.auto_record.is_none() {
            return;
        }
        // an iOS device: back to the connect screen in the Android frame, unless `--record-for` is done
        if self.is_ios() {
            eprintln!("the iOS device was disconnected");
            self.end_session();
            if self.auto_record.is_some() {
                el.exit();
                return;
            }
            self.switch_frame_family(false);
            self.waiting = Some(Waiting::new(None, false, Status::Looking));
            connect::spawn_poll(self.proxy.clone());
            self.draw_connect();
            return;
        }
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

    /// Mirror the iOS device `uid`, offered on the connect screen.
    #[cfg(target_os = "macos")]
    fn begin_ios(&mut self, uid: String) {
        let Some(wt) = self.waiting.as_mut() else { return };
        let name = wt.iphone.as_ref().filter(|(u, _)| *u == uid).map_or("iPhone".to_string(), |(_, n)| n.clone());
        wt.status = Status::Connecting(name.clone());
        self.draw_connect();
        let device = crate::ios::Device { uid, name };
        match Mirror::start_ios(
            &device,
            self.opts.audio,
            self.mute,
            self.proxy.clone(),
            self.slot.clone(),
            self.audio_sink.clone(),
        ) {
            Ok(mirror) => {
                self.with_audio = mirror.with_audio();
                self.control = Control::disconnected();
                self.title = mirror.device_name().to_string();
                if let Some(w) = &self.window {
                    w.set_title(&self.title);
                    w.set_cursor(Cursor::Icon(CursorIcon::Default));
                }
                self.mirror = Some(mirror);
                self.black = Default::default();
                self.hint = None;
                self.waiting = None;
                connect::stop_poll();
                connect::want_mdns(false);
                self.switch_frame_family(true);
            }
            Err(e) => {
                eprintln!("{e}");
                if let Some(wt) = self.waiting.as_mut() {
                    wt.status = Status::Failed(e);
                }
                self.draw_connect();
            }
        }
    }

    /// Going between an iOS device and Android: put on that family's saved frame. A frame of your own
    /// (`--frame` with a PNG) stays.
    fn switch_frame_family(&mut self, ios: bool) {
        if self.frame_spec.as_deref().is_some_and(|s| !frame::builtin_names().any(|n| n == s)) {
            return;
        }
        let saved = settings::Settings::load();
        let want = match saved.get(if ios { "frame-ios" } else { "frame" }) {
            Some("none") => None,
            Some(spec) => Some(spec.to_string()),
            None => Some(if ios { frame::IOS_DEFAULT } else { "obsidian" }.to_string()),
        };
        if want != self.frame_spec {
            self.set_frame(want);
        }
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
            self.sync.clone(),
        ) {
            Ok((mirror, control)) => {
                self.with_audio = mirror.with_audio();
                self.control = control;
                self.list_apps(mirror.serial().to_string());
                if let Some(name) = self.start_app.take() {
                    let _ = self.control.start_app(&name);
                }
                if let Some(w) = &self.window {
                    w.set_title(mirror.device_name());
                }
                self.title = mirror.device_name().to_string();
                self.mirror = Some(mirror);
                self.black = Default::default();
                self.hint = None;
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
        let ios = self.is_ios();
        let (Some(r), Some(w)) = (self.renderer.as_mut(), self.window.as_ref()) else { return };
        if !r.toolbar_visible() {
            self.toolbar_key = None;
            return;
        }
        let wh = w.inner_size().height;
        let key = (wh, self.hover, self.recorder.is_some(), ios, self.update.is_some());
        if self.toolbar_key != Some(key) {
            let order = toolbar::order(ios, self.update.is_some());
            r.set_toolbar_image(
                Some(&toolbar::raster(wh as f32, &order, self.hover, self.recorder.is_some())),
                order.len(),
            );
            self.toolbar_key = Some(key);
            w.request_redraw();
        }
    }

    /// Mirroring an iOS device: view only, and its own frames.
    fn is_ios(&self) -> bool {
        self.mirror.as_ref().is_some_and(|m| m.is_ios())
    }

    /// Copy the newest frame to the GPU, right before something is drawn. Never when it arrives: wgpu
    /// keeps each upload until the next submit, and a hidden or minimised window draws nothing, so
    /// frames (megabytes each, up to 60 a second) would pile up.
    fn upload_latest(&mut self) {
        let Some(r) = self.renderer.as_mut() else { return };
        let mut s = self.slot.lock().unwrap();
        if s.fresh {
            r.upload(&s.yuv);
            s.fresh = false;
        }
    }

    fn toolbar_button(&self) -> Option<Button> {
        let (r, w) = (self.renderer.as_ref()?, self.window.as_ref()?);
        if !r.toolbar_visible() {
            return None;
        }
        let s = w.inner_size();
        let order = toolbar::order(self.is_ios(), self.update.is_some());
        toolbar::button_at(s.width as f32, s.height as f32, self.cursor.0 as f32, self.cursor.1 as f32, &order)
    }

    fn on_button(&mut self, el: &ActiveEventLoop, b: Button) {
        if self.debug {
            eprintln!("toolbar: {b:?}");
        }
        match b {
            Button::Back => {
                let _ = self.control.key(keycode::BACK);
            }
            Button::Home => {
                let _ = self.control.key(keycode::HOME);
            }
            Button::Recents => {
                let _ = self.control.key(keycode::APP_SWITCH);
            }
            Button::Power => {
                let _ = self.control.key(keycode::POWER);
            }
            Button::VolumeUp => {
                let _ = self.control.key(keycode::VOLUME_UP);
            }
            Button::VolumeDown => {
                let _ = self.control.key(keycode::VOLUME_DOWN);
            }
            Button::Notifications => {
                // the phone does not tell us whether the shade is open, so this toggles what we last asked for
                self.notifications_open = !self.notifications_open;
                let _ = if self.notifications_open {
                    self.control.expand_notifications()
                } else {
                    self.control.collapse_panels()
                };
            }
            Button::Screenshot => self.on_menu(el, "shot"),
            Button::Record => self.on_menu(el, "rec"),
            Button::Rotate => self.on_menu(el, "rotate"),
            Button::Update => self.act_on_update(),
            Button::More => self.show_menu(),
            Button::Quit => el.exit(),
            Button::NextFrame => {
                // None -> each built-in frame (only the iOS ones for an iOS device) -> back to None
                let names = frame::names_for(self.is_ios());
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
        let ios = self.is_ios();
        let frames = frame::names_for(ios);
        let builtin = |s: &str| frame::builtin_names().any(|n| n == s);
        let state = menu::State {
            ios,
            frame: self.frame_spec.clone().filter(|s| frames.contains(&s.as_str())),
            frames,
            custom_frame: self.frame_spec.as_deref().is_some_and(|s| !builtin(s)),
            recording: self.recorder.is_some(),
            gif: self.cap.format == capture::Format::Gif,
            normalize: self.cap.normalize,
            audio: self.with_audio,
            muted: audio::is_muted(),
            toolbar: self.renderer.as_ref().is_some_and(|r| r.toolbar_visible()),
            touches: self.renderer.as_ref().is_some_and(|r| r.show_touches()),
            update_auto: self.update_auto,
            update: self.update.as_ref().map(|u| u.version.clone()),
            update_command: self.install.upgrade_command(),
            update_install: crate::updater::os_for(self.install).is_some(),
            clipboard: self.cap.clipboard,
            auto_style: self.cap.auto_style,
            mirroring: self.mirror.is_some(),
            usb: self.mirror.as_ref().is_some_and(|m| !connect::is_wireless(m.serial())),
            sync: self.sync.lock().unwrap().enabled,
            apps: self.apps.clone(),
            apps_loading: self.apps_loading,
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
        // an iOS device keeps its own frame, so going back to Android finds the phone's frame
        settings::save(if self.is_ios() { "frame-ios" } else { "frame" }, self.frame_spec_name(&spec));
        self.frame_spec = spec;
        // keep the height, follow the new aspect ratio
        let height = w.inner_size().height;
        if let Some(size) = self.size_for_height(height) {
            let _ = self.window.as_ref().unwrap().request_inner_size(size);
        }
        self.toolbar_key = None;
        self.window.as_ref().unwrap().request_redraw();
    }

    /// How a frame choice is written in the settings file.
    fn frame_spec_name<'a>(&self, spec: &'a Option<String>) -> &'a str {
        spec.as_deref().unwrap_or("none")
    }

    /// Scale the window by `k`, never past the screen, where the edges could not be reached again.
    fn zoom(&mut self, k: f64) {
        let Some(w) = &self.window else { return };
        let s = w.inner_size();
        let max_h = w.current_monitor().map_or(f64::MAX, |m| m.size().height as f64 * 0.95);
        let k = k.min((max_h / s.height as f64).max(1.0));
        let _ = w.request_inner_size(winit::dpi::PhysicalSize::new(
            (s.width as f64 * k).max(100.0),
            (s.height as f64 * k).max(100.0),
        ));
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
            "fmt:mp4" => {
                self.cap.format = capture::Format::Mp4;
                settings::save("format", "mp4");
            }
            "fmt:gif" => {
                self.cap.format = capture::Format::Gif;
                settings::save("format", "gif");
            }
            "norm" => {
                self.cap.normalize = !self.cap.normalize;
                settings::save_bool("normalize", self.cap.normalize);
            }
            "clip" => {
                self.cap.clipboard = !self.cap.clipboard;
                settings::save_bool("clipboard", self.cap.clipboard);
            }
            "style" => {
                self.cap.auto_style = !self.cap.auto_style;
                settings::save_bool("auto-style", self.cap.auto_style);
            }
            "sync" => {
                let mut s = self.sync.lock().unwrap();
                s.enabled = !s.enabled;
                let on = s.enabled;
                drop(s);
                settings::save_bool("sync-clipboard", on);
                self.show_toast(if on { "CLIPBOARD SYNC ON" } else { "CLIPBOARD SYNC OFF" });
            }
            "toolbar" => {
                if let Some(r) = self.renderer.as_mut() {
                    let on = !r.toolbar_visible();
                    r.set_toolbar_visible(on);
                    settings::save_bool("toolbar", on);
                }
                self.toolbar_key = None;
                if let Some(h) = self.window.as_ref().map(|w| w.inner_size().height)
                    && let Some(size) = self.size_for_height(h)
                {
                    let _ = self.window.as_ref().unwrap().request_inner_size(size);
                }
            }
            "update" => self.check_for_update(true),
            "update-open" => match self.install.upgrade_command() {
                Some(cmd) => {
                    crate::clipboard::write_text(cmd);
                    self.show_toast(&format!("COPIED: {}", cmd.to_uppercase()));
                }
                None => crate::tools::open_url(self.update.as_ref().map_or(update::RELEASES, |u| u.url.as_str())),
            },
            "update-install" => self.start_update(),
            "update-notes" => crate::tools::open_url(self.update.as_ref().map_or(update::RELEASES, |u| u.url.as_str())),
            "update-skip" => {
                if let Some(u) = self.update.take() {
                    settings::save("update-skip", &u.version);
                    self.show_toast(&format!("SKIPPING {}", u.version));
                }
                self.refresh_toolbar();
                self.refresh_banner();
            }
            "update-later" => {
                settings::save("update-remind", &(update::now() + update::SNOOZE).to_string());
                self.update = None;
                self.show_toast("WILL REMIND YOU IN 3 DAYS");
                self.refresh_toolbar();
                self.refresh_banner();
            }
            "update-auto" => {
                self.update_auto = !self.update_auto;
                settings::save_bool("update-check", self.update_auto);
            }
            "touches" => {
                if let Some(r) = self.renderer.as_mut() {
                    let on = !r.show_touches();
                    r.set_show_touches(on);
                    settings::save_bool("touches", on);
                }
            }
            "mute" => {
                audio::set_muted(!audio::is_muted());
                settings::save_bool("mute", audio::is_muted());
            }
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
            "zoom:in" => self.zoom(1.1),
            "zoom:out" => self.zoom(1.0 / 1.1),
            "zoom:reset" => {
                if let (Some(w), Some(size)) = (&self.window, self.size_for_height(self.home_height)) {
                    let _ = w.request_inner_size(size);
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
                        "style" => self.style_in_background(path),
                        "edit" => self.edit_capture(&path),
                        "copy" => {
                            let ok = capture::copy_to_clipboard(&path);
                            self.show_toast(if ok { "COPIED" } else { "COPY FAILED" });
                        }
                        _ => {}
                    }
                }
            }
            other if other.starts_with("app:") => {
                let package = &other["app:".len()..];
                let name = self.apps.iter().find(|a| a.package == package).map_or(package, |a| a.name.as_str());
                let toast = format!("OPENING {}", name.to_uppercase());
                let _ = self.control.start_app(package);
                self.show_toast(&toast);
            }
            other => {
                if let Some(name) = other.strip_prefix("frame:") {
                    self.set_frame(Some(name.to_string()));
                }
            }
        }
    }

    /// The window got focus: if the Mac's clipboard changed since the last sync, send it to the phone.
    fn send_clipboard(&mut self) {
        if self.mirror.is_none() || !self.sync.lock().unwrap().enabled {
            return;
        }
        let Some(current) = crate::clipboard::read_mac() else { return };
        let to_send = self.sync.lock().unwrap().mac_changed(&current);
        if let Some(text) = to_send {
            let _ = self.control.set_clipboard(&text);
        }
    }

    /// Show or hide the "black screen" message.
    fn set_black_hint(&mut self, on: bool) {
        if self.debug {
            eprintln!("black screen hint: {}", if on { "shown" } else { "cleared" });
        }
        self.hint = on.then(|| BLACK_HINT.to_string());
        self.refresh_overlay();
    }

    /// Mark a touch on the screen for the touch indicator (device pixels in, fractions of the video out).
    fn show_touch(&mut self, phase: Touch, x: i32, y: i32) {
        let (dw, dh) = (self.device.0.max(1) as f32, self.device.1.max(1) as f32);
        let Some(r) = self.renderer.as_mut() else { return };
        match phase {
            Touch::Down => r.touch_down(x as f32 / dw, y as f32 / dh),
            Touch::Move => r.touch_move(x as f32 / dw, y as f32 / dh),
            Touch::Up => r.touch_up(),
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    /// Ask GitHub for the latest release in the background.
    fn check_for_update(&self, manual: bool) {
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let _ = proxy.send_event(UserEvent::Update(update::latest(), manual));
        });
    }

    fn on_update(&mut self, result: Result<update::Release, String>, manual: bool) {
        match result {
            Ok(rel) => {
                let saved = settings::Settings::load();
                let skipped = saved.get("update-skip");
                let remind = saved.get("update-remind").and_then(|t| t.parse().ok());
                if update::is_newer(&rel.version, &update::current()) {
                    // the menu always offers the update; the toast only when it is welcome
                    let speak = update::announce(
                        &rel.version,
                        &update::current(),
                        skipped,
                        remind,
                        update::now(),
                        self.update_told.as_deref(),
                        manual,
                    );
                    if speak {
                        self.update_told = Some(rel.version.clone());
                        self.banner = Some(rel.version.clone());
                    }
                    self.update = Some(rel);
                    self.refresh_toolbar();
                    self.refresh_banner();
                } else if manual {
                    self.show_toast(&format!("UP TO DATE ({})", update::current()));
                }
            }
            Err(e) => {
                eprintln!("update check failed: {e}");
                if manual {
                    self.show_toast("COULD NOT CHECK FOR UPDATES");
                }
            }
        }
    }

    /// Download, check and unpack the waiting update in the background; when it is ready the app quits and a
    /// helper swaps it in and starts it (see `updater.rs`).
    /// Show the update banner while an announced update waits, as wide as the phone's screen.
    fn refresh_banner(&mut self) {
        let want = match (&self.banner, &self.update) {
            (Some(v), Some(rel)) if *v == rel.version && !self.updating => {
                // what is new on one line, what a click does on the next
                Some(crate::install::notice(self.install, &rel.version, &rel.url).text.replacen(" - ", "\n", 1))
            }
            _ => None,
        };
        let (Some(r), Some(w)) = (self.renderer.as_mut(), self.window.as_ref()) else { return };
        let key = want.map(|text| (text, r.overlay_max_width()));
        if self.banner_key != key {
            match &key {
                Some((text, width)) => {
                    let s = ((w.scale_factor() * 1.5).round() as u32).max(1);
                    let (pill, close_x) = overlay::banner(text, s, *width);
                    r.set_banner(Some(&pill));
                    self.banner_close_x = close_x;
                }
                None => r.set_banner(None),
            }
            self.banner_key = key;
            w.request_redraw();
        }
    }

    /// Where on the update banner the pointer is, in the banner's pixels.
    fn banner_at(&self) -> Option<(f32, f32)> {
        self.renderer.as_ref()?.banner_point(self.cursor.0 as f32, self.cursor.1 as f32)
    }

    /// The banner's close cross: hide it and stay quiet for a few days; the toolbar button still offers the update.
    fn dismiss_banner(&mut self) {
        settings::save("update-remind", &(update::now() + update::SNOOZE).to_string());
        self.banner = None;
        self.refresh_banner();
        self.show_toast("UPDATE STAYS IN THE TOOLBAR");
    }

    /// The toolbar's update button: what the update notice offers for this install (install in place, copy the
    /// package manager's command, or open the release page).
    fn act_on_update(&mut self) {
        let Some(rel) = &self.update else { return };
        let n = crate::install::notice(self.install, &rel.version, &rel.url);
        if n.install {
            self.start_update();
        } else if let Some(cmd) = n.copy {
            crate::clipboard::write_text(&cmd);
            self.show_toast(&format!("COPIED: {}", cmd.to_uppercase()));
        } else if let Some(url) = n.url {
            crate::tools::open_url(&url);
        }
    }

    fn start_update(&mut self) {
        let Some(rel) = self.update.clone() else { return };
        if self.updating {
            return;
        }
        if self.recorder.is_some() {
            self.show_toast("STOP RECORDING FIRST");
            return;
        }
        if crate::updater::os_for(self.install).is_none() {
            crate::tools::open_url(&rel.url);
            return;
        }
        self.updating = true;
        self.refresh_banner();
        self.show_toast("DOWNLOADING UPDATE...");
        let (proxy, source) = (self.proxy.clone(), self.install);
        std::thread::spawn(move || {
            let work = std::env::temp_dir().join(format!("handset-update-{}", std::process::id()));
            let exe = std::env::current_exe().unwrap_or_default();
            let tell = proxy.clone();
            let result = crate::updater::prepare(
                &rel.version,
                source,
                &exe,
                &work,
                &crate::verify::trusted_keys(),
                true,
                &|p| {
                    let _ = tell.send_event(UserEvent::UpdateProgress(p));
                },
            );
            let _ = proxy.send_event(UserEvent::UpdateReady(result.map(|swap| (swap, work)), rel.url));
        });
    }

    fn on_update_ready(
        &mut self,
        el: &ActiveEventLoop,
        result: Result<(crate::updater::Swap, std::path::PathBuf), crate::updater::Failure>,
        page: String,
    ) {
        use crate::updater::Failure;
        self.updating = false;
        match result {
            Ok((swap, work)) => match crate::updater::spawn_helper(&swap, &work) {
                Ok(()) => {
                    eprintln!("update: restarting to install it");
                    el.exit();
                }
                Err(e) => {
                    eprintln!("update failed: {e}");
                    self.show_toast("UPDATE FAILED");
                }
            },
            Err(Failure::Unsigned) => {
                eprintln!("update: the release is not signed, so it is not installed in place");
                crate::tools::open_url(&page);
                self.show_toast("NOT SIGNED - OPENING DOWNLOAD PAGE");
            }
            Err(Failure::NotWritable(dir)) => {
                eprintln!("update: Handset cannot write to {dir}, so it cannot replace itself there");
                crate::tools::open_url(&page);
                self.show_toast("NO PERMISSION TO UPDATE HERE - OPENING DOWNLOAD PAGE");
            }
            Err(Failure::Other(e)) => {
                eprintln!("update failed: {e}");
                self.show_toast("UPDATE FAILED - SEE THE LOG");
            }
        }
    }

    /// Install a dropped APK or copy another file to the phone's Downloads.
    fn drop_file(&mut self, path: std::path::PathBuf) {
        let Some(serial) = self.mirror.as_ref().map(|m| m.serial().to_string()) else {
            self.show_toast("CONNECT A PHONE FIRST");
            return;
        };
        if self.is_ios() {
            self.show_toast("NOT AVAILABLE ON IOS YET");
            return;
        }
        let verb = if transfer::kind(&path) == transfer::Kind::Install { "INSTALLING" } else { "COPYING" };
        self.show_toast(&format!("{verb} {}...", transfer::name(&path).to_uppercase()));
        if let Some(t) = self.toast.as_mut() {
            t.ttl = Duration::from_secs(600);
        }
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let _ = proxy.send_event(UserEvent::Transfer(transfer::send(&serial, &path)));
        });
    }

    fn show_toast(&mut self, text: &str) {
        self.toast = Some(Toast {
            text: text.to_string(),
            since: Instant::now(),
            ttl: TOAST,
            path: None,
            url: None,
            edit_at: None,
        });
        self.refresh_overlay();
    }

    /// Announce a saved capture; clicking the toast shows it in Finder. The
    /// capture is also copied to the clipboard unless that is switched off.
    fn show_saved(&mut self, what: &str, path: std::path::PathBuf) {
        let copied = self.cap.clipboard && capture::copy_to_clipboard(&path);
        if self.cap.clipboard {
            eprintln!("clipboard: {}", if copied { "copied" } else { "copy failed" });
        }
        let mut text = format!("{what} {} - SHOW", if copied { "COPIED" } else { "SAVED" });
        // a styled copy is the result of editing; everything else can be edited
        let edit_at = (what != "STYLED COPY").then(|| {
            let at = text.chars().count() + 1;
            text.push_str(" | EDIT");
            at
        });
        self.toast =
            Some(Toast { text, since: Instant::now(), ttl: TOAST_SAVED, path: Some(path), url: None, edit_at });
        self.refresh_overlay();
    }

    /// Is the cursor on a clickable capture toast?
    fn toast_hit(&self) -> bool {
        self.toast.as_ref().is_some_and(|t| t.path.is_some() || t.url.is_some())
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
        } else if let Some(b) = self.hover {
            // what the toolbar button under the pointer does
            let label = match (&self.update, b) {
                (Some(rel), Button::Update) => format!("UPDATE TO {}", rel.version),
                _ => b.label().to_string(),
            };
            Some((label, None, 1.0))
        } else if self.recorder.is_none()
            && let Some(h) = &self.hint
        {
            Some((h.clone(), Some(AMBER_DOT), 1.0))
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
                    let s = ((w.scale_factor() * 1.5).round() as u32).max(1);
                    let (text, dot) = key.clone().unwrap();
                    // the pill sits on the phone's screen: wrap long messages to fit it, and shrink only if they must
                    let (laid, s) = overlay::fit(&text, dot.is_some(), s, r.overlay_max_width());
                    r.set_overlay(Some(&overlay::pill(&laid, dot, s)));
                    self.overlay_layout = Some((laid, dot.is_some(), s));
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
        self.upload_latest();
        let Some(r) = self.renderer.as_mut() else { return };
        let Some((w, h)) = r.capture_size(self.cap.rec_size.max(1800)) else { return };
        match r
            .capture((w, h), None)
            .ok_or_else(|| "no video yet".to_string())
            .and_then(|rgba| capture::save_png(&rgba, w, h))
        {
            Ok(p) => {
                eprintln!("screenshot saved: {}", p.display());
                if self.cap.auto_style {
                    self.style_in_background(p.clone());
                }
                self.show_saved("SCREENSHOT", p);
            }
            Err(e) => {
                eprintln!("screenshot failed: {e}");
                self.show_toast("SCREENSHOT FAILED");
            }
        }
    }

    /// Open the style editor on a saved capture, in its own process.
    fn edit_capture(&self, path: &std::path::Path) {
        let exe = std::env::current_exe().unwrap_or_else(|_| "handset".into());
        let _ = crate::tools::command(exe).arg("--edit").arg(path).spawn();
    }

    /// Save a styled copy of a capture beside it, off the UI thread; the result arrives as `UserEvent::Styled`.
    fn style_in_background(&mut self, path: std::path::PathBuf) {
        let style = self.cap.style.clone();
        let proxy = self.proxy.clone();
        self.show_toast("STYLING...");
        self.styling.push(std::thread::spawn(move || {
            let _ = proxy.send_event(UserEvent::Styled(restyle::export(&path, &style)));
        }));
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
                // the phone's outline, so a styled copy can cut the phone out of the recording's backdrop
                self.upload_latest();
                if let Some(rgba) = self.renderer.as_mut().and_then(|r| r.capture(size, None)) {
                    let _ = restyle::save_matte(&rec.path, &rgba, size.0, size.1);
                }
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
            if self.cap.auto_style {
                self.style_in_background(path.clone());
            }
            self.show_saved("RECORDING", path);
        }
    }

    /// Which corner of the phone the pointer is on, for resizing the borderless frame by dragging.
    /// The window's own edge is transparent (the system lets clicks fall through it), so the band lies
    /// inside the bezel: a point counts as an edge when the bezel ends within a few pixels of it.
    fn resize_zone(&self) -> Option<winit::window::ResizeDirection> {
        use winit::window::ResizeDirection::*;
        self.bezel.as_ref()?;
        let s = self.window.as_ref()?.inner_size();
        let (w, h) = (s.width as f64, s.height as f64);
        // only the corners resize; the rest of the bezel moves the window
        let m = (w.min(h) * 0.012).clamp(4.0, 12.0);
        let at = |dx: f64, dy: f64| {
            let (x, y) = (self.cursor.0 + dx, self.cursor.1 + dy);
            self.on_bezel(((x / w) as f32, (y / h) as f32))
        };
        if !at(0.0, 0.0) {
            return None;
        }
        let c = m * 3.0;
        let (cl, cr, ct, cb) = (!at(-c, 0.0), !at(c, 0.0), !at(0.0, -c), !at(0.0, c));
        match () {
            _ if cl && ct => Some(NorthWest),
            _ if cr && ct => Some(NorthEast),
            _ if cl && cb => Some(SouthWest),
            _ if cr && cb => Some(SouthEast),
            _ => None,
        }
    }

    /// Where the pointer is on the screen, in pixels.
    fn screen_cursor(&self) -> Option<(f64, f64)> {
        let p = self.window.as_ref()?.outer_position().ok()?;
        Some((p.x as f64 + self.cursor.0, p.y as f64 + self.cursor.1))
    }

    /// Start resizing by dragging (the platform's own drag-resize is not available on macOS).
    fn begin_resize(&mut self, d: winit::window::ResizeDirection) {
        use winit::window::ResizeDirection::*;
        let Some(w) = &self.window else { return };
        let (Ok(p), s) = (w.outer_position(), w.outer_size()) else { return };
        let (x, y, r, b) = (p.x as f64, p.y as f64, p.x as f64 + s.width as f64, p.y as f64 + s.height as f64);
        let ax = if matches!(d, West | NorthWest | SouthWest) { r } else { x };
        let ay = if matches!(d, North | NorthWest | NorthEast) { b } else { y };
        self.resizing = Some((d, (ax, ay)));
    }

    fn drag_resize(&mut self) {
        use winit::window::ResizeDirection::*;
        let (Some((d, (ax, ay))), Some((gx, gy))) = (self.resizing, self.screen_cursor()) else { return };
        let Some(a) = self.renderer.as_ref().and_then(|r| r.window_aspect()) else { return };
        let Some(w) = &self.window else { return };
        let max_h = w.current_monitor().map_or(f64::MAX, |m| m.size().height as f64 * 0.95);
        let by_height = matches!(d, North | South);
        let (nw, nh) = if by_height {
            let h = if d == North { ay - gy } else { gy - ay };
            (h * a as f64, h)
        } else {
            let wd = if matches!(d, West | NorthWest | SouthWest) { ax - gx } else { gx - ax };
            (wd, wd / a as f64)
        };
        let k = (max_h / nh).min(1.0);
        let (nw, nh) = ((nw * k).max(100.0).round(), (nh * k).max(100.0 / a as f64).round());
        let x = if matches!(d, West | NorthWest | SouthWest) { ax - nw } else { ax };
        let y = if matches!(d, North | NorthWest | NorthEast) { ay - nh } else { ay };
        w.set_outer_position(winit::dpi::PhysicalPosition::new(x, y));
        let _ = w.request_inner_size(winit::dpi::PhysicalSize::new(nw as u32, nh as u32));
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
        let side = if self.toolbar_start { toolbar::SIDE_FRAC } else { 0.0 };
        let aspect = (self.bezel.as_ref().map_or(9.0 / 20.0, |b| b.aspect()) + side) as f64;
        // fit the screen (leaving room for a taskbar or menu bar) rather than a fixed size
        let monitor = el.primary_monitor().or_else(|| el.available_monitors().next());
        let scale = monitor.as_ref().map_or(1.0, |m| m.scale_factor());
        let area = work_area()
            .or_else(|| monitor.as_ref().map(|m| (m.position().x, m.position().y, m.size().width, m.size().height)));
        let height = area.map_or(820.0, |(_, _, _, h)| {
            (h as f64 / scale * if cfg!(windows) { 0.95 } else { 0.8 }).clamp(400.0, 1000.0)
        });
        let mut attrs = Window::default_attributes()
            .with_title(self.title.clone())
            .with_window_icon(if cfg!(target_os = "macos") { None } else { window_icon() })
            .with_inner_size(LogicalSize::new(height * aspect, height))
            .with_min_inner_size(LogicalSize::new(200.0 * aspect, 200.0));
        #[cfg(windows)]
        if let Some((x, y, w, h)) = area {
            let (win_w, win_h) = ((height * aspect * scale) as i32, (height * scale) as i32);
            attrs = attrs.with_position(winit::dpi::PhysicalPosition::new(
                x + (w as i32 - win_w) / 2,
                y + (h as i32 - win_h) / 2,
            ));
        }
        // desktops match the window to its `handset.desktop` entry (icon, taskbar grouping) by this class
        #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
        {
            use winit::platform::wayland::WindowAttributesExtWayland;
            use winit::platform::x11::WindowAttributesExtX11;
            attrs = WindowAttributesExtWayland::with_name(attrs, "handset", "handset");
            attrs = WindowAttributesExtX11::with_name(attrs, "handset", "handset");
        }
        if self.bezel.is_some() {
            attrs = attrs.with_transparent(true).with_decorations(false);
            // the window's own (white) backing bitmap would show through the composition swapchain
            #[cfg(windows)]
            {
                use winit::platform::windows::WindowAttributesExtWindows;
                attrs = attrs.with_no_redirection_bitmap(true);
            }
        }
        let window = Arc::new(el.create_window(attrs).expect("create window"));
        match Renderer::new(window.clone(), self.bezel.as_ref()) {
            Ok(mut r) => {
                r.set_toolbar_visible(self.toolbar_start);
                r.set_show_touches(self.touches_start);
                if update::check_on_start(self.update_auto, self.auto_record.is_some()) {
                    self.check_for_update(false);
                }
                self.renderer = Some(r);
            }
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
        self.home_height = window.inner_size().height;
        self.window = Some(window);
        if let Some(serial) = self.mirror.as_ref().filter(|m| !m.is_ios()).map(|m| m.serial().to_string()) {
            self.list_apps(serial);
            if let Some(name) = self.start_app.take() {
                let _ = self.control.start_app(&name);
            }
        }
        if self.waiting.is_some() {
            // HANDSET_LANDING=wifi|qr|address|code opens that view (or field) first, for looking at it without clicking
            match std::env::var("HANDSET_LANDING").as_deref() {
                Ok("wifi") => self.on_action(Action::Open(View::Wifi)),
                Ok("qr") => self.on_action(Action::Open(View::Qr)),
                Ok("address") => {
                    self.on_action(Action::Open(View::Wifi));
                    self.on_action(Action::TypeAddress);
                }
                Ok("code") => {
                    self.on_action(Action::Open(View::Qr));
                    self.on_action(Action::PairWithCode);
                }
                _ => self.draw_connect(),
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.renderer.is_some()
            && let Some(from) = self.just_updated.take()
        {
            eprintln!("updated from {from} to {}", update::CURRENT);
            self.show_toast(&format!("UPDATED TO {}", update::CURRENT));
        }
        if self.auto_stop_at.is_some_and(|t| Instant::now() >= t) {
            self.auto_stop_at = None;
            self.auto_record = None;
            self.stop_recording();
            // a run that ends by itself must not leave a styled copy half made
            for job in self.styling.drain(..) {
                let _ = job.join();
            }
            el.exit();
            return;
        }
        let connect_wake = self.tick_connect();
        if let Some(on) = self.black.tick(Instant::now()) {
            self.set_black_hint(on);
        }
        let connecting =
            self.waiting.as_ref().is_some_and(|w| w.starting.is_some() || matches!(w.status, Status::Connecting(_)));
        if let Some(w) = &self.window {
            crate::dock::set(crate::dock::state_for(self.recorder.is_some(), self.mirror.is_some(), connecting), w);
        }
        self.refresh_toolbar();
        let fading = self.renderer.as_ref().is_some_and(|r| r.touches_animating());
        if fading && let Some(w) = &self.window {
            w.request_redraw();
        }
        let animating = self.refresh_overlay() || fading;
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
            self.upload_latest();
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
            UserEvent::Apps(result) => {
                self.apps_loading = false;
                match result {
                    Ok(apps) => {
                        if self.debug {
                            eprintln!("{} apps listed", apps.len());
                        }
                        self.apps = apps;
                    }
                    Err(e) => eprintln!("could not list the phone's apps: {e}"),
                }
            }
            UserEvent::Update(result, manual) => self.on_update(result, manual),
            UserEvent::UpdateProgress(p) => {
                if self.updating {
                    self.show_toast(&format!("DOWNLOADING UPDATE {}%", (p * 100.0).round() as u32));
                }
            }
            UserEvent::UpdateReady(result, page) => self.on_update_ready(el, result, page),
            UserEvent::Transfer(result) => match result {
                Ok(text) => self.show_toast(&text.to_uppercase()),
                Err(e) => {
                    eprintln!("drop failed: {e}");
                    self.show_toast(&format!("FAILED: {}", e.to_uppercase()));
                }
            },
            UserEvent::Styled(result) => match result {
                Ok(path) => self.show_saved("STYLED COPY", path),
                Err(e) => {
                    eprintln!("styling failed: {e}");
                    self.show_toast("STYLING FAILED");
                }
            },
            UserEvent::Adb(result) => self.on_adb(result),
            UserEvent::Landing(event) => self.on_landing(event),
            UserEvent::Frame => {
                let (Some(r), Some(w)) = (self.renderer.as_mut(), self.window.as_ref()) else { return };
                // the frame itself goes to the GPU when it is drawn (`upload_latest`)
                let s = self.slot.lock().unwrap();
                let black = s.black;
                if s.fresh {
                    self.device = (s.yuv.width as u16, s.yuv.height as u16);
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
                if let Some(on) = self.black.frame(black, Instant::now()) {
                    self.set_black_hint(on);
                }
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
                    // a dragged edge gives any shape: snap back to the frame's proportions
                    if let Some(a) = self.renderer.as_ref().and_then(|r| r.window_aspect()) {
                        let want_w = (s.height as f32 * a).round() as u32;
                        if want_w.abs_diff(s.width) > 2 && s.height > 0 {
                            let _ = w.request_inner_size(winit::dpi::PhysicalSize::new(want_w, s.height));
                        }
                    }
                    w.request_redraw();
                }
            }
            WindowEvent::Occluded(o) => {
                self.occluded = o;
                if !o && let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            // a covered window cannot be drawn to; leave the newest frame in the slot for when it shows
            WindowEvent::RedrawRequested if self.occluded => {}
            WindowEvent::RedrawRequested => {
                self.refresh_banner();
                self.upload_latest();
                if let Some(r) = self.renderer.as_mut() {
                    r.render();
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::HoveredFile(path) => {
                let verb = if transfer::kind(&path) == transfer::Kind::Install { "INSTALL" } else { "COPY" };
                self.show_toast(&format!("DROP TO {verb} {}", transfer::name(&path).to_uppercase()));
            }
            WindowEvent::HoveredFileCancelled => {
                self.toast = None;
                self.refresh_overlay();
            }
            WindowEvent::DroppedFile(path) => self.drop_file(path),
            WindowEvent::Focused(true) => self.send_clipboard(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                if self.resizing.is_some() {
                    self.drag_resize();
                    return;
                }
                let hover = self.toolbar_button();
                if hover != self.hover {
                    self.hover = hover;
                    self.refresh_toolbar();
                    self.refresh_overlay();
                }
                if let Some(w) = &self.window
                    && self.waiting.is_none()
                {
                    use winit::window::{CursorIcon as C, ResizeDirection as D};
                    w.set_cursor(Cursor::Icon(match self.resize_zone() {
                        Some(D::North | D::South) => C::NsResize,
                        Some(D::East | D::West) => C::EwResize,
                        Some(D::NorthWest | D::SouthEast) => C::NwseResize,
                        Some(D::NorthEast | D::SouthWest) => C::NeswResize,
                        None => C::Default,
                    }));
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
                    self.show_touch(Touch::Move, x, y);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Right, .. } => {
                if self.touching {
                    self.touching = false;
                    let _ = self.control.touch(Touch::Up, 0, 0, self.device.0, self.device.1);
                    self.show_touch(Touch::Up, 0, 0);
                }
                self.show_menu();
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. }
                if self.resizing.is_some() =>
            {
                self.resizing = None;
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.resize_zone().is_some() && self.toolbar_button().is_none() =>
            {
                if let Some(d) = self.resize_zone() {
                    self.begin_resize(d);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.banner_at().is_some() =>
            {
                if self.banner_at().is_some_and(|(x, _)| x >= self.banner_close_x as f32) {
                    self.dismiss_banner();
                } else {
                    self.act_on_update();
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. }
                if self.toast_hit() =>
            {
                let on_edit = self.toast.as_ref().and_then(|t| t.edit_at).is_some_and(|at| {
                    let hit = self
                        .renderer
                        .as_ref()
                        .and_then(|r| r.overlay_point(self.cursor.0 as f32, self.cursor.1 as f32));
                    match (hit, &self.overlay_layout) {
                        (Some((x, y)), Some((laid, dot, s))) => overlay::char_at(laid, *dot, *s, x, y) >= at,
                        _ => false,
                    }
                });
                if let Some(t) = self.toast.take() {
                    if let Some(path) = t.path {
                        if on_edit {
                            self.edit_capture(&path);
                        } else {
                            capture::reveal(&path);
                        }
                    } else if let Some(url) = t.url {
                        crate::tools::open_url(&url);
                    }
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
                        self.show_touch(Touch::Down, x, y);
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
                        self.show_touch(Touch::Up, x, y);
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
                    self.zoom(1.0 + (dy as f64 * 0.04));
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
                if self.on_field_key(&event) {
                    return;
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
                } else if cmd && (ch("+") || ch("=")) {
                    self.on_menu(el, "zoom:in");
                } else if cmd && ch("-") {
                    self.on_menu(el, "zoom:out");
                } else if cmd && ch("0") {
                    self.on_menu(el, "zoom:reset");
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

/// The app icon for the title bar and taskbar (macOS takes it from the bundle instead).
fn window_icon() -> Option<winit::window::Icon> {
    static ICON: &[u8] = include_bytes!("../packaging/icon/handset-icon-512.png");
    let mut reader = png::Decoder::new(std::io::Cursor::new(ICON)).read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    winit::window::Icon::from_rgba(buf, info.width, info.height).ok()
}

/// The desktop area not covered by the taskbar, as (x, y, width, height) in pixels.
#[cfg(windows)]
fn work_area() -> Option<(i32, i32, u32, u32)> {
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    unsafe extern "system" {
        fn SystemParametersInfoW(action: u32, param: u32, pv: *mut Rect, flags: u32) -> i32;
    }
    let mut r = Rect::default();
    // SAFETY: SPI_GETWORKAREA fills a RECT
    let ok = unsafe { SystemParametersInfoW(0x30, 0, &mut r, 0) };
    (ok != 0).then(|| (r.left, r.top, (r.right - r.left) as u32, (r.bottom - r.top) as u32))
}

#[cfg(not(windows))]
fn work_area() -> Option<(i32, i32, u32, u32)> {
    None
}
