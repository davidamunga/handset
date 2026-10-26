//! Small on-screen pills (REC indicator, toasts) rasterized on the CPU with a
//! built-in 5x7 bitmap font. They are drawn in the window only, never in
//! screenshots or recordings.

/// Rows of a glyph, 5 bits each, most significant bit on the left.
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        ':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        ',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b00100, 0b01000],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '\'' => [0b00100, 0b00100, 0b01000, 0b00000, 0b00000, 0b00000, 0b00000],
        '<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
        '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        _ => [0; 7],
    }
}

pub struct Pill {
    pub width: u32,
    pub height: u32,
    /// Straight-alpha RGBA8.
    pub rgba: Vec<u8>,
}

/// A rounded dark pill with `text` and, optionally, a coloured dot on the
/// left. `s` is the size of one font pixel, in device pixels.
pub fn pill(text: &str, dot: Option<[u8; 3]>, s: u32) -> Pill {
    let pad = 3 * s;
    let text_w = text.chars().count() as u32 * 6 * s - s; // 5 columns + 1 gap per glyph
    let dot_w = if dot.is_some() { 5 * s + 2 * s } else { 0 };
    let (w, h) = (pad * 2 + dot_w + text_w, pad * 2 + 7 * s);
    let mut rgba = vec![0u8; (w * h * 4) as usize];

    // rounded background, anti-aliased through the distance to the end caps
    let r = h as f32 / 2.0;
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = px.clamp(r, w as f32 - r);
            let d = ((px - cx).powi(2) + (py - r).powi(2)).sqrt();
            let cover = (r - d + 0.5).clamp(0.0, 1.0);
            let i = ((y * w + x) * 4) as usize;
            // a thin lighter rim keeps the pill readable over dark content
            let rim = r - d < 0.7 * s as f32;
            let c = if rim { [96, 96, 110] } else { [20, 20, 24] };
            rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], (cover * 225.0) as u8]);
        }
    }

    let mut put = |x: u32, y: u32, c: [u8; 3]| {
        let i = ((y * w + x) * 4) as usize;
        rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
    };

    let mut x0 = pad;
    if let Some(c) = dot {
        // a round dot, 5 font pixels across
        let (cx, cy, rad) = (x0 as f32 + 2.5 * s as f32, h as f32 / 2.0, 2.5 * s as f32);
        for y in 0..h {
            for x in x0..x0 + 5 * s {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                if d <= rad {
                    put(x, y, c);
                }
            }
        }
        x0 += dot_w;
    }
    for (n, ch) in text.chars().enumerate() {
        let gx = x0 + n as u32 * 6 * s;
        for (row, bits) in glyph(ch).iter().enumerate() {
            for col in 0..5u32 {
                if bits & (1 << (4 - col)) != 0 {
                    for dy in 0..s {
                        for dx in 0..s {
                            put(gx + col * s + dx, pad + row as u32 * s + dy, [245, 245, 247]);
                        }
                    }
                }
            }
        }
    }
    Pill { width: w, height: h, rgba }
}

/// A translucent white disc with a light ring and a thin dark edge, `size` pixels across: where a finger is.
pub fn touch_dot(size: u32) -> Pill {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let r = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 + 0.5 - r).powi(2) + (y as f32 + 0.5 - r).powi(2)).sqrt() / r;
            let cover = ((1.0 - d) * r + 0.5).clamp(0.0, 1.0);
            // a see-through fill with a brighter ring at the edge
            let a = if d > 0.9 { 0.95 } else { 0.42 };
            let i = ((y * size + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[255, 255, 255, (cover * a * 255.0) as u8]);
        }
    }
    Pill { width: size, height: size, rgba }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_touch_dot_is_round_and_translucent() {
        let p = super::touch_dot(96);
        let alpha = |x: u32, y: u32| p.rgba[((y * 96 + x) * 4 + 3) as usize];
        assert_eq!(alpha(0, 0), 0, "corners are clear");
        assert!(alpha(48, 48) > 60 && alpha(48, 48) < 160, "the middle shows what is below");
        assert!(alpha(48, 3) > 200, "the ring is bright");
    }

    /// Writes the long toast to `$HANDSET_TEST_OUT` (if set) so it can be looked at.
    #[test]
    fn long_toast_renders() {
        let p = super::pill("SCREENSHOT COPIED - CLICK TO SHOW", None, 3);
        assert!(p.width > 100 && p.height > 20);
        assert_eq!(p.rgba.len(), (p.width * p.height * 4) as usize);
        if let Some(out) = std::env::var_os("HANDSET_TEST_OUT") {
            let mut enc = png::Encoder::new(std::fs::File::create(out).unwrap(), p.width, p.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&p.rgba).unwrap();
        }
    }
}
