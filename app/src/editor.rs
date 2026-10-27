//! The style editor (`handset --edit <file>`): a window with a live preview of a capture on its styled
//! background and controls for the background, padding, corner radius, shadow and aspect ratio.
//!
//! It runs as its own process so it never touches the mirroring window's event loop. The preview is a
//! downscaled picture styled by the same code as the export. A recording can be played and scrubbed: a
//! worker thread has ffmpeg decode small frames (silently; the preview has no sound), and only the picture
//! is blended again for each frame, over a backdrop that is drawn once per style change.

use crate::restyle;
use crate::style::{ASPECTS, Aspect, Background, GRADIENTS, Img, Layout, Style, WALLPAPERS};
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
    fn spawn(ctx: egui::Context, path: PathBuf, size: (u32, u32), fps: f32, matte: Option<Img>) -> Self {
        let (cmd, rx) = channel::<Cmd>();
        let (tx, msgs) = channel::<Msg>();
        std::thread::spawn(move || {
            let w = Worker { path, size, fps: fps.clamp(1.0, 30.0), matte, tx, ctx };
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
        let _ = child.kill();
        let _ = child.wait();
        result
    }
}

// ---- look -----------------------------------------------------------------------------------------

mod theme {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(0x0f, 0x0f, 0x12);
    pub const PANEL: Color32 = Color32::from_rgb(0x16, 0x16, 0x1a);
    pub const CARD: Color32 = Color32::from_rgb(0x1d, 0x1d, 0x23);
    pub const CARD_HI: Color32 = Color32::from_rgb(0x28, 0x28, 0x30);
    pub const LINE: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x35);
    pub const TEXT: Color32 = Color32::from_rgb(0xee, 0xee, 0xf2);
    pub const WEAK: Color32 = Color32::from_rgb(0x8d, 0x8d, 0x99);
    /// Handset's green.
    pub const ACCENT: Color32 = Color32::from_rgb(0x30, 0xdd, 0x81);
    pub const ON_ACCENT: Color32 = Color32::from_rgb(0x04, 0x2b, 0x19);
    pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x1d, 0x4a, 0x36);
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

    let mut v = egui::Visuals::dark();
    v.panel_fill = theme::PANEL;
    v.window_fill = theme::CARD;
    v.extreme_bg_color = theme::BG;
    v.faint_bg_color = theme::CARD;
    v.override_text_color = Some(theme::TEXT);
    v.selection.bg_fill = theme::ACCENT;
    v.selection.stroke = egui::Stroke::new(1.0, theme::ON_ACCENT);
    v.hyperlink_color = theme::ACCENT;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, theme::LINE);
    let radius = egui::CornerRadius::same(9);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = radius;
        w.bg_stroke = egui::Stroke::NONE;
    }
    v.widgets.inactive.weak_bg_fill = theme::CARD_HI;
    v.widgets.inactive.bg_fill = theme::CARD_HI;
    v.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(0x33, 0x33, 0x3d);
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(0x33, 0x33, 0x3d);
    v.widgets.active.weak_bg_fill = egui::Color32::from_rgb(0x3d, 0x3d, 0x48);
    v.widgets.active.bg_fill = egui::Color32::from_rgb(0x3d, 0x3d, 0x48);
    ctx.set_visuals(v);

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
    ui.label(egui::RichText::new(text).size(11.0).family(semibold()).color(theme::WEAK));
    ui.add_space(2.0);
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(theme::CARD).corner_radius(12).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

/// A row of options in one rounded track; returns the index clicked.
fn segmented(ui: &mut egui::Ui, id: &str, labels: &[&str], selected: Option<usize>) -> Option<usize> {
    let font = egui::FontId::new(12.0, semibold());
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| ui.painter().layout_no_wrap((*l).to_string(), font.clone(), theme::TEXT).size().x + 8.0)
        .collect();
    let total = ui.available_width();
    let extra = ((total - 4.0 - widths.iter().sum::<f32>()) / labels.len() as f32).max(0.0);
    let (track, _) = ui.allocate_exact_size(egui::vec2(total, 34.0), egui::Sense::hover());
    ui.painter().rect_filled(track, 10.0, theme::BG);
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
            ui.painter().rect_filled(seg, 8.0, theme::ACCENT_DIM);
        } else if resp.hovered() {
            ui.painter().rect_filled(seg, 8.0, theme::CARD);
        }
        let color = if on { theme::ACCENT } else { theme::WEAK };
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
        theme::ACCENT_DIM
    } else if resp.hovered() {
        theme::CARD_HI
    } else {
        off
    };
    ui.painter().rect_filled(rect, 9.0, fill);
    let color = if on { theme::ACCENT } else { theme::TEXT };
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
            ui.label(egui::RichText::new(label).color(theme::WEAK));
        });
        ui.spacing_mut().slider_width = (ui.available_width() - 52.0).max(40.0);
        changed = ui.add(egui::Slider::new(value, 0..=max).show_value(false).trailing_fill(true)).changed();
        let text = egui::RichText::new(format!("{value}{unit}")).family(semibold());
        ui.add_sized([36.0, 24.0], egui::Label::new(text));
    });
    changed
}

/// A slim track with a thumb; the new position when it is clicked or dragged.
fn scrubber(ui: &mut egui::Ui, pos: f64, duration: f64, width: f32) -> Option<f64> {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 28.0), egui::Sense::click_and_drag());
    let track = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width() - 14.0, 4.0));
    let frac = (pos / duration).clamp(0.0, 1.0) as f32;
    let thumb_x = track.left() + track.width() * frac;
    let p = ui.painter();
    p.rect_filled(track, 2.0, theme::LINE);
    p.rect_filled(egui::Rect::from_min_max(track.min, egui::pos2(thumb_x, track.max.y)), 2.0, theme::ACCENT);
    let r = if resp.hovered() || resp.dragged() { 8.0 } else { 6.5 };
    p.circle_filled(egui::pos2(thumb_x, track.center().y), r, egui::Color32::WHITE);
    if resp.hovered() {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
    }
    if resp.dragged() || resp.clicked() {
        let x = resp.interact_pointer_pos()?.x;
        return Some(((x - track.left()) / track.width()).clamp(0.0, 1.0) as f64 * duration);
    }
    None
}

fn play_button(ui: &mut egui::Ui, playing: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::click());
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let fill = if resp.hovered() { egui::Color32::from_rgb(0x5a, 0xe8, 0x9d) } else { theme::ACCENT };
    let c = rect.center();
    let p = ui.painter();
    p.circle_filled(c, 18.0, fill);
    if playing {
        for dx in [-3.6, 3.6] {
            p.rect_filled(
                egui::Rect::from_center_size(c + egui::vec2(dx, 0.0), egui::vec2(3.4, 12.0)),
                1.0,
                theme::ON_ACCENT,
            );
        }
    } else {
        let tri = vec![c + egui::vec2(-3.5, -7.0), c + egui::vec2(-3.5, 7.0), c + egui::vec2(7.0, 0.0)];
        p.add(egui::Shape::convex_polygon(tri, theme::ON_ACCENT, egui::Stroke::NONE));
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
                player: Player::spawn(ctx.clone(), path.clone(), pv, info.fps, matte),
                duration: info.duration.max(0.1),
                pos: 0.0,
                playing: false,
                looping: false,
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
        std::thread::spawn(move || {
            let _ = tx.send(restyle::export_with(&path, &style, &job));
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
            if v.pos >= v.duration - 0.1 {
                v.pos = 0.0;
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
                    v.pos = 0.0;
                    let _ = v.player.cmd.send(Cmd::Play(0.0));
                }
                Msg::Ended => {
                    v.playing = false;
                    v.pos = v.duration;
                }
            }
        }
        if let Some((t, img)) = newest {
            if v.playing {
                v.pos = t.min(v.duration);
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
                egui::Stroke::new(2.0, theme::ACCENT),
                egui::StrokeKind::Outside,
            );
        } else if resp.hovered() {
            ui.painter().rect_stroke(rect, 9.0, egui::Stroke::new(1.5, egui::Color32::WHITE), egui::StrokeKind::Inside);
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
                    ui.label(egui::RichText::new(text).color(theme::WEAK));
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
                        ui.label(egui::RichText::new("Custom").color(theme::WEAK));
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
                        ui.label(egui::RichText::new(name).color(theme::WEAK));
                    }
                }
            }
        });
        changed
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
                    if chip(ui, &a.label(), egui::vec2(w, 30.0), self.style.aspect == a, theme::BG) {
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
        let kind = if self.is_video() { "video" } else { "picture" };
        ui.label(egui::RichText::new(format!("{} x {} px {kind}", l.out.0, l.out.1)).size(11.5).color(theme::WEAK));
        let busy = self.export.is_some();
        let label = if busy { "Exporting..." } else { "Export" };
        let button =
            egui::Button::new(egui::RichText::new(label).size(14.5).family(semibold()).color(theme::ON_ACCENT))
                .fill(theme::ACCENT)
                .corner_radius(11)
                .min_size(egui::vec2(ui.available_width(), 40.0));
        if ui.add_enabled(!busy, button).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            self.start_export(ctx);
        }
        ui.horizontal(|ui| {
            if let Some((_, control)) = &self.export {
                let p = control.progress.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.0;
                if self.is_video() {
                    ui.add(egui::ProgressBar::new(p).desired_width(ui.available_width() - 64.0).fill(theme::ACCENT));
                } else {
                    ui.add(egui::Spinner::new().size(14.0));
                    ui.label(egui::RichText::new("Rendering").color(theme::WEAK));
                }
                if ui.link("Cancel").clicked() {
                    control.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            } else if ui.link("Reset look").clicked() {
                self.style = Style::default();
                self.tab = Tab::Gradient;
                self.style_dirty = true;
            }
        });
        match &self.status {
            Some(Ok(p)) => {
                let name: String = p.file_name().map_or(String::new(), |n| n.to_string_lossy().into());
                egui::Frame::new().fill(theme::ACCENT_DIM).corner_radius(10).inner_margin(egui::Margin::same(10)).show(
                    ui,
                    |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new("Saved").family(semibold()).color(theme::ACCENT));
                        ui.label(egui::RichText::new(name).size(12.0));
                        ui.horizontal(|ui| {
                            if ui.link("Show in folder").clicked() {
                                crate::capture::reveal(p);
                            }
                            if ui.link("Open").clicked() {
                                crate::capture::open_file(p);
                            }
                        });
                    },
                );
            }
            Some(Err(e)) => {
                ui.colored_label(egui::Color32::from_rgb(0xff, 0x8a, 0x80), e);
            }
            None => {}
        }
    }

    fn transport(&mut self, ui: &mut egui::Ui) {
        let Some(v) = self.video.as_mut() else { return };
        let mut seek = None;
        let mut toggle = false;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            toggle = play_button(ui, v.playing);
            let clock = |s: f64| format!("{}:{:02}", s as u64 / 60, s as u64 % 60);
            let label = format!("{} / {}", clock(v.pos), clock(v.duration));
            let w = (ui.available_width() - 92.0).max(80.0);
            let w = w - 60.0;
            seek = scrubber(ui, v.pos, v.duration, w);
            if chip(ui, "Loop", egui::vec2(48.0, 26.0), v.looping, theme::CARD) {
                v.looping = !v.looping;
            }
            ui.label(egui::RichText::new(label).size(12.0).color(theme::WEAK));
        });
        ui.add_space(8.0);
        if let Some(t) = seek {
            v.pos = t;
            v.playing = false;
            let _ = v.player.cmd.send(Cmd::Seek(t));
        }
        if toggle {
            self.toggle_play();
        }
    }
}

impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
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
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::symmetric(14, 0)))
            .show(ui, |ui| {
                ui.add_space(TOP);
                egui::Panel::bottom("export").show_separator_line(false).frame(egui::Frame::NONE).show(ui, |ui| {
                    ui.add_space(8.0);
                    self.export_area(ui, &ctx);
                    ui.add_space(14.0);
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut changed = self.background_card(ui);
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
                .frame(egui::Frame::new().fill(theme::BG).inner_margin(egui::Margin::symmetric(20, 0)))
                .show(ui, |ui| self.transport(ui));
        }
        if self.style_dirty || self.frame_dirty {
            self.render_preview(&ctx);
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG)).show(ui, |ui| {
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
            ui.painter().rect_stroke(rect, 10.0, egui::Stroke::new(1.0, theme::LINE), egui::StrokeKind::Outside);
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
                theme::WEAK,
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
    painter.rect_filled(rect, 10.0, egui::Color32::from_gray(40));
    let (cols, rows) = ((rect.width() / cell).ceil() as i32, (rect.height() / cell).ceil() as i32);
    for r in 0..rows {
        for c in 0..cols {
            if (r + c) % 2 == 0 {
                continue;
            }
            let min = rect.min + egui::vec2(c as f32 * cell, r as f32 * cell);
            let cell_rect = egui::Rect::from_min_size(min, egui::vec2(cell, cell)).intersect(rect);
            painter.rect_filled(cell_rect, 0.0, egui::Color32::from_gray(54));
        }
    }
}
