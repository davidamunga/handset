//! Sound for Share. The phone's audio arrives as raw PCM in chunks of any size; browsers want Opus. It is cut
//! into 20 ms frames and encoded once here, whatever the number of viewers, and each viewer is sent the packets.

use crate::tap::Pcm;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Samples per channel in 20 ms at 48 kHz.
const FRAME_SAMPLES: usize = 960;
/// Stereo, 16 bits.
const FRAME_BYTES: usize = FRAME_SAMPLES * 4;
const FRAME_US: u64 = 20_000;
/// Chunks that start further than this from where the last frame ended mean the sound had a gap.
const GAP_US: u64 = 100_000;
const BITRATE: i32 = 96_000;

/// One Opus packet, 20 ms long, with the time on the phone's clock where its sound starts.
pub struct Packet {
    pub pts_us: u64,
    pub data: Vec<u8>,
}

/// Where the packets go: one channel per viewer.
pub type Listeners = Arc<Mutex<Vec<SyncSender<Arc<Packet>>>>>;

/// Cuts a stream of PCM chunks into 20 ms frames, each with its start time.
#[derive(Default)]
struct Framer {
    pending: Vec<u8>,
    /// Start time of frame number 0, and how many frames have been cut since.
    base: Option<u64>,
    cut: u64,
}

impl Framer {
    fn push(&mut self, pts_us: u64, pcm: &[u8]) -> Vec<(u64, Vec<u8>)> {
        if self.pending.is_empty() {
            let expected = self.base.map(|b| b + self.cut * FRAME_US);
            if expected.is_none_or(|e| e.abs_diff(pts_us) > GAP_US) {
                self.base = Some(pts_us);
                self.cut = 0;
            }
        }
        self.pending.extend_from_slice(pcm);
        let mut frames = Vec::new();
        while self.pending.len() >= FRAME_BYTES {
            let start = self.base.unwrap_or(pts_us) + self.cut * FRAME_US;
            frames.push((start, self.pending.drain(..FRAME_BYTES).collect()));
            self.cut += 1;
        }
        frames
    }
}

/// Encode what arrives until sharing stops. Frames are only encoded while somebody listens.
pub fn spawn(pcm: Receiver<Arc<Pcm>>, listeners: Listeners, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut encoder = match opus::Encoder::new(48_000, opus::Channels::Stereo, opus::Application::Audio) {
            Ok(e) => e,
            Err(e) => return eprintln!("share: no Opus encoder, so no sound: {e}"),
        };
        let _ = encoder.set_bitrate(opus::Bitrate::Bits(BITRATE));
        let mut framer = Framer::default();
        let mut out = vec![0u8; 1500];
        while !stop.load(SeqCst) {
            let chunk = match pcm.recv_timeout(Duration::from_millis(500)) {
                Ok(c) => c,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return,
            };
            for (pts_us, frame) in framer.push(chunk.pts_us, &chunk.data) {
                let mut listeners = listeners.lock().unwrap();
                if listeners.is_empty() {
                    continue;
                }
                let samples: Vec<i16> = frame.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect();
                let Ok(n) = encoder.encode(&samples, &mut out) else { continue };
                let packet = Arc::new(Packet { pts_us, data: out[..n].to_vec() });
                listeners.retain(|tx| !matches!(tx.try_send(packet.clone()), Err(TrySendError::Disconnected(_))));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_of_any_size_become_twenty_millisecond_frames() {
        let mut f = Framer::default();
        assert!(f.push(1_000_000, &vec![0; 1000]).is_empty());
        let frames = f.push(1_005_000, &vec![0; FRAME_BYTES * 2]);
        assert_eq!(frames.len(), 2);
        assert!(frames.iter().all(|(_, d)| d.len() == FRAME_BYTES));
        assert_eq!(frames[0].0, 1_000_000);
        assert_eq!(frames[1].0, 1_000_000 + FRAME_US);
        // 1000 + 2 * 3840 bytes in, 2 frames out, 1000 bytes still waiting
        assert_eq!(f.pending.len(), 1000);
    }

    #[test]
    fn frame_times_follow_the_sample_count_not_the_chunk_times() {
        let mut f = Framer::default();
        let a = f.push(0, &vec![0; FRAME_BYTES]);
        // a little jitter in when the next chunk was stamped must not bend the spacing
        let b = f.push(FRAME_US + 3_000, &vec![0; FRAME_BYTES]);
        assert_eq!((a[0].0, b[0].0), (0, FRAME_US));
    }

    #[test]
    fn a_gap_in_the_sound_starts_the_clock_again() {
        let mut f = Framer::default();
        f.push(0, &vec![0; FRAME_BYTES]);
        let after = f.push(5_000_000, &vec![0; FRAME_BYTES]);
        assert_eq!(after[0].0, 5_000_000);
    }
}
