//! Turning a capture in 3D: the phone is tilted as if it floated in front of the background.
//!
//! The picture is a flat rectangle, so a turn, a lean and a roll move its four corners and nothing
//! else. We project those corners with a pinhole camera, solve the homography between the flat picture
//! and the projected quad, and look every output pixel up in the source. The result has the same size
//! as the input (the quad is scaled to fit), so padding and aspect ratio behave as without a tilt.

use crate::style::Img;

/// Rotation in whole degrees. Zero on every axis is a flat picture.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Tilt {
    /// Turn around the vertical axis; positive moves the right edge away.
    pub yaw: i32,
    /// Lean around the horizontal axis; positive moves the top edge away.
    pub pitch: i32,
    /// Turn in the picture's plane, clockwise.
    pub roll: i32,
}

pub const MAX_YAW: i32 = 60;
pub const MAX_PITCH: i32 = 60;
pub const MAX_ROLL: i32 = 45;

/// Camera distance in units of the picture's longer side; smaller is a stronger perspective.
const DISTANCE: f32 = 2.6;

impl Tilt {
    pub fn is_flat(self) -> bool {
        self == Self::default()
    }

    /// `yaw,pitch,roll`; missing trailing numbers are zero.
    pub fn parse(v: &str) -> Option<Self> {
        let mut n = v.split(',').map(|p| p.trim().parse::<i32>().ok());
        let mut next = |max: i32| Some(n.next().unwrap_or(Some(0))?.clamp(-max, max));
        let t = Self { yaw: next(MAX_YAW)?, pitch: next(MAX_PITCH)?, roll: next(MAX_ROLL)? };
        n.next().is_none().then_some(t)
    }

    /// Named looks for the live window's menu.
    pub const PRESETS: [(&'static str, Tilt); 5] = [
        ("Flat", Tilt { yaw: 0, pitch: 0, roll: 0 }),
        ("Turn Left", Tilt { yaw: -28, pitch: 6, roll: 0 }),
        ("Turn Right", Tilt { yaw: 28, pitch: 6, roll: 0 }),
        ("Lean Back", Tilt { yaw: 0, pitch: 30, roll: 0 }),
        ("Floating", Tilt { yaw: -24, pitch: 12, roll: -4 }),
    ];

    pub fn to_spec(self) -> String {
        format!("{},{},{}", self.yaw, self.pitch, self.roll)
    }

    /// The four corners of a `w` x `h` picture (top-left, top-right, bottom-right, bottom-left) after the
    /// rotation and the projection, scaled to fill a `w` x `h` canvas and centred in it.
    pub(crate) fn corners(self, w: f32, h: f32) -> [(f32, f32); 4] {
        let d = DISTANCE * w.max(h);
        let (sr, cr) = (self.roll as f32).to_radians().sin_cos();
        let (sp, cp) = (self.pitch as f32).to_radians().sin_cos();
        let (sy, cy) = (self.yaw as f32).to_radians().sin_cos();
        let project = |x: f32, y: f32| {
            let (x, y) = (x * cr - y * sr, x * sr + y * cr);
            let (y, z) = (y * cp, -y * sp);
            let (x, z) = (x * cy - z * sy, x * sy + z * cy);
            let k = d / (d + z);
            (x * k, y * k)
        };
        let (hw, hh) = (w / 2.0, h / 2.0);
        let p = [project(-hw, -hh), project(hw, -hh), project(hw, hh), project(-hw, hh)];
        let min = p.iter().fold((f32::MAX, f32::MAX), |m, q| (m.0.min(q.0), m.1.min(q.1)));
        let max = p.iter().fold((f32::MIN, f32::MIN), |m, q| (m.0.max(q.0), m.1.max(q.1)));
        // one pixel of margin keeps the anti-aliased edge inside the canvas
        let s = ((w - 2.0) / (max.0 - min.0)).min((h - 2.0) / (max.1 - min.1));
        let (cx, cy) = ((min.0 + max.0) / 2.0, (min.1 + max.1) / 2.0);
        p.map(|(x, y)| ((x - cx) * s + hw, (y - cy) * s + hh))
    }
}

/// Solve `a * x = b` for eight unknowns by Gaussian elimination with pivoting.
fn solve8(mut a: [[f32; 9]; 8]) -> Option<[f32; 8]> {
    for col in 0..8 {
        let pivot = (col..8).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-9 {
            return None;
        }
        a.swap(col, pivot);
        for row in 0..8 {
            if row != col {
                let f = a[row][col] / a[col][col];
                let pivot_row = a[col];
                for (v, p) in a[row].iter_mut().zip(pivot_row).skip(col) {
                    *v -= f * p;
                }
            }
        }
    }
    Some(std::array::from_fn(|i| a[i][8] / a[i][i]))
}

/// The homography taking the canvas quad `dst` back to the picture's corners `src`, as row-major 3x3.
pub(crate) fn homography(dst: [(f32, f32); 4], src: [(f32, f32); 4]) -> Option<[f32; 9]> {
    let mut a = [[0.0f32; 9]; 8];
    for i in 0..4 {
        let ((x, y), (u, v)) = (dst[i], src[i]);
        a[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        a[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    let h = solve8(a)?;
    Some([h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0])
}

/// Inverse of a row-major 3x3 matrix.
pub(crate) fn invert3(m: [f32; 9]) -> Option<[f32; 9]> {
    let c = [
        m[4] * m[8] - m[5] * m[7],
        m[2] * m[7] - m[1] * m[8],
        m[1] * m[5] - m[2] * m[4],
        m[5] * m[6] - m[3] * m[8],
        m[0] * m[8] - m[2] * m[6],
        m[2] * m[3] - m[0] * m[5],
        m[3] * m[7] - m[4] * m[6],
        m[1] * m[6] - m[0] * m[7],
        m[0] * m[4] - m[1] * m[3],
    ];
    let det = m[0] * c[0] + m[1] * c[3] + m[2] * c[6];
    (det.abs() > 1e-12).then(|| c.map(|v| v / det))
}

/// Apply a row-major 3x3 homography to a point; `None` behind the camera.
pub(crate) fn apply(m: &[f32; 9], (x, y): (f32, f32)) -> Option<(f32, f32)> {
    let w = m[6] * x + m[7] * y + m[8];
    (w > 1e-6).then(|| ((m[0] * x + m[1] * y + m[2]) / w, (m[3] * x + m[4] * y + m[5]) / w))
}

/// Premultiplied bilinear sample at picture coordinates; outside the picture is transparent.
fn sample(img: &Img, x: f32, y: f32) -> [f32; 4] {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let px = |ix: f32, iy: f32| -> [f32; 4] {
        if ix < 0.0 || iy < 0.0 || ix >= img.w as f32 || iy >= img.h as f32 {
            return [0.0; 4];
        }
        let i = ((iy as u32 * img.w + ix as u32) * 4) as usize;
        let a = img.rgba[i + 3] as f32;
        [img.rgba[i] as f32 * a / 255.0, img.rgba[i + 1] as f32 * a / 255.0, img.rgba[i + 2] as f32 * a / 255.0, a]
    };
    let (a, b, c, d) = (px(x0, y0), px(x0 + 1.0, y0), px(x0, y0 + 1.0), px(x0 + 1.0, y0 + 1.0));
    std::array::from_fn(|k| (a[k] * (1.0 - tx) + b[k] * tx) * (1.0 - ty) + (c[k] * (1.0 - tx) + d[k] * tx) * ty)
}

/// `src` turned by `tilt`, same size, transparent around the phone. `radius` rounds the picture's corners
/// first, in pixels (0 keeps the picture's own outline).
pub fn warp(src: &Img, tilt: Tilt, radius: f32) -> Img {
    let (w, h) = (src.w, src.h);
    let mut round;
    let src = if radius > 0.0 {
        round = Img { w, h, rgba: src.rgba.clone() };
        for y in 0..h {
            for x in 0..w {
                let c = crate::style::rounded_cover(x, y, w, h, radius);
                let a = &mut round.rgba[((y * w + x) * 4 + 3) as usize];
                *a = (*a as f32 * c).round() as u8;
            }
        }
        &round
    } else {
        src
    };
    let flat = [(0.0, 0.0), (w as f32, 0.0), (w as f32, h as f32), (0.0, h as f32)];
    let Some(m) = homography(tilt.corners(w as f32, h as f32), flat) else {
        return Img { w, h, rgba: src.rgba.clone() };
    };
    let mut out = vec![0u8; (w * h * 4) as usize];
    // 2x2 samples a pixel smooth the quad's edge and the minified far side
    const TAPS: [(f32, f32); 4] = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];
    crate::style::par_rows(&mut out, w, |y, row| {
        for x in 0..w {
            let mut acc = [0.0f32; 4];
            for (dx, dy) in TAPS {
                let (px, py) = (x as f32 + dx, y as f32 + dy);
                let q = m[6] * px + m[7] * py + m[8];
                if q <= 1e-6 {
                    continue;
                }
                let s = sample(src, (m[0] * px + m[1] * py + m[2]) / q, (m[3] * px + m[4] * py + m[5]) / q);
                for k in 0..4 {
                    acc[k] += s[k] / 4.0;
                }
            }
            let a = acc[3];
            if a > 0.0 {
                let px = &mut row[(x * 4) as usize..(x * 4 + 4) as usize];
                for k in 0..3 {
                    px[k] = (acc[k] * 255.0 / a).round().clamp(0.0, 255.0) as u8;
                }
                px[3] = a.round() as u8;
            }
        }
    });
    Img { w, h, rgba: out }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32) -> Img {
        Img { w, h, rgba: [200, 100, 50, 255].repeat((w * h) as usize) }
    }

    fn alpha(img: &Img, x: u32, y: u32) -> u8 {
        img.rgba[((y * img.w + x) * 4 + 3) as usize]
    }

    #[test]
    fn spec_round_trips_and_clamps() {
        let t = Tilt { yaw: -18, pitch: 8, roll: 0 };
        assert_eq!(Tilt::parse(&t.to_spec()), Some(t));
        assert_eq!(Tilt::parse("20"), Some(Tilt { yaw: 20, pitch: 0, roll: 0 }));
        assert_eq!(Tilt::parse("999,-999,999"), Some(Tilt { yaw: MAX_YAW, pitch: -MAX_PITCH, roll: MAX_ROLL }));
        assert_eq!(Tilt::parse("1,2,3,4"), None);
        assert_eq!(Tilt::parse("a"), None);
    }

    #[test]
    fn the_inverse_takes_a_tilted_point_back() {
        let flat = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let t = Tilt { yaw: 30, pitch: -10, roll: 5 }.corners(1.0, 2.0);
        let quad = t.map(|(x, y)| (x / 1.0, y / 2.0));
        let h = homography(flat, quad).unwrap();
        let inv = invert3(h).unwrap();
        for p in [(0.2, 0.7), (0.5, 0.5), (0.9, 0.1)] {
            let q = apply(&h, p).unwrap();
            let back = apply(&inv, q).unwrap();
            assert!((back.0 - p.0).abs() < 1e-3 && (back.1 - p.1).abs() < 1e-3, "{p:?} -> {q:?} -> {back:?}");
        }
        assert_eq!(
            apply(&h, (0.0, 0.0)).map(|(x, y)| ((x * 1e3).round(), (y * 1e3).round())),
            Some(((quad[0].0 * 1e3).round(), (quad[0].1 * 1e3).round()))
        );
    }

    #[test]
    fn a_flat_tilt_keeps_the_picture() {
        let img = solid(40, 60);
        let out = warp(&img, Tilt::default(), 0.0);
        // the fitted margin shrinks it by a pixel on each side
        assert_eq!(alpha(&out, 20, 30), 255);
        assert_eq!(alpha(&out, 0, 0), 0);
    }

    #[test]
    fn positive_yaw_moves_the_right_edge_away() {
        let c = Tilt { yaw: 30, pitch: 0, roll: 0 }.corners(100.0, 200.0);
        let left = c[3].1 - c[0].1;
        let right = c[2].1 - c[1].1;
        assert!(right < left, "right {right} should be shorter than left {left}");
        assert!(c.iter().all(|p| p.0 >= 0.0 && p.0 <= 100.0 && p.1 >= 0.0 && p.1 <= 200.0));
    }

    #[test]
    fn positive_pitch_moves_the_top_edge_away() {
        let c = Tilt { yaw: 0, pitch: 25, roll: 0 }.corners(100.0, 200.0);
        assert!(c[1].0 - c[0].0 < c[2].0 - c[3].0);
    }

    #[test]
    fn a_turned_picture_leaves_the_corners_empty_and_the_middle_filled() {
        let out = warp(&solid(80, 160), Tilt { yaw: -35, pitch: 10, roll: 8 }, 6.0);
        assert_eq!(alpha(&out, 40, 80), 255);
        assert!((0..4).any(|i| alpha(&out, if i % 2 == 0 { 0 } else { 79 }, if i < 2 { 0 } else { 159 }) == 0));
        // colour survives the unpremultiply
        let i = ((80 * 80 + 40) * 4) as usize;
        assert_eq!(&out.rgba[i..i + 3], &[200, 100, 50]);
    }
}
