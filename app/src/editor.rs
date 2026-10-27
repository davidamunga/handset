//! The style editor (`handset --edit <file>`): a window with a live preview of a capture on its styled
//! background and controls for the background, padding, corner radius, shadow and aspect ratio.
//!
//! It runs as its own process so it never touches the mirroring window's event loop. The preview is a
//! downscaled still (the picture, or the first frame of a recording) styled by the same code as the export.

use crate::restyle;
use crate::style::{ASPECTS, Aspect, Background, GRADIENTS, Img, Style, WALLPAPERS};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

const PREVIEW_LONG_SIDE: u32 = 900;
const SWATCH: f32 = 36.0;
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

pub fn run(path: PathBuf, style: Style) -> Result<(), String> {
    let source = load_source(&path)?;
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!(
                "Handset Style - {}",
                path.file_name().map_or(String::new(), |n| n.to_string_lossy().into())
            ))
            .with_inner_size([1100.0, 860.0])
            .with_min_inner_size([760.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Handset Style",
        opts,
        Box::new(move |cc| Ok(Box::new(Editor::new(&cc.egui_ctx, path, source, style)))),
    )
    .map_err(|e| e.to_string())
}

/// The picture to preview: the PNG itself, or the first frame of a video.
fn load_source(path: &Path) -> Result<Img, String> {
    if !restyle::is_video(path) {
        return restyle::read_png(path);
    }
    restyle::first_frame(path)
}

fn downscale(src: &Img, long_side: u32) -> Img {
    let long = src.w.max(src.h);
    if long <= long_side {
        return Img { w: src.w, h: src.h, rgba: src.rgba.clone() };
    }
    let f = long as f32 / long_side as f32;
    let (w, h) = (((src.w as f32 / f).round() as u32).max(1), ((src.h as f32 / f).round() as u32).max(1));
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            // average the source pixels under this one (premultiplied, so transparent edges do not bleed black)
            let (x0, x1) = ((x as f32 * f) as u32, (((x + 1) as f32 * f) as u32).clamp(1, src.w));
            let (y0, y1) = ((y as f32 * f) as u32, (((y + 1) as f32 * f) as u32).clamp(1, src.h));
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
            let a = acc[3] / n;
            let c = |v: f32| if acc[3] > 0.0 { (v / acc[3]).round() as u8 } else { 0 };
            rgba.extend_from_slice(&[c(acc[0]), c(acc[1]), c(acc[2]), (a * 255.0).round() as u8]);
        }
    }
    Img { w, h, rgba }
}

struct Editor {
    path: PathBuf,
    video: bool,
    preview_src: Img,
    style: Style,
    /// The picture chosen as background, decoded once, with the path it came from.
    image: Option<(PathBuf, Img)>,
    custom: [u8; 3],
    texture: Option<egui::TextureHandle>,
    dirty: bool,
    swatches: Vec<(Background, egui::TextureHandle)>,
    export: Option<Receiver<Result<PathBuf, String>>>,
    status: Option<Result<PathBuf, String>>,
}

impl Editor {
    fn new(ctx: &egui::Context, path: PathBuf, source: Img, style: Style) -> Self {
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
        let mut ed = Self {
            video: restyle::is_video(&path),
            preview_src: downscale(&source, PREVIEW_LONG_SIDE),
            path,
            custom: match style.background {
                Background::Color(c) => c,
                _ => [40, 40, 60],
            },
            style,
            image: None,
            texture: None,
            dirty: true,
            swatches,
            export: None,
            status: None,
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
                Ok(img) => self.image = Some((p.clone(), downscale(&img, PREVIEW_LONG_SIDE * 2))),
                Err(e) => {
                    self.status = Some(Err(e));
                    self.style.background = Background::Gradient(0);
                }
            }
        }
    }

    fn render_preview(&mut self, ctx: &egui::Context) {
        let style = if self.video { restyle::video_style(&self.style) } else { self.style.clone() };
        let out = style.compose(&self.preview_src, self.image.as_ref().map(|(_, i)| i));
        let img = egui::ColorImage::from_rgba_unmultiplied([out.w as usize, out.h as usize], &out.rgba);
        match &mut self.texture {
            Some(t) => t.set(img, egui::TextureOptions::LINEAR),
            None => self.texture = Some(ctx.load_texture("preview", img, egui::TextureOptions::LINEAR)),
        }
        self.dirty = false;
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

    fn section(ui: &mut egui::Ui, title: &str) {
        ui.add_space(10.0);
        ui.label(egui::RichText::new(title).color(ui.visuals().weak_text_color()));
        ui.add_space(2.0);
    }

    fn swatch(&mut self, ui: &mut egui::Ui, selected: bool, paint: impl FnOnce(&egui::Painter, egui::Rect)) -> bool {
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
        resp.clicked()
    }

    fn export_row(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut changed = false;
        ui.add_space(16.0);
        let busy = self.export.is_some();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new(if busy { "Exporting..." } else { "Export" }).min_size(egui::vec2(96.0, 32.0)),
                )
                .clicked()
            {
                self.start_export(ctx);
            }
            if ui.button("Reset").clicked() {
                self.style = Style::default();
                changed = true;
            }
        });
        match &self.status {
            Some(Ok(p)) => {
                ui.add_space(6.0);
                ui.label(format!("Saved {}", p.file_name().map_or(String::new(), |n| n.to_string_lossy().into())));
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
        if self.video {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Preview shows the first frame.").small().weak());
        }
        self.dirty |= changed;
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        Self::section(ui, "Aspect ratio");
        egui::ComboBox::from_id_salt("aspect")
            .width(ui.available_width())
            .selected_text(self.style.aspect.label())
            .show_ui(ui, |ui| {
                for a in ASPECTS {
                    if ui.selectable_label(self.style.aspect == a, a.label()).clicked() {
                        self.style.aspect = a;
                        changed = true;
                    }
                }
            });
        Self::section(ui, "Background");
        let none = matches!(self.style.background, Background::None);
        if ui.add_sized([ui.available_width(), 30.0], egui::Button::new("None").selected(none)).clicked() {
            self.style.background = Background::None;
            changed = true;
        }

        let grad = GRADIENTS.len();
        for (title, range) in [("Gradients", 0..grad), ("Wallpapers", grad..self.swatches.len())] {
            Self::section(ui, title);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                for i in range {
                    let bg = self.swatches[i].0.clone();
                    let tex = self.swatches[i].1.clone();
                    let sel = self.style.background == bg;
                    if self.swatch(ui, sel, |p, r| {
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
                if title == "Wallpapers" {
                    let picked = matches!(self.style.background, Background::Image(_));
                    if self.swatch(ui, picked, |p, r| {
                        p.rect_stroke(r, 8.0, egui::Stroke::new(1.0, egui::Color32::GRAY), egui::StrokeKind::Inside);
                        p.text(
                            r.center(),
                            egui::Align2::CENTER_CENTER,
                            "+",
                            egui::FontId::proportional(22.0),
                            egui::Color32::GRAY,
                        );
                    }) && let Some(file) = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "heic", "bmp", "gif", "tiff"])
                        .pick_file()
                    {
                        self.style.background = Background::Image(file);
                        self.ensure_image();
                        changed = true;
                    }
                }
            });
        }

        Self::section(ui, "Plain color");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
            for c in COLORS {
                let sel = self.style.background == Background::Color(c);
                if self.swatch(ui, sel, |p, r| {
                    p.rect_filled(r, 8.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                }) {
                    self.style.background = Background::Color(c);
                    self.custom = c;
                    changed = true;
                }
            }
            if egui::color_picker::color_edit_button_srgb(ui, &mut self.custom).changed() {
                self.style.background = Background::Color(self.custom);
                changed = true;
            }
        });

        Self::section(ui, "Padding");
        let mut v = self.style.padding;
        if ui.add(egui::Slider::new(&mut v, 0..=crate::style::MAX_PADDING).suffix("%")).changed() {
            self.style.padding = v;
            changed = true;
        }
        Self::section(ui, "Corner radius");
        let mut v = self.style.radius;
        if ui.add(egui::Slider::new(&mut v, 0..=crate::style::MAX_RADIUS).suffix("%")).changed() {
            self.style.radius = v;
            changed = true;
        }
        Self::section(ui, "Shadow");
        let mut v = self.style.shadow;
        if ui.add(egui::Slider::new(&mut v, 0..=100)).changed() {
            self.style.shadow = v;
            changed = true;
        }

        self.dirty |= changed;
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
        egui::Panel::left("controls").exact_size(300.0).resizable(false).show(ui, |ui| {
            egui::Panel::bottom("export").show_separator_line(false).show(ui, |ui| {
                ui.add_space(8.0);
                self.export_row(ui, &ctx);
                ui.add_space(8.0);
            });
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(8.0);
                self.controls(ui);
            });
        });
        if self.dirty {
            self.render_preview(&ctx);
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(t) = &self.texture {
                let avail = ui.available_size() - egui::vec2(32.0, 32.0);
                let size = t.size_vec2();
                let scale = (avail.x / size.x).min(avail.y / size.y).clamp(0.05, 1.0);
                ui.centered_and_justified(|ui| {
                    ui.add(egui::Image::new((t.id(), size * scale)));
                });
            }
        });
    }
}
