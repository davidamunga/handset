//! The part of a recording to keep when it is exported from the editor.

/// The shortest clip that can be kept, in seconds.
pub const MIN_LEN: f64 = 0.2;
/// A trim this close to the whole recording counts as no trim, so a stray pixel of dragging does not re-cut it.
const SLACK: f64 = 0.05;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Trim {
    pub start: f64,
    pub end: f64,
}

impl Trim {
    pub fn full(duration: f64) -> Self {
        Self { start: 0.0, end: duration }
    }

    pub fn len(&self) -> f64 {
        self.end - self.start
    }

    pub fn is_full(&self, duration: f64) -> bool {
        self.start <= SLACK && self.end >= duration - SLACK
    }

    /// Move the start to `t`, staying inside the recording and leaving at least [`MIN_LEN`].
    pub fn with_start(self, t: f64) -> Self {
        Self { start: t.clamp(0.0, (self.end - MIN_LEN).max(0.0)), ..self }
    }

    /// Move the end to `t`, staying inside the recording and leaving at least [`MIN_LEN`].
    pub fn with_end(self, t: f64, duration: f64) -> Self {
        Self { end: t.clamp((self.start + MIN_LEN).min(duration), duration), ..self }
    }

    /// Keep a playback position inside the trim.
    pub fn clamp_pos(&self, t: f64) -> f64 {
        t.clamp(self.start, self.end)
    }
}

/// The time at horizontal position `x` of a track that starts at `left` and is `width` long.
pub fn time_at(x: f32, left: f32, width: f32, duration: f64) -> f64 {
    (((x - left) / width).clamp(0.0, 1.0) as f64) * duration
}

/// The horizontal position of time `t` on that track.
pub fn x_at(t: f64, left: f32, width: f32, duration: f64) -> f32 {
    left + width * (t / duration).clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_trim_keeps_everything() {
        let t = Trim::full(5.0);
        assert_eq!((t.start, t.end, t.len()), (0.0, 5.0, 5.0));
        assert!(t.is_full(5.0));
    }

    #[test]
    fn tiny_nudges_still_count_as_no_trim() {
        assert!(Trim { start: 0.03, end: 4.98 }.is_full(5.0));
        assert!(!Trim { start: 0.5, end: 5.0 }.is_full(5.0));
        assert!(!Trim { start: 0.0, end: 4.5 }.is_full(5.0));
    }

    #[test]
    fn the_start_cannot_pass_the_end_or_leave_the_recording() {
        let t = Trim { start: 1.0, end: 3.0 };
        assert_eq!(t.with_start(2.0).start, 2.0);
        assert!((t.with_start(9.0).start - (3.0 - MIN_LEN)).abs() < 1e-9);
        assert_eq!(t.with_start(-4.0).start, 0.0);
        assert_eq!(t.with_start(2.0).end, 3.0);
    }

    #[test]
    fn the_end_cannot_pass_the_start_or_leave_the_recording() {
        let t = Trim { start: 1.0, end: 3.0 };
        assert_eq!(t.with_end(4.0, 5.0).end, 4.0);
        assert!((t.with_end(0.0, 5.0).end - (1.0 + MIN_LEN)).abs() < 1e-9);
        assert_eq!(t.with_end(99.0, 5.0).end, 5.0);
        assert_eq!(t.with_end(4.0, 5.0).start, 1.0);
    }

    #[test]
    fn a_clip_shorter_than_the_minimum_still_has_a_valid_trim() {
        let t = Trim::full(0.1);
        assert!(t.with_end(0.05, 0.1).end <= 0.1);
        assert!(t.with_start(0.09).start >= 0.0);
    }

    #[test]
    fn playback_stays_inside_the_trim() {
        let t = Trim { start: 1.0, end: 3.0 };
        assert_eq!((t.clamp_pos(0.2), t.clamp_pos(2.0), t.clamp_pos(4.0)), (1.0, 2.0, 3.0));
    }

    #[test]
    fn positions_and_times_convert_both_ways() {
        let (left, width, duration) = (20.0, 400.0, 8.0);
        assert_eq!(time_at(220.0, left, width, duration), 4.0);
        assert_eq!(x_at(4.0, left, width, duration), 220.0);
        // outside the track clamps to its ends
        assert_eq!(time_at(0.0, left, width, duration), 0.0);
        assert_eq!(time_at(999.0, left, width, duration), 8.0);
        let t = 2.5;
        assert!((time_at(x_at(t, left, width, duration), left, width, duration) - t).abs() < 1e-4);
    }
}
