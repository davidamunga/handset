//! Spotting a phone screen that is black on purpose or by accident. Apps that forbid screen capture
//! (banking, some streaming) come through as black frames, which looks like a frozen Handset.

use std::time::{Duration, Instant};

/// How long the screen must stay black before saying anything.
const AFTER: Duration = Duration::from_secs(3);
/// Luma (limited range: black is 16) up to which a sample counts as black.
const BLACK: u8 = 24;

/// Is this luma plane (almost) entirely black? Looks at every 17th sample.
pub fn is_black(luma: &[u8]) -> bool {
    !luma.is_empty() && luma.iter().step_by(17).all(|&v| v <= BLACK)
}

/// Decides when a run of black frames is long enough to mention, and when it has ended.
#[derive(Default)]
pub struct BlackWatch {
    since: Option<Instant>,
    shown: bool,
}

impl BlackWatch {
    /// A frame arrived. Returns `Some(false)` when a shown message should go away.
    pub fn frame(&mut self, black: bool, now: Instant) -> Option<bool> {
        if black {
            self.since.get_or_insert(now);
            return self.tick(now);
        }
        self.since = None;
        std::mem::take(&mut self.shown).then_some(false)
    }

    /// Time passed with no news. Returns `Some(true)` once the screen has been black long enough.
    pub fn tick(&mut self, now: Instant) -> Option<bool> {
        let long_enough = self.since.is_some_and(|s| now.duration_since(s) >= AFTER);
        (long_enough && !std::mem::replace(&mut self.shown, true)).then_some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_and_not_black() {
        assert!(is_black(&[16; 1000]));
        assert!(!is_black(&[]));
        let mut lit = vec![16u8; 1000];
        lit[170] = 120; // a sampled pixel (index 170 is a multiple of 17)
        assert!(!is_black(&lit));
        assert!(!is_black(&[200; 1000]));
    }

    #[test]
    fn speaks_only_after_a_while_and_clears_on_the_first_lit_frame() {
        let t0 = Instant::now();
        let mut w = BlackWatch::default();
        assert_eq!(w.frame(true, t0), None);
        assert_eq!(w.frame(true, t0 + Duration::from_secs(2)), None);
        assert_eq!(w.tick(t0 + Duration::from_secs(3)), Some(true));
        assert_eq!(w.tick(t0 + Duration::from_secs(4)), None); // said once
        assert_eq!(w.frame(true, t0 + Duration::from_secs(5)), None);
        assert_eq!(w.frame(false, t0 + Duration::from_secs(6)), Some(false));
        assert_eq!(w.frame(false, t0 + Duration::from_secs(7)), None);
    }

    #[test]
    fn a_lit_frame_restarts_the_count() {
        let t0 = Instant::now();
        let mut w = BlackWatch::default();
        w.frame(true, t0);
        assert_eq!(w.frame(false, t0 + Duration::from_secs(2)), None); // never said anything
        w.frame(true, t0 + Duration::from_secs(2));
        assert_eq!(w.tick(t0 + Duration::from_secs(4)), None); // only 2 s black since
        assert_eq!(w.tick(t0 + Duration::from_secs(5)), Some(true));
    }
}
