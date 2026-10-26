//! Video thread: read packets from the server, decode H.264, hand the most
//! recent frame to the UI thread.

use crate::render::Yuv;
use crate::server::{self, Packet};
use openh264::decoder::{Decoder, DecoderConfig, Flush};
use openh264::formats::YUVSource;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use winit::event_loop::EventLoopProxy;

#[derive(Debug)]
pub enum UserEvent {
    Frame,
    Disconnected,
    /// The stream cannot be shown; the message says why.
    Failed(String),
    /// A native menu item was chosen.
    Menu(String),
    /// The latest `adb devices`, or why it could not be run.
    Adb(Result<Vec<(String, String)>, String>),
    /// Something the landing screen was waiting for finished.
    Landing(crate::connect::LandingEvent),
}

#[derive(Default)]
pub struct Slot {
    pub yuv: Yuv,
    /// A frame is waiting to be picked up by the UI thread.
    pub fresh: bool,
    /// The frame in `yuv` is (almost) all black.
    pub black: bool,
}

/// Consecutive decode failures tolerated (each one rebuilds the decoder) before giving up.
const MAX_RESETS: u32 = 3;

/// One-line description of an Annex B packet: NAL types and sizes, and for an SPS the
/// profile/level bytes. Shown with the first packets so a bad stream can be diagnosed.
fn describe(data: &[u8]) -> String {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 < data.len() {
        if data[i..i + 3] == [0, 0, 1] {
            let ty = data[i + 3] & 0x1f;
            out.push(if ty == 7 && i + 7 < data.len() {
                format!("SPS(profile {} level {})", data[i + 4], data[i + 6])
            } else {
                format!("nal{ty}")
            });
            i += 3;
        } else {
            i += 1;
        }
    }
    format!("{} bytes [{}]", data.len(), out.join(" "))
}

/// Frames are shown as soon as they decode. OpenH264's per-call flush leaks a picture per
/// frame on some streams (the UNIWA W888's), which exhausts its pool a few frames in.
fn new_decoder() -> Result<Decoder, openh264::Error> {
    let config = DecoderConfig::new().flush_after_decode(Flush::NoFlush);
    Decoder::with_api_config(openh264::OpenH264API::from_source(), config)
}

pub fn spawn(mut video: TcpStream, slot: Arc<Mutex<Slot>>, proxy: EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || {
        let fail = |msg: String| {
            eprintln!("{msg}");
            let _ = proxy.send_event(UserEvent::Failed(msg));
        };
        let mut decoder = match new_decoder() {
            Ok(d) => d,
            Err(e) => return fail(format!("could not start the H.264 decoder: {e}")),
        };
        let mut config: Vec<u8> = Vec::new();
        // After a rebuild, wait for a keyframe: delta frames cannot decode without one.
        let mut need_key = false;
        let (mut resets, mut seen, mut dropped) = (0u32, 0u32, 0u32);
        loop {
            let (data, key) = match server::read_packet(&mut video) {
                Ok(Packet::Session { width, height }) => {
                    eprintln!("video session: {width}x{height}");
                    continue;
                }
                Ok(Packet::Config(d)) => {
                    eprintln!("video config: {}", describe(&d));
                    config = d.clone();
                    (d, false)
                }
                Ok(Packet::Frame { data, key, .. }) => {
                    if seen < 3 {
                        seen += 1;
                        eprintln!("video frame (key={key}): {}", describe(&data));
                    }
                    if need_key && !key {
                        dropped += 1;
                        continue;
                    }
                    (data, key)
                }
                Err(e) => {
                    eprintln!("video stream ended: {e}");
                    break;
                }
            };
            match decoder.decode(&data) {
                Ok(Some(img)) => {
                    resets = 0;
                    if key {
                        need_key = false;
                    }
                    let (w, h) = img.dimensions();
                    let (sy, su, _) = img.strides();
                    let mut s = slot.lock().unwrap();
                    let was_fresh = s.fresh;
                    s.yuv.width = w as u32;
                    s.yuv.height = h as u32;
                    s.yuv.stride_y = sy as u32;
                    s.yuv.stride_uv = su as u32;
                    s.yuv.y.clear();
                    s.yuv.y.extend_from_slice(img.y());
                    s.yuv.u.clear();
                    s.yuv.u.extend_from_slice(img.u());
                    s.yuv.v.clear();
                    s.yuv.v.extend_from_slice(img.v());
                    s.black = crate::black::is_black(img.y());
                    s.fresh = true;
                    drop(s);
                    if !was_fresh {
                        let _ = proxy.send_event(UserEvent::Frame);
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    // A failed SPS/PPS leaves openh264 returning "no param sets" for every
                    // later packet, so rebuild it and replay the config instead of looping.
                    resets += 1;
                    eprintln!("decode error: {e} ({resets}/{MAX_RESETS}); resetting decoder");
                    if resets >= MAX_RESETS {
                        return fail(format!(
                            "the phone's video stream could not be decoded ({e}). \
                             Try a smaller size, e.g. `handset --max-size 1024`"
                        ));
                    }
                    decoder = match new_decoder() {
                        Ok(d) => d,
                        Err(e) => return fail(format!("could not restart the H.264 decoder: {e}")),
                    };
                    if !config.is_empty() {
                        let _ = decoder.decode(&config);
                    }
                    need_key = true;
                }
            }
        }
        if dropped > 0 {
            eprintln!("dropped {dropped} frames while waiting for a keyframe");
        }
        let _ = proxy.send_event(UserEvent::Disconnected);
    });
}

#[cfg(test)]
mod tests {
    use super::describe;

    #[test]
    fn describe_reads_sps_profile_and_level() {
        let d = [0, 0, 0, 1, 0x67, 100, 0, 40, 0xac, 0, 0, 0, 1, 0x68, 0xee];
        let s = describe(&d);
        assert!(s.contains("SPS(profile 100 level 40)"), "{s}");
        assert!(s.contains("nal8"), "{s}");
    }
}
