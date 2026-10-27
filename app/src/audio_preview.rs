//! Sound for the editor's preview: ffmpeg decodes a recording's audio to raw PCM, and it is fed to the
//! speakers in step with the video frames. The decoder is stopped (and the queue flushed) on pause, seek and
//! when the video ends. Nothing here is used when a recording has no audio.

use crate::audio::{self, CHANNELS, Ring, SAMPLE_RATE};
use std::io::Read;
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How much audio is handed to the speakers at a time.
pub const CHUNK_MS: u64 = 20;

/// Bytes of 16-bit PCM in one chunk.
pub fn chunk_bytes() -> usize {
    SAMPLE_RATE as usize * CHANNELS as usize * 2 * CHUNK_MS as usize / 1000
}

/// The ffmpeg arguments that decode the audio of `path` from `from` seconds to raw PCM on stdout.
pub fn decode_args(path: &Path, from: f64) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> =
        ["-hide_banner", "-loglevel", "error", "-ss"].iter().map(Into::into).collect();
    args.push(format!("{from:.3}").into());
    args.push("-i".into());
    args.push(path.into());
    for a in ["-vn", "-f", "s16le", "-ar"] {
        args.push(a.into());
    }
    args.push(SAMPLE_RATE.to_string().into());
    args.push("-ac".into());
    args.push(CHANNELS.to_string().into());
    args.push("-".into());
    args
}

/// Read `reader` in chunks and hand each to `sink` no earlier than its time: chunk `n` is due `n` chunks
/// after `start`. A chunk that is late is passed on at once, so the audio catches up. Stops at the end of the
/// data or when `stop` is set.
pub fn feed(mut reader: impl Read, start: Instant, stop: &AtomicBool, mut sink: impl FnMut(&[u8])) {
    let mut buf = vec![0u8; chunk_bytes()];
    let mut n = 0u32;
    while !stop.load(Ordering::Relaxed) {
        let len = read_full(&mut reader, &mut buf);
        if len == 0 {
            break;
        }
        let due = start + Duration::from_millis(CHUNK_MS * u64::from(n));
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
        sink(&buf[..len]);
        n += 1;
    }
}

/// Fill `buf` unless the data ends first; how many bytes were read.
fn read_full(reader: &mut impl Read, buf: &mut [u8]) -> usize {
    let mut got = 0;
    while got < buf.len() {
        match reader.read(&mut buf[got..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => got += n,
        }
    }
    got
}

/// A running decoder feeding the speakers.
pub struct Feeder {
    stop: Arc<AtomicBool>,
    child: Child,
    thread: JoinHandle<()>,
    ring: Ring,
}

impl Feeder {
    /// Decode `path` from `from` seconds into `ring`, chunk `n` due `n` chunks after `start`.
    pub fn start(path: &Path, from: f64, start: Instant, ring: Ring) -> Option<Self> {
        let mut child = crate::tools::command(crate::tools::find("ffmpeg"))
            .args(decode_args(path, from))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let out = child.stdout.take()?;
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, sink) = (stop.clone(), ring.clone());
        let thread = std::thread::spawn(move || feed(out, start, &flag, |pcm| audio::push_samples(&sink, pcm)));
        Some(Self { stop, child, thread, ring })
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = self.thread.join();
        self.ring.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn a_chunk_is_twenty_milliseconds_of_stereo_16_bit_audio() {
        assert_eq!(chunk_bytes(), 48_000 * 2 * 2 * 20 / 1000);
    }

    #[test]
    fn decoding_starts_at_the_requested_time_and_asks_for_raw_pcm() {
        let args: Vec<String> =
            decode_args(Path::new("a b.mp4"), 1.5).iter().map(|a| a.to_string_lossy().into()).collect();
        let at = |s: &str| args.iter().position(|a| a == s).unwrap();
        assert_eq!(args[at("-ss") + 1], "1.500");
        assert_eq!(args[at("-i") + 1], "a b.mp4");
        assert_eq!(args[at("-f") + 1], "s16le");
        assert_eq!(args[at("-ar") + 1], "48000");
        assert_eq!(args[at("-ac") + 1], "2");
        assert!(args.contains(&"-vn".to_string()), "video is not decoded");
        assert_eq!(args.last().unwrap(), "-");
    }

    #[test]
    fn feeding_passes_on_every_byte_in_order_in_whole_chunks() {
        let data: Vec<u8> = (0..chunk_bytes() * 3 + 100).map(|i| (i % 251) as u8).collect();
        let got = Mutex::new(Vec::new());
        let sizes = Mutex::new(Vec::new());
        feed(std::io::Cursor::new(data.clone()), Instant::now(), &AtomicBool::new(false), |c| {
            got.lock().unwrap().extend_from_slice(c);
            sizes.lock().unwrap().push(c.len());
        });
        assert_eq!(*got.lock().unwrap(), data);
        assert_eq!(*sizes.lock().unwrap(), vec![chunk_bytes(), chunk_bytes(), chunk_bytes(), 100]);
    }

    #[test]
    fn feeding_is_paced_to_real_time() {
        let data = vec![0u8; chunk_bytes() * 6];
        let t = Instant::now();
        feed(std::io::Cursor::new(data), t, &AtomicBool::new(false), |_| {});
        // the sixth chunk is due 5 chunks after the start
        assert!(t.elapsed() >= Duration::from_millis(CHUNK_MS * 5 - 2), "{:?}", t.elapsed());
    }

    #[test]
    fn late_audio_is_passed_on_at_once_to_catch_up() {
        let data = vec![0u8; chunk_bytes() * 10];
        // the start was long ago: every chunk is already due
        let long_ago = Instant::now() - Duration::from_secs(5);
        let t = Instant::now();
        feed(std::io::Cursor::new(data), long_ago, &AtomicBool::new(false), |_| {});
        assert!(t.elapsed() < Duration::from_millis(100), "{:?}", t.elapsed());
    }

    #[test]
    fn feeding_stops_when_asked_to() {
        let stop = AtomicBool::new(false);
        let mut seen = 0;
        feed(std::io::Cursor::new(vec![0u8; chunk_bytes() * 50]), Instant::now(), &stop, |_| {
            seen += 1;
            if seen == 2 {
                stop.store(true, Ordering::Relaxed);
            }
        });
        assert_eq!(seen, 2);
    }

    #[test]
    fn ffmpeg_decodes_a_recordings_audio_into_the_expected_amount_of_pcm() {
        let ffmpeg = crate::tools::find("ffmpeg");
        if !crate::tools::command(&ffmpeg).arg("-version").output().is_ok_and(|o| o.status.success()) {
            return;
        }
        let dir = std::env::temp_dir().join(format!("handset-test-preview-audio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let clip = dir.join("clip.mp4");
        assert!(
            crate::tools::command(&ffmpeg)
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=64x64:rate=10:duration=1"
                ])
                .args(["-f", "lavfi", "-i", "sine=duration=1", "-pix_fmt", "yuv420p", "-shortest"])
                .arg(&clip)
                .status()
                .unwrap()
                .success()
        );
        // from 0.5 s of a 1 s clip: about half a second of 48 kHz stereo 16-bit audio (96 000 bytes)
        let mut child = crate::tools::command(&ffmpeg)
            .args(decode_args(&clip, 0.5))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut total = 0usize;
        let long_ago = Instant::now() - Duration::from_secs(10);
        feed(child.stdout.take().unwrap(), long_ago, &AtomicBool::new(false), |c| total += c.len());
        let _ = child.wait();
        assert!((total as i64 - 96_000).abs() < 12_000, "decoded {total} bytes");

        // started for real, then stopped: some audio reached the queue, and stopping empties it
        let ring: Ring = Default::default();
        let feeder = Feeder::start(&clip, 0.0, Instant::now(), ring.clone()).expect("ffmpeg starts");
        std::thread::sleep(Duration::from_millis(600));
        assert!(!ring.lock().unwrap().is_empty(), "some audio arrived");
        feeder.stop();
        assert!(ring.lock().unwrap().is_empty(), "stopping flushes the queue");
        let _ = std::fs::remove_dir_all(dir);
    }
}
