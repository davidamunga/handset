//! A copy of the phone's H.264 packets, taken before they are decoded, for anything that wants the
//! stream itself rather than pictures (Share forwards it to a browser without re-encoding).
//!
//! The video thread publishes every packet. With nobody subscribed that costs one lock and no copy.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex};

/// Units a subscriber may fall behind by before its delta frames are dropped.
const BACKLOG: usize = 64;

/// One picture's worth of Annex B data. A keyframe starts with the SPS and PPS, so a unit stands on its own.
pub struct Unit {
    pub pts_us: u64,
    pub key: bool,
    pub data: Arc<[u8]>,
}

struct Subscriber {
    tx: SyncSender<Arc<Unit>>,
    /// Delta frames cannot be decoded without the keyframe before them: skip them until the next one.
    wait_key: bool,
}

#[derive(Default)]
struct State {
    config: Vec<u8>,
    subscribers: Vec<Subscriber>,
}

#[derive(Default)]
pub struct Tap {
    state: Mutex<State>,
    /// The phone screen's width and height in pixels, from the last session packet.
    size: AtomicU32,
}

impl Tap {
    /// Units from the next keyframe on. Dropping the receiver unsubscribes.
    pub fn subscribe(&self) -> Receiver<Arc<Unit>> {
        let (tx, rx) = sync_channel(BACKLOG);
        self.state.lock().unwrap().subscribers.push(Subscriber { tx, wait_key: true });
        rx
    }

    #[cfg(test)]
    pub fn subscribers(&self) -> usize {
        self.state.lock().unwrap().subscribers.len()
    }

    pub fn set_size(&self, width: u32, height: u32) {
        let (w, h) = (width.min(0xffff), height.min(0xffff));
        self.size.store(w << 16 | h, Ordering::Relaxed);
    }

    /// Width and height, or zeros before the first session packet.
    pub fn size(&self) -> (u16, u16) {
        let v = self.size.load(Ordering::Relaxed);
        ((v >> 16) as u16, v as u16)
    }

    /// The SPS and PPS the server sends apart from the pictures; the next keyframes carry them.
    pub fn config(&self, data: &[u8]) {
        let mut st = self.state.lock().unwrap();
        st.config.clear();
        st.config.extend_from_slice(data);
    }

    /// Never blocks: a subscriber that cannot keep up loses frames, not the video thread its time.
    pub fn frame(&self, pts_us: u64, key: bool, data: &[u8]) {
        let mut st = self.state.lock().unwrap();
        if st.subscribers.is_empty() {
            return;
        }
        let data: Arc<[u8]> = if key { [st.config.as_slice(), data].concat().into() } else { data.into() };
        let unit = Arc::new(Unit { pts_us, key, data });
        st.subscribers.retain_mut(|s| {
            if s.wait_key && !key {
                return true;
            }
            match s.tx.try_send(unit.clone()) {
                Ok(()) => {
                    s.wait_key = false;
                    true
                }
                Err(TrySendError::Full(_)) => {
                    s.wait_key = true;
                    true
                }
                Err(TrySendError::Disconnected(_)) => false,
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &[u8] = &[0, 0, 0, 1, 0x67, 1, 2, 0, 0, 0, 1, 0x68, 3];

    fn drain(rx: &Receiver<Arc<Unit>>) -> Vec<(bool, Vec<u8>)> {
        rx.try_iter().map(|u| (u.key, u.data.to_vec())).collect()
    }

    #[test]
    fn the_screen_size_is_kept_for_touches() {
        let tap = Tap::default();
        assert_eq!(tap.size(), (0, 0));
        tap.set_size(576, 1280);
        assert_eq!(tap.size(), (576, 1280));
    }

    #[test]
    fn nobody_subscribed_is_a_no_op() {
        let tap = Tap::default();
        tap.config(CONFIG);
        tap.frame(0, true, &[9]);
        assert_eq!(tap.subscribers(), 0);
    }

    #[test]
    fn a_subscriber_starts_at_a_keyframe() {
        let tap = Tap::default();
        let rx = tap.subscribe();
        tap.frame(0, false, &[1]);
        tap.frame(1, false, &[2]);
        tap.frame(2, true, &[3]);
        tap.frame(3, false, &[4]);
        let got = drain(&rx);
        assert_eq!(got.iter().map(|(k, _)| *k).collect::<Vec<_>>(), [true, false]);
        assert_eq!(got[1].1, [4]);
    }

    #[test]
    fn keyframes_carry_the_latest_config_and_deltas_do_not() {
        let tap = Tap::default();
        let rx = tap.subscribe();
        tap.config(CONFIG);
        tap.frame(0, true, &[0, 0, 0, 1, 0x65, 7]);
        tap.frame(1, false, &[0, 0, 0, 1, 0x41, 8]);
        let got = drain(&rx);
        assert_eq!(got[0].1, [CONFIG, &[0, 0, 0, 1, 0x65, 7][..]].concat());
        assert_eq!(got[1].1, [0, 0, 0, 1, 0x41, 8]);

        // a rotation brings new config, which the next keyframe uses
        let newer = [0, 0, 0, 1, 0x67, 9];
        tap.config(&newer);
        tap.frame(2, true, &[5]);
        assert_eq!(drain(&rx)[0].1, [&newer[..], &[5][..]].concat());
    }

    #[test]
    fn units_keep_their_timestamps() {
        let tap = Tap::default();
        let rx = tap.subscribe();
        tap.frame(40_000, true, &[1]);
        tap.frame(73_000, false, &[2]);
        assert_eq!(rx.try_iter().map(|u| u.pts_us).collect::<Vec<_>>(), [40_000, 73_000]);
    }

    #[test]
    fn each_subscriber_gets_its_own_copy() {
        let tap = Tap::default();
        let (a, b) = (tap.subscribe(), tap.subscribe());
        tap.frame(0, true, &[1]);
        assert_eq!(drain(&a).len(), 1);
        assert_eq!(drain(&b).len(), 1);
    }

    #[test]
    fn a_late_subscriber_waits_for_its_own_keyframe() {
        let tap = Tap::default();
        let first = tap.subscribe();
        tap.frame(0, true, &[1]);
        let late = tap.subscribe();
        tap.frame(1, false, &[2]);
        assert_eq!(drain(&first).len(), 2);
        assert!(drain(&late).is_empty());
        tap.frame(2, true, &[3]);
        assert_eq!(drain(&late).len(), 1);
    }

    #[test]
    fn a_slow_subscriber_loses_frames_not_the_video_thread_and_resumes_at_a_keyframe() {
        let tap = Tap::default();
        let rx = tap.subscribe();
        tap.frame(0, true, &[0]);
        for i in 1..=BACKLOG as u64 + 5 {
            tap.frame(i, false, &[1]); // never read, so the channel fills and publishing must not block
        }
        assert_eq!(tap.subscribers(), 1);
        let backlog = drain(&rx);
        assert_eq!(backlog.len(), BACKLOG);

        // after the overflow, deltas are held back until a keyframe restarts the picture
        tap.frame(100, false, &[2]);
        assert!(drain(&rx).is_empty());
        tap.frame(101, true, &[3]);
        tap.frame(102, false, &[4]);
        assert_eq!(drain(&rx).iter().map(|(k, _)| *k).collect::<Vec<_>>(), [true, false]);
    }

    #[test]
    fn a_dropped_receiver_unsubscribes() {
        let tap = Tap::default();
        let rx = tap.subscribe();
        assert_eq!(tap.subscribers(), 1);
        drop(rx);
        tap.frame(0, true, &[1]);
        assert_eq!(tap.subscribers(), 0);
    }
}
