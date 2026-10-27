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

#[cfg(windows)]
pub mod win {
    use std::ffi::c_void;

    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 2;
    const CF_DIB: u32 = 8;
    const CF_HDROP: u32 = 15;

    unsafe extern "system" {
        fn OpenClipboard(hwnd: *mut c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn EmptyClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut c_void;
        fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
        fn GlobalLock(mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(mem: *mut c_void) -> i32;
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalFree(mem: *mut c_void) -> *mut c_void;
        fn RegisterClipboardFormatW(name: *const u16) -> u32;
    }

    /// Another program may hold the clipboard for a moment, so retry briefly.
    fn open() -> bool {
        for _ in 0..10 {
            // SAFETY: a null owner window is allowed
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    pub fn read() -> Option<String> {
        if !open() {
            return None;
        }
        // SAFETY: the clipboard is open; the handle is locked while we copy the NUL-terminated UTF-16
        let text = unsafe {
            let h = GetClipboardData(CF_UNICODETEXT);
            if h.is_null() {
                None
            } else {
                let p = GlobalLock(h) as *const u16;
                if p.is_null() {
                    None
                } else {
                    let mut n = 0;
                    while *p.add(n) != 0 {
                        n += 1;
                    }
                    let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
                    GlobalUnlock(h);
                    Some(s)
                }
            }
        };
        // SAFETY: matches the successful open above
        unsafe { CloseClipboard() };
        text
    }

    fn set_data(format: u32, bytes: &[u8]) -> bool {
        // SAFETY: the clipboard is open; on success the system owns the allocation, otherwise we free it
        unsafe {
            let mem = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
            if mem.is_null() {
                return false;
            }
            let p = GlobalLock(mem) as *mut u8;
            if p.is_null() {
                GlobalFree(mem);
                return false;
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            GlobalUnlock(mem);
            if SetClipboardData(format, mem).is_null() {
                GlobalFree(mem);
                return false;
            }
            true
        }
    }

    /// A 32-bit bottom-up DIB, composited over white because many apps ignore its alpha.
    fn dib(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(40 + rgba.len());
        out.extend(40u32.to_le_bytes());
        out.extend((w as i32).to_le_bytes());
        out.extend((h as i32).to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend([0u8; 24]);
        for row in rgba.chunks_exact(w as usize * 4).rev() {
            for px in row.chunks_exact(4) {
                let a = px[3] as u32;
                let over = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
                out.extend([over(px[2]), over(px[1]), over(px[0]), 255]);
            }
        }
        out
    }

    /// A PNG goes on as an image (PNG and DIB flavours), anything else as a file reference.
    pub fn copy_capture(path: &std::path::Path) -> bool {
        let is_png = path.extension().is_some_and(|e| e == "png");
        let image = if is_png {
            (|| {
                let bytes = std::fs::read(path).ok()?;
                let mut reader = png::Decoder::new(std::io::Cursor::new(&bytes)).read_info().ok()?;
                let mut buf = vec![0; reader.output_buffer_size()?];
                let info = reader.next_frame(&mut buf).ok()?;
                (info.color_type == png::ColorType::Rgba && info.bit_depth == png::BitDepth::Eight)
                    .then(|| (bytes, dib(&buf[..info.buffer_size()], info.width, info.height)))
            })()
        } else {
            None
        };
        if is_png && image.is_none() {
            return false;
        }
        if !open() {
            return false;
        }
        // SAFETY: the clipboard is open; the format name is NUL-terminated UTF-16
        let ok = unsafe {
            EmptyClipboard();
            if let Some((png_bytes, dib_bytes)) = &image {
                let name: Vec<u16> = "PNG".encode_utf16().chain(Some(0)).collect();
                let png_format = RegisterClipboardFormatW(name.as_ptr());
                let a = png_format != 0 && set_data(png_format, png_bytes);
                let b = set_data(CF_DIB, dib_bytes);
                a || b
            } else {
                // DROPFILES: offset of the list, point, non-client flag, wide-char flag, then the path
                let mut bytes = Vec::new();
                bytes.extend(20u32.to_le_bytes());
                bytes.extend([0u8; 12]);
                bytes.extend(1u32.to_le_bytes());
                for u in path.as_os_str().to_string_lossy().encode_utf16().chain([0, 0]) {
                    bytes.extend(u.to_le_bytes());
                }
                set_data(CF_HDROP, &bytes)
            }
        };
        // SAFETY: matches the successful open above
        unsafe { CloseClipboard() };
        ok
    }

    pub fn write(text: &str) {
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        if !open() {
            return;
        }
        // SAFETY: the clipboard is open; on success it owns the allocation, otherwise we free it
        unsafe {
            EmptyClipboard();
            let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
            if !mem.is_null() {
                let p = GlobalLock(mem) as *mut u16;
                if p.is_null() {
                    GlobalFree(mem);
                } else {
                    std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                    GlobalUnlock(mem);
                    if SetClipboardData(CF_UNICODETEXT, mem).is_null() {
                        GlobalFree(mem);
                    }
                }
            }
            CloseClipboard();
        }
    }
}

/// The Mac's clipboard as text, or `None` if it holds something else.
#[cfg(target_os = "macos")]
pub fn read_mac() -> Option<String> {
    let out = std::process::Command::new("pbpaste").env("LANG", "en_US.UTF-8").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(windows)]
pub fn read_mac() -> Option<String> {
    win::read()
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn read_mac() -> Option<String> {
    crate::tools::linux::paste()
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

#[cfg(windows)]
fn write_mac(text: &str) {
    win::write(text);
}

#[cfg(not(any(target_os = "macos", windows)))]
fn write_mac(text: &str) {
    crate::tools::linux::copy(text.as_bytes(), None);
}

/// Put `text` on the Mac's (or PC's) clipboard.
pub fn write_text(text: &str) {
    write_mac(text);
}

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
