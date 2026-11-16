//! What is on the phone's screen as text: the elements, their labels and where to tap them.
//!
//! Android's own `uiautomator dump` describes the screen as XML. This reads it, keeps the elements an agent
//! can use, and turns their positions into pixels of the picture Handset shows (which may be scaled down
//! from the real screen), so `tap-text "Sign in"` lands where `screenshot` shows the button.

use std::io::Read;
use std::process::Stdio;
use std::time::{Duration, Instant};

/// The longest a dump may take; a busy phone needs a few seconds.
const LIMIT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub class: String,
    pub text: String,
    pub desc: String,
    pub id: String,
    pub package: String,
    /// left, top, right, bottom, in screen pixels
    pub bounds: (i32, i32, i32, i32),
    pub clickable: bool,
    pub scrollable: bool,
    pub checked: bool,
    pub focused: bool,
    pub enabled: bool,
    pub password: bool,
}

impl Node {
    /// What a person would call it: its text, else what it says to a screen reader, else its id.
    pub fn label(&self) -> &str {
        [&self.text, &self.desc].into_iter().find(|s| !s.is_empty()).map_or_else(|| self.short_id(), |s| s.as_str())
    }

    /// `com.app:id/login` is `login`.
    pub fn short_id(&self) -> &str {
        self.id.rsplit_once('/').map_or(self.id.as_str(), |(_, s)| s)
    }

    fn kind(&self) -> &str {
        self.class.rsplit('.').next().unwrap_or(&self.class)
    }

    /// Worth showing: it says something, or it can be used.
    fn useful(&self) -> bool {
        let (l, t, r, b) = self.bounds;
        r > l
            && b > t
            && (!self.text.is_empty()
                || !self.desc.is_empty()
                || self.clickable
                || self.scrollable
                || self.kind() == "EditText")
    }

    /// The middle of it, in screen pixels.
    pub fn center(&self) -> (i32, i32) {
        ((self.bounds.0 + self.bounds.2) / 2, (self.bounds.1 + self.bounds.3) / 2)
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct Tree {
    pub nodes: Vec<Node>,
    /// 0 to 3, quarter turns
    pub rotation: u8,
    /// The size of the real screen, as the phone reports it (`wm size`), before it is turned.
    pub display: Option<(i32, i32)>,
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';') else { break };
        let ent = &rest[1..end];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The attributes of one tag: `a="1" b="2"`. A value may hold a `>`.
fn attributes(tag: &str) -> Vec<(&str, String)> {
    let mut out = Vec::new();
    let mut rest = tag;
    while let Some(eq) = rest.find("=\"") {
        let name = rest[..eq].rsplit(char::is_whitespace).next().unwrap_or("");
        let after = &rest[eq + 2..];
        let Some(close) = after.find('"') else { break };
        out.push((name, unescape(&after[..close])));
        rest = &after[close + 1..];
    }
    out
}

fn bounds(s: &str) -> Option<(i32, i32, i32, i32)> {
    // [l,t][r,b]
    let n: Vec<i32> =
        s.split(['[', ']', ',']).filter(|p| !p.is_empty()).map(str::parse).collect::<Result<_, _>>().ok()?;
    (n.len() == 4).then(|| (n[0], n[1], n[2], n[3]))
}

/// Read `uiautomator dump` output (anything before the XML, such as `wm size`, is ignored).
pub fn parse(text: &str) -> Result<Tree, String> {
    let start = text.find("<hierarchy").ok_or("the phone gave no description of the screen")?;
    let xml = &text[start..];
    let mut tree = Tree::default();
    let mut at = 0;
    while let Some(open) = xml[at..].find('<') {
        let from = at + open + 1;
        // the end of the tag, skipping a `>` inside a quoted value
        let mut quoted = false;
        let Some(len) = xml[from..].find(|c| {
            if c == '"' {
                quoted = !quoted;
            }
            c == '>' && !quoted
        }) else {
            break;
        };
        let tag = &xml[from..from + len];
        at = from + len + 1;
        if let Some(h) = tag.strip_prefix("hierarchy") {
            tree.rotation =
                attributes(h).iter().find(|(k, _)| *k == "rotation").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
        } else if let Some(n) = tag.strip_prefix("node ") {
            let mut node = Node { enabled: true, ..Default::default() };
            for (k, v) in attributes(n) {
                let on = v == "true";
                match k {
                    "class" => node.class = v,
                    "text" => node.text = v,
                    "content-desc" => node.desc = v,
                    "resource-id" => node.id = v,
                    "package" => node.package = v,
                    "bounds" => node.bounds = bounds(&v).unwrap_or_default(),
                    "clickable" => node.clickable = on,
                    "scrollable" => node.scrollable = on,
                    "checked" => node.checked = on,
                    "focused" => node.focused = on,
                    "enabled" => node.enabled = on,
                    "password" => node.password = on,
                    _ => {}
                }
            }
            tree.nodes.push(node);
        }
    }
    if tree.nodes.is_empty() {
        return Err("nothing is on the screen that Android can describe".into());
    }
    tree.display = text[..start].lines().filter_map(size_line).next_back();
    Ok(tree)
}

/// `Physical size: 720x1600`, or the `Override size:` that replaces it.
fn size_line(line: &str) -> Option<(i32, i32)> {
    let v = line.strip_prefix("Override size:").or_else(|| line.strip_prefix("Physical size:"))?;
    let (w, h) = v.trim().split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

impl Tree {
    /// The elements an agent can use, in reading order. A tappable row with no words of its own (a list item
    /// made of a few text views) is folded into the words inside it: they become the tappable thing.
    pub fn useful(&self) -> Vec<Node> {
        let mut nodes: Vec<Node> = self.nodes.iter().filter(|n| n.useful()).cloned().collect();
        let holders: Vec<(i32, i32, i32, i32)> =
            nodes.iter().filter(|n| n.clickable && n.label().is_empty()).map(|n| n.bounds).collect();
        // wholly inside the row: a scroll view that merely passes under it is not part of it
        let inside = |b: &(i32, i32, i32, i32), n: &Node| {
            n.bounds.0 >= b.0 && n.bounds.1 >= b.1 && n.bounds.2 <= b.2 && n.bounds.3 <= b.3
        };
        let mut used = vec![false; holders.len()];
        for n in nodes.iter_mut().filter(|n| !n.label().is_empty()) {
            for (i, b) in holders.iter().enumerate() {
                if inside(b, n) {
                    n.clickable = true;
                    used[i] = true;
                }
            }
        }
        let folded: Vec<_> = holders.iter().zip(&used).filter(|(_, u)| **u).map(|(b, _)| *b).collect();
        nodes.retain(|n| !(n.clickable && n.label().is_empty() && folded.contains(&n.bounds)));
        nodes
    }

    /// The factor from screen pixels to the picture's pixels, given the picture's width.
    pub fn scale(&self, picture_width: u16) -> f32 {
        let Some((w, h)) = self.display else { return 1.0 };
        // the phone reports its size upright; turned on its side, the picture is as wide as the screen is tall
        let across = if self.rotation % 2 == 1 { h } else { w };
        if across <= 0 { 1.0 } else { picture_width as f32 / across as f32 }
    }

    /// The list an agent reads: one line per element with where to tap it, in the picture's pixels.
    pub fn render(&self, picture: (u16, u16)) -> String {
        let scale = self.scale(picture.0);
        let at = |n: &Node| {
            let (x, y) = n.center();
            ((x as f32 * scale).round() as i32, (y as f32 * scale).round() as i32)
        };
        let package = self.nodes.first().map_or("", |n| n.package.as_str());
        let mut out = format!("screen {}x{} package {package}\n", picture.0, picture.1);
        for (i, n) in self.useful().iter().enumerate() {
            let (x, y) = at(n);
            out.push_str(&format!("{} {} {:?} @{x},{y}", i + 1, n.kind(), n.label()));
            for (on, word) in [
                (n.clickable, "clickable"),
                (n.scrollable, "scrollable"),
                (n.checked, "checked"),
                (n.focused, "focused"),
                (!n.enabled, "disabled"),
                (n.password, "password"),
            ] {
                if on {
                    out.push(' ');
                    out.push_str(word);
                }
            }
            if !n.id.is_empty() && n.label() != n.short_id() {
                out.push_str(&format!(" id={}", n.short_id()));
            }
            out.push('\n');
        }
        out
    }

    /// What to add to "not found" when the screen scrolls: it may be past the edge.
    fn further_down(&self) -> &'static str {
        if self.nodes.iter().any(|n| n.scrollable) { "; the screen scrolls, so it may be out of view" } else { "" }
    }

    /// Whether `text` is on the screen (an element whose words contain it, any case), and where: for waiting.
    pub fn has_text(&self, text: &str) -> Option<&Node> {
        let want = text.to_lowercase();
        let seen = |n: &&Node| n.bounds.2 > n.bounds.0 && n.bounds.3 > n.bounds.1;
        let says = |n: &Node, exact: bool| {
            [&n.text, &n.desc].iter().any(|s| {
                !s.is_empty() && if exact { s.to_lowercase() == want } else { s.to_lowercase().contains(&want) }
            })
        };
        let mut seen_nodes = self.nodes.iter().filter(seen);
        seen_nodes.clone().find(|n| says(n, true)).or_else(|| seen_nodes.find(|n| says(n, false)))
    }

    /// The one element to tap for `text`: an exact match, else one that contains it (any case). Several that
    /// fit is an error that lists them, so the agent can pick by position instead.
    pub fn find_text(&self, text: &str) -> Result<&Node, String> {
        let want = text.to_lowercase();
        let named: Vec<&Node> =
            self.nodes.iter().filter(|n| n.bounds.2 > n.bounds.0 && n.bounds.3 > n.bounds.1).collect();
        for exact in [true, false] {
            let hits: Vec<&Node> = named
                .iter()
                .copied()
                .filter(|n| {
                    [&n.text, &n.desc].iter().any(|s| {
                        !s.is_empty() && if exact { s.to_lowercase() == want } else { s.to_lowercase().contains(&want) }
                    })
                })
                .collect();
            if let Some(n) = pick(&hits)? {
                return Ok(n);
            }
        }
        Err(format!("nothing on the screen says {text:?}{} (see `handset ctl ui`)", self.further_down()))
    }

    /// The element whose id is `id`, with or without its package (`login` or `com.app:id/login`).
    pub fn find_id(&self, id: &str) -> Result<&Node, String> {
        let hits: Vec<&Node> = self
            .nodes
            .iter()
            .filter(|n| !n.id.is_empty() && (n.id == id || n.short_id() == id) && n.bounds.2 > n.bounds.0)
            .collect();
        pick(&hits)?.ok_or_else(|| {
            format!("nothing on the screen has the id {id:?}{} (see `handset ctl ui`)", self.further_down())
        })
    }
}

/// One of several matches: if only one can be tapped, that one; if still several, say so.
fn pick<'a>(hits: &[&'a Node]) -> Result<Option<&'a Node>, String> {
    match hits {
        [] => Ok(None),
        [one] => Ok(Some(one)),
        many => {
            let tappable: Vec<&&Node> = many.iter().filter(|n| n.clickable).collect();
            if let [one] = tappable.as_slice() {
                return Ok(Some(one));
            }
            let list: Vec<String> =
                many.iter().take(5).map(|n| format!("{:?} at {},{}", n.label(), n.center().0, n.center().1)).collect();
            Err(format!("{} elements match ({}); use `tap` with a position from `ui`", many.len(), list.join("; ")))
        }
    }
}

/// A small grey copy of the picture, to tell whether the screen has changed since a dump was taken. A phone at
/// rest still shows tiny differences from frame to frame, so two copies count as the same when they are close.
#[derive(Debug, Clone, Default)]
pub struct Thumb {
    cells: Vec<u8>,
    size: (u32, u32),
}

const GRID: (u32, u32) = (24, 48);

impl Thumb {
    pub fn of(y: &crate::render::Yuv) -> Thumb {
        let (w, h) = (y.width, y.height);
        if w < GRID.0 || h < GRID.1 {
            return Thumb::default();
        }
        let mut cells = Vec::with_capacity((GRID.0 * GRID.1) as usize);
        for gy in 0..GRID.1 {
            for gx in 0..GRID.0 {
                // the mean of a few points in the cell
                let (mut sum, mut n) = (0u32, 0u32);
                for sy in 0..3 {
                    for sx in 0..3 {
                        let px = (gx * w + (sx * 2 + 1) * w / 6) / GRID.0;
                        let py = (gy * h + (sy * 2 + 1) * h / 6) / GRID.1;
                        if let Some(v) = y.y.get((py * y.stride_y + px) as usize) {
                            sum += *v as u32;
                            n += 1;
                        }
                    }
                }
                cells.push(sum.checked_div(n).unwrap_or(0) as u8);
            }
        }
        Thumb { cells, size: (w, h) }
    }

    /// No picture yet, so nothing to compare with.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The same screen: the same size, and almost no difference anywhere.
    pub fn same(&self, other: &Thumb) -> bool {
        if self.is_empty() || self.size != other.size {
            return false;
        }
        let diffs = self.cells.iter().zip(&other.cells).map(|(a, b)| a.abs_diff(*b) as u32);
        let (total, worst) = diffs.fold((0, 0), |(t, m), d| (t + d, m.max(d)));
        total as f32 / self.cells.len() as f32 <= 1.5 && worst <= 30
    }
}

/// Ask the phone for its screen. Retried once: Android refuses while something is still animating.
pub fn dump(serial: &str) -> Result<Tree, String> {
    let mut last = String::new();
    for attempt in 0..2 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(600));
        }
        match run_dump(serial).and_then(|t| parse(&t)) {
            Ok(tree) => return Ok(tree),
            Err(e) => last = e,
        }
    }
    Err(last)
}

fn run_dump(serial: &str) -> Result<String, String> {
    let mut cmd = crate::adb::command(serial);
    cmd.args(["exec-out", "sh", "-c", "wm size; uiautomator dump --compressed /dev/tty"]);
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("adb: {e}"))?;
    // read while it runs: a big screen fills the pipe, and a child that cannot write never finishes
    let mut out = child.stdout.take().ok_or("adb gave no output")?;
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = out.read_to_string(&mut text);
        text
    });
    let deadline = Instant::now() + LIMIT;
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("the phone did not describe its screen in time".into());
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    let text = reader.join().unwrap_or_default();
    if text.contains("<hierarchy") {
        Ok(text)
    } else if text.contains("idle state") {
        Err("the screen is moving (an animation or a game), so Android could not read it; try again, or use `screenshot`".into())
    } else {
        Err(format!("Android could not describe the screen: {}", text.lines().last().unwrap_or("no answer").trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(class: &str, text: &str, id: &str, bounds: &str, clickable: bool) -> String {
        format!(
            "<node index=\"0\" text=\"{text}\" resource-id=\"{id}\" class=\"{class}\" package=\"com.app\" content-desc=\"\" clickable=\"{clickable}\" enabled=\"true\" scrollable=\"false\" checked=\"false\" focused=\"false\" password=\"false\" bounds=\"{bounds}\" />"
        )
    }

    fn dump_of(nodes: &[String], rotation: u8) -> String {
        format!(
            "Physical size: 720x1600\n<?xml version='1.0' ?><hierarchy rotation=\"{rotation}\">{}</hierarchy>UI hierchary dumped to: /dev/tty",
            nodes.concat()
        )
    }

    #[test]
    fn entities_and_quoted_angle_brackets_are_read() {
        assert_eq!(unescape("Tom &amp; Jerry &lt;3 &#10;&#x41;&quot;&apos;"), "Tom & Jerry <3 \nA\"'");
        assert_eq!(unescape("a &bogus; b & c"), "a &bogus; b & c");
        let t = parse(&dump_of(
            &[node("android.widget.Button", "a &gt; b &amp; c", "com.app:id/go", "[0,0][100,50]", true)],
            0,
        ))
        .unwrap();
        assert_eq!(t.nodes[0].text, "a > b & c");
        let odd = "<hierarchy rotation=\"0\"><node text=\"x > y\" class=\"a.B\" bounds=\"[0,0][10,10]\" /></hierarchy>";
        assert_eq!(parse(odd).unwrap().nodes[0].text, "x > y");
    }

    #[test]
    fn the_screen_is_listed_in_the_pictures_pixels() {
        let xml = dump_of(
            &[
                node("android.widget.FrameLayout", "", "", "[0,0][720,1520]", false),
                node("android.widget.TextView", "Welcome", "", "[0,100][720,200]", false),
                node("android.widget.Button", "Sign in", "com.app:id/login", "[100,800][500,900]", true),
            ],
            0,
        );
        let t = parse(&xml).unwrap();
        assert_eq!(t.display, Some((720, 1600)));
        // the picture is half the size of the screen
        let text = t.render((360, 800));
        assert!(text.starts_with("screen 360x800 package com.app\n"), "{text}");
        assert!(text.contains("1 TextView \"Welcome\" @180,75\n"), "{text}");
        assert!(text.contains("2 Button \"Sign in\" @150,425 clickable id=login\n"), "{text}");
        assert!(!text.contains("FrameLayout"), "containers with nothing to say are left out");
    }

    #[test]
    fn a_tappable_row_is_folded_into_the_words_inside_it() {
        let t = parse(&dump_of(
            &[
                node("android.widget.LinearLayout", "", "", "[0,400][720,500]", true),
                node("android.widget.TextView", "Network", "com.app:id/title", "[100,410][400,450]", false),
                node("android.widget.TextView", "Wi-Fi", "com.app:id/summary", "[100,450][400,490]", false),
                node("android.widget.LinearLayout", "", "", "[0,600][720,700]", true),
            ],
            0,
        ))
        .unwrap();
        let text = t.render((720, 1600));
        assert!(text.contains("1 TextView \"Network\" @250,430 clickable id=title"), "{text}");
        assert!(text.contains("2 TextView \"Wi-Fi\" @250,470 clickable"), "{text}");
        assert!(!text.contains("LinearLayout \"\" @360,450"), "the row with words is folded away: {text}");
        assert!(text.contains("LinearLayout \"\" @360,650 clickable"), "an empty tappable thing is kept: {text}");
    }

    #[test]
    fn a_turned_phone_scales_by_its_long_side() {
        let t = parse(&dump_of(&[node("a.B", "x", "", "[0,0][10,10]", true)], 1)).unwrap();
        assert_eq!(t.scale(1600), 1.0); // wide picture of a screen 1600 across when turned
        assert_eq!(t.scale(800), 0.5);
        let up = parse(&dump_of(&[node("a.B", "x", "", "[0,0][10,10]", true)], 0)).unwrap();
        assert_eq!(up.scale(360), 0.5);
        assert_eq!(
            parse("<hierarchy rotation=\"0\"><node text=\"x\" bounds=\"[0,0][1,1]\"/></hierarchy>").unwrap().scale(360),
            1.0
        );
    }

    #[test]
    fn text_is_found_exactly_then_loosely_and_ambiguity_is_explained() {
        let t = parse(&dump_of(
            &[
                node("android.widget.Button", "Sign in", "", "[0,0][100,50]", true),
                node("android.widget.TextView", "Sign in to continue", "", "[0,60][100,110]", false),
                node("android.widget.Button", "Cancel", "com.app:id/cancel", "[0,120][100,170]", true),
                node("android.widget.TextView", "Cancel", "", "[0,200][100,250]", false),
                node("android.widget.TextView", "Help", "", "[0,300][100,350]", false),
                node("android.widget.TextView", "Help", "", "[0,400][100,450]", false),
            ],
            0,
        ))
        .unwrap();
        assert_eq!(t.find_text("sign in").unwrap().text, "Sign in"); // exact beats a longer text
        assert_eq!(t.find_text("continue").unwrap().text, "Sign in to continue");
        assert_eq!(t.find_text("Cancel").unwrap().id, "com.app:id/cancel"); // only one can be tapped
        let e = t.find_text("Help").unwrap_err();
        assert!(e.contains("2 elements match") && e.contains("at 50,325"), "{e}");
        assert!(t.find_text("Nope").unwrap_err().contains("nothing on the screen says"));
        assert_eq!(t.find_id("cancel").unwrap().text, "Cancel");
        assert_eq!(t.find_id("com.app:id/cancel").unwrap().text, "Cancel");
        assert!(t.find_id("missing").is_err());
    }

    fn grey(w: u32, h: u32, f: impl Fn(u32, u32) -> u8) -> crate::render::Yuv {
        let mut y = vec![0u8; (w * h) as usize];
        for r in 0..h {
            for c in 0..w {
                y[(r * w + c) as usize] = f(c, r);
            }
        }
        crate::render::Yuv { width: w, height: h, y, u: vec![], v: vec![], stride_y: w, stride_uv: w / 2 }
    }

    #[test]
    fn a_still_screen_is_the_same_even_with_noise_and_a_changed_one_is_not() {
        let base = Thumb::of(&grey(240, 480, |c, r| ((c / 8 + r / 8) % 200) as u8));
        let noisy = Thumb::of(&grey(240, 480, |c, r| ((c / 8 + r / 8) % 200) as u8 + (c % 2) as u8));
        let moved = Thumb::of(&grey(240, 480, |c, r| ((c / 8 + r / 8 + 60) % 200) as u8));
        let other_size = Thumb::of(&grey(480, 240, |c, r| ((c / 8 + r / 8) % 200) as u8));
        assert!(base.same(&base) && base.same(&noisy));
        assert!(!base.same(&moved) && !base.same(&other_size));
        assert!(!Thumb::default().same(&Thumb::default()), "no picture is never the same as no picture");
    }

    #[test]
    fn waiting_looks_for_words_and_not_finding_them_hints_at_scrolling() {
        let mut scroller = node("android.widget.ScrollView", "", "", "[0,0][720,1500]", false);
        scroller = scroller.replace("scrollable=\"false\"", "scrollable=\"true\"");
        let t = parse(&dump_of(
            &[scroller, node("android.widget.TextView", "Loading results", "", "[0,0][100,50]", false)],
            0,
        ))
        .unwrap();
        assert_eq!(t.has_text("loading").unwrap().text, "Loading results");
        assert!(t.has_text("done").is_none());
        assert!(t.find_text("Battery").unwrap_err().contains("it may be out of view"));
        assert!(t.find_id("nope").unwrap_err().contains("it may be out of view"));
        let still = parse(&dump_of(&[node("android.widget.TextView", "x", "", "[0,0][10,10]", false)], 0)).unwrap();
        assert!(!still.find_text("Battery").unwrap_err().contains("scrolls"));
    }

    #[test]
    fn what_cannot_be_read_is_an_error() {
        assert!(parse("ERROR: could not get idle state.").is_err());
        assert!(parse("<hierarchy rotation=\"0\"></hierarchy>").is_err());
    }
}
