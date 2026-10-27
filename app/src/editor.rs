//! The style editor (`handset --edit <file>`): a window with a live preview of a capture on its styled
//! background and controls for the background, padding, corner radius, shadow and aspect ratio.
//!
//! It runs as its own process so it never touches the mirroring window's event loop. The preview is a
//! downscaled picture styled by the same code as the export. A recording can be played and scrubbed: a
//! worker thread has ffmpeg decode small frames (silently; the preview has no sound), and only the picture
//! is blended again for each frame, over a backdrop that is drawn once per style change.

use crate::presets::{self, Preset};
use crate::restyle;
use crate::style::{ASPECTS, Aspect, Background, GRADIENTS, Img, Layout, Style, WALLPAPERS};
use crate::trim::{self, Trim};
use eframe::egui;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

const PREVIEW_LONG_SIDE: u32 = 720;
const SWATCH: f32 = 36.0;
/// The controls take a third of the window, within these limits.
const PANEL_MIN: f32 = 320.0;
const PANEL_MAX: f32 = 360.0;
/// The window cannot be made smaller or larger than this, so nothing has to squeeze or stretch.
const MIN_SIZE: [f32; 2] = [860.0, 600.0];
const MAX_SIZE: [f32; 2] = [1360.0, 960.0];
const COLORS: [[u8; 3]; 16] = [
    [24, 24, 27],
    [255, 255, 255],
    [220, 38, 38],
    [234, 108, 24],
    [234, 179, 8],
    [22, 163, 74],
    [37, 99, 235],
    [124, 58, 237],
    [236, 72, 153],
    [82, 82, 91],
    [228, 228, 231],
    [252, 165, 165],
    [253, 186, 116],
    [254, 240, 138],
    [134, 239, 172],
    [147, 197, 253],
];

/// The editor's own icon, so it is not mistaken for the mirroring window in the Dock or taskbar.
const ICON: &[u8] = include_bytes!("../packaging/icon/editor-icon-512.png");

fn window_icon() -> Option<egui::IconData> {
    let (rgba, width, height) = crate::dock::decode_png(ICON)?;
    Some(egui::IconData { rgba, width, height })
}

pub fn run(path: PathBuf, style: Style) -> Result<(), String> {
    let source = Source::load(&path)?;
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Handset Style")
            .with_inner_size([960.0, 700.0])
            .with_min_inner_size(MIN_SIZE)
            .with_max_inner_size(MAX_SIZE)
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false)
            .with_icon(window_icon().unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "Handset Style",
        opts,
        Box::new(move |cc| {
            // the Dock icon is separate from the window icon on macOS
            #[cfg(target_os = "macos")]
            crate::dock::set_image(ICON);
            Ok(Box::new(Editor::new(&cc.egui_ctx, path, source, style)))
        }),
    )
    .map_err(|e| e.to_string())
}

/// What is being edited: the picture, or a recording's first frame plus what is needed to play it.
struct Source {
    first: Img,
    /// Full size of the capture, for the output size shown.
    size: (u32, u32),
    video: Option<restyle::Info>,
    matte: Option<Img>,
}

impl Source {
    fn load(path: &Path) -> Result<Self, String> {
        if !restyle::is_video(path) {
            let first = restyle::read_png(path)?;
            return Ok(Self { size: (first.w, first.h), first, video: None, matte: None });
        }
        let info = restyle::probe(path)?;
        let mut first = restyle::first_frame(path)?;
        // a recording of a framed phone has its outline saved beside it: cut the phone out of its backdrop
        let matte = restyle::load_matte(path, (first.w, first.h));
        if let Some(m) = &matte {
            apply_matte(&mut first, m);
        }
        Ok(Self { size: info.size, first, video: Some(info), matte })
    }
}

fn apply_matte(frame: &mut Img, matte: &Img) {
    for (px, m) in frame.rgba.as_chunks_mut::<4>().0.iter_mut().zip(matte.rgba.as_chunks::<4>().0) {
        px[3] = m[3];
    }
}

fn preview_size(w: u32, h: u32) -> (u32, u32) {
    let long = w.max(h);
    if long <= PREVIEW_LONG_SIDE {
        return (w, h);
    }
    let f = long as f32 / PREVIEW_LONG_SIDE as f32;
    (((w as f32 / f).round() as u32).max(1), ((h as f32 / f).round() as u32).max(1))
}

/// The size a preset thumbnail's picture is drawn at: the picture's shape, its long edge 180 px.
fn thumb_size(w: u32, h: u32) -> (u32, u32) {
    let f = 180.0 / w.max(h).max(1) as f32;
    (((w as f32 * f).round() as u32).max(1), ((h as f32 * f).round() as u32).max(1))
}

fn downscale(src: &Img, (w, h): (u32, u32)) -> Img {
    if (w, h) == (src.w, src.h) {
        return Img { w, h, rgba: src.rgba.clone() };
    }
    let (fx, fy) = (src.w as f32 / w as f32, src.h as f32 / h as f32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            // average the source pixels under this one (premultiplied, so transparent edges do not bleed black)
            let (x0, x1) = ((x as f32 * fx) as u32, (((x + 1) as f32 * fx) as u32).clamp(1, src.w));
            let (y0, y1) = ((y as f32 * fy) as u32, (((y + 1) as f32 * fy) as u32).clamp(1, src.h));
            let (mut acc, mut n) = ([0.0f32; 4], 0.0);
            for sy in y0..y1.max(y0 + 1).min(src.h) {
                for sx in x0..x1.max(x0 + 1).min(src.w) {
                    let p = &src.rgba[((sy * src.w + sx) * 4) as usize..][..4];
                    let a = p[3] as f32 / 255.0;
                    acc[0] += p[0] as f32 * a;
                    acc[1] += p[1] as f32 * a;
                    acc[2] += p[2] as f32 * a;
                    acc[3] += a;
                    n += 1.0;
                }
            }
            let c = |v: f32| if acc[3] > 0.0 { (v / acc[3]).round() as u8 } else { 0 };
            rgba.extend_from_slice(&[c(acc[0]), c(acc[1]), c(acc[2]), (acc[3] / n * 255.0).round() as u8]);
        }
    }
    Img { w, h, rgba }
}

// ---- playback -------------------------------------------------------------------------------------

enum Cmd {
    /// Show the frame at this many seconds.
    Seek(f64),
    /// Play from this many seconds.
    Play(f64),
    Pause,
}

enum Msg {
    Frame(f64, Img),
    Ended,
}

struct Player {
    cmd: Sender<Cmd>,
    msgs: Receiver<Msg>,
}

impl Player {
    fn spawn(
        ctx: egui::Context,
        path: PathBuf,
        size: (u32, u32),
        fps: f32,
        matte: Option<Img>,
        has_audio: bool,
    ) -> Self {
        let (cmd, rx) = channel::<Cmd>();
        let (tx, msgs) = channel::<Msg>();
        std::thread::spawn(move || {
            // the speakers' stream stays on this thread; a recording without sound, or a machine without an
            // output device, is simply silent
            let playback = has_audio.then(|| crate::audio::Playback::new().ok()).flatten();
            let w = Worker { path, size, fps: fps.clamp(1.0, 30.0), matte, tx, ctx, playback };
            while let Ok(mut c) = rx.recv() {
                loop {
                    // only the newest request matters: dragging the scrubber sends many
                    while let Ok(next) = rx.try_recv() {
                        c = next;
                    }
                    match c {
                        Cmd::Pause => break,
                        Cmd::Seek(t) => {
                            w.still(t);
                            break;
                        }
                        Cmd::Play(t) => match w.play(t, &rx) {
                            Some(next) => c = next,
                            None => break,
                        },
                    }
                }
            }
        });
        Self { cmd, msgs }
    }
}

struct Worker {
    path: PathBuf,
    size: (u32, u32),
    fps: f32,
    matte: Option<Img>,
    tx: Sender<Msg>,
    ctx: egui::Context,
    playback: Option<crate::audio::Playback>,
}

impl Worker {
    fn ffmpeg(&self, t: f64) -> std::process::Command {
        let mut cmd = crate::tools::command(crate::tools::find("ffmpeg"));
        cmd.args(["-hide_banner", "-loglevel", "error", "-ss", &format!("{t:.3}"), "-i"]).arg(&self.path);
        cmd
    }

    fn finish(&self, rgba: Vec<u8>) -> Img {
        let mut img = Img { w: self.size.0, h: self.size.1, rgba };
        if let Some(m) = &self.matte {
            apply_matte(&mut img, m);
        }
        img
    }

    fn frame_len(&self) -> usize {
        (self.size.0 * self.size.1 * 4) as usize
    }

    fn send(&self, t: f64, img: Img) {
        let _ = self.tx.send(Msg::Frame(t, img));
        self.ctx.request_repaint();
    }

    fn still(&self, t: f64) {
        let scale = format!("scale={}:{}", self.size.0, self.size.1);
        let out = self
            .ffmpeg(t)
            .args(["-frames:v", "1", "-vf", &scale, "-f", "rawvideo", "-pix_fmt", "rgba", "-an", "-"])
            .stderr(Stdio::null())
            .output();
        if let Ok(o) = out
            && o.stdout.len() == self.frame_len()
        {
            self.send(t, self.finish(o.stdout));
        }
    }

    /// Stream frames at their real pace until the video ends (`None`) or another request comes in.
    fn play(&self, from: f64, rx: &Receiver<Cmd>) -> Option<Cmd> {
        let vf = format!("fps={},scale={}:{}", self.fps, self.size.0, self.size.1);
        let mut child = self
            .ffmpeg(from)
            .args(["-vf", &vf, "-f", "rawvideo", "-pix_fmt", "rgba", "-an", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut out = child.stdout.take()?;
        let start = Instant::now();
        let feeder =
            self.playback.as_ref().and_then(|p| crate::audio_preview::Feeder::start(&self.path, from, start, p.ring()));
        let mut n = 0u32;
        let result = loop {
            let mut buf = vec![0u8; self.frame_len()];
            if out.read_exact(&mut buf).is_err() {
                let _ = self.tx.send(Msg::Ended);
                self.ctx.request_repaint();
                break None;
            }
            let due = Duration::from_secs_f64(n as f64 / self.fps as f64);
            if let Some(wait) = due.checked_sub(start.elapsed()) {
                std::thread::sleep(wait);
            }
            if let Ok(next) = rx.try_recv() {
                break Some(next);
            }
            self.send(from + n as f64 / self.fps as f64, self.finish(buf));
            n += 1;
        };
        if let Some(f) = feeder {
            f.stop();
        }
        let _ = child.kill();
        let _ = child.wait();
        result
    }
}

// ---- look -----------------------------------------------------------------------------------------

mod theme {
    //! Device Gray: the greys of the app icon, one set for each OS theme. The dark set is near black, like the connect screen. `set_dark` is called every frame
    //! from the theme egui reports, so the palette follows the system setting.
    use eframe::egui::Color32;
    use std::sync::atomic::{AtomicBool, Ordering};

    static DARK: AtomicBool = AtomicBool::new(true);

    pub fn set_dark(dark: bool) {
        DARK.store(dark, Ordering::Relaxed);
    }

    pub fn dark() -> bool {
        DARK.load(Ordering::Relaxed)
    }

    const fn rgb(v: u32) -> Color32 {
        Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    fn pick(dark_value: u32, light_value: u32) -> Color32 {
        rgb(if dark() { dark_value } else { light_value })
    }

    pub fn bg() -> Color32 {
        pick(0x1a1a1a, 0xe3e5e9)
    }
    pub fn panel() -> Color32 {
        pick(0x232325, 0xf2f3f5)
    }
    pub fn card() -> Color32 {
        pick(0x2d2d2f, 0xffffff)
    }
    pub fn card_hi() -> Color32 {
        pick(0x38383c, 0xe7e8eb)
    }
    pub fn line() -> Color32 {
        pick(0x3f3f44, 0xd5d8dd)
    }
    pub fn text() -> Color32 {
        pick(0xeceef0, 0x1f2024)
    }
    pub fn weak() -> Color32 {
        pick(0x9b9ea6, 0x6b6e75)
    }
    /// The one accent: near-white on dark, graphite on light.
    pub fn accent() -> Color32 {
        pick(0xe7e8eb, 0x2e3036)
    }
    pub fn accent_hover() -> Color32 {
        pick(0xffffff, 0x1f2024)
    }
    pub fn on_accent() -> Color32 {
        pick(0x1f2024, 0xffffff)
    }
    pub fn accent_dim() -> Color32 {
        pick(0x4e4e53, 0xcdd0d6)
    }
    pub fn hover() -> Color32 {
        pick(0x444449, 0xd5d8dd)
    }
    pub fn pressed() -> Color32 {
        pick(0x4e4e54, 0xc5c8cf)
    }
    /// The part of a recording that is kept, brighter than the track.
    pub fn kept() -> Color32 {
        pick(0x4e4e53, 0xb9bcc2)
    }
    /// The playhead knob.
    pub fn knob() -> Color32 {
        pick(0xffffff, 0x2e3036)
    }
    pub fn error() -> Color32 {
        pick(0xff8a80, 0xc5221a)
    }
    pub fn checker() -> (Color32, Color32) {
        if dark() {
            (Color32::from_gray(40), Color32::from_gray(54))
        } else {
            (Color32::from_gray(222), Color32::from_gray(238))
        }
    }
}

fn semibold() -> egui::FontFamily {
    egui::FontFamily::Name("semibold".into())
}

fn setup_style(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "inter".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-Regular.ttf"))),
    );
    fonts.font_data.insert(
        "inter-semibold".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../assets/fonts/Inter-SemiBold.ttf"))),
    );
    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "inter".into());
    fonts.families.insert(semibold(), vec!["inter-semibold".into()]);
    ctx.set_fonts(fonts);

    for dark in [true, false] {
        theme::set_dark(dark);
        let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.panel_fill = theme::panel();
        v.window_fill = theme::card();
        v.extreme_bg_color = theme::bg();
        v.faint_bg_color = theme::card();
        v.override_text_color = Some(theme::text());
        v.selection.bg_fill = theme::accent();
        v.selection.stroke = egui::Stroke::new(1.0, theme::on_accent());
        v.hyperlink_color = theme::accent();
        v.handle_shape = egui::style::HandleShape::Circle;
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, theme::line());
        let radius = egui::CornerRadius::same(9);
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = radius;
            w.bg_stroke = egui::Stroke::NONE;
        }
        v.widgets.inactive.weak_bg_fill = theme::card_hi();
        v.widgets.inactive.bg_fill = theme::card_hi();
        v.widgets.hovered.weak_bg_fill = theme::hover();
        v.widgets.hovered.bg_fill = theme::hover();
        v.widgets.active.weak_bg_fill = theme::pressed();
        v.widgets.active.bg_fill = theme::pressed();
        ctx.set_visuals_of(if dark { egui::Theme::Dark } else { egui::Theme::Light }, v);
    }
    // follow the operating system's light or dark setting
    ctx.set_theme(egui::ThemePreference::System);
    theme::set_dark(ctx.theme() == egui::Theme::Dark);

    ctx.all_styles_mut(|s| {
        use egui::{FontFamily::Proportional, FontId, TextStyle};
        s.text_styles.insert(TextStyle::Body, FontId::new(13.5, Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(13.5, semibold()));
        s.text_styles.insert(TextStyle::Small, FontId::new(11.5, Proportional));
        s.text_styles.insert(TextStyle::Heading, FontId::new(16.0, semibold()));
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.slider_rail_height = 4.0;
        // text never wraps to a second line: it is cut short instead
        s.wrap_mode = Some(egui::TextWrapMode::Truncate);
    });
}

fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(11.0).family(semibold()).color(theme::weak()));
    ui.add_space(2.0);
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(theme::card()).corner_radius(12).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

/// A row of options in one rounded track; returns the index clicked.
fn segmented(ui: &mut egui::Ui, id: &str, labels: &[&str], selected: Option<usize>) -> Option<usize> {
    let font = egui::FontId::new(12.0, semibold());
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| ui.painter().layout_no_wrap((*l).to_string(), font.clone(), theme::text()).size().x + 8.0)
        .collect();
    let total = ui.available_width();
    let extra = ((total - 4.0 - widths.iter().sum::<f32>()) / labels.len() as f32).max(0.0);
    let (track, _) = ui.allocate_exact_size(egui::vec2(total, 34.0), egui::Sense::hover());
    ui.painter().rect_filled(track, 10.0, theme::bg());
    let mut x = track.min.x + 2.0;
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let w = widths[i] + extra;
        let seg = egui::Rect::from_min_size(egui::pos2(x, track.min.y + 2.0), egui::vec2(w, 30.0));
        let resp = ui
            .interact(seg, ui.make_persistent_id((id, i)), egui::Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let on = selected == Some(i);
        if on {
            ui.painter().rect_filled(seg, 8.0, theme::accent_dim());
        } else if resp.hovered() {
            ui.painter().rect_filled(seg, 8.0, theme::card());
        }
        let color = if on { theme::accent() } else { theme::weak() };
        ui.painter().text(seg.center(), egui::Align2::CENTER_CENTER, *label, font.clone(), color);
        if resp.clicked() {
            clicked = Some(i);
        }
        x += w;
    }
    clicked
}

fn chip(ui: &mut egui::Ui, label: &str, size: egui::Vec2, on: bool, off: egui::Color32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let fill = if on {
        theme::accent_dim()
    } else if resp.hovered() {
        theme::card_hi()
    } else {
        off
    };
    ui.painter().rect_filled(rect, 9.0, fill);
    let color = if on { theme::accent() } else { theme::text() };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::new(11.5, semibold()), color);
    resp.clicked()
}

/// Swatches in rows of exactly `cols`, filling the width; `ratio` is height over width.
fn grid(
    ui: &mut egui::Ui,
    cols: usize,
    ratio: f32,
    count: usize,
    mut cell: impl FnMut(&mut egui::Ui, usize, egui::Vec2),
) {
    let gap = 8.0;
    let w = ((ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32).floor();
    let size = egui::vec2(w, (w * ratio).round());
    for row in (0..count).step_by(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for i in row..(row + cols).min(count) {
                cell(ui, i, size);
            }
        });
    }
}

fn slider_row(ui: &mut egui::Ui, label: &str, value: &mut u32, max: u32, unit: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(98.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(98.0);
            ui.label(egui::RichText::new(label).color(theme::weak()));
        });
        ui.spacing_mut().slider_width = (ui.available_width() - 52.0).max(40.0);
        changed = ui.add(egui::Slider::new(value, 0..=max).show_value(false).trailing_fill(true)).changed();
        let text = egui::RichText::new(format!("{value}{unit}")).family(semibold());
        ui.add_sized([36.0, 24.0], egui::Label::new(text));
    });
    changed
}

/// What the person did to the scrubber.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Scrub {
    /// Moved the playhead.
    Seek(f64),
    /// Dragged the start of the kept part.
    Start(f64),
    /// Dragged the end of the kept part.
    End(f64),
}

/// A slim track with a playhead and, at the ends of the kept part, two handles to trim by.
fn scrubber(ui: &mut egui::Ui, pos: f64, duration: f64, trim: Trim, width: f32) -> Option<Scrub> {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 30.0), egui::Sense::click_and_drag());
    let track = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width() - 20.0, 4.0));
    let x = |t: f64| trim::x_at(t, track.left(), track.width(), duration);
    let (x0, x1, xp) = (x(trim.start), x(trim.end), x(pos));
    let mut result = None;

    // the two handles sit over the track, so they take the pointer first
    let handle = |ui: &mut egui::Ui, id: &str, cx: f32| {
        let area = egui::Rect::from_center_size(egui::pos2(cx, rect.center().y), egui::vec2(18.0, rect.height()));
        ui.interact(area, ui.make_persistent_id(id), egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
    };
    let start = handle(ui, "trim-start", x0);
    let end = handle(ui, "trim-end", x1);
    let pointer = |r: &egui::Response| {
        r.interact_pointer_pos().map(|p| trim::time_at(p.x, track.left(), track.width(), duration))
    };
    if start.dragged() {
        result = pointer(&start).map(Scrub::Start);
    } else if end.dragged() {
        result = pointer(&end).map(Scrub::End);
    } else if (resp.dragged() || resp.clicked())
        && !start.is_pointer_button_down_on()
        && !end.is_pointer_button_down_on()
    {
        result = pointer(&resp).map(Scrub::Seek);
    }

    let p = ui.painter();
    let dim = theme::line();
    p.rect_filled(track, 2.0, dim);
    // the kept part is brighter than what will be cut away
    let kept = egui::Rect::from_min_max(egui::pos2(x0, track.min.y - 1.0), egui::pos2(x1, track.max.y + 1.0));
    p.rect_filled(kept, 2.0, theme::kept());
    p.rect_filled(
        egui::Rect::from_min_max(egui::pos2(x0, track.min.y), egui::pos2(xp.max(x0), track.max.y)),
        2.0,
        theme::accent(),
    );
    for (cx, edge) in [(x0, -1.0), (x1, 1.0)] {
        let bar = egui::Rect::from_center_size(egui::pos2(cx + edge * 2.0, rect.center().y), egui::vec2(8.0, 22.0));
        p.rect_filled(bar, 3.0, theme::accent());
        p.rect_filled(egui::Rect::from_center_size(bar.center(), egui::vec2(2.0, 10.0)), 1.0, theme::on_accent());
    }
    let r = if resp.hovered() || resp.dragged() { 8.0 } else { 6.5 };
    p.circle_filled(egui::pos2(xp, track.center().y), r, theme::knob());
    result
}

fn play_button(ui: &mut egui::Ui, playing: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let fill = if resp.hovered() { theme::accent_hover() } else { theme::accent() };
    let c = rect.center();
    let p = ui.painter();
    p.circle_filled(c, 18.0, fill);
    if playing {
        for dx in [-3.6, 3.6] {
            p.rect_filled(
                egui::Rect::from_center_size(c + egui::vec2(dx, 0.0), egui::vec2(3.4, 12.0)),
                1.0,
                theme::on_accent(),
            );
        }
    } else {
        let tri = vec![c + egui::vec2(-3.5, -7.0), c + egui::vec2(-3.5, 7.0), c + egui::vec2(7.0, 0.0)];
        p.add(egui::Shape::convex_polygon(tri, theme::on_accent(), egui::Stroke::NONE));
    }
    resp.clicked()
}

// ---- the window -----------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    None,
    Gradient,
    Wallpaper,
    Color,
    Image,
}

fn tab_of(bg: &Background) -> Tab {
    match bg {
        Background::None => Tab::None,
        Background::Color(_) => Tab::Color,
        Background::Gradient(_) => Tab::Gradient,
        Background::Wallpaper(_) => Tab::Wallpaper,
        Background::Image(_) => Tab::Image,
    }
}

const TABS: [(Tab, &str); 5] = [
    (Tab::None, "None"),
    (Tab::Gradient, "Gradient"),
    (Tab::Wallpaper, "Wallpaper"),
    (Tab::Color, "Color"),
    (Tab::Image, "Image"),
];

/// Height of the strip along the top: the window buttons on macOS (which hides the title bar), the file name
/// everywhere, and a place to drag the window.
const TOP: f32 = 34.0;

struct Editor {
    path: PathBuf,
    /// Drawn along the top of the window.
    name: String,
    full_size: (u32, u32),
    style: Style,
    tab: Tab,
    /// The recording has its phone outline saved, so the corner radius does not apply.
    matte: bool,
    /// The picture (or video frame) being styled, at preview size.
    src: Img,
    /// The picture chosen as background, decoded once, with the path it came from.
    image: Option<(PathBuf, Img)>,
    custom: [u8; 3],
    /// Background and shadow at preview size, redrawn only when the style changes.
    backdrop: Option<(Vec<u8>, Layout)>,
    texture: Option<egui::TextureHandle>,
    style_dirty: bool,
    frame_dirty: bool,
    swatches: Vec<(Background, egui::TextureHandle)>,
    /// The presets you saved, the name being typed for the next one, and whether that field is open.
    saved_presets: Vec<Preset>,
    preset_name: String,
    naming: bool,
    /// The picture at thumbnail size, and each preset drawn over it, keyed by the preset's style.
    thumb_src: Img,
    thumbs: std::collections::HashMap<String, egui::TextureHandle>,
    export: Option<Export>,
    /// Earlier looks, for undo, and the look as of the last frame.
    history: Vec<Style>,
    snapshot: Style,
    last_push: Instant,
    status: Option<Result<PathBuf, String>>,
    video: Option<VideoState>,
}

/// An export in progress: its result when it finishes, and its progress and cancel switch.
type Export = (Receiver<Result<PathBuf, String>>, std::sync::Arc<restyle::Control>);

struct VideoState {
    player: Player,
    duration: f64,
    pos: f64,
    playing: bool,
    looping: bool,
    /// The part of the recording that is played and exported.
    trim: Trim,
    /// The recording has sound, which plays in the preview unless muted.
    has_audio: bool,
}

impl Editor {
    fn new(ctx: &egui::Context, path: PathBuf, source: Source, style: Style) -> Self {
        setup_style(ctx);
        let mut swatches = Vec::new();
        let thumb = Style { padding: 0, radius: 0, shadow: 0, aspect: Aspect::Auto, ..Style::default() };
        let l = thumb.layout((SWATCH as u32 * 2, SWATCH as u32 * 2));
        let mut add = |bg: Background, name: String| {
            let s = Style { background: bg.clone(), ..thumb.clone() };
            let px = s.backdrop(&l, None, None);
            let tex = ctx.load_texture(
                name,
                egui::ColorImage::from_rgba_unmultiplied([l.out.0 as usize, l.out.1 as usize], &px),
                egui::TextureOptions::LINEAR,
            );
            swatches.push((bg, tex));
        };
        for i in 0..GRADIENTS.len() {
            add(Background::Gradient(i), format!("g{i}"));
        }
        for i in 0..WALLPAPERS.len() {
            add(Background::Wallpaper(i), format!("w{i}"));
        }

        let pv = preview_size(source.first.w, source.first.h);
        let video = source.video.as_ref().map(|info| {
            let matte = source.matte.as_ref().map(|m| downscale(m, pv));
            VideoState {
                player: Player::spawn(ctx.clone(), path.clone(), pv, info.fps, matte, info.has_audio),
                has_audio: info.has_audio,
                duration: info.duration.max(0.1),
                pos: 0.0,
                playing: false,
                looping: false,
                trim: Trim::full(info.duration.max(0.1)),
            }
        });
        let tab = tab_of(&style.background);
        let mut ed = Self {
            name: path.file_name().map_or(String::new(), |n| n.to_string_lossy().into()),
            full_size: source.size,
            matte: source.matte.is_some(),
            src: downscale(&source.first, pv),
            path,
            custom: match style.background {
                Background::Color(c) => c,
                _ => [40, 40, 60],
            },
            snapshot: style.clone(),
            style,
            tab,
            image: None,
            backdrop: None,
            texture: None,
            style_dirty: true,
            frame_dirty: true,
            swatches,
            saved_presets: presets::load(),
            preset_name: String::new(),
            naming: false,
            thumb_src: downscale(&source.first, thumb_size(source.first.w, source.first.h)),
            thumbs: std::collections::HashMap::new(),
            export: None,
            history: Vec::new(),
            last_push: Instant::now(),
            status: None,
            video,
        };
        ed.ensure_image();
        ed
    }

    /// Decode the chosen background picture if it is not already.
    fn ensure_image(&mut self) {
        if let Background::Image(p) = &self.style.background
            && self.image.as_ref().is_none_or(|(have, _)| have != p)
        {
            match restyle::load_image(p) {
                Ok(img) => {
                    let long = img.w.max(img.h);
                    let f = (long as f32 / 1600.0).max(1.0);
                    let size = (((img.w as f32 / f) as u32).max(1), ((img.h as f32 / f) as u32).max(1));
                    self.image = Some((p.clone(), downscale(&img, size)));
                }
                Err(e) => {
                    self.status = Some(Err(e));
                    self.style.background = Background::Gradient(0);
                }
            }
        }
    }

    fn is_video(&self) -> bool {
        self.video.is_some()
    }

    /// The style as it is drawn in the preview.
    fn effective_style(&self) -> Style {
        let mut style = if self.is_video() { restyle::video_style(&self.style) } else { self.style.clone() };
        if self.matte {
            style.radius = 0;
        }
        style
    }

    fn render_preview(&mut self, ctx: &egui::Context) {
        let style = self.effective_style();
        if self.style_dirty || self.backdrop.is_none() {
            let l = style.layout((self.src.w, self.src.h));
            let bd = style.backdrop(&l, Some(&self.src), self.image.as_ref().map(|(_, i)| i));
            self.backdrop = Some((bd, l));
        }
        let (bd, l) = self.backdrop.as_ref().expect("backdrop just drawn");
        let out = style.compose_over(bd, l, &self.src);
        let img = egui::ColorImage::from_rgba_unmultiplied([out.w as usize, out.h as usize], &out.rgba);
        match &mut self.texture {
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => self.texture = Some(ctx.load_texture("preview", img, egui::TextureOptions::LINEAR)),
        }
        self.style_dirty = false;
        self.frame_dirty = false;
    }

    fn start_export(&mut self, ctx: &egui::Context) {
        crate::settings::save("style", &self.style.to_spec());
        let (tx, rx) = channel();
        let control = std::sync::Arc::new(restyle::Control::default());
        let (path, style, ctx, job) = (self.path.clone(), self.style.clone(), ctx.clone(), control.clone());
        let trim = self.video.as_ref().filter(|v| !v.trim.is_full(v.duration)).map(|v| v.trim);
        std::thread::spawn(move || {
            let _ = tx.send(restyle::export_with(&path, &style, trim, &job));
            ctx.request_repaint();
        });
        self.export = Some((rx, control));
        self.status = None;
    }

    fn toggle_play(&mut self) {
        let Some(v) = self.video.as_mut() else { return };
        if v.playing {
            let _ = v.player.cmd.send(Cmd::Pause);
            v.playing = false;
        } else {
            // from the start of the kept part when the playhead is outside it or at its end
            if v.pos < v.trim.start || v.pos >= v.trim.end - 0.1 {
                v.pos = v.trim.start;
            }
            let _ = v.player.cmd.send(Cmd::Play(v.pos));
            v.playing = true;
        }
    }

    /// Take what the player has produced since the last frame of the window.
    fn poll_player(&mut self) {
        let Some(v) = self.video.as_mut() else { return };
        let mut newest = None;
        while let Ok(m) = v.player.msgs.try_recv() {
            match m {
                Msg::Frame(t, img) => newest = Some((t, img)),
                Msg::Ended if v.looping => {
                    v.pos = v.trim.start;
                    let _ = v.player.cmd.send(Cmd::Play(v.trim.start));
                }
                Msg::Ended => {
                    v.playing = false;
                    v.pos = v.trim.end;
                }
            }
        }
        if let Some((t, img)) = newest {
            if v.playing {
                v.pos = t.min(v.duration);
                // the kept part ends here, even if the recording goes on
                if v.pos >= v.trim.end {
                    if v.looping {
                        v.pos = v.trim.start;
                        let _ = v.player.cmd.send(Cmd::Play(v.trim.start));
                    } else {
                        v.playing = false;
                        v.pos = v.trim.end;
                        let _ = v.player.cmd.send(Cmd::Pause);
                    }
                }
            }
            self.src = img;
            self.frame_dirty = true;
        }
    }

    fn swatch(
        ui: &mut egui::Ui,
        size: egui::Vec2,
        selected: bool,
        paint: impl FnOnce(&egui::Painter, egui::Rect),
    ) -> bool {
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
        paint(ui.painter(), rect);
        if selected {
            ui.painter().rect_stroke(
                rect.expand(3.0),
                11.0,
                egui::Stroke::new(2.0, theme::accent()),
                egui::StrokeKind::Outside,
            );
        } else if resp.hovered() {
            ui.painter().rect_stroke(rect, 9.0, egui::Stroke::new(1.5, theme::accent()), egui::StrokeKind::Inside);
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
    }

    fn texture_grid(&mut self, ui: &mut egui::Ui, range: std::ops::Range<usize>, cols: usize, ratio: f32) -> bool {
        let items: Vec<usize> = range.collect();
        let mut changed = false;
        grid(ui, cols, ratio, items.len(), |ui, k, size| {
            let i = items[k];
            let (bg, tex) = (self.swatches[i].0.clone(), self.swatches[i].1.clone());
            let selected = self.style.background == bg;
            if Self::swatch(ui, size, selected, |p, r| {
                p.add(
                    egui::epaint::RectShape::filled(r, 9.0, egui::Color32::WHITE)
                        .with_texture(tex.id(), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))),
                );
            }) {
                self.style.background = bg;
                changed = true;
            }
        });
        changed
    }

    fn background_card(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        card(ui, |ui| {
            section_title(ui, "BACKGROUND");
            let labels: Vec<&str> = TABS.iter().map(|t| t.1).collect();
            let current = TABS.iter().position(|t| t.0 == self.tab);
            if let Some(i) = segmented(ui, "tabs", &labels, current) {
                self.tab = TABS[i].0;
                if self.tab == Tab::None {
                    self.style.background = Background::None;
                    changed = true;
                }
            }
            ui.add_space(4.0);
            let grad = GRADIENTS.len();
            match self.tab {
                Tab::None => {
                    let text = if self.is_video() { "Recordings use a dark color." } else { "Transparent background." };
                    ui.label(egui::RichText::new(text).color(theme::weak()));
                }
                Tab::Gradient => changed |= self.texture_grid(ui, 0..grad, 5, 1.0),
                Tab::Wallpaper => changed |= self.texture_grid(ui, grad..self.swatches.len(), 3, 0.62),
                Tab::Color => {
                    grid(ui, 8, 1.0, COLORS.len(), |ui, i, size| {
                        let c = COLORS[i];
                        let selected = self.style.background == Background::Color(c);
                        if Self::swatch(ui, size, selected, |p, r| {
                            p.rect_filled(r, 8.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                        }) {
                            self.style.background = Background::Color(c);
                            self.custom = c;
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Custom").color(theme::weak()));
                        if egui::color_picker::color_edit_button_srgb(ui, &mut self.custom).changed() {
                            self.style.background = Background::Color(self.custom);
                            changed = true;
                        }
                    });
                }
                Tab::Image => {
                    let wide =
                        egui::Button::new("Choose a picture...").min_size(egui::vec2(ui.available_width(), 32.0));
                    if ui.add(wide).clicked()
                        && let Some(file) = rfd::FileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "heic", "bmp", "gif", "tiff"])
                            .pick_file()
                    {
                        self.style.background = Background::Image(file);
                        changed = true;
                    }
                    if let Background::Image(p) = &self.style.background {
                        let name: String = p.file_name().map_or(String::new(), |n| n.to_string_lossy().into());
                        ui.label(egui::RichText::new(name).color(theme::weak()));
                    }
                }
            }
        });
        changed
    }

    /// Switch to a saved look. The change is picked up (and made undoable) with any other change to the style.
    fn apply_preset(&mut self, style: &Style) {
        self.style = style.clone();
        self.tab = tab_of(&self.style.background);
        if let Background::Color(c) = self.style.background {
            self.custom = c;
        }
        self.ensure_image();
    }

    /// Keep the current style under `raw_name`; false when the name is empty.
    fn save_preset(&mut self, raw_name: &str) -> bool {
        let Some(name) = presets::clean_name(raw_name) else { return false };
        presets::upsert(&mut self.saved_presets, Preset { name, style: self.style.clone() });
        true
    }

    fn forget_preset(&mut self, name: &str) -> bool {
        presets::remove(&mut self.saved_presets, name)
    }

    /// The presets (the built-in ones, then yours) as thumbnails of this picture, with a tile to save the current
    /// style. Returns whether the style changed.
    fn presets_card(&mut self, ui: &mut egui::Ui) -> bool {
        let all: Vec<(Preset, bool)> = self
            .saved_presets
            .iter()
            .map(|p| (p.clone(), true))
            .chain(
                presets::builtin()
                    .into_iter()
                    .filter(|b| !self.saved_presets.iter().any(|s| s.name.eq_ignore_ascii_case(&b.name)))
                    .map(|p| (p, false)),
            )
            .collect();
        for (preset, _) in &all {
            let key = preset.style.to_spec();
            if !self.thumbs.contains_key(&key) {
                let l = preset.style.layout((self.thumb_src.w, self.thumb_src.h));
                let bd = preset.style.backdrop(&l, None, None);
                let out = preset.style.compose_over(&bd, &l, &self.thumb_src);
                let img = egui::ColorImage::from_rgba_unmultiplied([out.w as usize, out.h as usize], &out.rgba);
                let tex = ui.ctx().load_texture(format!("preset-{key}"), img, egui::TextureOptions::LINEAR);
                self.thumbs.insert(key, tex);
            }
        }
        let mut apply = None;
        let mut delete = None;
        let mut reset = false;
        let mut open_naming = false;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                section_title(ui, "PRESETS");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .link(egui::RichText::new("Reset").size(12.0))
                        .on_hover_text("Back to the default style")
                        .clicked()
                    {
                        reset = true;
                    }
                });
            });
            // the save tile comes first so it stays in reach, and a long list scrolls inside the card instead of
            // pushing the other controls away; a sliver of the third row shows that there is more
            let tiles = all.len() + 1;
            let cell = ((ui.available_width() - 16.0) / 3.0).floor();
            let rows_shown = 2.4;
            let max_height = (cell * 0.95).round() * rows_shown + 8.0 * (rows_shown - 1.0);
            let scroll = tiles > 6;
            egui::ScrollArea::vertical()
                .id_salt("presets")
                .max_height(if scroll { max_height } else { f32::INFINITY })
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    grid(ui, 3, 0.95, tiles, |ui, k, size| {
                        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
                        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                        let art = egui::Rect::from_min_size(rect.min, egui::vec2(size.x, size.y - 20.0));
                        let p = ui.painter();
                        p.rect_filled(art, 9.0, theme::card_hi());
                        let label_pos = egui::pos2(rect.center().x, art.max.y + 11.0);
                        let font = egui::FontId::new(11.0, egui::FontFamily::Proportional);
                        let mut name = String::from("Save current");
                        if let Some((preset, mine)) = k.checked_sub(1).and_then(|i| all.get(i)) {
                            name = preset.name.clone();
                            if let Some(tex) = self.thumbs.get(&preset.style.to_spec()) {
                                let t = tex.size_vec2();
                                let room = art.shrink(5.0);
                                let scale = (room.width() / t.x).min(room.height() / t.y);
                                let img = egui::Rect::from_center_size(art.center(), t * scale);
                                p.image(
                                    tex.id(),
                                    img,
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                            }
                            if *mine {
                                p.circle_filled(art.right_top() + egui::vec2(-8.0, 8.0), 3.0, theme::accent());
                            }
                            if preset.style == self.style {
                                p.rect_stroke(
                                    art.expand(2.0),
                                    11.0,
                                    egui::Stroke::new(2.0, theme::accent()),
                                    egui::StrokeKind::Outside,
                                );
                            }
                            if resp.clicked() {
                                apply = Some(preset.style.clone());
                            }
                            if *mine {
                                resp.clone().context_menu(|ui| {
                                    if ui.button("Delete preset").clicked() {
                                        delete = Some(preset.name.clone());
                                        ui.close();
                                    }
                                });
                            }
                        } else {
                            // the last tile: a plus that opens the name field
                            let c = art.center();
                            let stroke = egui::Stroke::new(2.0, theme::weak());
                            p.line_segment([c + egui::vec2(-8.0, 0.0), c + egui::vec2(8.0, 0.0)], stroke);
                            p.line_segment([c + egui::vec2(0.0, -8.0), c + egui::vec2(0.0, 8.0)], stroke);
                            if resp.clicked() {
                                open_naming = true;
                            }
                        }
                        if resp.hovered() {
                            p.rect_stroke(art, 9.0, egui::Stroke::new(1.5, theme::weak()), egui::StrokeKind::Inside);
                        }
                        let mut job = egui::text::LayoutJob::simple_singleline(name, font, theme::text());
                        job.wrap = egui::text::TextWrapping {
                            max_width: size.x,
                            max_rows: 1,
                            break_anywhere: true,
                            overflow_character: Some('…'),
                        };
                        let galley = ui.painter().layout_job(job);
                        let at = label_pos - egui::vec2(galley.size().x / 2.0, galley.size().y / 2.0);
                        ui.painter().galley(at, galley, theme::text());
                    });
                });
            if self.naming {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let w = ui.available_width() - 130.0;
                    let edit = egui::TextEdit::singleline(&mut self.preset_name)
                        .hint_text("Name this preset")
                        .desired_width(w)
                        .char_limit(32);
                    let resp = ui.add(edit);
                    if open_naming {
                        resp.request_focus();
                    }
                    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let name = presets::clean_name(&self.preset_name);
                    let save = ui.add_enabled(name.is_some(), egui::Button::new("Save")).clicked();
                    if name.is_some() && (save || enter) && self.save_preset(&self.preset_name.clone()) {
                        presets::save(&self.saved_presets);
                        self.preset_name.clear();
                        self.naming = false;
                    }
                    if ui.link("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.preset_name.clear();
                        self.naming = false;
                    }
                });
            }
        });
        if open_naming {
            self.naming = true;
        }
        if let Some(name) = delete
            && self.forget_preset(&name)
        {
            presets::save(&self.saved_presets);
        }
        if reset {
            self.style = Style::default();
            self.tab = Tab::Gradient;
            return true;
        }
        if let Some(style) = apply {
            self.apply_preset(&style);
            return true;
        }
        false
    }

    fn layout_card(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        card(ui, |ui| {
            section_title(ui, "LAYOUT");
            changed |= slider_row(ui, "Padding", &mut self.style.padding, crate::style::MAX_PADDING, "%");
            if !self.matte {
                changed |= slider_row(ui, "Corner radius", &mut self.style.radius, crate::style::MAX_RADIUS, "%");
            }
            changed |= slider_row(ui, "Shadow", &mut self.style.shadow, 100, "");
        });
        card(ui, |ui| {
            section_title(ui, "ASPECT RATIO");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let n = ASPECTS.len() as f32;
                let w = ((ui.available_width() - 4.0 * (n - 1.0)) / n).floor();
                for a in ASPECTS {
                    if chip(ui, &a.label(), egui::vec2(w, 30.0), self.style.aspect == a, theme::bg()) {
                        self.style.aspect = a;
                        changed = true;
                    }
                }
            });
        });
        changed
    }

    fn export_area(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let l = self.effective_style().layout(self.full_size);
        let kind = match &self.video {
            Some(v) if !v.trim.is_full(v.duration) => format!("video, {}", clock(v.trim.len())),
            Some(_) => "video".to_string(),
            None => "picture".to_string(),
        };
        ui.label(egui::RichText::new(format!("{} x {} px {kind}", l.out.0, l.out.1)).size(11.5).color(theme::weak()));
        let busy = self.export.is_some();
        let label = if busy { "Exporting..." } else { "Export" };
        let button =
            egui::Button::new(egui::RichText::new(label).size(14.5).family(semibold()).color(theme::on_accent()))
                .fill(theme::accent())
                .corner_radius(11)
                .min_size(egui::vec2(ui.available_width(), 40.0));
        if ui.add_enabled(!busy, button).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            self.start_export(ctx);
        }
        if let Some((_, control)) = &self.export {
            ui.horizontal(|ui| {
                let p = control.progress.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
                if self.is_video() {
                    ui.add(egui::ProgressBar::new(p).desired_width(ui.available_width() - 64.0).fill(theme::accent()));
                } else {
                    ui.add(egui::Spinner::new().size(14.0));
                    ui.label(egui::RichText::new("Rendering").color(theme::weak()));
                }
                if ui.link("Cancel").clicked() {
                    control.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            });
        }
        match &self.status {
            Some(Ok(p)) => {
                let name: String = p.file_name().map_or(String::new(), |n| n.to_string_lossy().into());
                egui::Frame::new()
                    .fill(theme::accent_dim())
                    .corner_radius(10)
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new("Saved").family(semibold()).color(theme::accent()));
                        ui.label(egui::RichText::new(name).size(12.0));
                        ui.horizontal(|ui| {
                            if ui.link("Show in folder").clicked() {
                                crate::capture::reveal(p);
                            }
                            if ui.link("Open").clicked() {
                                crate::capture::open_file(p);
                            }
                        });
                    });
            }
            Some(Err(e)) => {
                ui.colored_label(theme::error(), e);
            }
            None => {}
        }
    }

    fn transport(&mut self, ui: &mut egui::Ui) {
        let Some(v) = self.video.as_mut() else { return };
        let mut scrub = None;
        let mut toggle = false;
        let mut reset = false;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            toggle = play_button(ui, v.playing);
            let label = format!("{} / {}", clock(v.pos - v.trim.start), clock(v.trim.len()));
            let w = (ui.available_width() - 92.0).max(80.0) - 60.0 - if v.has_audio { 68.0 } else { 0.0 };
            scrub = scrubber(ui, v.pos, v.duration, v.trim, w);
            if chip(ui, "Loop", egui::vec2(48.0, 26.0), v.looping, theme::card()) {
                v.looping = !v.looping;
            }
            if v.has_audio && chip(ui, "Sound", egui::vec2(56.0, 26.0), !crate::audio::is_muted(), theme::card()) {
                crate::audio::set_muted(!crate::audio::is_muted());
            }
            ui.label(egui::RichText::new(label).size(12.0).color(theme::weak()));
        });
        if !v.trim.is_full(v.duration) {
            ui.horizontal(|ui| {
                ui.add_space(48.0);
                let kept = format!("Keeping {} of {}", clock(v.trim.len()), clock(v.duration));
                ui.label(egui::RichText::new(kept).size(11.5).color(theme::weak()));
                reset = ui.link("Reset trim").clicked();
            });
        }
        ui.add_space(8.0);
        let mut seek_to = None;
        match scrub {
            Some(Scrub::Seek(t)) => seek_to = Some(v.trim.clamp_pos(t)),
            Some(Scrub::Start(t)) => {
                v.trim = v.trim.with_start(t);
                seek_to = Some(v.trim.start);
            }
            Some(Scrub::End(t)) => {
                v.trim = v.trim.with_end(t, v.duration);
                seek_to = Some(v.trim.end);
            }
            None => {}
        }
        if reset {
            v.trim = Trim::full(v.duration);
        }
        if let Some(t) = seek_to {
            v.pos = v.trim.clamp_pos(t);
            v.playing = false;
            let _ = v.player.cmd.send(Cmd::Seek(v.pos));
        }
        if toggle {
            self.toggle_play();
        }
    }
}

/// `m:ss` for a number of seconds.
fn clock(s: f64) -> String {
    let s = s.max(0.0);
    format!("{}:{:02}", s as u64 / 60, s as u64 % 60)
}

impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        theme::set_dark(ctx.theme() == egui::Theme::Dark);
        if let Some((rx, _)) = &self.export {
            if let Ok(result) = rx.try_recv() {
                // a cancelled export is not an error worth showing
                self.status = result.map_or_else(|e| (e != "Cancelled").then_some(Err(e)), |p| Some(Ok(p)));
                self.export = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
        self.poll_player();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.is_video() && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.toggle_play();
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z))
            && let Some(prev) = self.history.pop()
        {
            self.style = prev.clone();
            self.snapshot = prev;
            self.tab = tab_of(&self.style.background);
            self.ensure_image();
            self.style_dirty = true;
        }
        // a capture dropped on the window opens in an editor of its own
        if let Some(path) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf())) {
            let exe = std::env::current_exe().unwrap_or_else(|_| "handset".into());
            let _ = crate::tools::command(exe).arg("--edit").arg(path).spawn();
        }

        let panel = (ctx.content_rect().width() * 0.34).clamp(PANEL_MIN, PANEL_MAX);
        egui::Panel::left("controls")
            .exact_size(panel)
            .resizable(false)
            .frame(egui::Frame::new().fill(theme::panel()).inner_margin(egui::Margin::symmetric(14, 0)))
            .show(ui, |ui| {
                ui.add_space(TOP);
                egui::Panel::bottom("export").show_separator_line(false).frame(egui::Frame::NONE).show(ui, |ui| {
                    ui.add_space(8.0);
                    self.export_area(ui, &ctx);
                    ui.add_space(14.0);
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut changed = self.presets_card(ui);
                    changed |= self.background_card(ui);
                    changed |= self.layout_card(ui);
                    if changed && matches!(self.style.background, Background::Image(_)) {
                        self.ensure_image();
                    }
                    self.style_dirty |= changed;
                });
                // remember the look before a change, a burst of slider moves counting as one
                if self.style != self.snapshot {
                    if self.last_push.elapsed() > Duration::from_millis(400) {
                        self.history.push(self.snapshot.clone());
                        self.history.truncate(200);
                    }
                    self.last_push = Instant::now();
                    self.snapshot = self.style.clone();
                    self.style_dirty = true;
                }
            });
        if self.is_video() {
            egui::Panel::bottom("transport")
                .show_separator_line(false)
                .frame(egui::Frame::new().fill(theme::bg()).inner_margin(egui::Margin::symmetric(20, 0)))
                .show(ui, |ui| self.transport(ui));
        }
        if self.style_dirty || self.frame_dirty {
            self.render_preview(&ctx);
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::bg())).show(ui, |ui| {
            let area = ui.max_rect();
            let Some(t) = &self.texture else { return };
            let avail = egui::vec2(area.width() - 56.0, area.height() - TOP - 24.0);
            let size = t.size_vec2();
            let scale = (avail.x / size.x).min(avail.y / size.y).clamp(0.05, 1.5);
            let shown = size * scale;
            let centre = egui::pos2(area.center().x, area.min.y + TOP + (area.height() - TOP) / 2.0);
            let rect = egui::Rect::from_center_size(centre, shown);
            if self.style.background == Background::None && !self.is_video() {
                checkerboard(ui.painter(), rect);
            }
            ui.put(rect, egui::Image::new((t.id(), shown)).corner_radius(10.0));
            ui.painter().rect_stroke(rect, 10.0, egui::Stroke::new(1.0, theme::line()), egui::StrokeKind::Outside);
        });

        {
            // the title bar is hidden: draw the file name there, and let the strip drag the window
            let screen = ctx.content_rect();
            let strip = egui::Rect::from_min_max(
                egui::pos2(screen.min.x + panel + 90.0, screen.min.y),
                egui::pos2(screen.max.x, screen.min.y + TOP),
            );
            ui.painter().text(
                egui::pos2((screen.min.x + panel + screen.max.x) / 2.0, screen.min.y + TOP / 2.0),
                egui::Align2::CENTER_CENTER,
                &self.name,
                egui::FontId::new(12.5, egui::FontFamily::Proportional),
                theme::weak(),
            );
            let drag = ui.interact(strip, ui.make_persistent_id("title-drag"), egui::Sense::click_and_drag());
            if drag.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }
    }
}

fn checkerboard(painter: &egui::Painter, rect: egui::Rect) {
    let cell = 12.0;
    let (dark_cell, light_cell) = theme::checker();
    painter.rect_filled(rect, 10.0, dark_cell);
    let (cols, rows) = ((rect.width() / cell).ceil() as i32, (rect.height() / cell).ceil() as i32);
    for r in 0..rows {
        for c in 0..cols {
            if (r + c) % 2 == 0 {
                continue;
            }
            let min = rect.min + egui::vec2(c as f32 * cell, r as f32 * cell);
            let cell_rect = egui::Rect::from_min_size(min, egui::vec2(cell, cell)).intersect(rect);
            painter.rect_filled(cell_rect, 0.0, light_cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run the scrubber in a headless egui context, moving the pointer through `steps` (a press at the first
    /// point, then drags), and return the last thing it reported.
    fn drive(trim: Trim, steps: &[egui::Pos2]) -> Option<Scrub> {
        let ctx = egui::Context::default();
        let mut found = None;
        let frame = |events: Vec<egui::Event>, found: &mut Option<Scrub>| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(600.0, 100.0))),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    if let Some(s) = scrubber(ui, 2.0, 8.0, trim, 500.0) {
                        *found = Some(s);
                    }
                });
            });
            out.textures_delta.clear();
        };
        frame(vec![], &mut found);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![egui::Event::PointerMoved(steps[0])], &mut found);
        frame(vec![button(steps[0], true)], &mut found);
        for p in &steps[1..] {
            frame(vec![egui::Event::PointerMoved(*p)], &mut found);
        }
        found
    }

    /// Where the scrubber puts the track, found from the content it lays out: the panel's margin and the
    /// 10 px of padding either side of the track.
    fn track_x(trim_t: f64) -> f32 {
        let (left, width) = (8.0 + 10.0, 500.0 - 20.0);
        trim::x_at(trim_t, left, width, 8.0)
    }

    #[test]
    fn dragging_the_start_handle_reports_a_new_start() {
        let trim = Trim { start: 1.0, end: 6.0 };
        let y = 8.0 + 15.0;
        let got = drive(trim, &[egui::pos2(track_x(1.0), y), egui::pos2(track_x(2.0), y)]);
        match got {
            Some(Scrub::Start(t)) => assert!((t - 2.0).abs() < 0.3, "{t}"),
            other => panic!("expected a start drag, got {other:?}"),
        }
    }

    #[test]
    fn dragging_the_end_handle_reports_a_new_end() {
        let trim = Trim { start: 1.0, end: 6.0 };
        let y = 8.0 + 15.0;
        let got = drive(trim, &[egui::pos2(track_x(6.0), y), egui::pos2(track_x(5.0), y)]);
        match got {
            Some(Scrub::End(t)) => assert!((t - 5.0).abs() < 0.3, "{t}"),
            other => panic!("expected an end drag, got {other:?}"),
        }
    }

    #[test]
    fn dragging_the_track_moves_the_playhead() {
        let trim = Trim { start: 1.0, end: 6.0 };
        let y = 8.0 + 15.0;
        let got = drive(trim, &[egui::pos2(track_x(3.0), y), egui::pos2(track_x(4.0), y)]);
        match got {
            Some(Scrub::Seek(t)) => assert!((t - 4.0).abs() < 0.3, "{t}"),
            other => panic!("expected a seek, got {other:?}"),
        }
    }

    #[test]
    fn clock_shows_minutes_and_seconds() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(75.4), "1:15");
        assert_eq!(clock(-3.0), "0:00");
    }

    fn editor() -> Editor {
        let ctx = egui::Context::default();
        let picture = |_: ()| Img { w: 40, h: 80, rgba: vec![200; 40 * 80 * 4] };
        let source = Source { first: picture(()), size: (40, 80), video: None, matte: None };
        let mut ed = Editor::new(&ctx, PathBuf::from("shot.png"), source, Style::default());
        ed.saved_presets.clear();
        ed
    }

    #[test]
    fn applying_a_look_switches_the_style_the_tab_and_the_custom_colour() {
        let mut ed = editor();
        let dark = Style { background: Background::Color([1, 2, 3]), padding: 22, ..Style::default() };
        ed.apply_preset(&dark);
        assert_eq!(ed.style, dark);
        assert!(ed.tab == Tab::Color);
        assert_eq!(ed.custom, [1, 2, 3]);
        ed.apply_preset(&Style { background: Background::Wallpaper(2), ..Style::default() });
        assert!(ed.tab == Tab::Wallpaper);
    }

    #[test]
    fn saving_a_look_keeps_the_current_style_under_a_tidy_name() {
        let mut ed = editor();
        ed.style.padding = 31;
        assert!(ed.save_preset("  Launch   post "));
        assert_eq!(ed.saved_presets.len(), 1);
        assert_eq!(ed.saved_presets[0].name, "Launch post");
        assert_eq!(ed.saved_presets[0].style.padding, 31);
        // the same name again replaces it
        ed.style.padding = 4;
        assert!(ed.save_preset("launch post"));
        assert_eq!(ed.saved_presets.len(), 1);
        assert_eq!(ed.saved_presets[0].style.padding, 4);
    }

    #[test]
    fn a_blank_name_saves_nothing_and_a_look_can_be_forgotten() {
        let mut ed = editor();
        assert!(!ed.save_preset("   "));
        assert!(ed.saved_presets.is_empty());
        assert!(ed.save_preset("Keep"));
        assert!(!ed.forget_preset("missing"));
        assert!(ed.forget_preset("KEEP"));
        assert!(ed.saved_presets.is_empty());
    }

    /// A 2 s clip with sound, or `None` when ffmpeg is not installed.
    fn clip_with_sound(name: &str) -> Option<PathBuf> {
        let ffmpeg = crate::tools::find("ffmpeg");
        if !crate::tools::command(&ffmpeg).arg("-version").output().is_ok_and(|o| o.status.success()) {
            return None;
        }
        let dir = std::env::temp_dir().join(format!("handset-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let clip = dir.join("clip.mp4");
        let ok = crate::tools::command(&ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=64x96:rate=10:duration=2",
            ])
            .args(["-f", "lavfi", "-i", "sine=duration=2", "-pix_fmt", "yuv420p", "-shortest"])
            .arg(&clip)
            .status()
            .unwrap()
            .success();
        ok.then_some(clip)
    }

    fn frames_within(player: &Player, wait: Duration) -> usize {
        let until = Instant::now() + wait;
        let mut n = 0;
        while Instant::now() < until {
            while let Ok(m) = player.msgs.try_recv() {
                if matches!(m, Msg::Frame(..)) {
                    n += 1;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        n
    }

    #[test]
    fn a_recording_with_sound_plays_frames_even_without_an_audio_device() {
        let Some(clip) = clip_with_sound("player-sound") else { return };
        let player = Player::spawn(egui::Context::default(), clip.clone(), (64, 96), 10.0, None, true);
        player.cmd.send(Cmd::Play(0.0)).unwrap();
        // at 10 fps about 8 frames arrive in a second, whether or not the speakers could be opened
        assert!(frames_within(&player, Duration::from_millis(1200)) >= 4);
        let _ = std::fs::remove_dir_all(clip.parent().unwrap());
    }

    #[test]
    fn pausing_stops_the_frames_and_seeking_shows_one_frame() {
        let Some(clip) = clip_with_sound("player-pause") else { return };
        let player = Player::spawn(egui::Context::default(), clip.clone(), (64, 96), 10.0, None, true);
        player.cmd.send(Cmd::Play(0.0)).unwrap();
        assert!(frames_within(&player, Duration::from_millis(700)) >= 2);
        player.cmd.send(Cmd::Pause).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let _ = frames_within(&player, Duration::from_millis(50)); // whatever was already on its way
        assert_eq!(frames_within(&player, Duration::from_millis(500)), 0, "no frames after a pause");
        player.cmd.send(Cmd::Seek(1.0)).unwrap();
        assert_eq!(frames_within(&player, Duration::from_millis(800)), 1, "a seek shows exactly one frame");
        let _ = std::fs::remove_dir_all(clip.parent().unwrap());
    }
}
