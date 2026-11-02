//! What a Share viewer may ask of the phone. The browser sends short text messages over a data channel:
//!
//! ```text
//! d,x,y  m,x,y  u,x,y   touch down, move, up; x and y are fractions of the picture, 0 to 1
//! s,x,y,h,v             scroll at a point, in notches
//! k,back|home|recents|enter|del
//! t,<text>              type text
//! r                     turn the phone
//! ```
//!
//! The browser is not trusted: everything is parsed strictly, and a `Gate` keeps a viewer from moving or
//! lifting a finger it never put down.

use crate::control::{Touch, keycode};

/// The longest text one message may type.
const MAX_TEXT: usize = 200;

#[derive(Debug, PartialEq)]
pub enum Input {
    Touch(Touch, f32, f32),
    Scroll { x: f32, y: f32, h: f32, v: f32 },
    Key(u32),
    Text(String),
    Rotate,
}

pub fn parse(msg: &str) -> Option<Input> {
    let (kind, rest) = msg.split_once(',').unwrap_or((msg, ""));
    let numbers = |n: usize| -> Option<Vec<f32>> {
        let v: Vec<f32> = rest.split(',').map(str::parse).collect::<Result<_, _>>().ok()?;
        (v.len() == n && v.iter().all(|x| x.is_finite())).then_some(v)
    };
    // a finger dragged past the picture's edge stays on the edge
    let unit = |x: f32| x.clamp(0.0, 1.0);
    match kind {
        "d" | "m" | "u" => {
            let p = numbers(2)?;
            let action = match kind {
                "d" => Touch::Down,
                "m" => Touch::Move,
                _ => Touch::Up,
            };
            Some(Input::Touch(action, unit(p[0]), unit(p[1])))
        }
        "s" => {
            let p = numbers(4)?;
            Some(Input::Scroll { x: unit(p[0]), y: unit(p[1]), h: p[2].clamp(-16.0, 16.0), v: p[3].clamp(-16.0, 16.0) })
        }
        "k" => Some(Input::Key(match rest {
            "back" => keycode::BACK,
            "home" => keycode::HOME,
            "recents" => keycode::APP_SWITCH,
            "enter" => keycode::ENTER,
            "del" => keycode::DEL,
            _ => return None,
        })),
        "t" => {
            let text: String = rest.chars().filter(|c| !c.is_control()).take(MAX_TEXT).collect();
            (!text.is_empty()).then_some(Input::Text(text))
        }
        "r" if rest.is_empty() => Some(Input::Rotate),
        _ => None,
    }
}

/// One viewer's finger: lets through only a touch that makes sense.
#[derive(Default)]
pub struct Gate {
    /// Where the finger is, while it is down.
    down: Option<(f32, f32)>,
}

impl Gate {
    pub fn allow(&mut self, input: &Input) -> bool {
        match *input {
            Input::Touch(Touch::Down, x, y) => {
                self.down = Some((x, y));
                true
            }
            Input::Touch(Touch::Move, x, y) => self.down.is_some_and(|_| {
                self.down = Some((x, y));
                true
            }),
            Input::Touch(Touch::Up, ..) => self.down.take().is_some(),
            _ => true,
        }
    }

    /// The viewer left with a finger down: where to lift it, so the phone is not left held.
    pub fn release(&mut self) -> Option<(f32, f32)> {
        self.down.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touches_are_read_as_fractions_and_kept_on_the_picture() {
        assert_eq!(parse("d,0.25,0.5"), Some(Input::Touch(Touch::Down, 0.25, 0.5)));
        assert_eq!(parse("m,0.1,0.2"), Some(Input::Touch(Touch::Move, 0.1, 0.2)));
        assert_eq!(parse("u,1,0"), Some(Input::Touch(Touch::Up, 1.0, 0.0)));
        assert_eq!(parse("m,-3,7"), Some(Input::Touch(Touch::Move, 0.0, 1.0)));
    }

    #[test]
    fn nonsense_is_ignored() {
        for bad in ["", "x", "d", "d,1", "d,1,2,3", "d,a,b", "d,NaN,0", "d,inf,0", "k,power", "k", "s,1,2", "r,1", "t,"]
        {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn keys_scroll_and_rotate() {
        assert_eq!(parse("k,back"), Some(Input::Key(keycode::BACK)));
        assert_eq!(parse("k,recents"), Some(Input::Key(keycode::APP_SWITCH)));
        assert_eq!(parse("s,0.5,0.5,0,-40"), Some(Input::Scroll { x: 0.5, y: 0.5, h: 0.0, v: -16.0 }));
        assert_eq!(parse("r"), Some(Input::Rotate));
    }

    #[test]
    fn text_keeps_commas_drops_control_characters_and_is_capped() {
        assert_eq!(parse("t,hi, there"), Some(Input::Text("hi, there".into())));
        assert_eq!(parse("t,a\u{0}b\nc"), Some(Input::Text("abc".into())));
        let long = format!("t,{}", "x".repeat(500));
        assert!(matches!(parse(&long), Some(Input::Text(t)) if t.len() == MAX_TEXT));
    }

    #[test]
    fn a_viewer_cannot_move_or_lift_a_finger_it_did_not_put_down() {
        let mut g = Gate::default();
        assert!(!g.allow(&Input::Touch(Touch::Move, 0.5, 0.5)));
        assert!(!g.allow(&Input::Touch(Touch::Up, 0.5, 0.5)));
        assert!(g.allow(&Input::Touch(Touch::Down, 0.5, 0.5)));
        assert!(g.allow(&Input::Touch(Touch::Move, 0.6, 0.6)));
        assert!(g.allow(&Input::Touch(Touch::Up, 0.6, 0.6)));
        assert!(!g.allow(&Input::Touch(Touch::Up, 0.6, 0.6)));
    }

    #[test]
    fn leaving_with_a_finger_down_lifts_it_where_it_was() {
        let mut g = Gate::default();
        assert_eq!(g.release(), None);
        g.allow(&Input::Touch(Touch::Down, 0.2, 0.3));
        g.allow(&Input::Touch(Touch::Move, 0.4, 0.5));
        assert_eq!(g.release(), Some((0.4, 0.5)));
        assert_eq!(g.release(), None);
    }
}
