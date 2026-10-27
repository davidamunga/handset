//! Styling for captures: a background, padding, a shadow, rounded corners and an output aspect ratio.
//!
//! Everything here is pure pixel work on RGBA buffers, shared by the editor's live preview, the
//! screenshot path and the ffmpeg path for recordings (see `restyle.rs`). All amounts are relative to
//! the capture's size, so a small preview and the full-size export look the same.

use std::path::PathBuf;

pub const MAX_PADDING: u32 = 40;
pub const MAX_RADIUS: u32 = 25;

#[derive(Clone, PartialEq, Debug)]
pub enum Background {
    /// Transparent surroundings (screenshots); recordings cannot be transparent and use a dark colour.
    None,
    Color([u8; 3]),
    /// Index into [`GRADIENTS`].
    Gradient(usize),
    /// Index into [`WALLPAPERS`].
    Wallpaper(usize),
    Image(PathBuf),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Aspect {
    Auto,
    Ratio(u32, u32),
}

#[allow(dead_code)] // the editor's choices
pub const ASPECTS: [Aspect; 7] = [
    Aspect::Auto,
    Aspect::Ratio(1, 1),
    Aspect::Ratio(16, 9),
    Aspect::Ratio(9, 16),
    Aspect::Ratio(4, 5),
    Aspect::Ratio(4, 3),
    Aspect::Ratio(3, 2),
];

impl Aspect {
    #[allow(dead_code)]
    pub fn label(self) -> String {
        match self {
            Self::Auto => "Auto".into(),
            Self::Ratio(a, b) => format!("{a}:{b}"),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Style {
    pub background: Background,
    /// Space around the capture, in percent of its longer side.
    pub padding: u32,
    /// Corner radius, in percent of the capture's shorter side.
    pub radius: u32,
    /// Shadow strength, 0 (none) to 100.
    pub shadow: u32,
    pub aspect: Aspect,
}

impl Default for Style {
    fn default() -> Self {
        Self { background: Background::Gradient(0), padding: 10, radius: 6, shadow: 60, aspect: Aspect::Auto }
    }
}

/// Dark surroundings used when a recording is styled with no background.
pub const DARK: [u8; 3] = [16, 16, 20];

pub struct Gradient {
    pub name: &'static str,
    /// Direction in degrees, 0 = left to right, 90 = top to bottom.
    pub angle: f32,
    pub stops: &'static [u32],
}

pub const GRADIENTS: &[Gradient] = &[
    Gradient { name: "sunset", angle: 135.0, stops: &[0xff5f6d, 0xffc371] },
    Gradient { name: "dusk", angle: 135.0, stops: &[0x4776e6, 0x8e54e9] },
    Gradient { name: "midnight", angle: 160.0, stops: &[0x0f2027, 0x203a43, 0x2c5364] },
    Gradient { name: "lagoon", angle: 135.0, stops: &[0x11998e, 0x38ef7d] },
    Gradient { name: "candy", angle: 135.0, stops: &[0xf093fb, 0xf5576c] },
    Gradient { name: "peach", angle: 120.0, stops: &[0xff9a9e, 0xfad0c4] },
    Gradient { name: "ocean", angle: 150.0, stops: &[0x2193b0, 0x6dd5ed] },
    Gradient { name: "mist", angle: 160.0, stops: &[0xbdc3c7, 0xeef2f3] },
    Gradient { name: "ink", angle: 160.0, stops: &[0x232526, 0x414345] },
    Gradient { name: "bolt", angle: 135.0, stops: &[0x00c6ff, 0x0072ff] },
];

/// Soft blobs of colour on a base, bent by a gentle warp: made up here, not taken from anyone's artwork.
pub struct Wallpaper {
    pub name: &'static str,
    pub base: u32,
    /// `(x, y, radius, colour)` in unit coordinates.
    pub blobs: &'static [(f32, f32, f32, u32)],
    pub warp: f32,
}

pub const WALLPAPERS: &[Wallpaper] = &[
    Wallpaper {
        name: "dune",
        base: 0xf6a04d,
        blobs: &[
            (0.15, 0.2, 0.35, 0xff6b35),
            (0.8, 0.1, 0.3, 0xffd166),
            (0.5, 0.9, 0.4, 0xd1495b),
            (0.95, 0.8, 0.3, 0xf77f00),
        ],
        warp: 0.07,
    },
    Wallpaper {
        name: "aurora",
        base: 0x0b1d3a,
        blobs: &[(0.2, 0.8, 0.35, 0x00f5a0), (0.75, 0.3, 0.35, 0x7b2ff7), (0.5, 0.5, 0.25, 0x00b4d8)],
        warp: 0.09,
    },
    Wallpaper {
        name: "ember",
        base: 0x1a0b2e,
        blobs: &[(0.1, 0.1, 0.4, 0xff3d6e), (0.9, 0.9, 0.4, 0xff9a3c), (0.6, 0.2, 0.3, 0x7a1fa2)],
        warp: 0.06,
    },
    Wallpaper {
        name: "tide",
        base: 0x0a3d62,
        blobs: &[(0.2, 0.3, 0.4, 0x3dc1d3), (0.8, 0.7, 0.4, 0x546de5), (0.5, 1.0, 0.3, 0x63cdda)],
        warp: 0.08,
    },
    Wallpaper {
        name: "orchid",
        base: 0xf8e1f4,
        blobs: &[(0.2, 0.2, 0.35, 0xff9de2), (0.85, 0.3, 0.35, 0xa29bfe), (0.5, 0.9, 0.4, 0xffd6a5)],
        warp: 0.06,
    },
    Wallpaper {
        name: "meadow",
        base: 0xe9f5db,
        blobs: &[(0.1, 0.8, 0.4, 0x8fd694), (0.8, 0.2, 0.35, 0xfdf0a6), (0.6, 0.7, 0.35, 0x4fb286)],
        warp: 0.07,
    },
];

pub struct Img {
    pub w: u32,
    pub h: u32,
    /// Straight-alpha RGBA8.
    pub rgba: Vec<u8>,
}

/// Where the capture sits in the styled output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub out: (u32, u32),
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Corner radius in output pixels.
    pub radius: f32,
}

fn rgb(hex: u32) -> [f32; 3] {
    [(hex >> 16 & 255) as f32, (hex >> 8 & 255) as f32, (hex & 255) as f32]
}

fn noise(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9e37_79b1) ^ y.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    (h & 0xffff) as f32 / 65535.0
}

fn gradient_at(g: &Gradient, u: f32, v: f32, aspect: f32) -> [f32; 3] {
    let (s, c) = g.angle.to_radians().sin_cos();
    // project onto the direction in a space where the longer side is 1, so the angle looks the same on any ratio
    let (px, py) = if aspect >= 1.0 { ((u - 0.5) * aspect, v - 0.5) } else { (u - 0.5, (v - 0.5) / aspect) };
    let extent = (aspect.max(1.0 / aspect) * c.abs() + s.abs()).max(1e-3) * 0.5;
    let t = ((px * c + py * s) / extent * 0.5 + 0.5).clamp(0.0, 1.0);
    let seg = (g.stops.len() - 1) as f32;
    let i = ((t * seg).floor() as usize).min(g.stops.len() - 2);
    let f = t * seg - i as f32;
    let (a, b) = (rgb(g.stops[i]), rgb(g.stops[i + 1]));
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * f)
}

fn wallpaper_at(w: &Wallpaper, u: f32, v: f32) -> [f32; 3] {
    let u = u + w.warp * (v * 6.0 + 1.0).sin() + w.warp * 0.5 * (v * 13.0).sin();
    let v = v + w.warp * (u * 5.0 + 2.0).sin();
    let mut sum = rgb(w.base).map(|c| c * 0.04);
    let mut total = 0.04;
    for &(x, y, r, col) in w.blobs {
        let d2 = (u - x).powi(2) + (v - y).powi(2);
        let wt = (-d2 / (2.0 * (r * 0.55).powi(2))).exp();
        let c = rgb(col);
        for k in 0..3 {
            sum[k] += c[k] * wt;
        }
        total += wt;
    }
    sum.map(|c| c / total)
}

/// Bilinear sample of an image scaled to cover `ow` x `oh`.
fn image_at(img: &Img, x: u32, y: u32, ow: u32, oh: u32) -> [f32; 3] {
    let scale = (ow as f32 / img.w as f32).max(oh as f32 / img.h as f32);
    let sx = ((x as f32 + 0.5 - ow as f32 / 2.0) / scale + img.w as f32 / 2.0 - 0.5).clamp(0.0, img.w as f32 - 1.0);
    let sy = ((y as f32 + 0.5 - oh as f32 / 2.0) / scale + img.h as f32 / 2.0 - 0.5).clamp(0.0, img.h as f32 - 1.0);
    let (x0, y0) = (sx.floor() as u32, sy.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(img.w - 1), (y0 + 1).min(img.h - 1));
    let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
    let px = |x: u32, y: u32, k: usize| img.rgba[((y * img.w + x) * 4) as usize + k] as f32;
    [0, 1, 2].map(|k| {
        let top = px(x0, y0, k) * (1.0 - fx) + px(x1, y0, k) * fx;
        let bot = px(x0, y1, k) * (1.0 - fx) + px(x1, y1, k) * fx;
        top * (1.0 - fy) + bot * fy
    })
}

/// Coverage (0..=1, anti-aliased) of a rounded rectangle at pixel `(x, y)`.
fn rounded_cover(x: u32, y: u32, w: u32, h: u32, r: f32) -> f32 {
    let (hx, hy) = (w as f32 / 2.0, h as f32 / 2.0);
    let r = r.min(hx).min(hy);
    let (px, py) = ((x as f32 + 0.5 - hx).abs(), (y as f32 + 0.5 - hy).abs());
    let (qx, qy) = (px - (hx - r), py - (hy - r));
    let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
    (0.5 - d).clamp(0.0, 1.0)
}

/// Run `f(row, pixels_of_row)` over the rows of an RGBA buffer on several threads.
fn par_rows(buf: &mut [u8], w: u32, f: impl Fn(u32, &mut [u8]) + Sync) {
    let row_len = (w * 4) as usize;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let rows_per = (buf.len() / row_len).div_ceil(threads).max(1);
    std::thread::scope(|s| {
        for (i, chunk) in buf.chunks_mut(rows_per * row_len).enumerate() {
            let f = &f;
            s.spawn(move || {
                for (j, row) in chunk.chunks_exact_mut(row_len).enumerate() {
                    f((i * rows_per + j) as u32, row);
                }
            });
        }
    });
}

/// Three box passes along each axis, close enough to a Gaussian; zero outside the buffer.
fn blur(buf: &mut [f32], w: usize, h: usize, r: usize) {
    let mut src = vec![0.0f32; w.max(h)];
    let mut out = vec![0.0f32; w.max(h)];
    for _ in 0..3 {
        for y in 0..h {
            src[..w].copy_from_slice(&buf[y * w..(y + 1) * w]);
            box_line(&src[..w], &mut out[..w], r);
            buf[y * w..(y + 1) * w].copy_from_slice(&out[..w]);
        }
        for x in 0..w {
            for y in 0..h {
                src[y] = buf[y * w + x];
            }
            box_line(&src[..h], &mut out[..h], r);
            for y in 0..h {
                buf[y * w + x] = out[y];
            }
        }
    }
}

fn box_line(src: &[f32], out: &mut [f32], r: usize) {
    let n = src.len();
    let norm = 1.0 / (2 * r + 1) as f32;
    let mut sum: f32 = src[..r.min(n)].iter().sum();
    for i in 0..n {
        if i + r < n {
            sum += src[i + r];
        }
        out[i] = sum * norm;
        if i >= r {
            sum -= src[i - r];
        }
    }
}

impl Style {
    /// `none`, `color:#rrggbb`, `gradient:<name>`, `wallpaper:<name>` or `image:<path>` followed by
    /// `;padding=N;radius=N;shadow=N;aspect=A:B|auto`; the form kept in the settings file.
    pub fn parse(spec: &str) -> Option<Self> {
        let mut s = Self::default();
        for part in spec.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            let (k, v) = part.split_once('=').unwrap_or(("bg", part));
            match k {
                "bg" => s.background = parse_background(v)?,
                "padding" => s.padding = v.parse::<u32>().ok()?.min(MAX_PADDING),
                "radius" => s.radius = v.parse::<u32>().ok()?.min(MAX_RADIUS),
                "shadow" => s.shadow = v.parse::<u32>().ok()?.min(100),
                "aspect" => {
                    s.aspect = if v == "auto" {
                        Aspect::Auto
                    } else {
                        let (a, b) = v.split_once(':')?;
                        let (a, b) = (a.parse::<u32>().ok()?, b.parse::<u32>().ok()?);
                        if a == 0 || b == 0 {
                            return None;
                        }
                        Aspect::Ratio(a, b)
                    }
                }
                _ => return None,
            }
        }
        Some(s)
    }

    #[allow(dead_code)]
    pub fn to_spec(&self) -> String {
        let bg = match &self.background {
            Background::None => "none".to_string(),
            Background::Color([r, g, b]) => format!("color:#{r:02x}{g:02x}{b:02x}"),
            Background::Gradient(i) => format!("gradient:{}", GRADIENTS[*i].name),
            Background::Wallpaper(i) => format!("wallpaper:{}", WALLPAPERS[*i].name),
            Background::Image(p) => format!("image:{}", p.display()),
        };
        let aspect = match self.aspect {
            Aspect::Auto => "auto".to_string(),
            Aspect::Ratio(a, b) => format!("{a}:{b}"),
        };
        format!("bg={bg};padding={};radius={};shadow={};aspect={aspect}", self.padding, self.radius, self.shadow)
    }

    /// Output size and where a `content`-sized capture sits in it. The output is never smaller than the
    /// capture plus its padding; a ratio widens whichever side is short.
    pub fn layout(&self, (cw, ch): (u32, u32)) -> Layout {
        let pad = (self.padding as f32 / 100.0 * cw.max(ch) as f32).round() as u32;
        let (mut w, mut h) = (cw + 2 * pad, ch + 2 * pad);
        if let Aspect::Ratio(a, b) = self.aspect {
            let r = a as f32 / b as f32;
            if (w as f32 / h as f32) < r {
                w = (h as f32 * r).ceil() as u32;
            } else {
                h = (w as f32 / r).ceil() as u32;
            }
        }
        // even sizes and offsets: video encoders need them
        let (w, h) = ((w + 1) & !1, (h + 1) & !1);
        Layout {
            out: (w, h),
            x: ((w - cw) / 2) & !1,
            y: ((h - ch) / 2) & !1,
            w: cw,
            h: ch,
            radius: self.radius as f32 / 100.0 * cw.min(ch) as f32,
        }
    }

    /// The background and shadow at output size, straight alpha, with the capture's area still in place.
    /// `silhouette` is the capture itself, when its own transparency should shape the shadow.
    pub fn backdrop(&self, l: &Layout, silhouette: Option<&Img>, image: Option<&Img>) -> Vec<u8> {
        let (ow, oh) = l.out;
        let mut buf = vec![0u8; (ow * oh * 4) as usize];
        let aspect = ow as f32 / oh as f32;
        let bg = &self.background;
        if *bg != Background::None {
            par_rows(&mut buf, ow, |y, row| {
                for x in 0..ow {
                    let (u, v) = ((x as f32 + 0.5) / ow as f32, (y as f32 + 0.5) / oh as f32);
                    let c = match bg {
                        Background::None => unreachable!(),
                        Background::Color(c) => c.map(f32::from),
                        Background::Gradient(i) => gradient_at(&GRADIENTS[*i], u, v, aspect),
                        Background::Wallpaper(i) => wallpaper_at(&WALLPAPERS[*i], u, v),
                        Background::Image(_) => match image {
                            Some(img) => image_at(img, x, y, ow, oh),
                            None => DARK.map(f32::from),
                        },
                    };
                    // a little noise hides banding in smooth gradients
                    let n = noise(x, y) - 0.5;
                    let px = &mut row[(x * 4) as usize..(x * 4 + 4) as usize];
                    for k in 0..3 {
                        px[k] = (c[k] + n).round().clamp(0.0, 255.0) as u8;
                    }
                    px[3] = 255;
                }
            });
        }
        if self.shadow > 0 {
            self.cast_shadow(&mut buf, l, silhouette);
        }
        buf
    }

    fn cast_shadow(&self, buf: &mut [u8], l: &Layout, silhouette: Option<&Img>) {
        let (ow, oh) = l.out;
        let size = l.w.max(l.h) as f32;
        let amount = self.shadow as f32 / 100.0;
        let sigma = size * 0.03 * (0.6 + 0.8 * amount);
        let offset = size * 0.018 * (0.5 + amount);
        let opacity = 0.7 * amount;

        // work on a small grid: a blurred shadow has no detail to lose
        let s = ow.max(oh).div_ceil(480).max(1);
        let (sw, sh) = (ow.div_ceil(s) as usize, oh.div_ceil(s) as usize);
        let mut mask = vec![0.0f32; sw * sh];
        for sy in 0..sh {
            for sx in 0..sw {
                let cx = (sx as f32 + 0.5) * s as f32 - l.x as f32;
                let cy = (sy as f32 + 0.5) * s as f32 - offset - l.y as f32;
                if cx < 0.0 || cy < 0.0 || cx >= l.w as f32 || cy >= l.h as f32 {
                    continue;
                }
                let (ix, iy) = (cx as u32, cy as u32);
                let mut a = rounded_cover(ix, iy, l.w, l.h, l.radius);
                if let Some(img) = silhouette {
                    a *= img.rgba[((iy * img.w + ix) * 4 + 3) as usize] as f32 / 255.0;
                }
                mask[sy * sw + sx] = a;
            }
        }
        blur(&mut mask, sw, sh, ((sigma / s as f32).round() as usize).max(1));

        par_rows(buf, ow, |y, row| {
            let fy = ((y as f32 + 0.5) / s as f32 - 0.5).clamp(0.0, sh as f32 - 1.0);
            let (y0, ty) = (fy.floor() as usize, fy.fract());
            let y1 = (y0 + 1).min(sh - 1);
            for x in 0..ow {
                let fx = ((x as f32 + 0.5) / s as f32 - 0.5).clamp(0.0, sw as f32 - 1.0);
                let (x0, tx) = (fx.floor() as usize, fx.fract());
                let x1 = (x0 + 1).min(sw - 1);
                let top = mask[y0 * sw + x0] * (1.0 - tx) + mask[y0 * sw + x1] * tx;
                let bot = mask[y1 * sw + x0] * (1.0 - tx) + mask[y1 * sw + x1] * tx;
                let a = (top * (1.0 - ty) + bot * ty) * opacity;
                if a <= 0.0 {
                    continue;
                }
                let px = &mut row[(x * 4) as usize..(x * 4 + 4) as usize];
                let ba = px[3] as f32 / 255.0;
                // black shadow over the background
                let out_a = a + ba * (1.0 - a);
                for c in &mut px[..3] {
                    *c = (*c as f32 * ba * (1.0 - a) / out_a).round() as u8;
                }
                px[3] = (out_a * 255.0).round() as u8;
            }
        });
    }

    /// Make the capture's rounded rectangle see-through in a backdrop, so that video laid underneath
    /// shows through it. Edge pixels keep the background in proportion, which anti-aliases the corners.
    pub fn punch_hole(&self, backdrop: &mut [u8], l: &Layout) {
        for y in 0..l.h {
            for x in 0..l.w {
                let cover = rounded_cover(x, y, l.w, l.h, l.radius);
                let i = (((l.y + y) * l.out.0 + l.x + x) * 4 + 3) as usize;
                backdrop[i] = (backdrop[i] as f32 * (1.0 - cover)).round() as u8;
            }
        }
    }

    /// Put a captured picture on its styled background.
    pub fn compose(&self, src: &Img, image: Option<&Img>) -> Img {
        let l = self.layout((src.w, src.h));
        let mut out = self.backdrop(&l, Some(src), image);
        let ow = l.out.0;
        par_rows(&mut out[((l.y * ow) * 4) as usize..(((l.y + l.h) * ow) * 4) as usize], ow, |row_i, row| {
            let y = row_i;
            for x in 0..l.w {
                let s = &src.rgba[((y * src.w + x) * 4) as usize..][..4];
                let ca = rounded_cover(x, y, l.w, l.h, l.radius) * s[3] as f32 / 255.0;
                if ca <= 0.0 {
                    continue;
                }
                let px = &mut row[((l.x + x) * 4) as usize..((l.x + x) * 4 + 4) as usize];
                let ba = px[3] as f32 / 255.0;
                let ra = ca + ba * (1.0 - ca);
                for k in 0..3 {
                    px[k] = ((s[k] as f32 * ca + px[k] as f32 * ba * (1.0 - ca)) / ra).round().clamp(0.0, 255.0) as u8;
                }
                px[3] = (ra * 255.0).round() as u8;
            }
        });
        Img { w: ow, h: l.out.1, rgba: out }
    }
}

fn parse_background(v: &str) -> Option<Background> {
    if v == "none" {
        return Some(Background::None);
    }
    let (kind, arg) = v.split_once(':')?;
    match kind {
        "color" => crate::capture::parse_color(arg).map(|c| Background::Color(c.map(|c| (c * 255.0).round() as u8))),
        "gradient" => GRADIENTS.iter().position(|g| g.name == arg).map(Background::Gradient),
        "wallpaper" => WALLPAPERS.iter().position(|w| w.name == arg).map(Background::Wallpaper),
        "image" => Some(Background::Image(arg.into())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Img {
        Img { w, h, rgba: rgba.repeat((w * h) as usize) }
    }

    #[test]
    fn spec_round_trips() {
        let s = Style {
            background: Background::Wallpaper(2),
            padding: 14,
            radius: 9,
            shadow: 30,
            aspect: Aspect::Ratio(16, 9),
        };
        assert_eq!(Style::parse(&s.to_spec()), Some(s));
        let c = Style { background: Background::Color([1, 2, 255]), ..Style::default() };
        assert_eq!(Style::parse(&c.to_spec()), Some(c));
        assert_eq!(Style::parse("bg=sparkles"), None);
        assert_eq!(Style::parse("aspect=0:3"), None);
        assert_eq!(Style::parse("padding=999").unwrap().padding, MAX_PADDING);
    }

    #[test]
    fn auto_layout_adds_padding_and_centres() {
        let s = Style { padding: 10, aspect: Aspect::Auto, ..Style::default() };
        let l = s.layout((720, 1600));
        assert_eq!(l.out, (720 + 320, 1600 + 320));
        assert_eq!((l.x, l.y), (160, 160));
    }

    #[test]
    fn ratio_widens_the_short_side_and_keeps_even_sizes() {
        let s = Style { padding: 5, aspect: Aspect::Ratio(1, 1), ..Style::default() };
        let l = s.layout((720, 1600));
        assert_eq!(l.out.0, l.out.1);
        assert!(l.out.1 >= 1600 && l.out.0.is_multiple_of(2) && l.x.is_multiple_of(2) && l.y.is_multiple_of(2));
        let wide = Style { aspect: Aspect::Ratio(16, 9), ..s }.layout((720, 1600));
        let r = wide.out.0 as f32 / wide.out.1 as f32;
        assert!((r - 16.0 / 9.0).abs() < 0.01);
    }

    #[test]
    fn rounded_corners_cut_the_corner_but_not_the_middle() {
        assert!(rounded_cover(0, 0, 100, 100, 30.0) < 0.01);
        assert!(rounded_cover(50, 50, 100, 100, 30.0) > 0.99);
        assert!(rounded_cover(0, 50, 100, 100, 30.0) > 0.99);
    }

    #[test]
    fn composed_picture_has_background_around_and_content_inside() {
        let s = Style {
            background: Background::Color([255, 0, 0]),
            padding: 20,
            radius: 0,
            shadow: 0,
            aspect: Aspect::Auto,
        };
        let out = s.compose(&solid(100, 100, [0, 0, 255, 255]), None);
        assert_eq!((out.w, out.h), (140, 140));
        let at = |x: u32, y: u32| out.rgba[((y * out.w + x) * 4) as usize..][..4].to_vec();
        assert_eq!(at(70, 70), vec![0, 0, 255, 255]);
        let corner = at(2, 2);
        assert!(corner[0] > 250 && corner[2] < 5 && corner[3] == 255);
    }

    #[test]
    fn shadow_darkens_below_more_than_above() {
        let s = Style {
            background: Background::Color([255, 255, 255]),
            padding: 25,
            radius: 0,
            shadow: 100,
            aspect: Aspect::Auto,
        };
        let out = s.compose(&solid(200, 200, [0, 0, 255, 255]), None);
        let l = s.layout((200, 200));
        let at = |x: u32, y: u32| out.rgba[((y * out.w + x) * 4) as usize] as i32;
        let (mid, below, above) = (l.x + 100, l.y + l.h + 8, l.y - 8);
        assert!(at(mid, below) < at(mid, above), "shadow should sit lower than the capture");
        assert!(at(mid, below) < 250);
    }

    #[test]
    fn no_background_keeps_the_surroundings_transparent() {
        let s = Style { background: Background::None, padding: 20, shadow: 0, ..Style::default() };
        let out = s.compose(&solid(100, 100, [0, 255, 0, 255]), None);
        assert_eq!(out.rgba[3], 0);
    }

    #[test]
    fn every_built_in_background_renders() {
        let l = Style::default().layout((120, 160));
        for i in 0..GRADIENTS.len() {
            let s = Style { background: Background::Gradient(i), shadow: 0, ..Style::default() };
            assert_eq!(s.backdrop(&l, None, None).len(), (l.out.0 * l.out.1 * 4) as usize);
        }
        for i in 0..WALLPAPERS.len() {
            let s = Style { background: Background::Wallpaper(i), shadow: 0, ..Style::default() };
            assert!(s.backdrop(&l, None, None).chunks(4).all(|p| p[3] == 255));
        }
    }

    #[test]
    fn hole_is_transparent_inside_and_opaque_outside() {
        let s = Style { background: Background::Color([9, 9, 9]), radius: 0, shadow: 0, ..Style::default() };
        let l = s.layout((100, 100));
        let mut bd = s.backdrop(&l, None, None);
        s.punch_hole(&mut bd, &l);
        let a = |x: u32, y: u32| bd[((y * l.out.0 + x) * 4 + 3) as usize];
        assert_eq!(a(l.x + 50, l.y + 50), 0);
        assert_eq!(a(1, 1), 255);
    }
}
