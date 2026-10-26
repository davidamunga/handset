//! Control messages sent to the scrcpy server (client -> device).
//! Layouts follow the server's `ControlMessageReader.java`.

use std::io::{self, Write};
use std::net::TcpStream;

const TYPE_KEYCODE: u8 = 0;
const TYPE_TEXT: u8 = 1;
const TYPE_TOUCH: u8 = 2;
const TYPE_SCROLL: u8 = 3;
const TYPE_ROTATE_DEVICE: u8 = 11;
const TYPE_START_APP: u8 = 16;

#[derive(Clone, Copy, PartialEq)]
pub enum Touch {
    Down = 0,
    Up = 1,
    Move = 2,
}

pub mod keycode {
    pub const HOME: u32 = 3;
    pub const BACK: u32 = 4;
    pub const DPAD_UP: u32 = 19;
    pub const DPAD_DOWN: u32 = 20;
    pub const DPAD_LEFT: u32 = 21;
    pub const DPAD_RIGHT: u32 = 22;
    pub const TAB: u32 = 61;
    pub const ENTER: u32 = 66;
    pub const DEL: u32 = 67;
    pub const APP_SWITCH: u32 = 187;
}

pub struct Control {
    /// `None` until a phone is connected: messages are dropped meanwhile.
    stream: Option<TcpStream>,
    /// Sequence number of the next clipboard we send.
    clip_seq: u64,
}

impl Control {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream: Some(stream), clip_seq: 0 }
    }

    /// A control that is not connected to a phone yet.
    pub fn disconnected() -> Self {
        Self { stream: None, clip_seq: 0 }
    }

    fn send(&mut self, msg: &[u8]) -> io::Result<()> {
        let Some(stream) = self.stream.as_mut() else { return Ok(()) };
        let r = stream.write_all(msg);
        if let Err(e) = &r {
            eprintln!("control write failed: {e}");
        }
        r
    }

    /// `x`, `y` are device pixels; `w`, `h` the device screen size.
    pub fn touch(&mut self, action: Touch, x: i32, y: i32, w: u16, h: u16) -> io::Result<()> {
        let mut m = Vec::with_capacity(32);
        m.push(TYPE_TOUCH);
        m.push(action as u8);
        m.extend_from_slice(&u64::MAX.to_be_bytes()); // pointer id: mouse (-1)
        m.extend_from_slice(&x.to_be_bytes());
        m.extend_from_slice(&y.to_be_bytes());
        m.extend_from_slice(&w.to_be_bytes());
        m.extend_from_slice(&h.to_be_bytes());
        let pressure: u16 = if action == Touch::Up { 0 } else { 0xffff };
        m.extend_from_slice(&pressure.to_be_bytes());
        m.extend_from_slice(&1u32.to_be_bytes()); // action button: primary
        let buttons: u32 = if action == Touch::Up { 0 } else { 1 };
        m.extend_from_slice(&buttons.to_be_bytes());
        self.send(&m)
    }

    /// `h_scroll` / `v_scroll` in notches, range [-16, 16].
    pub fn scroll(&mut self, x: i32, y: i32, w: u16, h: u16, h_scroll: f32, v_scroll: f32) -> io::Result<()> {
        let fixed = |v: f32| ((v / 16.0).clamp(-1.0, 1.0) * 32767.0) as i16;
        let mut m = Vec::with_capacity(21);
        m.push(TYPE_SCROLL);
        m.extend_from_slice(&x.to_be_bytes());
        m.extend_from_slice(&y.to_be_bytes());
        m.extend_from_slice(&w.to_be_bytes());
        m.extend_from_slice(&h.to_be_bytes());
        m.extend_from_slice(&fixed(h_scroll).to_be_bytes());
        m.extend_from_slice(&fixed(v_scroll).to_be_bytes());
        m.extend_from_slice(&0u32.to_be_bytes());
        self.send(&m)
    }

    pub fn key(&mut self, keycode: u32) -> io::Result<()> {
        for action in [0u8, 1] {
            let mut m = Vec::with_capacity(14);
            m.push(TYPE_KEYCODE);
            m.push(action);
            m.extend_from_slice(&keycode.to_be_bytes());
            m.extend_from_slice(&0u32.to_be_bytes()); // repeat
            m.extend_from_slice(&0u32.to_be_bytes()); // meta state
            self.send(&m)?;
        }
        Ok(())
    }

    /// Toggle the device between portrait and landscape.
    pub fn rotate_device(&mut self) -> io::Result<()> {
        self.send(&[TYPE_ROTATE_DEVICE])
    }

    /// Put `text` on the phone's clipboard (without pasting it).
    pub fn set_clipboard(&mut self, text: &str) -> io::Result<()> {
        self.clip_seq += 1;
        self.send(&crate::clipboard::encode_set(self.clip_seq, text, false))
    }

    /// Open an app by package name (a leading `?` makes the server search app names instead).
    pub fn start_app(&mut self, name: &str) -> io::Result<()> {
        let bytes = name.as_bytes();
        let len = bytes.len().min(255);
        let mut m = Vec::with_capacity(2 + len);
        m.push(TYPE_START_APP);
        m.push(len as u8);
        m.extend_from_slice(&bytes[..len]);
        self.send(&m)
    }

    pub fn text(&mut self, text: &str) -> io::Result<()> {
        let mut m = Vec::with_capacity(5 + text.len());
        m.push(TYPE_TEXT);
        m.extend_from_slice(&(text.len() as u32).to_be_bytes());
        m.extend_from_slice(text.as_bytes());
        self.send(&m)
    }
}
