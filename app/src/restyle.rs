//! Put a saved capture (PNG, MP4 or GIF) on a styled background and save the result next to it.
//!
//! The original is never touched: `handset-<time>.png` gains a sibling `handset-<time>-styled.png`.
//! Pictures are composed in process; video goes through ffmpeg, which lays the capture over a
//! backdrop picture that has a hole cut in it for the capture (see `Style::punch_hole`).

use crate::style::{Background, DARK, Img, Style};
use crate::trim::Trim;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::process::Stdio;

pub fn is_video(path: &Path) -> bool {
    matches!(ext(path).as_str(), "mp4" | "gif" | "mov" | "m4v")
}

fn ext(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// The outline of the phone in a recording, saved beside it as `<name>.matte` (a greyscale PNG).
///
/// A recording is flattened onto an opaque colour, so its own transparency is lost; the outline of the
/// framed phone does not change during a recording, so one picture of it is enough to cut the phone out
/// again when the recording is styled.
pub fn matte_path(video: &Path) -> PathBuf {
    video.with_extension("matte")
}

/// Save the alpha of a transparent capture as the matte of the recording at `video`.
pub fn save_matte(video: &Path, rgba: &[u8], w: u32, h: u32) -> Result<(), String> {
    let file = std::fs::File::create(matte_path(video)).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    let alpha: Vec<u8> = rgba.as_chunks::<4>().0.iter().map(|p| p[3]).collect();
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&alpha).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}

/// The edge pixels of a recording are blended with its dark backdrop, which shows as a faint dark rim once
/// the phone is on a light background. Shaving the soft edge of the outline hides most of it.
fn tighten(alpha: u8) -> u8 {
    ((alpha as f32 / 255.0 - 0.25) / 0.5).clamp(0.0, 1.0).mul_add(255.0, 0.0).round() as u8
}

/// The matte of a recording as an image whose alpha is the outline, if there is one of the right size.
pub fn load_matte(video: &Path, (w, h): (u32, u32)) -> Option<Img> {
    let g = read_png(&matte_path(video)).ok()?;
    // read_png turns grey into opaque RGBA: the alpha of the matte is its grey level
    (g.w == w && g.h == h).then(|| Img {
        w,
        h,
        rgba: g.rgba.as_chunks::<4>().0.iter().flat_map(|p| [0, 0, 0, tighten(p[0])]).collect(),
    })
}

/// Where the styled copy of `input` goes: `<stem>-styled[-N].<ext>` beside it.
pub fn styled_path(input: &Path) -> PathBuf {
    let dir = input.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("capture");
    crate::capture::unique(dir, &format!("{stem}-styled"), &ext(input))
}

/// Progress and cancellation of a running export, shared with the window that started it.
#[derive(Default)]
pub struct Control {
    /// 0..=1000.
    pub progress: std::sync::atomic::AtomicU32,
    pub cancel: std::sync::atomic::AtomicBool,
}

/// Style a saved capture and return the path of the new file.
pub fn export(input: &Path, style: &Style) -> Result<PathBuf, String> {
    export_with(input, style, None, &Control::default())
}

/// Like [`export`], keeping only `trim` of a recording (ignored for pictures) and reporting to `control`.
pub fn export_with(input: &Path, style: &Style, trim: Option<Trim>, control: &Control) -> Result<PathBuf, String> {
    let out = styled_path(input);
    let image = load_background(style)?;
    if is_video(input) {
        style_video(input, &out, style, image.as_ref(), trim, control)?;
    } else if ext(input) == "png" {
        let src = read_png(input)?;
        write_png(&out, &style.compose(&src, image.as_ref()))?;
    } else {
        return Err(format!("{}: only PNG, MP4 and GIF captures can be styled", input.display()));
    }
    Ok(out)
}

pub fn load_background(style: &Style) -> Result<Option<Img>, String> {
    match &style.background {
        Background::Image(p) => load_image(p).map(Some),
        _ => Ok(None),
    }
}

pub fn read_png(path: &Path) -> Result<Img, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut dec = png::Decoder::new(BufReader::new(file));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("picture too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
    let data = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => {
            data.as_chunks::<2>().0.iter().flat_map(|p| [p[0], p[0], p[0], p[1]]).collect()
        }
        png::ColorType::Indexed => return Err(format!("{}: unsupported PNG", path.display())),
    };
    Ok(Img { w: info.width, h: info.height, rgba })
}

pub fn write_png(path: &Path, img: &Img) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), img.w, img.h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&img.rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())
}

/// A picture to use as a background: PNG directly, anything else (JPEG, WebP, ...) converted by ffmpeg.
pub fn load_image(path: &Path) -> Result<Img, String> {
    if ext(path) == "png" {
        return read_png(path);
    }
    let tmp = scratch("png");
    let out = crate::tools::command(crate::tools::find("ffmpeg"))
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(path)
        .args(["-frames:v", "1"])
        .arg(&tmp)
        .output()
        .map_err(|e| format!("cannot run ffmpeg to read {} (is it installed?): {e}", path.display()))?;
    let img = if out.status.success() {
        read_png(&tmp)
    } else {
        Err(format!("cannot read {}: {}", path.display(), String::from_utf8_lossy(&out.stderr).trim()))
    };
    let _ = std::fs::remove_file(&tmp);
    img
}

fn scratch(ext: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    std::env::temp_dir().join(format!("handset-{}-{nanos}.{ext}", std::process::id()))
}

/// What ffmpeg's banner says about a video.
pub struct Info {
    pub size: (u32, u32),
    pub fps: f32,
    /// Seconds; 0 when unknown.
    pub duration: f64,
    pub has_audio: bool,
}

pub fn probe(path: &Path) -> Result<Info, String> {
    let out = crate::tools::command(crate::tools::find("ffmpeg"))
        .args(["-hide_banner", "-i"])
        .arg(path)
        .output()
        .map_err(|e| format!("cannot run ffmpeg (is it installed?): {e}"))?;
    let banner = String::from_utf8_lossy(&out.stderr);
    let size = parse_size(&banner).ok_or_else(|| format!("cannot read the size of {}", path.display()))?;
    Ok(Info {
        size,
        fps: parse_fps(&banner).unwrap_or(30.0),
        duration: parse_duration(&banner).unwrap_or(0.0),
        has_audio: parse_has_audio(&banner),
    })
}

/// Does ffmpeg list an audio stream?
fn parse_has_audio(banner: &str) -> bool {
    banner.lines().any(|l| l.trim_start().starts_with("Stream #") && l.contains("Audio:"))
}

fn parse_fps(banner: &str) -> Option<f32> {
    let line = banner.lines().find(|l| l.contains("Video:"))?;
    line.split(',').find_map(|part| part.trim().strip_suffix(" fps")?.parse::<f32>().ok()).filter(|f| *f > 0.0)
}

/// `Duration: 00:00:03.00, start: ...`
fn parse_duration(banner: &str) -> Option<f64> {
    let rest = banner.lines().find_map(|l| l.trim().strip_prefix("Duration: "))?;
    let mut parts = rest.split(',').next()?.trim().split(':');
    let (h, m, s) =
        (parts.next()?.parse::<f64>().ok()?, parts.next()?.parse::<f64>().ok()?, parts.next()?.parse::<f64>().ok()?);
    Some(h * 3600.0 + m * 60.0 + s)
}

fn parse_size(banner: &str) -> Option<(u32, u32)> {
    let line = banner.lines().find(|l| l.contains("Video:"))?;
    // `Video: h264 (High) (avc1 / 0x31637661), yuv420p(...), 720x1600 [SAR 1:1 ...], ...`
    line.split(|c: char| c == ',' || c.is_whitespace()).find_map(|tok| {
        let (w, h) = tok.split_once('x')?;
        let (w, h) = (w.parse::<u32>().ok()?, h.parse::<u32>().ok()?);
        (w >= 16 && h >= 16).then_some((w, h))
    })
}

/// A video cannot be transparent: styled with no background it sits on a dark colour.
pub fn video_style(style: &Style) -> Style {
    match style.background {
        Background::None => Style { background: Background::Color(DARK), ..style.clone() },
        _ => style.clone(),
    }
}

/// The first frame of a video, for previews.
pub fn first_frame(path: &Path) -> Result<Img, String> {
    let tmp = scratch("png");
    let out = crate::tools::command(crate::tools::find("ffmpeg"))
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(path)
        .args(["-frames:v", "1"])
        .arg(&tmp)
        .output()
        .map_err(|e| format!("cannot run ffmpeg (is it installed?): {e}"))?;
    let img = if out.status.success() {
        read_png(&tmp)
    } else {
        Err(format!("cannot read {}: {}", path.display(), String::from_utf8_lossy(&out.stderr).trim()))
    };
    let _ = std::fs::remove_file(&tmp);
    img
}

fn style_video(
    input: &Path,
    out: &Path,
    style: &Style,
    image: Option<&Img>,
    trim: Option<Trim>,
    control: &Control,
) -> Result<(), String> {
    let info = probe(input)?;
    let size = info.size;
    let matte = load_matte(input, size);
    let mut layout = style.layout(size);
    if matte.is_some() {
        // the phone's own outline is the shape: no rounded rectangle on top of it
        layout.radius = 0.0;
    }
    let style = &video_style(style);
    let mut backdrop = style.backdrop(&layout, matte.as_ref(), image);
    style.punch_hole(&mut backdrop, &layout, matte.as_ref());
    let bd_path = scratch("png");
    write_png(&bd_path, &Img { w: layout.out.0, h: layout.out.1, rgba: backdrop })?;

    let gif = ext(out) == "gif";
    let (ow, oh) = layout.out;
    // the capture goes onto a canvas of the output size; the backdrop, with its hole, goes over that
    let mut graph = format!(
        "[0:v]format=rgba,pad={ow}:{oh}:{}:{}:color=black[v];[v][1:v]overlay=format=auto:shortest=1",
        layout.x, layout.y
    );
    if gif {
        graph.push_str(",fps=15,scale='min(720,iw)':-2:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse[o]");
    } else {
        graph.push_str(",format=yuv420p[o]");
    }
    let mut cmd = crate::tools::command(crate::tools::find("ffmpeg"));
    cmd.args(["-hide_banner", "-loglevel", "error", "-nostats", "-progress", "pipe:1", "-y"]);
    if let Some(t) = trim {
        // cut at the input, so only the kept part is decoded and styled
        cmd.args(["-ss", &format!("{:.3}", t.start), "-t", &format!("{:.3}", t.len())]);
    }
    cmd.arg("-i").arg(input);
    cmd.args(["-loop", "1", "-i"]).arg(&bd_path);
    cmd.args(["-filter_complex", &graph, "-map", "[o]"]);
    if !gif {
        cmd.args(["-map", "0:a?", "-c:a", "copy"]);
        cmd.args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-movflags", "+faststart"]);
    }
    cmd.arg(out).stdout(Stdio::piped()).stderr(Stdio::piped());
    let result = run_with_progress(cmd, trim.map_or(info.duration, |t| t.len()), control);
    let _ = std::fs::remove_file(&bd_path);
    if result.is_err() {
        let _ = std::fs::remove_file(out);
    }
    result
}

/// Run ffmpeg, publishing how far it is (from `-progress`) and stopping it when asked to.
fn run_with_progress(mut cmd: std::process::Command, duration: f64, control: &Control) -> Result<(), String> {
    use std::io::{BufRead, Read};
    use std::sync::atomic::Ordering;
    let mut child = cmd.spawn().map_err(|e| format!("cannot run ffmpeg (is it installed?): {e}"))?;
    let mut stderr = child.stderr.take();
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(e) = stderr.as_mut() {
            let _ = e.read_to_string(&mut text);
        }
        text
    });
    if let Some(out) = child.stdout.take() {
        for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
            if control.cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Cancelled".into());
            }
            if let Some(us) = line.strip_prefix("out_time_us=").and_then(|v| v.trim().parse::<f64>().ok())
                && duration > 0.0
            {
                control.progress.store(((us / 1e6 / duration) * 1000.0).clamp(0.0, 1000.0) as u32, Ordering::Relaxed);
            }
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let stderr = errors.join().unwrap_or_default();
    if control.cancel.load(Ordering::Relaxed) {
        Err("Cancelled".into())
    } else if status.success() {
        control.progress.store(1000, Ordering::Relaxed);
        Ok(())
    } else {
        Err(format!("ffmpeg failed: {}", stderr.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ffmpeg_present() -> bool {
        crate::tools::command(crate::tools::find("ffmpeg")).arg("-version").output().is_ok_and(|o| o.status.success())
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("handset-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn export_reports_progress_and_can_be_cancelled() {
        if !ffmpeg_present() {
            return;
        }
        use std::sync::atomic::Ordering;
        let dir = temp_dir("progress");
        let src = dir.join("clip.mp4");
        assert!(
            crate::tools::command(crate::tools::find("ffmpeg"))
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=160x240:rate=10:duration=2"
                ])
                .args(["-pix_fmt", "yuv420p"])
                .arg(&src)
                .status()
                .unwrap()
                .success()
        );
        let done = Control::default();
        let out = export_with(&src, &Style::default(), None, &done).unwrap();
        assert!(out.exists());
        assert_eq!(done.progress.load(Ordering::Relaxed), 1000);

        let stopped = Control::default();
        stopped.cancel.store(true, Ordering::Relaxed);
        let err = export_with(&src, &Style::default(), None, &stopped).unwrap_err();
        assert_eq!(err, "Cancelled");
        assert!(!dir.join("clip-styled-2.mp4").exists(), "a cancelled export leaves no file behind");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_trimmed_export_keeps_only_that_part_with_its_audio() {
        if !ffmpeg_present() {
            return;
        }
        let dir = temp_dir("trim");
        let src = dir.join("clip.mp4");
        assert!(
            crate::tools::command(crate::tools::find("ffmpeg"))
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=160x240:rate=10:duration=4"
                ])
                .args(["-f", "lavfi", "-i", "sine=duration=4", "-pix_fmt", "yuv420p", "-shortest"])
                .arg(&src)
                .status()
                .unwrap()
                .success()
        );
        let whole = export(&src, &Style::default()).unwrap();
        let cut =
            export_with(&src, &Style::default(), Some(Trim { start: 1.0, end: 2.5 }), &Control::default()).unwrap();
        let (whole, cut) = (probe(&whole).unwrap(), probe(&cut).unwrap());
        assert!((whole.duration - 4.0).abs() < 0.3, "untrimmed export keeps all of it: {}", whole.duration);
        assert!((cut.duration - 1.5).abs() < 0.3, "trimmed export is 1.5 s long: {}", cut.duration);
        assert_eq!(cut.size, whole.size);
        let banner = crate::tools::command(crate::tools::find("ffmpeg"))
            .args(["-hide_banner", "-i"])
            .arg(dir.join("clip-styled-2.mp4"))
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&banner.stderr).contains("Audio:"), "the audio is kept");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tightening_keeps_solid_and_clear_and_shaves_the_edge() {
        assert_eq!((tighten(0), tighten(255)), (0, 255));
        assert_eq!(tighten(40), 0);
        assert!(tighten(128) > 100 && tighten(128) < 160);
    }

    #[test]
    fn reads_the_size_from_ffmpegs_banner() {
        let banner = "  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(tv, bt709), 720x1600 [SAR 1:1 DAR 9:20], 1234 kb/s, 30 fps";
        assert_eq!(parse_size(banner), Some((720, 1600)));
        assert_eq!(parse_size("Audio: aac"), None);
    }

    #[test]
    fn spots_an_audio_stream_in_the_banner() {
        let with =
            "  Stream #0:0: Video: h264, yuv420p, 720x1600\n  Stream #0:1[0x2](und): Audio: aac (LC), 44100 Hz, mono";
        assert!(parse_has_audio(with));
        assert!(!parse_has_audio("  Stream #0:0: Video: h264, yuv420p, 720x1600"));
        // the word in a metadata line is not a stream
        assert!(!parse_has_audio("    comment: Audio: none\n  Stream #0:0: Video: gif"));
    }

    #[test]
    fn probe_tells_a_recording_with_sound_from_one_without() {
        if !ffmpeg_present() {
            return;
        }
        let dir = temp_dir("probe-audio");
        let (loud, quiet) = (dir.join("loud.mp4"), dir.join("quiet.mp4"));
        let ff = |extra: &[&str], out: &Path| {
            crate::tools::command(crate::tools::find("ffmpeg"))
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=64x64:rate=10:duration=1",
                ])
                .args(extra)
                .args(["-pix_fmt", "yuv420p"])
                .arg(out)
                .status()
                .unwrap()
                .success()
        };
        assert!(ff(&["-f", "lavfi", "-i", "sine=duration=1", "-shortest"], &loud));
        assert!(ff(&[], &quiet));
        assert!(probe(&loud).unwrap().has_audio);
        assert!(!probe(&quiet).unwrap().has_audio);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reads_fps_and_duration_from_the_banner() {
        let banner = "  Duration: 00:01:02.50, start: 0.000000, bitrate: 346 kb/s\n  Stream #0:0: Video: h264, yuv420p, 720x1600, 1234 kb/s, 29.97 fps, 30 tbr";
        assert_eq!(parse_duration(banner), Some(62.5));
        assert_eq!(parse_fps(banner), Some(29.97));
        assert_eq!(parse_duration("no duration here"), None);
    }

    #[test]
    fn styled_copy_sits_beside_the_original() {
        let p = styled_path(Path::new("/tmp/nowhere/handset-1.png"));
        assert_eq!(p, Path::new("/tmp/nowhere/handset-1-styled.png"));
    }

    #[test]
    fn styles_a_png_and_leaves_the_original() {
        let dir = temp_dir("png");
        let src = dir.join("shot.png");
        write_png(&src, &Img { w: 100, h: 200, rgba: [0u8, 128, 255, 255].repeat(100 * 200) }).unwrap();
        let before = std::fs::read(&src).unwrap();
        let out = export(&src, &Style { aspect: crate::style::Aspect::Ratio(1, 1), ..Style::default() }).unwrap();
        assert_eq!(out, dir.join("shot-styled.png"));
        let styled = read_png(&out).unwrap();
        assert_eq!(styled.w, styled.h);
        assert_eq!(std::fs::read(&src).unwrap(), before);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_matte_cuts_the_phone_out_of_the_recording() {
        if !ffmpeg_present() {
            return;
        }
        let dir = temp_dir("matte");
        for ext in ["mp4", "gif"] {
            matte_cuts_out(&dir, ext);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    fn matte_cuts_out(dir: &Path, ext: &str) {
        let src = dir.join(format!("clip.{ext}"));
        assert!(
            crate::tools::command(crate::tools::find("ffmpeg"))
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=160x240:rate=10:duration=1"
                ])
                .args(["-pix_fmt", "yuv420p"])
                .arg(&src)
                .status()
                .unwrap()
                .success()
        );
        // the left half of the picture is outside the phone
        let rgba: Vec<u8> = (0..160 * 240).flat_map(|i| [0, 0, 0, if i % 160 < 80 { 0 } else { 255 }]).collect();
        save_matte(&src, &rgba, 160, 240).unwrap();
        let style = Style { background: Background::Color([200, 0, 0]), shadow: 0, radius: 20, ..Style::default() };
        let out = export(&src, &style).unwrap();
        let png = dir.join("frame.png");
        assert!(
            crate::tools::command(crate::tools::find("ffmpeg"))
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(&out)
                .args(["-frames:v", "1"])
                .arg(&png)
                .status()
                .unwrap()
                .success()
        );
        let f = read_png(&png).unwrap();
        let l = style.layout((160, 240));
        let at = |x: u32, y: u32| f.rgba[(((l.y + y) * f.w + l.x + x) * 4) as usize..][..3].to_vec();
        let outside = at(10, 120);
        assert!(outside[0] > 180 && outside[1] < 40, "outside the matte shows the background: {outside:?}");
        assert_ne!(at(150, 120), outside, "inside the matte shows the video");
    }

    #[test]
    fn styles_mp4_and_gif_through_ffmpeg() {
        if !ffmpeg_present() {
            return;
        }
        let dir = temp_dir("video");
        for ext in ["mp4", "gif"] {
            let src = dir.join(format!("clip.{ext}"));
            let ok = crate::tools::command(crate::tools::find("ffmpeg"))
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=160x240:rate=10:duration=1",
                ])
                .args(["-pix_fmt", "yuv420p"])
                .arg(&src)
                .status()
                .unwrap()
                .success();
            assert!(ok);
            let style = Style { aspect: crate::style::Aspect::Ratio(16, 9), ..Style::default() };
            let out = export(&src, &style).unwrap();
            let want = style.layout((160, 240)).out;
            let got = probe(&out).unwrap().size;
            if ext == "mp4" {
                assert_eq!(got, want);
            } else {
                assert_eq!(got.0, want.0.min(720));
            }
            assert!(src.exists());
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
