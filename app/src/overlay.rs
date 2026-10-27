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

/// Space between the lines of a wrapped pill, in font pixels.
const LINE_GAP: u32 = 3;

/// Break `text` at spaces so no line is longer than `max_chars`, by turning those spaces into newlines, so every
/// character keeps its index. A word longer than a line stays whole.
pub fn wrap(text: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut line = 0;
    for (i, word) in text.split(' ').enumerate() {
        let n = word.chars().count();
        if i > 0 {
            if line > 0 && line + 1 + n > max_chars {
                out.push('\n');
                line = 0;
            } else {
                out.push(' ');
                line += 1;
            }
        }
        out.push_str(word);
        line += n;
    }
    out
}

/// Lay `text` out to fit `max_w` pixels: the largest font size up to `s` at which it fits on at most three lines,
/// wrapped at spaces. Returns the wrapped text and the font size; at size 1 it wraps as much as it has to.
pub fn fit(text: &str, dot: bool, s: u32, max_w: u32) -> (String, u32) {
    let cols = |s: u32| {
        let room = max_w.saturating_sub(6 * s + if dot { 7 * s } else { 0 }) + s;
        ((room / (6 * s)) as usize).max(1)
    };
    for s in (2..=s.max(1)).rev() {
        let laid = wrap(text, cols(s));
        if laid.lines().count() <= 3 && laid.lines().all(|l| l.chars().count() <= cols(s)) {
            return (laid, s);
        }
    }
    (wrap(text, cols(1)), 1)
}

/// Which character of `text`, laid out by [`pill`] at font size `s`, is at pill pixel (`x`, `y`). The nearest one
/// on the nearest line, so every part of the pill counts.
pub fn char_at(text: &str, dot: bool, s: u32, x: f32, y: f32) -> usize {
    let (s, pad) = (s as f32, 3.0 * s as f32);
    let lines: Vec<&str> = text.split('\n').collect();
    let line = (((y - pad) / ((7 + LINE_GAP) as f32 * s)).max(0.0) as usize).min(lines.len() - 1);
    let col = ((x - pad - if dot { 7.0 * s } else { 0.0 }) / (6.0 * s)).max(0.0) as usize;
    let start: usize = lines[..line].iter().map(|l| l.chars().count() + 1).sum();
    start + col.min(lines[line].chars().count().saturating_sub(1))
}

/// A rounded dark pill with `text` and, optionally, a coloured dot on the
/// left. `s` is the size of one font pixel, in device pixels. A newline in `text` starts another line.
pub fn pill(text: &str, dot: Option<[u8; 3]>, s: u32) -> Pill {
    let pad = 3 * s;
    let lines: Vec<&str> = text.split('\n').collect();
    let cols = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u32;
    let text_w = (cols * 6 * s).saturating_sub(s); // 5 columns + 1 gap per glyph
    let dot_w = if dot.is_some() { 5 * s + 2 * s } else { 0 };
    let rows = lines.len() as u32;
    let (w, h) = (pad * 2 + dot_w + text_w, pad * 2 + rows * 7 * s + (rows - 1) * LINE_GAP * s);
    let mut rgba = vec![0u8; (w * h * 4) as usize];

    // rounded background, anti-aliased through the distance to the corners; one line is a pill with round ends
    let r = (pad * 2 + 7 * s) as f32 / 2.0;
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let (cx, cy) = (px.clamp(r, w as f32 - r), py.clamp(r, h as f32 - r));
            let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
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
        let (cx, cy, rad) = (x0 as f32 + 2.5 * s as f32, r, 2.5 * s as f32);
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
    text_into(&mut rgba, w, &lines, x0, pad, s, [245, 245, 247]);
    Pill { width: w, height: h, rgba }
}

/// Draw `lines` of the bitmap font into `rgba` (`w` pixels wide), the first line's top left at (`x0`, `y0`).
fn text_into(rgba: &mut [u8], w: u32, lines: &[&str], x0: u32, y0: u32, s: u32, c: [u8; 3]) {
    for (l, line) in lines.iter().enumerate() {
        let gy = y0 + l as u32 * (7 + LINE_GAP) * s;
        for (n, ch) in line.chars().enumerate() {
            let gx = x0 + n as u32 * 6 * s;
            for (row, bits) in glyph(ch).iter().enumerate() {
                for col in (0..5u32).filter(|col| bits & (1 << (4 - col)) != 0) {
                    for dy in 0..s {
                        for dx in 0..s {
                            let i = (((gy + row as u32 * s + dy) * w + gx + col * s + dx) * 4) as usize;
                            rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                        }
                    }
                }
            }
        }
    }
}

/// The banner's colour: solid, so the text reads over any screen.
const BANNER_BLUE: [u8; 3] = [10, 104, 224];

/// A solid blue bar `width` pixels wide with white `text` (each line of it wrapped, to at most three lines in all,
/// at font size `s` or smaller if it must) and a close cross at the right end: for something that waits until it
/// is dealt with.
/// Returns the bar and the x, in its pixels, where the close cross's clickable part begins.
pub fn banner(text: &str, s: u32, width: u32) -> (Pill, u32) {
    let layout = |s: u32| {
        let pad = 4 * s;
        let close_x = width.saturating_sub(2 * pad + 5 * s);
        let cols = ((close_x.saturating_sub(pad) + s) / (6 * s)).max(1) as usize;
        (pad, close_x, cols, text.split('\n').map(|l| wrap(l, cols)).collect::<Vec<_>>().join("\n"))
    };
    let s = (2..=s.max(1))
        .rev()
        .find(|&s| {
            let (.., cols, laid) = layout(s);
            laid.lines().count() <= 3 && laid.lines().all(|l| l.chars().count() <= cols)
        })
        .unwrap_or(1);
    let (pad, close_x, _, laid) = layout(s);
    let lines: Vec<&str> = laid.split('\n').collect();
    let rows = lines.len() as u32;
    let h = pad * 2 + rows * 7 * s + (rows - 1) * LINE_GAP * s;
    let mut rgba = vec![0u8; (width * h * 4) as usize];
    // a rounded rectangle, anti-aliased at the corners
    let r = 3.0 * s as f32;
    for y in 0..h {
        for x in 0..width {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let (cx, cy) = (px.clamp(r, width as f32 - r), py.clamp(r, h as f32 - r));
            let cover = (r - ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() + 0.5).clamp(0.0, 1.0);
            let i = ((y * width + x) * 4) as usize;
            let c = BANNER_BLUE;
            rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], (cover * 250.0) as u8]);
        }
    }
    text_into(&mut rgba, width, &lines, pad, pad, s, [255, 255, 255]);
    text_into(&mut rgba, width, &["X"], width - pad - 5 * s, (h - 7 * s) / 2, s, [255, 255, 255]);
    (Pill { width, height: h, rgba }, close_x)
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

    #[test]
    fn wrapping_turns_spaces_into_line_breaks_and_keeps_every_index() {
        let text = "SCREENSHOT COPIED - SHOW | EDIT";
        let laid = super::wrap(text, 14);
        assert_eq!(laid, "SCREENSHOT\nCOPIED - SHOW\n| EDIT");
        assert_eq!(laid.chars().count(), text.chars().count());
        assert_eq!(super::wrap(text, 80), text, "a line that fits is left alone");
        assert_eq!(super::wrap("SUPERCALIFRAGILISTIC", 5), "SUPERCALIFRAGILISTIC", "one long word stays whole");
    }

    #[test]
    fn a_long_notice_wraps_at_a_readable_size_before_it_shrinks() {
        let text = "HANDSET 0.8.1 IS OUT - CLICK TO COPY: BREW UPGRADE --CASK HANDSET";
        let (laid, s) = super::fit(text, false, 3, 600);
        assert_eq!(s, 3, "it stays at full size");
        assert!(laid.lines().count() > 1 && laid.lines().count() <= 3, "{laid}");
        assert!(super::pill(&laid, None, s).width <= 600);
        // a short one stays on one line
        assert_eq!(super::fit("REC 0:01", true, 3, 600), ("REC 0:01".to_string(), 3));
        // a very narrow space gets the smallest size and as many lines as it needs
        let (laid, s) = super::fit(text, false, 3, 120);
        assert_eq!(s, 1);
        assert!(super::pill(&laid, None, s).width <= 120 || laid.lines().any(|l| !l.contains(' ')));
    }

    #[test]
    fn a_click_finds_the_character_under_it_on_any_line() {
        let laid = "SCREENSHOT\nCOPIED - SHOW\n| EDIT";
        let p = super::pill(laid, None, 2);
        let edit = laid.find('|').unwrap();
        // the middle of the last line is the EDIT part, the first line is not
        assert!(super::char_at(laid, false, 2, p.width as f32 * 0.4, p.height as f32 - 8.0) >= edit);
        assert!(super::char_at(laid, false, 2, p.width as f32 * 0.4, 8.0) < edit);
        // outside the text it snaps to the nearest character
        assert_eq!(super::char_at(laid, false, 2, 0.0, 0.0), 0);
        assert_eq!(super::char_at(laid, false, 2, 1e6, 1e6), laid.chars().count() - 1);
    }

    #[test]
    fn the_banner_fills_its_width_wraps_its_text_and_keeps_room_for_the_close_cross() {
        let text = "HANDSET 0.8.1 IS OUT - CLICK TO COPY: BREW UPGRADE --CASK HANDSET";
        let (b, close_x) = super::banner(text, 3, 600);
        assert_eq!(b.width, 600);
        assert_eq!(b.rgba.len(), (b.width * b.height * 4) as usize);
        assert!(close_x > 450 && close_x < 600, "the cross sits at the right end ({close_x})");
        // solid blue in the middle, clear at the very corner
        let px = |x: u32, y: u32| &b.rgba[((y * b.width + x) * 4) as usize..][..4];
        assert_eq!(px(0, 0)[3], 0);
        assert!(px(300, 2)[2] > 200 && px(300, 2)[3] > 240);
        // two or three lines at full size, not one tiny one
        assert!(b.height > 3 * (2 * 4 + 7 + 3), "{}", b.height);
        if let Some(dir) = std::env::var_os("HANDSET_TEST_OUT") {
            let path = std::path::Path::new(&dir).join("banner.png");
            let mut enc = png::Encoder::new(std::fs::File::create(path).unwrap(), b.width, b.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&b.rgba).unwrap();
        }
    }

    /// Writes the long toast to `$HANDSET_TEST_OUT` (if set) so it can be looked at.
    #[test]
    fn long_toast_renders() {
        let p = super::pill("RECORDING COPIED - SHOW | EDIT", None, 3);
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
