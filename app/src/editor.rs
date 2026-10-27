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
const SWATCH: f32 = 34.0;
const PANEL: f32 = 322.0;
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
    let mut reader = png::Decoder::new(std::io::Cursor::new(ICON)).read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    Some(egui::IconData { rgba: buf, width: info.width, height: info.height })
}

pub fn run(path: PathBuf, style: Style) -> Result<(), String> {
    let source = Source::load(&path)?;
    let name = path.file_name().map_or(String::new(), |n| n.to_string_lossy().into());
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Style - {name}"))
            .with_inner_size([940.0, 620.0])
            .with_min_inner_size([800.0, 540.0])
            .with_icon(window_icon().unwrap_or_default()),
        ..Default::default()
    };
    eframe::run_native(
        "Handset Style",
        opts,
        Box::new(move |cc| {
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

// ---- the window -----------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    None,
    Gradient,
    Wallpaper,
    Color,
    Image,
}

struct Editor {
    path: PathBuf,
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
    export: Option<Receiver<Result<PathBuf, String>>>,
    status: Option<Result<PathBuf, String>>,
    video: Option<VideoState>,
}

struct VideoState {
    player: Player,
    duration: f64,
    pos: f64,
    playing: bool,
}

impl Editor {
    fn new(ctx: &egui::Context, path: PathBuf, source: Source, style: Style) -> Self {
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
            }
        });
        let tab = match style.background {
            Background::None => Tab::None,
            Background::Color(_) => Tab::Color,
            Background::Gradient(_) => Tab::Gradient,
            Background::Wallpaper(_) => Tab::Wallpaper,
            Background::Image(_) => Tab::Image,
        };
        let mut ed = Self {
            full_size: source.size,
            matte: source.matte.is_some(),
            src: downscale(&source.first, pv),
            path,
            custom: match style.background {
                Background::Color(c) => c,
                _ => [40, 40, 60],
            },
            style,
            tab,
            image: None,
            backdrop: None,
            texture: None,
            style_dirty: true,
            frame_dirty: true,
            swatches,
            export: None,
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
        let (path, style, ctx) = (self.path.clone(), self.style.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send(restyle::export(&path, &style));
            ctx.request_repaint();
        });
        self.export = Some(rx);
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

    fn heading(ui: &mut egui::Ui, text: &str) {
        ui.add_space(12.0);
        ui.label(egui::RichText::new(text).small().strong().color(ui.visuals().weak_text_color()));
        ui.add_space(4.0);
    }

    fn swatch(ui: &mut egui::Ui, selected: bool, paint: impl FnOnce(&egui::Painter, egui::Rect)) -> bool {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(SWATCH, SWATCH), egui::Sense::click());
        paint(ui.painter(), rect);
        if selected {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                8.0,
                egui::Stroke::new(2.0, ui.visuals().selection.bg_fill),
                egui::StrokeKind::Outside,
            );
        } else if resp.hovered() {
            ui.painter().rect_stroke(
                rect,
                8.0,
                egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
                egui::StrokeKind::Inside,
            );
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
    }

    fn texture_swatches(&mut self, ui: &mut egui::Ui, range: std::ops::Range<usize>) -> bool {
        let mut changed = false;
        for i in range {
            let (bg, tex) = (self.swatches[i].0.clone(), self.swatches[i].1.clone());
            let selected = self.style.background == bg;
            if Self::swatch(ui, selected, |p, r| {
                p.image(
                    tex.id(),
                    r,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }) {
                self.style.background = bg;
                changed = true;
            }
        }
        changed
    }

    fn background_controls(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        Self::heading(ui, "BACKGROUND");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 4.0);
            for (tab, label) in [
                (Tab::None, "None"),
                (Tab::Gradient, "Gradient"),
                (Tab::Wallpaper, "Wallpaper"),
                (Tab::Color, "Color"),
                (Tab::Image, "Image"),
            ] {
                if ui.selectable_label(self.tab == tab, egui::RichText::new(label).size(13.0)).clicked() {
                    self.tab = tab;
                    if tab == Tab::None {
                        self.style.background = Background::None;
                        changed = true;
                    }
                }
            }
        });
        ui.add_space(8.0);
        let grad = GRADIENTS.len();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            match self.tab {
                Tab::None => {
                    ui.label(
                        egui::RichText::new("Transparent around the capture. Recordings use a dark color.").weak(),
                    );
                }
                Tab::Gradient => changed |= self.texture_swatches(ui, 0..grad),
                Tab::Wallpaper => changed |= self.texture_swatches(ui, grad..self.swatches.len()),
                Tab::Color => {
                    for c in COLORS {
                        let selected = self.style.background == Background::Color(c);
                        if Self::swatch(ui, selected, |p, r| {
                            p.rect_filled(r, 8.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                        }) {
                            self.style.background = Background::Color(c);
                            self.custom = c;
                            changed = true;
                        }
                    }
                    ui.label("Custom");
                    if egui::color_picker::color_edit_button_srgb(ui, &mut self.custom).changed() {
                        self.style.background = Background::Color(self.custom);
                        changed = true;
                    }
                }
                Tab::Image => {
                    if ui.button("Choose a picture...").clicked()
                        && let Some(file) = rfd::FileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "heic", "bmp", "gif", "tiff"])
                            .pick_file()
                    {
                        self.style.background = Background::Image(file);
                        changed = true;
                    }
                    if let Background::Image(p) = &self.style.background {
                        ui.label(
                            egui::RichText::new(p.file_name().map_or(String::new(), |n| n.to_string_lossy().into()))
                                .weak(),
                        );
                    }
                }
            }
        });
        changed
    }

    fn slider(ui: &mut egui::Ui, label: &str, value: &mut u32, max: u32, suffix: &str) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.add_sized([86.0, 20.0], egui::Label::new(label));
            ui.spacing_mut().slider_width = 112.0;
            changed = ui.add(egui::Slider::new(value, 0..=max).suffix(suffix)).changed();
        });
        changed
    }

    fn layout_controls(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        Self::heading(ui, "LAYOUT");
        changed |= Self::slider(ui, "Padding", &mut self.style.padding, crate::style::MAX_PADDING, "%");
        if !self.matte {
            changed |= Self::slider(ui, "Corner radius", &mut self.style.radius, crate::style::MAX_RADIUS, "%");
        }
        changed |= Self::slider(ui, "Shadow", &mut self.style.shadow, 100, "");
        Self::heading(ui, "ASPECT RATIO");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for a in ASPECTS {
                if ui.selectable_label(self.style.aspect == a, egui::RichText::new(a.label()).size(13.0)).clicked() {
                    self.style.aspect = a;
                    changed = true;
                }
            }
        });
        changed
    }

    fn export_row(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let l = self.effective_style().layout(self.full_size);
        ui.label(egui::RichText::new(format!("Output {} x {} px", l.out.0, l.out.1)).small().weak());
        ui.add_space(6.0);
        let busy = self.export.is_some();
        ui.horizontal(|ui| {
            let label = if busy { "Exporting..." } else { "Export" };
            let button = egui::Button::new(egui::RichText::new(label).strong())
                .fill(ui.visuals().selection.bg_fill)
                .min_size(egui::vec2(120.0, 34.0));
            if ui.add_enabled(!busy, button).clicked() {
                self.start_export(ctx);
            }
            if busy {
                ui.add(egui::Spinner::new());
            } else if ui.button("Reset").clicked() {
                self.style = Style::default();
                self.tab = Tab::Gradient;
                self.style_dirty = true;
            }
        });
        match &self.status {
            Some(Ok(p)) => {
                ui.add_space(6.0);
                let name = p.file_name().map_or(String::new(), |n| n.to_string_lossy().into());
                ui.label(egui::RichText::new(format!("Saved {name}")).color(egui::Color32::LIGHT_GREEN));
                ui.horizontal(|ui| {
                    if ui.link("Show in folder").clicked() {
                        crate::capture::reveal(p);
                    }
                    if ui.link("Open").clicked() {
                        crate::capture::open_file(p);
                    }
                });
            }
            Some(Err(e)) => {
                ui.add_space(6.0);
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
            None => {}
        }
    }

    fn transport(&mut self, ui: &mut egui::Ui) {
        let Some(v) = self.video.as_mut() else { return };
        let mut seek = None;
        let mut toggle = false;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let icon = if v.playing { "||" } else { ">" };
            if ui.add(egui::Button::new(egui::RichText::new(icon).strong()).min_size(egui::vec2(34.0, 28.0))).clicked()
            {
                toggle = true;
            }
            ui.spacing_mut().slider_width = (ui.available_width() - 110.0).max(80.0);
            let mut pos = v.pos;
            if ui.add(egui::Slider::new(&mut pos, 0.0..=v.duration).show_value(false)).changed() {
                seek = Some(pos);
            }
            let clock = |s: f64| format!("{}:{:02}", s as u64 / 60, s as u64 % 60);
            ui.label(egui::RichText::new(format!("{} / {}", clock(v.pos), clock(v.duration))).monospace().small());
        });
        ui.add_space(6.0);
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
        if let Some(rx) = &self.export
            && let Ok(result) = rx.try_recv()
        {
            self.status = Some(result);
            self.export = None;
        }
        self.poll_player();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.is_video() && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            self.toggle_play();
        }

        egui::Panel::left("controls").exact_size(PANEL).resizable(false).show(ui, |ui| {
            egui::Panel::bottom("export").show_separator_line(false).show(ui, |ui| {
                ui.add_space(8.0);
                self.export_row(ui, &ctx);
                ui.add_space(10.0);
            });
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(4.0);
                let mut changed = self.background_controls(ui);
                changed |= self.layout_controls(ui);
                if changed && matches!(self.style.background, Background::Image(_)) {
                    self.ensure_image();
                }
                self.style_dirty |= changed;
            });
        });
        if self.is_video() {
            egui::Panel::bottom("transport").show(ui, |ui| self.transport(ui));
        }
        if self.style_dirty || self.frame_dirty {
            self.render_preview(&ctx);
        }
        egui::CentralPanel::default().show(ui, |ui| {
            let Some(t) = &self.texture else { return };
            let avail = ui.available_size() - egui::vec2(32.0, 32.0);
            let size = t.size_vec2();
            let scale = (avail.x / size.x).min(avail.y / size.y).clamp(0.05, 1.5);
            let shown = size * scale;
            let rect = egui::Rect::from_center_size(ui.max_rect().center(), shown);
            if self.style.background == Background::None && !self.is_video() {
                checkerboard(ui.painter(), rect);
            }
            ui.painter().image(
                t.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        });
    }
}

fn checkerboard(painter: &egui::Painter, rect: egui::Rect) {
    let cell = 12.0;
    painter.rect_filled(rect, 0.0, egui::Color32::from_gray(52));
    let (cols, rows) = ((rect.width() / cell).ceil() as i32, (rect.height() / cell).ceil() as i32);
    for r in 0..rows {
        for c in 0..cols {
            if (r + c) % 2 == 0 {
                continue;
            }
            let min = rect.min + egui::vec2(c as f32 * cell, r as f32 * cell);
            let cell_rect = egui::Rect::from_min_size(min, egui::vec2(cell, cell)).intersect(rect);
            painter.rect_filled(cell_rect, 0.0, egui::Color32::from_gray(68));
        }
    }
}
