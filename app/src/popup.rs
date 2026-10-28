//! The right-click menu drawn in the window (Linux, or `HANDSET_DRAWN_MENU=1`), from the same entries as the
//! native menu. Everything here is in window pixels; the panels are rasterized on the CPU and drawn as
//! window-only layers, so screenshots and recordings never contain them.

use crate::connect::{Weight, font, measure};
use crate::menu::Entry;
use crate::overlay::Pill;
use ab_glyph::{Font, PxScale, ScaleFont, point};

// Sizes in points; multiplied by the window's scale factor.
const TEXT: f32 = 13.0;
const ROW: f32 = 22.0;
const SEPARATOR: f32 = 9.0;
const PAD_Y: f32 = 5.0;
const PAD_X: f32 = 10.0;
/// Room left of every label for a check mark, so labels line up whether or not they have one.
const CHECK_W: f32 = 18.0;
const ARROW_W: f32 = 12.0;
const GAP: f32 = 24.0;
const MIN_W: f32 = 170.0;
const MAX_W: f32 = 320.0;
/// The nearest a panel comes to the window's edge.
const MARGIN: f32 = 6.0;
const RADIUS: f32 = 8.0;
const HOVER_INSET: f32 = 4.0;
/// The strip at the top or bottom of a panel that scrolls, while there is more that way.
const SCROLL_STRIP: f32 = 14.0;

const BG: [u8; 3] = [20, 20, 24];
const RIM: [u8; 3] = [96, 96, 110];
const HIGHLIGHT: [u8; 3] = [10, 104, 224];
const LABEL: [u8; 3] = [245, 245, 247];
const DIM: [u8; 3] = [124, 124, 134];
const SHORTCUT: [u8; 3] = [168, 168, 178];
const RULE: [u8; 3] = [64, 64, 74];

/// Where the menu opens.
#[derive(Debug, Clone, Copy)]
pub enum Anchor {
    /// At the pointer (a right-click).
    Point(f32, f32),
    /// Beside a rectangle (x, y, w, h), such as the toolbar's More button.
    Beside([f32; 4]),
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// Still open (it may look different now).
    Open,
    Closed,
    /// An item was chosen: its menu id. The menu is closed.
    Chose(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
}

struct Row {
    /// Top, from the top of the panel's content.
    y: f32,
    h: f32,
}

struct Panel {
    /// Which submenu this is: entry indices from the top of the menu down. Empty for the top.
    path: Vec<usize>,
    /// x, y, w, h in window pixels, whole numbers so the image lands on pixels.
    rect: [f32; 4],
    rows: Vec<Row>,
    content_h: f32,
    scroll: f32,
    hover: Option<usize>,
    /// The row whose submenu is the next panel.
    open: Option<usize>,
}

impl Panel {
    fn max_scroll(&self) -> f32 {
        (self.content_h - self.rect[3]).max(0.0)
    }

    fn contains(&self, x: f32, y: f32) -> bool {
        let [rx, ry, rw, rh] = self.rect;
        x >= rx && x < rx + rw && y >= ry && y < ry + rh
    }

    /// A row's rectangle in window pixels, where it is now (it may be scrolled out of sight).
    fn row_rect(&self, i: usize) -> [f32; 4] {
        let r = &self.rows[i];
        [self.rect[0], self.rect[1] + r.y - self.scroll, self.rect[2], r.h]
    }
}

pub struct Popup {
    entries: Vec<Entry>,
    /// Window pixels per point.
    k: f32,
    win: (f32, f32),
    /// The open panels: the top one first, then each open submenu.
    panels: Vec<Panel>,
    last: Option<(f32, f32)>,
}

fn selectable(e: &Entry) -> bool {
    match e {
        Entry::Item(i) => i.enabled && i.id.is_some(),
        Entry::Submenu { .. } | Entry::About => true,
        Entry::Separator => false,
    }
}

/// `v` moved as little as needed for `size` to fit between `lo` and `hi`; at `lo` when it cannot fit.
fn fit(v: f32, size: f32, lo: f32, hi: f32) -> f32 {
    v.min(hi - size).max(lo)
}

/// Top left for a panel of `size` opening at a point: right of and below it, flipped to the other side of the
/// point where it would leave the window, and pushed inside when neither side has room.
pub fn place_at(p: (f32, f32), size: (f32, f32), win: (f32, f32), m: f32) -> (f32, f32) {
    let along = |p: f32, s: f32, w: f32| {
        if p + s <= w - m {
            p
        } else if p - s >= m {
            p - s
        } else {
            fit(p, s, m, w - m)
        }
    };
    (along(p.0, size.0, win.0), along(p.1, size.1, win.1))
}

/// Top left for a panel of `size` beside `a` (x, y, w, h): to its right with its top at `a`'s top, else to its
/// left, else on whichever side has more room and pushed inside; moved up where it would run off the bottom.
pub fn place_beside(a: [f32; 4], size: (f32, f32), win: (f32, f32), m: f32) -> (f32, f32) {
    let right = a[0] + a[2];
    let x = if right + size.0 <= win.0 - m {
        right
    } else if a[0] - size.0 >= m {
        a[0] - size.0
    } else if win.0 - right > a[0] {
        fit(right, size.0, m, win.0 - m)
    } else {
        fit(a[0] - size.0, size.0, m, win.0 - m)
    };
    (x, fit(a[1], size.1, m, win.1 - m))
}

/// Is `p` inside the triangle `a`, `b`, `c`?
fn in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let side = |u: (f32, f32), v: (f32, f32)| (v.0 - u.0) * (p.1 - u.1) - (v.1 - u.1) * (p.0 - u.0);
    let (d1, d2, d3) = (side(a, b), side(b, c), side(c, a));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

impl Popup {
    /// `win` is the window's size in pixels and `k` its scale factor.
    pub fn new(entries: Vec<Entry>, anchor: Anchor, win: (f32, f32), k: f32) -> Self {
        let mut p = Popup { entries, k, win, panels: Vec::new(), last: None };
        let (rows, w, content_h) = p.measure(&[]);
        let h = content_h.min(win.1 - 2.0 * p.px(MARGIN)).max(0.0);
        let m = p.px(MARGIN);
        let (x, y) = match anchor {
            Anchor::Point(x, y) => place_at((x, y), (w, h), win, m),
            Anchor::Beside(a) => place_beside(a, (w, h), win, m),
        };
        p.panels.push(Panel {
            path: Vec::new(),
            rect: [x.round(), y.round(), w, h],
            rows,
            content_h,
            scroll: 0.0,
            hover: None,
            open: None,
        });
        p
    }

    /// How tall an item is, in window pixels: what one line of a mouse wheel scrolls by.
    pub fn row_height(&self) -> f32 {
        self.px(ROW)
    }

    fn px(&self, pt: f32) -> f32 {
        pt * self.k
    }

    fn entries_at(&self, path: &[usize]) -> &[Entry] {
        let mut list = self.entries.as_slice();
        for &i in path {
            match &list[i] {
                Entry::Submenu { items, .. } => list = items,
                _ => return &[],
            }
        }
        list
    }

    /// Rows, width (whole pixels) and content height of the panel for `path`.
    fn measure(&self, path: &[usize]) -> (Vec<Row>, f32, f32) {
        let text = self.px(TEXT);
        let mut rows = Vec::new();
        let mut y = self.px(PAD_Y);
        let mut widest: f32 = 0.0;
        for e in self.entries_at(path) {
            let (h, w) = match e {
                Entry::Separator => (self.px(SEPARATOR), 0.0),
                Entry::About => (self.px(ROW), measure(Weight::Regular, text, "About Handset")),
                Entry::Submenu { label, .. } => {
                    (self.px(ROW), measure(Weight::Regular, text, label) + self.px(GAP + ARROW_W))
                }
                Entry::Item(i) => {
                    let shortcut = i.shortcut_text().map_or(0.0, |s| self.px(GAP) + measure(Weight::Regular, text, &s));
                    (self.px(ROW), measure(Weight::Regular, text, &i.label) + shortcut)
                }
            };
            rows.push(Row { y, h });
            y += h;
            widest = widest.max(w);
        }
        let w = (widest + self.px(CHECK_W + 2.0 * PAD_X))
            .clamp(self.px(MIN_W), self.px(MAX_W))
            .min(self.win.0 - 2.0 * self.px(MARGIN))
            .ceil();
        (rows, w, y + self.px(PAD_Y))
    }

    /// Open the submenu of row `row` in panel `p`, closing anything deeper.
    fn open_sub(&mut self, p: usize, row: usize) {
        self.panels.truncate(p + 1);
        let mut path = self.panels[p].path.clone();
        path.push(row);
        let (rows, w, content_h) = self.measure(&path);
        let m = self.px(MARGIN);
        let h = content_h.min(self.win.1 - 2.0 * m).max(0.0);
        // the first row lines up with the row it opened from
        let mut a = self.panels[p].row_rect(row);
        a[1] -= self.px(PAD_Y);
        let (x, y) = place_beside(a, (w, h), self.win, m);
        self.panels[p].open = Some(row);
        self.panels.push(Panel {
            path,
            rect: [x.round(), y.round(), w, h],
            rows,
            content_h,
            scroll: 0.0,
            hover: None,
            open: None,
        });
    }

    fn close_after(&mut self, p: usize) {
        self.panels.truncate(p + 1);
        self.panels[p].open = None;
    }

    /// The panel under (`x`, `y`), the top one first, and the row there: none on the padding or on a scroll
    /// strip.
    fn hit(&self, x: f32, y: f32) -> Option<(usize, Option<usize>)> {
        let p = self.panels.iter().rposition(|p| p.contains(x, y))?;
        let panel = &self.panels[p];
        if self.strip_at(p, y).is_some() {
            return Some((p, None));
        }
        let cy = y - panel.rect[1] + panel.scroll;
        Some((p, panel.rows.iter().position(|r| cy >= r.y && cy < r.y + r.h)))
    }

    /// -1 on the top scroll strip of panel `p`, 1 on the bottom one, while there is more that way.
    fn strip_at(&self, p: usize, y: f32) -> Option<f32> {
        let panel = &self.panels[p];
        let strip = self.px(SCROLL_STRIP);
        if panel.scroll > 0.0 && y < panel.rect[1] + strip {
            Some(-1.0)
        } else if panel.scroll < panel.max_scroll() && y >= panel.rect[1] + panel.rect[3] - strip {
            Some(1.0)
        } else {
            None
        }
    }

    /// Is the pointer, coming from `from`, heading for the submenu that panel `p` has open? Then the rows it
    /// crosses on the way must not take the submenu away.
    fn heading_for_sub(&self, p: usize, from: (f32, f32), to: (f32, f32)) -> bool {
        let Some(sub) = self.panels.get(p + 1) else { return false };
        let [sx, sy, sw, sh] = sub.rect;
        let near = if sx >= self.panels[p].rect[0] { sx } else { sx + sw };
        from != to && in_triangle(to, from, (near, sy), (near, sy + sh))
    }

    /// Follow the pointer: highlight the row under it and open its submenu. True when something changed.
    pub fn pointer_moved(&mut self, x: f32, y: f32) -> bool {
        let from = self.last.replace((x, y));
        let Some((p, row)) = self.hit(x, y) else {
            // off the menu: keep the open submenus, drop the highlight where nothing hangs off it
            let mut changed = false;
            for panel in &mut self.panels {
                if panel.hover.is_some() && panel.hover != panel.open {
                    panel.hover = panel.open;
                    changed = true;
                }
            }
            return changed;
        };
        if from.is_some_and(|f| self.heading_for_sub(p, f, (x, y))) {
            return false;
        }
        let row = row.filter(|&r| selectable(&self.entries_at(&self.panels[p].path)[r]));
        let was = (self.panels.len(), self.panels[p].hover, self.panels[p].open);
        self.panels[p].hover = row;
        match row {
            Some(r) if matches!(self.entries_at(&self.panels[p].path)[r], Entry::Submenu { .. }) => {
                if self.panels[p].open != Some(r) {
                    self.open_sub(p, r);
                }
            }
            _ => self.close_after(p),
        }
        was != (self.panels.len(), self.panels[p].hover, self.panels[p].open)
    }

    /// A click (left or right) at (`x`, `y`).
    pub fn click(&mut self, x: f32, y: f32) -> Outcome {
        let Some((p, row)) = self.hit(x, y) else {
            self.panels.clear();
            return Outcome::Closed;
        };
        if let Some(dir) = self.strip_at(p, y) {
            self.scroll_panel(p, dir * 3.0 * self.px(ROW));
            return Outcome::Open;
        }
        match row {
            Some(r) => self.activate(p, r, false),
            None => Outcome::Open,
        }
    }

    /// Choose row `r` of panel `p`: an item closes the menu, a submenu opens.
    fn activate(&mut self, p: usize, r: usize, by_key: bool) -> Outcome {
        let chosen = match &self.entries_at(&self.panels[p].path)[r] {
            Entry::Item(i) if i.enabled => i.id.clone(),
            Entry::About => Some("about".to_string()),
            Entry::Submenu { .. } => {
                self.panels[p].hover = Some(r);
                if self.panels[p].open != Some(r) {
                    self.open_sub(p, r);
                }
                if by_key {
                    self.step(p + 1, 1);
                }
                return Outcome::Open;
            }
            _ => None,
        };
        match chosen {
            Some(id) => {
                self.panels.clear();
                Outcome::Chose(id)
            }
            None => Outcome::Open,
        }
    }

    /// Scroll the panel under the pointer by `dy` pixels (positive shows what is further down).
    pub fn scroll(&mut self, x: f32, y: f32, dy: f32) -> bool {
        let Some((p, _)) = self.hit(x, y) else { return false };
        let changed = self.scroll_panel(p, dy);
        if changed {
            self.last = None;
            self.pointer_moved(x, y);
        }
        changed
    }

    fn scroll_panel(&mut self, p: usize, dy: f32) -> bool {
        let panel = &mut self.panels[p];
        let to = (panel.scroll + dy).clamp(0.0, panel.max_scroll());
        if to == panel.scroll {
            return false;
        }
        panel.scroll = to;
        // a submenu hangs off a row that has moved
        self.close_after(p);
        true
    }

    /// Move the highlight in panel `p` by one selectable row in direction `dir`, wrapping, and scroll it into view.
    fn step(&mut self, p: usize, dir: isize) {
        let n = self.panels[p].rows.len() as isize;
        let entries = self.entries_at(&self.panels[p].path);
        let start = self.panels[p].hover.map_or(if dir > 0 { -1 } else { n }, |h| h as isize);
        let next = (1..=n).map(|d| (start + d * dir).rem_euclid(n) as usize).find(|&i| selectable(&entries[i]));
        let Some(r) = next else { return };
        self.close_after(p);
        let strip = self.px(SCROLL_STRIP);
        let panel = &mut self.panels[p];
        panel.hover = Some(r);
        let row = &panel.rows[r];
        let max = panel.max_scroll();
        if row.y - strip < panel.scroll {
            panel.scroll = (row.y - strip).clamp(0.0, max);
        } else if row.y + row.h + strip > panel.scroll + panel.rect[3] {
            panel.scroll = (row.y + row.h + strip - panel.rect[3]).clamp(0.0, max);
        }
    }

    pub fn key(&mut self, key: Key) -> Outcome {
        // the deepest panel with a highlight, else the deepest one
        let p =
            self.panels.iter().rposition(|p| p.hover.is_some() && p.open.is_none()).unwrap_or(self.panels.len() - 1);
        match key {
            Key::Escape => {
                self.panels.clear();
                return Outcome::Closed;
            }
            Key::Down => self.step(p, 1),
            Key::Up => self.step(p, -1),
            Key::Left if p > 0 => self.close_after(p - 1),
            Key::Right | Key::Enter => {
                if let Some(r) = self.panels[p].hover {
                    let sub = matches!(self.entries_at(&self.panels[p].path)[r], Entry::Submenu { .. });
                    if key == Key::Enter || sub {
                        return self.activate(p, r, true);
                    }
                }
            }
            Key::Left => {}
        }
        Outcome::Open
    }

    /// Each open panel as an image and the rectangle (x, y, w, h) it goes in, in window pixels.
    pub fn raster(&self) -> Vec<(Pill, [f32; 4])> {
        (0..self.panels.len()).map(|p| (self.raster_panel(p), self.panels[p].rect)).collect()
    }

    fn raster_panel(&self, p: usize) -> Pill {
        let panel = &self.panels[p];
        let (w, h) = (panel.rect[2] as u32, panel.rect[3] as u32);
        let mut c = Canvas { w, h, rgba: vec![0; (w * h * 4) as usize] };
        let (pw, ph) = (w as f32, h as f32);
        let radius = self.px(RADIUS);
        let rim = self.k.max(1.0);
        // the panel: rounded, with a thin lighter rim so it reads over dark content
        c.shape(|x, y| rrect(x, y, [0.0, 0.0, pw, ph], radius), |d| if d > -rim { (RIM, 250) } else { (BG, 250) });

        let text = self.px(TEXT);
        let pad = self.px(PAD_X);
        let entries = self.entries_at(&panel.path);
        for (i, (e, row)) in entries.iter().zip(&panel.rows).enumerate() {
            let top = row.y - panel.scroll;
            if top + row.h < 0.0 || top > ph {
                continue;
            }
            let mid = top + row.h / 2.0;
            if let Entry::Separator = e {
                c.fill([pad, mid.floor(), pw - 2.0 * pad, self.k.max(1.0).round()], RULE);
                continue;
            }
            let lit = panel.hover == Some(i) || panel.open == Some(i);
            if lit {
                let inset = self.px(HOVER_INSET);
                let r = [inset, top, pw - 2.0 * inset, row.h];
                c.shape(|x, y| rrect(x, y, r, self.px(5.0)), |_| (HIGHLIGHT, 255));
            }
            let (label, shortcut, enabled, check, arrow) = match e {
                Entry::Item(it) => (it.label.as_str(), it.shortcut_text(), it.enabled, it.check, false),
                Entry::Submenu { label, .. } => (label.as_str(), None, true, None, true),
                _ => ("About Handset", None, true, None, false),
            };
            let colour = if enabled { LABEL } else { DIM };
            if check == Some(true) {
                let x0 = pad + self.px(2.0);
                let t = self.px(1.6);
                let (a, b, d) =
                    ((x0, mid), (x0 + self.px(3.5), mid + self.px(3.5)), (x0 + self.px(10.0), mid - self.px(4.0)));
                c.shape(|x, y| seg(x, y, a, b).min(seg(x, y, b, d)) - t / 2.0, |_| (colour, 255));
            }
            let mut right = pw - pad;
            if arrow {
                let t = self.px(1.5);
                let (ax, s) = (right - self.px(2.0), self.px(3.5));
                let (a, b, d) = ((ax - s, mid - 2.0 * s / 1.6), (ax, mid), (ax - s, mid + 2.0 * s / 1.6));
                c.shape(|x, y| seg(x, y, a, b).min(seg(x, y, b, d)) - t / 2.0, |_| (colour, 255));
                right -= self.px(ARROW_W);
            }
            if let Some(s) = &shortcut {
                let sw = measure(Weight::Regular, text, s);
                c.text(right - sw, mid, text, if enabled && !lit { SHORTCUT } else { colour }, s);
                right -= sw + self.px(GAP);
            }
            let x = pad + self.px(CHECK_W);
            c.text(x, mid, text, colour, &ellipsize(label, text, right - x));
        }

        // more above or below: a strip with a chevron, over the rows
        let strip = self.px(SCROLL_STRIP);
        for (more, y0, up) in [(panel.scroll > 0.0, 0.0, true), (panel.scroll < panel.max_scroll(), ph - strip, false)]
        {
            if !more {
                continue;
            }
            let band = [rim, y0.max(rim), pw - 2.0 * rim, strip - rim];
            c.shape(|x, y| rrect(x, y, band, radius - rim).max(rrect(x, y, [0.0, 0.0, pw, ph], radius)), |_| (BG, 250));
            let (cx, cy, s) = (pw / 2.0, y0 + strip / 2.0, self.px(3.5));
            let dy = if up { -s / 2.0 } else { s / 2.0 };
            let (a, b, d) = ((cx - s, cy - dy), (cx, cy + dy), (cx + s, cy - dy));
            let t = self.px(1.5);
            c.shape(|x, y| seg(x, y, a, b).min(seg(x, y, b, d)) - t / 2.0, |_| (SHORTCUT, 255));
        }
        Pill { width: w, height: h, rgba: c.rgba }
    }
}

/// `text` cut short with an ellipsis so it is at most `max_w` pixels wide at size `px`.
fn ellipsize(text: &str, px: f32, max_w: f32) -> String {
    if measure(Weight::Regular, px, text) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    (0..chars.len())
        .rev()
        .map(|n| format!("{}\u{2026}", chars[..n].iter().collect::<String>().trim_end()))
        .find(|s| measure(Weight::Regular, px, s) <= max_w)
        .unwrap_or_default()
}

/// Signed distance from (x, y) to a rounded rectangle (x, y, w, h): negative inside.
fn rrect(x: f32, y: f32, r: [f32; 4], radius: f32) -> f32 {
    let (cx, cy, hw, hh) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, r[2] / 2.0, r[3] / 2.0);
    let radius = radius.min(hw).min(hh);
    let (qx, qy) = ((x - cx).abs() - hw + radius, (y - cy).abs() - hh + radius);
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - radius
}

/// Distance from (x, y) to the segment `a`-`b`.
fn seg(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (pa, ba) = ((x - a.0, y - a.1), (b.0 - a.0, b.1 - a.1));
    let t = ((pa.0 * ba.0 + pa.1 * ba.1) / (ba.0 * ba.0 + ba.1 * ba.1)).clamp(0.0, 1.0);
    ((pa.0 - ba.0 * t).powi(2) + (pa.1 - ba.1 * t).powi(2)).sqrt()
}

/// Straight-alpha RGBA pixels.
struct Canvas {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

impl Canvas {
    /// Lay `c` over the pixel at (x, y) with coverage `a`.
    fn blend(&mut self, x: i64, y: i64, c: [u8; 3], alpha: u8, a: f32) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 || a <= 0.0 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        let px = &mut self.rgba[i..i + 4];
        let src = a.min(1.0) * alpha as f32 / 255.0;
        let dst = px[3] as f32 / 255.0;
        let out = src + dst * (1.0 - src);
        if out <= 0.0 {
            return;
        }
        for (d, s) in px[..3].iter_mut().zip(c) {
            *d = ((s as f32 * src + *d as f32 * dst * (1.0 - src)) / out).round() as u8;
        }
        px[3] = (out * 255.0).round() as u8;
    }

    /// Fill where the distance `d(x, y)` is negative, anti-aliased, in the colour and alpha `look(d)` gives.
    fn shape(&mut self, d: impl Fn(f32, f32) -> f32, look: impl Fn(f32) -> ([u8; 3], u8)) {
        for y in 0..self.h {
            for x in 0..self.w {
                let dist = d(x as f32 + 0.5, y as f32 + 0.5);
                let cover = (0.5 - dist).clamp(0.0, 1.0);
                if cover > 0.0 {
                    let (c, a) = look(dist);
                    self.blend(x as i64, y as i64, c, a, cover);
                }
            }
        }
    }

    fn fill(&mut self, r: [f32; 4], c: [u8; 3]) {
        for y in r[1] as i64..(r[1] + r[3]) as i64 {
            for x in r[0] as i64..(r[0] + r[2]) as i64 {
                self.blend(x, y, c, 255, 1.0);
            }
        }
    }

    /// One line of Inter at `px`, its capitals centred on `mid`.
    fn text(&mut self, x: f32, mid: f32, px: f32, c: [u8; 3], text: &str) {
        let f = font(Weight::Regular);
        let scaled = f.as_scaled(PxScale::from(px));
        let baseline = (mid + px * 0.727 / 2.0).round();
        let mut caret = x;
        let mut prev = None;
        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(p) = prev {
                caret += scaled.kern(p, id);
            }
            if let Some(outline) =
                f.outline_glyph(id.with_scale_and_position(PxScale::from(px), point(caret, baseline)))
            {
                let b = outline.px_bounds();
                outline.draw(|gx, gy, a| self.blend(b.min.x as i64 + gx as i64, b.min.y as i64 + gy as i64, c, 255, a));
            }
            caret += scaled.h_advance(id);
            prev = Some(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{self, Item};

    fn it(id: &str, label: &str) -> Entry {
        Entry::Item(Item { id: Some(id.into()), label: label.into(), shortcut: None, enabled: true, check: None })
    }

    fn off(label: &str) -> Entry {
        Entry::Item(Item { id: None, label: label.into(), shortcut: None, enabled: false, check: None })
    }

    fn sample() -> Vec<Entry> {
        vec![
            Entry::Submenu { label: "Frame".into(), items: vec![it("frame:none", "None"), it("frame:pixel", "Pixel")] },
            Entry::Separator,
            it("shot", "Take Screenshot"),
            off("Nothing here"),
            Entry::Submenu {
                label: "Open App".into(),
                items: (0..60).map(|i| it(&format!("app:{i}"), "App")).collect(),
            },
            Entry::About,
            it("quit", "Quit"),
        ]
    }

    const WIN: (f32, f32) = (800.0, 1200.0);

    /// The middle of row `r` of panel `p`, in window pixels.
    fn mid(m: &Popup, p: usize, r: usize) -> (f32, f32) {
        let [x, y, w, h] = m.panels[p].row_rect(r);
        (x + w / 2.0, y + h / 2.0)
    }

    #[test]
    fn rows_stack_with_separators_thinner_and_the_panel_fits_its_widest_label() {
        let m = Popup::new(sample(), Anchor::Point(10.0, 10.0), WIN, 2.0);
        let p = &m.panels[0];
        assert_eq!(p.rows.len(), 7);
        assert_eq!(p.rows[0].y, 10.0, "the top padding");
        assert_eq!(p.rows[1].h, 18.0);
        assert_eq!(p.rows[2].y, p.rows[1].y + p.rows[1].h);
        assert_eq!(p.content_h, p.rows[6].y + 44.0 + 10.0);
        assert_eq!(p.rect, [10.0, 10.0, p.rect[2], p.content_h]);
        let widest = measure(Weight::Regular, 26.0, "Take Screenshot");
        assert!(p.rect[2] >= widest + 2.0 * (CHECK_W + 2.0 * PAD_X) / 2.0, "{}", p.rect[2]);
        assert!(p.rect[2] >= 2.0 * MIN_W && p.rect[2] <= 2.0 * MAX_W);
    }

    #[test]
    fn hit_testing_finds_the_row_under_the_pointer_and_nothing_on_the_padding() {
        let m = Popup::new(sample(), Anchor::Point(10.0, 10.0), WIN, 2.0);
        let (x, y) = mid(&m, 0, 0);
        assert_eq!(m.hit(x, y), Some((0, Some(0))));
        let (x, y) = mid(&m, 0, 2);
        assert_eq!(m.hit(x, y), Some((0, Some(2))));
        assert_eq!(m.hit(x, 12.0), Some((0, None)), "the top padding");
        assert_eq!(m.hit(5.0, y), None, "left of the panel");
        assert_eq!(m.hit(x, m.panels[0].rect[1] + m.panels[0].rect[3] + 1.0), None, "below it");
    }

    #[test]
    fn a_menu_near_an_edge_opens_the_other_way_and_stays_in_the_window() {
        // room on both sides: right of and below the point
        assert_eq!(place_at((100.0, 100.0), (200.0, 300.0), (800.0, 800.0), 6.0), (100.0, 100.0));
        // near the right and bottom edges: flipped to the left and above
        assert_eq!(place_at((700.0, 700.0), (200.0, 300.0), (800.0, 800.0), 6.0), (500.0, 400.0));
        // too narrow for either side: pushed inside
        assert_eq!(place_at((150.0, 10.0), (200.0, 100.0), (300.0, 800.0), 6.0), (94.0, 10.0));
        // wider than the window: at the margin
        assert_eq!(place_at((150.0, 10.0), (400.0, 100.0), (300.0, 800.0), 6.0).0, 6.0);
    }

    #[test]
    fn a_menu_taller_than_the_window_is_capped_and_scrolls() {
        let entries: Vec<Entry> = (0..100).map(|i| it(&format!("app:{i}"), "App")).collect();
        let mut m = Popup::new(entries, Anchor::Point(10.0, 300.0), (400.0, 600.0), 1.0);
        let p = &m.panels[0];
        assert_eq!(p.rect[1], 6.0);
        assert_eq!(p.rect[3], 600.0 - 12.0);
        assert!(p.max_scroll() > 0.0);
        let (x, y) = (50.0, 300.0);
        assert!(m.scroll(x, y, 100.0));
        assert_eq!(m.panels[0].scroll, 100.0);
        assert!(m.scroll(x, y, 1e6));
        assert_eq!(m.panels[0].scroll, m.panels[0].max_scroll(), "never past the end");
        assert!(!m.scroll(x, y, 10.0));
        // a click on the bottom strip does nothing more; on the top one it scrolls back up
        assert_eq!(m.click(x, 10.0), Outcome::Open);
        assert!(m.panels[0].scroll < m.panels[0].max_scroll());
    }

    #[test]
    fn a_submenu_opens_beside_its_row_and_flips_or_shifts_at_the_edges() {
        let win = (800.0, 800.0);
        // to the right, its first row level with the row
        assert_eq!(place_beside([100.0, 200.0, 200.0, 22.0], (180.0, 100.0), win, 6.0), (300.0, 200.0));
        // no room on the right: to the left
        assert_eq!(place_beside([500.0, 200.0, 200.0, 22.0], (180.0, 100.0), win, 6.0), (320.0, 200.0));
        // near the bottom: moved up to fit
        assert_eq!(place_beside([100.0, 750.0, 200.0, 22.0], (180.0, 100.0), win, 6.0), (300.0, 694.0));
        // no room either side: over the parent, on the roomier side, inside the window
        let (x, _) = place_beside([50.0, 10.0, 250.0, 22.0], (200.0, 100.0), (400.0, 800.0), 6.0);
        assert_eq!(x, 194.0);
        let (x, _) = place_beside([100.0, 10.0, 250.0, 22.0], (200.0, 100.0), (400.0, 800.0), 6.0);
        assert_eq!(x, 6.0);
    }

    #[test]
    fn hovering_a_submenu_opens_it_and_another_row_closes_it() {
        let mut m = Popup::new(sample(), Anchor::Point(10.0, 10.0), WIN, 1.0);
        let (x, y) = mid(&m, 0, 0);
        assert!(m.pointer_moved(x, y));
        assert_eq!(m.panels.len(), 2);
        assert_eq!(m.panels[1].path, vec![0]);
        assert_eq!(m.panels[1].rect[0], m.panels[0].rect[0] + m.panels[0].rect[2], "beside the menu");
        // straight down to Take Screenshot: the submenu closes
        let (x, y) = mid(&m, 0, 2);
        m.last = None;
        assert!(m.pointer_moved(x, y));
        assert_eq!(m.panels.len(), 1);
        assert_eq!(m.panels[0].hover, Some(2));
        // a disabled line is not highlighted
        let (x, y) = mid(&m, 0, 3);
        m.pointer_moved(x, y);
        assert_eq!(m.panels[0].hover, None);
    }

    #[test]
    fn moving_toward_an_open_submenu_keeps_it_open_across_other_rows() {
        let mut m = Popup::new(sample(), Anchor::Point(10.0, 10.0), WIN, 1.0);
        let (x, y) = mid(&m, 0, 4);
        m.pointer_moved(x, y);
        assert_eq!(m.panels[1].path, vec![4]);
        // diagonally right and up, over the row above, toward the tall submenu
        let r = m.panels[0].rect;
        assert!(!m.pointer_moved(r[0] + r[2] - 4.0, y - 20.0));
        assert_eq!(m.panels.len(), 2, "still open");
    }

    #[test]
    fn clicks_choose_items_open_submenus_and_close_outside() {
        let mut m = Popup::new(sample(), Anchor::Point(10.0, 10.0), WIN, 1.0);
        let (x, y) = mid(&m, 0, 3);
        assert_eq!(m.click(x, y), Outcome::Open, "a disabled line does nothing");
        let (x, y) = mid(&m, 0, 0);
        assert_eq!(m.click(x, y), Outcome::Open);
        assert_eq!(m.panels.len(), 2);
        let (x, y) = mid(&m, 1, 1);
        assert_eq!(m.click(x, y), Outcome::Chose("frame:pixel".into()));

        let mut m = Popup::new(sample(), Anchor::Point(10.0, 10.0), WIN, 1.0);
        let (x, y) = mid(&m, 0, 5);
        assert_eq!(m.click(x, y), Outcome::Chose("about".into()));

        let mut m = Popup::new(sample(), Anchor::Point(10.0, 10.0), WIN, 1.0);
        assert_eq!(m.click(700.0, 700.0), Outcome::Closed);
    }

    #[test]
    fn the_keyboard_walks_the_menu_and_escape_closes_it() {
        let mut m = Popup::new(sample(), Anchor::Point(10.0, 10.0), WIN, 1.0);
        assert_eq!(m.key(Key::Down), Outcome::Open);
        assert_eq!(m.panels[0].hover, Some(0));
        m.key(Key::Down);
        assert_eq!(m.panels[0].hover, Some(2), "past the separator");
        m.key(Key::Down);
        assert_eq!(m.panels[0].hover, Some(4), "past the disabled line");
        m.key(Key::Up);
        m.key(Key::Up);
        m.key(Key::Up);
        assert_eq!(m.panels[0].hover, Some(6), "wraps around");
        m.key(Key::Down);
        m.key(Key::Right);
        assert_eq!(m.panels.len(), 2);
        assert_eq!(m.panels[1].hover, Some(0), "the submenu's first item");
        m.key(Key::Down);
        m.key(Key::Left);
        assert_eq!(m.panels.len(), 1);
        assert_eq!(m.panels[0].hover, Some(0));
        m.key(Key::Enter);
        assert_eq!(m.key(Key::Enter), Outcome::Chose("frame:none".into()));

        let mut m = Popup::new(sample(), Anchor::Point(10.0, 10.0), WIN, 1.0);
        let (x, y) = mid(&m, 0, 0);
        m.pointer_moved(x, y);
        assert_eq!(m.key(Key::Escape), Outcome::Closed);
    }

    #[test]
    fn the_keyboard_scrolls_a_long_submenu_to_its_highlight() {
        let mut m = Popup::new(sample(), Anchor::Point(10.0, 10.0), (800.0, 400.0), 1.0);
        let (x, y) = mid(&m, 0, 4);
        m.click(x, y);
        for _ in 0..40 {
            m.key(Key::Down);
        }
        let sub = &m.panels[1];
        let r = sub.row_rect(sub.hover.unwrap());
        assert!(sub.scroll > 0.0);
        assert!(r[1] >= sub.rect[1] && r[1] + r[3] <= sub.rect[1] + sub.rect[3], "the highlight shows");
    }

    fn state() -> menu::State {
        menu::State {
            ios: false,
            frames: vec!["pixel", "galaxy-black", "galaxy-white"],
            frame: Some("pixel".into()),
            custom_frame: false,
            recording: false,
            gif: false,
            normalize: false,
            audio: true,
            muted: false,
            toolbar: true,
            touches: false,
            update_auto: true,
            update: Some("9.9.9".into()),
            update_command: None,
            update_install: true,
            clipboard: true,
            auto_style: false,
            mirroring: true,
            usb: true,
            sync: false,
            apps: vec![
                crate::apps::AppEntry { name: "Maps".into(), package: "com.maps".into(), system: false },
                crate::apps::AppEntry { name: "Settings".into(), package: "com.settings".into(), system: true },
            ],
            apps_loading: false,
            recent: vec![std::path::PathBuf::from("/tmp/handset-2026.png")],
        }
    }

    /// Every id in `entries`, depth first.
    fn ids(entries: &[Entry], out: &mut Vec<String>) {
        for e in entries {
            match e {
                Entry::Item(i) => out.extend(i.id.clone()),
                Entry::Submenu { items, .. } => ids(items, out),
                Entry::About => out.push("about".into()),
                Entry::Separator => {}
            }
        }
    }

    #[test]
    fn the_drawn_menu_shows_what_the_native_one_is_built_from() {
        let s = state();
        let m = Popup::new(menu::build(&s), Anchor::Point(10.0, 10.0), WIN, 1.0);
        assert_eq!(m.entries, menu::build(&s));
        // each submenu, opened, has a row per native entry
        for (r, e) in m.entries.clone().iter().enumerate() {
            if let Entry::Submenu { items, .. } = e {
                let mut m = Popup::new(menu::build(&s), Anchor::Point(10.0, 10.0), WIN, 1.0);
                let (x, y) = mid(&m, 0, r);
                m.click(x, y);
                assert_eq!(m.panels[1].rows.len(), items.len());
            }
        }
        // the ids are the ones App::on_menu handles
        let mut all = Vec::new();
        ids(&m.entries, &mut all);
        let known = [
            "conn:wifi",
            "conn:another",
            "conn:disconnect",
            "shot",
            "rec",
            "fmt:mp4",
            "fmt:gif",
            "norm",
            "clip",
            "style",
            "mute",
            "rotate",
            "sync",
            "toolbar",
            "touches",
            "zoom:in",
            "zoom:out",
            "zoom:reset",
            "update-install",
            "update-open",
            "update-notes",
            "update-skip",
            "update-later",
            "update",
            "update-auto",
            "about",
            "quit",
        ];
        for id in &all {
            let prefixed =
                ["frame:", "app:", "recent:open:", "recent:edit:", "recent:style:", "recent:reveal:", "recent:copy:"]
                    .iter()
                    .any(|p| id.starts_with(p));
            assert!(known.contains(&id.as_str()) || prefixed, "{id}");
        }
        for id in known {
            assert!(all.iter().any(|a| a == id), "{id} is missing");
        }
        assert!(all.contains(&"frame:galaxy-white".to_string()) && all.contains(&"app:com.settings".to_string()));
    }

    #[test]
    fn the_shortcut_glyphs_are_in_the_font() {
        let f = font(Weight::Regular);
        for i in menu::build(&state()).iter().filter_map(|e| if let Entry::Item(i) = e { Some(i) } else { None }) {
            for ch in i.shortcut_text().unwrap_or_default().chars().chain(i.label.chars()) {
                assert_ne!(f.glyph_id(ch).0, 0, "{ch:?} in {:?}", i.label);
            }
        }
        assert_ne!(f.glyph_id('\u{2026}').0, 0);
    }

    #[test]
    fn long_labels_are_cut_with_an_ellipsis() {
        let long = "handset-2026-10-28-at-12.34.56-a-very-long-capture-name.mp4";
        let cut = ellipsize(long, 13.0, 120.0);
        assert!(cut.ends_with('\u{2026}') && measure(Weight::Regular, 13.0, &cut) <= 120.0, "{cut}");
        assert_eq!(ellipsize("Quit", 13.0, 120.0), "Quit");
    }

    /// Writes the menu with a submenu open to `$HANDSET_TEST_OUT` (if set) so it can be looked at.
    #[test]
    fn the_panels_render_at_their_size_with_the_highlight_on_the_hovered_row() {
        let mut m = Popup::new(menu::build(&state()), Anchor::Point(20.0, 20.0), WIN, 2.0);
        let (x, y) = mid(&m, 0, 1);
        m.pointer_moved(x, y);
        let panels = m.raster();
        assert_eq!(panels.len(), 2);
        for (pill, rect) in &panels {
            assert_eq!((pill.width as f32, pill.height as f32), (rect[2], rect[3]));
            assert_eq!(pill.rgba.len(), (pill.width * pill.height * 4) as usize);
        }
        let (pill, rect) = &panels[0];
        let px = |x: f32, y: f32| {
            &pill.rgba[(((y - rect[1]) as u32 * pill.width + (x - rect[0]) as u32) * 4) as usize..][..4]
        };
        assert_eq!(px(rect[0], rect[1])[3], 0, "round corners");
        let lit = px(rect[0] + 2.0 * HOVER_INSET + 2.0, y);
        assert_eq!(&lit[..3], &HIGHLIGHT);
        let (_, y0) = mid(&m, 0, 2);
        assert_eq!(&px(rect[0] + 2.0 * HOVER_INSET + 2.0, y0)[..3], &BG);
        if let Some(dir) = std::env::var_os("HANDSET_TEST_OUT") {
            for (n, (pill, _)) in panels.iter().enumerate() {
                let path = std::path::Path::new(&dir).join(format!("menu-{n}.png"));
                let mut enc = png::Encoder::new(std::fs::File::create(path).unwrap(), pill.width, pill.height);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                enc.write_header().unwrap().write_image_data(&pill.rgba).unwrap();
            }
        }
    }
}
