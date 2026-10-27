//! Always-visible toolbar beside the phone: a vertical pill of round icon
//! buttons, rasterized on the CPU with simple signed-distance shapes. It is
//! drawn in the window only, never in screenshots or recordings.

use crate::overlay::Pill;

/// Width reserved beside the phone, as a fraction of the window height.
pub const SIDE_FRAC: f32 = 0.075;
/// Width of the pill itself, as a fraction of the window height.
const WIDTH_FRAC: f32 = 0.05;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Button {
    Back,
    Home,
    Recents,
    Power,
    VolumeUp,
    VolumeDown,
    Notifications,
    Screenshot,
    Record,
    Rotate,
    NextFrame,
    /// Shown only while a newer release is known.
    Update,
    More,
    Quit,
}

/// Top to bottom: the phone's own buttons first, then Handset's.
const ORDER: [Button; 13] = [
    Button::Back,
    Button::Home,
    Button::Recents,
    Button::Power,
    Button::VolumeUp,
    Button::VolumeDown,
    Button::Notifications,
    Button::Screenshot,
    Button::Record,
    Button::Rotate,
    Button::NextFrame,
    Button::More,
    Button::Quit,
];

/// Mirroring an iOS device is view only: just Handset's own buttons.
const IOS_ORDER: [Button; 5] = [Button::Screenshot, Button::Record, Button::NextFrame, Button::More, Button::Quit];

/// The buttons shown, top to bottom. `update` adds the update button above the menu while a newer release is known.
pub fn order(ios: bool, update: bool) -> Vec<Button> {
    let mut list = if ios { IOS_ORDER.to_vec() } else { ORDER.to_vec() };
    if update && let Some(i) = list.iter().position(|b| *b == Button::More) {
        list.insert(i, Button::Update);
    }
    list
}

impl Button {
    /// Shown while the pointer is over the button.
    pub fn label(self) -> &'static str {
        match self {
            Button::Back => "BACK",
            Button::Home => "HOME",
            Button::Recents => "RECENTS",
            Button::Power => "POWER",
            Button::VolumeUp => "VOLUME UP",
            Button::VolumeDown => "VOLUME DOWN",
            Button::Notifications => "NOTIFICATIONS",
            Button::Screenshot => "SCREENSHOT",
            Button::Record => "RECORD",
            Button::Rotate => "ROTATE",
            Button::NextFrame => "NEXT FRAME",
            Button::Update => "UPDATE",
            Button::More => "MENU",
            Button::Quit => "QUIT",
        }
    }
}

/// Size of one button cell, in pixels, for a window `wh` pixels high.
fn cell(wh: f32) -> f32 {
    (wh * WIDTH_FRAC).round().max(24.0)
}

/// Toolbar rectangle in window pixels: x, y, w, h. The pill is centred in the
/// strip reserved at the right edge, and vertically centred. `buttons` is how many it holds.
pub fn rect(ww: f32, wh: f32, buttons: usize) -> [f32; 4] {
    let c = cell(wh);
    let (w, h) = (c, c * buttons as f32);
    let strip = SIDE_FRAC * wh;
    [ww - strip + (strip - w) / 2.0, (wh - h) / 2.0, w, h]
}

/// Which button is under the window-pixel point, if any.
pub fn button_at(ww: f32, wh: f32, x: f32, y: f32, order: &[Button]) -> Option<Button> {
    let [rx, ry, rw, rh] = rect(ww, wh, order.len());
    if x < rx || x >= rx + rw || y < ry || y >= ry + rh {
        return None;
    }
    order.get(((y - ry) / (rh / order.len() as f32)) as usize).copied()
}

fn cov(d: f32, px: f32) -> f32 {
    (0.5 - d / px).clamp(0.0, 1.0)
}

type V = (f32, f32);
fn len(p: V) -> f32 {
    (p.0 * p.0 + p.1 * p.1).sqrt()
}
fn circle(p: V, c: V, r: f32) -> f32 {
    len((p.0 - c.0, p.1 - c.1)) - r
}
fn rrect(p: V, c: V, half: V, r: f32) -> f32 {
    let q = ((p.0 - c.0).abs() - half.0 + r, (p.1 - c.1).abs() - half.1 + r);
    len((q.0.max(0.0), q.1.max(0.0))) + q.0.max(q.1).min(0.0) - r
}
fn seg(p: V, a: V, b: V) -> f32 {
    let (pa, ba) = ((p.0 - a.0, p.1 - a.1), (b.0 - a.0, b.1 - a.1));
    let h = ((pa.0 * ba.0 + pa.1 * ba.1) / (ba.0 * ba.0 + ba.1 * ba.1)).clamp(0.0, 1.0);
    len((pa.0 - ba.0 * h, pa.1 - ba.1 * h))
}
fn triangle(p: V, a: V, b: V, c: V) -> f32 {
    // distance to the closest edge, negative inside (convex polygon)
    let edge = |u: V, v: V| {
        let (e, w) = ((v.0 - u.0, v.1 - u.1), (p.0 - u.0, p.1 - u.1));
        let l = len(e);
        (e.0 * w.1 - e.1 * w.0) / l
    };
    let orient = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let s = if orient > 0.0 { 1.0 } else { -1.0 };
    -(edge(a, b) * s).min(edge(b, c) * s).min(edge(c, a) * s)
}

const WHITE: [f32; 3] = [245.0, 245.0, 247.0];
const RED: [f32; 3] = [235.0, 60.0, 60.0];
/// The update button stands out in green: it is there only when there is something to do.
const GREEN: [f32; 3] = [70.0, 205.0, 115.0];

/// Coverage and colour of the icon of `button` at `p` (cell units, origin at
/// the button centre, y down).
fn icon(button: Button, p: V, px: f32, recording: bool) -> Option<(f32, [f32; 3])> {
    match button {
        Button::Back => {
            // a filled triangle pointing left
            let t = triangle(p, (-0.11, 0.0), (0.09, -0.13), (0.09, 0.13));
            Some((cov(t, px), WHITE))
        }
        Button::Home => Some((cov(circle(p, (0.0, 0.0), 0.12), px), WHITE)),
        Button::Recents => Some((cov(rrect(p, (0.0, 0.0), (0.105, 0.105), 0.03), px), WHITE)),
        Button::Power => {
            let r = len(p);
            let ang = p.1.atan2(p.0).to_degrees();
            // a ring open at the top, with a bar through the gap
            let gap = ang > -125.0 && ang < -55.0;
            let ring = cov((r - 0.15).abs() - 0.02, px) * if gap { 0.0 } else { 1.0 };
            let bar = cov(seg(p, (0.0, -0.19), (0.0, -0.02)) - 0.02, px);
            Some((ring.max(bar), WHITE))
        }
        Button::VolumeUp | Button::VolumeDown => {
            // a speaker, and a plus or a minus beside it
            let body = cov(rrect(p, (-0.13, 0.0), (0.04, 0.06), 0.01), px);
            let cone = cov(triangle(p, (-0.1, -0.06), (-0.1, 0.06), (-0.015, 0.14)), px)
                .max(cov(triangle(p, (-0.1, -0.06), (-0.1, 0.06), (-0.015, -0.14)), px));
            let dash = cov(seg(p, (0.06, 0.0), (0.18, 0.0)) - 0.016, px);
            let sign = if button == Button::VolumeUp {
                dash.max(cov(seg(p, (0.12, -0.06), (0.12, 0.06)) - 0.016, px))
            } else {
                dash
            };
            Some((body.max(cone).max(sign), WHITE))
        }
        Button::Notifications => {
            // a bell: a dome, a flared skirt and a clapper
            let dome = cov(circle(p, (0.0, -0.04), 0.1), px);
            let skirt = cov(triangle(p, (-0.1, -0.04), (0.1, -0.04), (0.0, 0.09)), px)
                .max(cov(rrect(p, (0.0, 0.075), (0.14, 0.022), 0.02), px));
            let hang = cov(triangle(p, (-0.1, -0.04), (-0.14, 0.075), (0.0, 0.075)), px)
                .max(cov(triangle(p, (0.1, -0.04), (0.14, 0.075), (0.0, 0.075)), px));
            let clapper = cov(circle(p, (0.0, 0.14), 0.03), px);
            Some((dome.max(skirt).max(hang).max(clapper), WHITE))
        }
        Button::Screenshot => {
            let body =
                cov(rrect(p, (0.0, 0.03), (0.2, 0.13), 0.04), px) * (1.0 - cov(circle(p, (0.0, 0.03), 0.085), px));
            let bump = cov(rrect(p, (-0.09, -0.115), (0.06, 0.03), 0.015), px);
            let dot = cov(circle(p, (0.0, 0.03), 0.04), px);
            Some((body.max(bump).max(dot), WHITE))
        }
        Button::Record => {
            let ring = cov(circle(p, (0.0, 0.0), 0.225).abs() - 0.013, px);
            let fill = if recording {
                cov(rrect(p, (0.0, 0.0), (0.12, 0.12), 0.03), px)
            } else {
                cov(circle(p, (0.0, 0.0), 0.16), px)
            };
            if fill > 0.0 { Some((fill, RED)) } else { Some((ring, WHITE)) }
        }
        Button::Rotate => {
            let r = len(p);
            let ang = p.1.atan2(p.0).to_degrees();
            // a ring open at the top right, with an arrow head at one end
            let gap = ang > -75.0 && ang < -15.0;
            let ring = cov((r - 0.16).abs() - 0.02, px) * if gap { 0.0 } else { 1.0 };
            let a = (-15.0f32).to_radians();
            let e = (0.16 * a.cos(), 0.16 * a.sin());
            let t = (-a.sin(), a.cos()); // clockwise tangent
            let n = (a.cos(), a.sin());
            let tri = cov(
                triangle(
                    p,
                    (e.0 + n.0 * 0.07, e.1 + n.1 * 0.07),
                    (e.0 - n.0 * 0.07, e.1 - n.1 * 0.07),
                    (e.0 + t.0 * 0.1, e.1 + t.1 * 0.1),
                ),
                px,
            );
            Some((ring.max(tri), WHITE))
        }
        Button::NextFrame => {
            let outline = cov(rrect(p, (0.0, 0.0), (0.11, 0.19), 0.05).abs() - 0.017, px);
            let cam = cov(circle(p, (0.0, -0.13), 0.02), px);
            Some((outline.max(cam), WHITE))
        }
        Button::Update => {
            // an arrow down into a tray
            let shaft = cov(seg(p, (0.0, -0.17), (0.0, 0.0)) - 0.02, px);
            let head = cov(triangle(p, (-0.08, -0.02), (0.08, -0.02), (0.0, 0.07)), px);
            let tray = [((-0.15, 0.05), (-0.15, 0.15)), ((-0.15, 0.15), (0.15, 0.15)), ((0.15, 0.15), (0.15, 0.05))]
                .iter()
                .map(|(a, b)| cov(seg(p, *a, *b) - 0.018, px))
                .fold(0.0, f32::max);
            Some((shaft.max(head).max(tray), GREEN))
        }
        Button::More => {
            let d = [-0.12, 0.0, 0.12].iter().map(|x| cov(circle(p, (*x, 0.0), 0.035), px)).fold(0.0, f32::max);
            Some((d, WHITE))
        }
        Button::Quit => {
            let a = cov(seg(p, (-0.1, -0.1), (0.1, 0.1)) - 0.018, px);
            let b = cov(seg(p, (-0.1, 0.1), (0.1, -0.1)) - 0.018, px);
            Some((a.max(b), WHITE))
        }
    }
}

/// Rasterize the toolbar for a window `wh` pixels high. `hover` highlights a
/// button; `recording` turns the record button into a stop square.
pub fn raster(wh: f32, order: &[Button], hover: Option<Button>, recording: bool) -> Pill {
    let c = cell(wh);
    let (w, h) = (c as u32, c as u32 * order.len() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let r = w as f32 / 2.0;
    let px = 1.0 / c; // one pixel, in cell units

    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // pill background with a thin lighter rim
            let cy = fy.clamp(r, h as f32 - r);
            let d = ((fx - r).powi(2) + (fy - cy).powi(2)).sqrt();
            let a = (r - d + 0.5).clamp(0.0, 1.0);
            let rim = r - d < 0.04 * c;
            let (mut col, mut alpha) = (if rim { [96.0, 96.0, 110.0] } else { [20.0, 20.0, 24.0] }, a * 225.0);

            let i = ((fy / c) as usize).min(order.len() - 1);
            let p = (fx / c - 0.5, fy / c - (i as f32 + 0.5));
            if hover == Some(order[i]) {
                let k = cov(circle(p, (0.0, 0.0), 0.38), px) * 0.22;
                for c in col.iter_mut() {
                    *c += (255.0 - *c) * k;
                }
                alpha = alpha.max(k * 255.0);
            }
            if let Some((k, ic)) = icon(order[i], p, px, recording) {
                for (c, i) in col.iter_mut().zip(ic) {
                    *c += (i - *c) * k;
                }
                alpha = alpha.max(k * 255.0);
            }
            let o = ((y * w + x) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&[col[0] as u8, col[1] as u8, col[2] as u8, alpha as u8]);
        }
    }
    Pill { width: w, height: h, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_button_is_hit_in_its_own_cell() {
        let (ww, wh) = (700.0, 820.0);
        for order in [order(false, false), order(true, false), order(false, true), order(true, true)] {
            let [rx, ry, rw, rh] = rect(ww, wh, order.len());
            for (i, b) in order.iter().enumerate() {
                let y = ry + rh / order.len() as f32 * (i as f32 + 0.5);
                assert_eq!(button_at(ww, wh, rx + rw / 2.0, y, &order), Some(*b), "{b:?}");
            }
            assert_eq!(button_at(ww, wh, rx - 5.0, ry + 3.0, &order), None);
            // the pill fits in the window with room to spare
            assert!(rh < wh * 0.8, "toolbar is {rh} px in a {wh} px window");
        }
    }

    #[test]
    fn ios_shows_only_what_works_without_control() {
        let ios = order(true, false);
        assert!(ios.iter().all(|b| order(false, false).contains(b)));
        for b in [Button::Back, Button::Home, Button::Power, Button::VolumeUp, Button::Rotate, Button::Notifications] {
            assert!(!ios.contains(&b), "{b:?}");
        }
    }

    #[test]
    fn the_update_button_shows_only_when_there_is_an_update_and_sits_above_the_menu() {
        for ios in [false, true] {
            assert!(!order(ios, false).contains(&Button::Update));
            let with = order(ios, true);
            let at = with.iter().position(|b| *b == Button::Update).unwrap();
            assert_eq!(with[at + 1], Button::More);
            assert_eq!(with.len(), order(ios, false).len() + 1);
        }
    }

    #[test]
    fn every_button_draws_an_icon_and_has_a_label() {
        for b in order(false, true) {
            let covered: f32 = (0..21)
                .flat_map(|y| (0..21).map(move |x| (x, y)))
                .filter_map(|(x, y)| icon(b, ((x as f32 - 10.0) / 40.0, (y as f32 - 10.0) / 40.0), 1.0 / 40.0, false))
                .map(|(k, _)| k)
                .sum();
            assert!(covered > 6.0, "{b:?} draws almost nothing ({covered})");
            assert!(!b.label().is_empty());
        }
    }

    /// Writes the toolbar to `$HANDSET_TEST_OUT/toolbar.png` (if set) so it can be looked at.
    #[test]
    fn toolbar_renders() {
        let p = raster(820.0, &order(false, true), Some(Button::Notifications), false);
        assert_eq!(p.rgba.len(), (p.width * p.height * 4) as usize);
        if let Some(dir) = std::env::var_os("HANDSET_TEST_OUT") {
            let path = std::path::Path::new(&dir).join("toolbar.png");
            let mut enc = png::Encoder::new(std::fs::File::create(path).unwrap(), p.width, p.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&p.rgba).unwrap();
        }
    }
}
