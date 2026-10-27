//! Put a saved capture (PNG, MP4 or GIF) on a styled background and save the result next to it.
//!
//! The original is never touched: `handset-<time>.png` gains a sibling `handset-<time>-styled.png`.
//! Pictures are composed in process; video goes through ffmpeg, which lays the capture over a
//! backdrop picture that has a hole cut in it for the capture (see `Style::punch_hole`).

use crate::style::{Background, DARK, Img, Style};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

pub fn is_video(path: &Path) -> bool {
    matches!(ext(path).as_str(), "mp4" | "gif" | "mov" | "m4v")
}

fn ext(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

/// Where the styled copy of `input` goes: `<stem>-styled[-N].<ext>` beside it.
pub fn styled_path(input: &Path) -> PathBuf {
    let dir = input.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("capture");
    crate::capture::unique(dir, &format!("{stem}-styled"), &ext(input))
}

/// Style a saved capture and return the path of the new file.
pub fn export(input: &Path, style: &Style) -> Result<PathBuf, String> {
    let out = styled_path(input);
    let image = load_background(style)?;
    if is_video(input) {
        style_video(input, &out, style, image.as_ref())?;
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

/// Width and height of the first video stream, read from ffmpeg's banner.
pub fn probe_size(path: &Path) -> Result<(u32, u32), String> {
    let out = crate::tools::command(crate::tools::find("ffmpeg"))
        .args(["-hide_banner", "-i"])
        .arg(path)
        .output()
        .map_err(|e| format!("cannot run ffmpeg (is it installed?): {e}"))?;
    parse_size(&String::from_utf8_lossy(&out.stderr))
        .ok_or_else(|| format!("cannot read the size of {}", path.display()))
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

fn style_video(input: &Path, out: &Path, style: &Style, image: Option<&Img>) -> Result<(), String> {
    let size = probe_size(input)?;
    let layout = style.layout(size);
    // a video cannot be transparent
    let solid;
    let style = if style.background == Background::None {
        solid = Style { background: Background::Color(DARK), ..style.clone() };
        &solid
    } else {
        style
    };
    let mut backdrop = style.backdrop(&layout, None, image);
    style.punch_hole(&mut backdrop, &layout);
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
    cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-i"]).arg(input);
    cmd.args(["-loop", "1", "-i"]).arg(&bd_path);
    cmd.args(["-filter_complex", &graph, "-map", "[o]"]);
    if !gif {
        cmd.args(["-map", "0:a?", "-c:a", "copy"]);
        cmd.args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-movflags", "+faststart"]);
    }
    cmd.arg(out);
    let result = cmd.output();
    let _ = std::fs::remove_file(&bd_path);
    let result = result.map_err(|e| format!("cannot run ffmpeg (is it installed?): {e}"))?;
    if result.status.success() {
        Ok(())
    } else {
        let _ = std::fs::remove_file(out);
        Err(format!("ffmpeg failed: {}", String::from_utf8_lossy(&result.stderr).trim()))
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
    fn reads_the_size_from_ffmpegs_banner() {
        let banner = "  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(tv, bt709), 720x1600 [SAR 1:1 DAR 9:20], 1234 kb/s, 30 fps";
        assert_eq!(parse_size(banner), Some((720, 1600)));
        assert_eq!(parse_size("Audio: aac"), None);
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
            let got = probe_size(&out).unwrap();
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
