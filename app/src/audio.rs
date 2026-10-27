//! Device audio: read the raw PCM stream from the server, play it locally and
//! hand a copy to the recorder.
//!
//! The server is asked for `audio_codec=raw`: 48 kHz, stereo, signed 16-bit
//! little-endian, so no decoder is needed.

use crate::server::{self, Packet};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: u16 = 2;
const CODEC_RAW: u32 = 0x0072_6177; // "\0raw"

/// Where the recorder, when one is running, gets its copy of the PCM stream.
pub type RecSink = Arc<Mutex<Option<Sender<Vec<u8>>>>>;

pub type Ring = Arc<Mutex<VecDeque<i16>>>;

// Playback health, printed with HANDSET_DEBUG.
static UNDERRUNS: AtomicU64 = AtomicU64::new(0);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static RECEIVED: AtomicU64 = AtomicU64::new(0);
static MUTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_muted(m: bool) {
    MUTED.store(m, Relaxed);
}

pub fn is_muted() -> bool {
    MUTED.load(Relaxed)
}

/// Keeps the output stream alive. `cpal::Stream` is not `Send`, so this stays
/// on the thread that created it.
pub struct Playback {
    ring: Ring,
    _stream: cpal::Stream,
}

/// Samples (not frames) of audio kept buffered for playback: about 120 ms.
const MAX_BUFFER: usize = (SAMPLE_RATE as usize * CHANNELS as usize) * 120 / 1000;
/// Do not start draining before about 40 ms are queued, to avoid stutter.
const PREBUFFER: usize = (SAMPLE_RATE as usize * CHANNELS as usize) * 40 / 1000;

impl Playback {
    pub fn new() -> Result<Self, String> {
        let device = cpal::default_host().default_output_device().ok_or("no audio output device")?;
        let config =
            cpal::StreamConfig { channels: CHANNELS, sample_rate: SAMPLE_RATE, buffer_size: cpal::BufferSize::Default };
        let ring: Ring = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_BUFFER * 2)));
        let reader = ring.clone();
        let mut started = false;
        let stream = device
            .build_output_stream(
                config,
                move |out: &mut [f32], _| {
                    let mut q = reader.lock().unwrap();
                    if !started && q.len() >= PREBUFFER {
                        started = true;
                    }
                    for s in out.iter_mut() {
                        *s = if started {
                            match q.pop_front() {
                                Some(v) => v as f32 / 32768.0,
                                None => {
                                    started = false; // underrun: refill before resuming
                                    UNDERRUNS.fetch_add(1, Relaxed);
                                    0.0
                                }
                            }
                        } else {
                            0.0
                        };
                    }
                },
                |e| eprintln!("audio playback error: {e}"),
                None,
            )
            .map_err(|e| format!("cannot open audio output (48 kHz stereo): {e}"))?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self { ring, _stream: stream })
    }

    pub fn ring(&self) -> Ring {
        self.ring.clone()
    }
}

pub fn push_samples(ring: &Ring, pcm: &[u8]) {
    let mut q = ring.lock().unwrap();
    if MUTED.load(Relaxed) {
        q.clear();
        return;
    }
    q.extend(pcm.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)));
    // never let playback fall behind: drop the oldest audio
    let excess = q.len().saturating_sub(MAX_BUFFER);
    if excess > 0 {
        DROPPED.fetch_add(excess as u64, Relaxed);
        q.drain(..excess);
    }
}

pub fn spawn(mut stream: TcpStream, ring: Option<Ring>, sink: RecSink) {
    std::thread::spawn(move || {
        use std::io::Read;
        let mut id = [0u8; 4];
        if stream.read_exact(&mut id).is_err() {
            return;
        }
        match u32::from_be_bytes(id) {
            CODEC_RAW => {}
            0 => {
                eprintln!("audio is not available on this device (it needs Android 11 or later)");
                return;
            }
            1 => {
                eprintln!("the device reported an audio capture error");
                return;
            }
            other => {
                eprintln!("unexpected audio codec 0x{other:08x}");
                return;
            }
        }
        eprintln!("audio: capturing (48 kHz stereo)");
        if let (Some(r), true) = (ring.clone(), std::env::var_os("HANDSET_DEBUG").is_some()) {
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let buffered_ms = r.lock().unwrap().len() * 1000 / (SAMPLE_RATE as usize * CHANNELS as usize);
                    eprintln!(
                        "audio stats: buffered {buffered_ms} ms, received {} ms, underruns {}, dropped {} ms",
                        RECEIVED.load(Relaxed) * 1000 / (SAMPLE_RATE as u64 * CHANNELS as u64),
                        UNDERRUNS.load(Relaxed),
                        DROPPED.load(Relaxed) * 1000 / (SAMPLE_RATE as u64 * CHANNELS as u64),
                    );
                }
            });
        }
        while let Ok(packet) = server::read_packet(&mut stream) {
            let Packet::Frame { data, .. } = packet else { continue };
            RECEIVED.fetch_add(data.len() as u64 / 2, Relaxed);
            if let Some(r) = &ring {
                push_samples(r, &data);
            }
            if let Some(tx) = sink.lock().unwrap().as_ref() {
                let _ = tx.send(data);
            }
        }
    });
}
