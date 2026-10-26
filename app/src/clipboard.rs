//! Clipboard text shared with the phone. The scrcpy server pushes the phone's clipboard to us as device
//! messages on the control socket (it does so on its own, unless told not to), and we send the Mac's
//! clipboard to the phone when the window gets focus. `Sync` remembers the last text moved either way, so
//! the same text never bounces back and forth.

use std::io::Read;
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

/// scrcpy's limit for a control message is 256 KiB, with 14 bytes of header.
const MAX_TEXT: usize = (1 << 18) - 14;
const TYPE_SET_CLIPBOARD: u8 = 9;

/// `SET_CLIPBOARD`: type, sequence (u64), paste flag, length (u32), text, all big endian.
pub fn encode_set(sequence: u64, text: &str, paste: bool) -> Vec<u8> {
    let mut end = text.len().min(MAX_TEXT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let bytes = &text.as_bytes()[..end];
    let mut m = Vec::with_capacity(14 + bytes.len());
    m.push(TYPE_SET_CLIPBOARD);
    m.extend_from_slice(&sequence.to_be_bytes());
    m.push(paste as u8);
    m.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    m.extend_from_slice(bytes);
    m
}

#[derive(Debug, PartialEq)]
pub enum DeviceMsg {
    Clipboard(String),
    /// The phone acknowledged a `SET_CLIPBOARD` we sent with this sequence number.
    Ack(u64),
    /// A virtual HID device wrote back to us; not used by Handset.
    UhidOutput,
}

/// Take every complete device message off the front of `buf`, leaving a partial one in place. `None` means
/// the stream is not something we understand (an unknown message type), and nothing more can be read from it.
pub fn parse(buf: &mut Vec<u8>) -> Option<Vec<DeviceMsg>> {
    let mut out = Vec::new();
    let mut at = 0;
    loop {
        let rest = &buf[at..];
        let Some(&kind) = rest.first() else { break };
        let (msg, used) = match kind {
            0 => {
                if rest.len() < 5 {
                    break;
                }
                let len = u32::from_be_bytes(rest[1..5].try_into().unwrap()) as usize;
                if rest.len() < 5 + len {
                    break;
                }
                (DeviceMsg::Clipboard(String::from_utf8_lossy(&rest[5..5 + len]).into_owned()), 5 + len)
            }
            1 => {
                if rest.len() < 9 {
                    break;
                }
                (DeviceMsg::Ack(u64::from_be_bytes(rest[1..9].try_into().unwrap())), 9)
            }
            2 => {
                if rest.len() < 5 {
                    break;
                }
                let len = u16::from_be_bytes(rest[3..5].try_into().unwrap()) as usize;
                if rest.len() < 5 + len {
                    break;
                }
                (DeviceMsg::UhidOutput, 5 + len)
            }
            _ => return None,
        };
        out.push(msg);
        at += used;
    }
    buf.drain(..at);
    Some(out)
}

/// What was last moved between the Mac and the phone, and whether syncing is on.
pub struct Sync {
    pub enabled: bool,
    last: Option<String>,
}

impl Sync {
    pub fn new(enabled: bool) -> Self {
        Self { enabled, last: None }
    }

    /// The Mac's clipboard now reads `current`: the text to send to the phone, if it is new.
    pub fn mac_changed(&mut self, current: &str) -> Option<String> {
        if !self.enabled || current.is_empty() || self.last.as_deref() == Some(current) {
            return None;
        }
        self.last = Some(current.to_string());
        Some(current.to_string())
    }

    /// The phone's clipboard now reads `text`: whether to put it on the Mac's.
    pub fn phone_changed(&mut self, text: &str) -> bool {
        if !self.enabled || text.is_empty() || self.last.as_deref() == Some(text) {
            return false;
        }
        self.last = Some(text.to_string());
        true
    }
}

/// Read device messages from the control socket until it closes. Clipboard text goes to the Mac when
/// syncing is on. This also keeps the server from blocking on a full socket.
pub fn spawn_reader(mut control: TcpStream, sync: Arc<Mutex<Sync>>) {
    std::thread::spawn(move || {
        let mut pending = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(n) = control.read(&mut chunk) {
            if n == 0 {
                break;
            }
            pending.extend_from_slice(&chunk[..n]);
            let Some(messages) = parse(&mut pending) else {
                // an unknown message: stop interpreting, but keep draining so the server never blocks
                while control.read(&mut chunk).map(|n| n > 0).unwrap_or(false) {}
                break;
            };
            for m in messages {
                if let DeviceMsg::Clipboard(text) = m
                    && sync.lock().unwrap().phone_changed(&text)
                {
                    write_mac(&text);
                }
            }
        }
    });
}

/// The Mac's clipboard as text, or `None` if it holds something else.
#[cfg(target_os = "macos")]
pub fn read_mac() -> Option<String> {
    let out = std::process::Command::new("pbpaste").env("LANG", "en_US.UTF-8").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(not(target_os = "macos"))]
pub fn read_mac() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn write_mac(text: &str) {
    use std::io::Write;
    let Ok(mut child) =
        std::process::Command::new("pbcopy").env("LANG", "en_US.UTF-8").stdin(std::process::Stdio::piped()).spawn()
    else {
        return;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let _ = child.wait();
}

#[cfg(not(target_os = "macos"))]
fn write_mac(_text: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_clipboard_layout() {
        let m = encode_set(7, "héllo", false);
        assert_eq!(m[0], 9);
        assert_eq!(u64::from_be_bytes(m[1..9].try_into().unwrap()), 7);
        assert_eq!(m[9], 0);
        let len = u32::from_be_bytes(m[10..14].try_into().unwrap()) as usize;
        assert_eq!(len, "héllo".len());
        assert_eq!(&m[14..], "héllo".as_bytes());
        assert_eq!(encode_set(1, "x", true)[9], 1);
    }

    #[test]
    fn long_text_is_cut_on_a_character_boundary() {
        let text = "é".repeat(MAX_TEXT); // two bytes each
        let m = encode_set(0, &text, false);
        let len = u32::from_be_bytes(m[10..14].try_into().unwrap()) as usize;
        assert!(len <= MAX_TEXT && len.is_multiple_of(2));
        assert!(String::from_utf8(m[14..].to_vec()).is_ok());
    }

    #[test]
    fn reads_whole_messages_and_keeps_a_partial_one() {
        let mut buf = vec![0, 0, 0, 0, 3, b'a', b'b', b'c', 1, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0];
        let got = parse(&mut buf).unwrap();
        assert_eq!(got, vec![DeviceMsg::Clipboard("abc".into()), DeviceMsg::Ack(9)]);
        assert_eq!(buf, vec![0, 0, 0]); // the start of the next clipboard message
        buf.extend_from_slice(&[0, 2, b'h', b'i']);
        assert_eq!(parse(&mut buf).unwrap(), vec![DeviceMsg::Clipboard("hi".into())]);
        assert!(buf.is_empty());
    }

    #[test]
    fn skips_hid_output_and_rejects_unknown_types() {
        let mut buf = vec![2, 0, 1, 0, 2, 0xAA, 0xBB, 0, 0, 0, 0, 1, b'x'];
        assert_eq!(parse(&mut buf).unwrap(), vec![DeviceMsg::UhidOutput, DeviceMsg::Clipboard("x".into())]);
        assert!(parse(&mut vec![99, 1, 2]).is_none());
    }

    #[test]
    fn the_same_text_does_not_bounce() {
        let mut s = Sync::new(true);
        assert_eq!(s.mac_changed("hello"), Some("hello".into()));
        assert_eq!(s.mac_changed("hello"), None); // unchanged on the Mac
        assert!(!s.phone_changed("hello")); // the phone echoing what we just sent
        assert!(s.phone_changed("from the phone"));
        assert_eq!(s.mac_changed("from the phone"), None); // the Mac now has it
        assert_eq!(s.mac_changed(""), None);
    }

    #[test]
    fn nothing_moves_when_off() {
        let mut s = Sync::new(false);
        assert_eq!(s.mac_changed("x"), None);
        assert!(!s.phone_changed("x"));
        s.enabled = true;
        assert_eq!(s.mac_changed("x"), Some("x".into()));
    }
}
