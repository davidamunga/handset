//! The app icon (the Dock icon on macOS, the window and taskbar icon elsewhere) follows what Handset is doing: the robot's antennae are grey while it waits for a phone,
//! amber while connecting, green while mirroring and red, with a red dot, while recording. The icons are
//! made from the app icon by `scripts/make-icon.sh`.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum State {
    Idle,
    Connecting,
    Live,
    Recording,
}

pub fn state_for(recording: bool, mirroring: bool, connecting: bool) -> State {
    if recording {
        State::Recording
    } else if mirroring {
        State::Live
    } else if connecting {
        State::Connecting
    } else {
        State::Idle
    }
}

fn icon(state: State) -> &'static [u8] {
    match state {
        State::Idle => include_bytes!("../packaging/icon/state-idle.png"),
        State::Connecting => include_bytes!("../packaging/icon/state-connecting.png"),
        State::Live => include_bytes!("../packaging/icon/state-live.png"),
        State::Recording => include_bytes!("../packaging/icon/state-recording.png"),
    }
}

/// Show the icon for `state`: the Dock icon on macOS, the window (and taskbar) icon elsewhere. Cheap to call
/// often: it only does anything when the state changes.
pub fn set(state: State, window: &winit::window::Window) {
    use std::sync::Mutex;

    static CURRENT: Mutex<Option<State>> = Mutex::new(None);
    let mut current = CURRENT.lock().unwrap();
    if *current == Some(state) {
        return;
    }
    if !show(icon(state), window) {
        return;
    }
    *current = Some(state);
    if std::env::var_os("HANDSET_DEBUG").is_some() {
        eprintln!("app icon: {state:?}");
    }
}

#[cfg(target_os = "macos")]
fn show(png: &'static [u8], _window: &winit::window::Window) -> bool {
    set_image(png)
}

#[cfg(not(target_os = "macos"))]
fn show(png: &'static [u8], window: &winit::window::Window) -> bool {
    let Some((rgba, w, h)) = decode_png(png) else { return false };
    let Ok(icon) = winit::window::Icon::from_rgba(rgba, w, h) else { return false };
    window.set_window_icon(Some(icon));
    true
}

/// Show this PNG as the Dock icon; false when it could not be set (not on the main thread, or not an image).
#[cfg(target_os = "macos")]
pub fn set_image(png: &'static [u8]) -> bool {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    // AppKit objects belong to the main thread, which is where the event loop runs
    let Some(mtm) = objc2::MainThreadMarker::new() else { return false };
    let data = NSData::with_bytes(png);
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else { return false };
    // SAFETY: called on the main thread with a valid image; AppKit retains it
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    true
}

/// A PNG as straight RGBA8 with its size.
pub fn decode_png(png: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png)).read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    Some((buf, info.width, info.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_wins() {
        assert_eq!(state_for(false, false, false), State::Idle);
        assert_eq!(state_for(false, false, true), State::Connecting);
        assert_eq!(state_for(false, true, false), State::Live);
        assert_eq!(state_for(true, true, false), State::Recording);
        // recording outranks everything, mirroring outranks connecting
        assert_eq!(state_for(true, false, true), State::Recording);
        assert_eq!(state_for(false, true, true), State::Live);
    }

    #[test]
    fn every_icon_is_a_square_rgba_png() {
        for s in [State::Idle, State::Connecting, State::Live, State::Recording] {
            let reader = png::Decoder::new(std::io::Cursor::new(icon(s))).read_info().unwrap();
            let info = reader.info();
            assert_eq!((info.width, info.height), (512, 512), "{s:?}");
            assert_eq!(reader.output_color_type().0, png::ColorType::Rgba, "{s:?}");
        }
    }
}
