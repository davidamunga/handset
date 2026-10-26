//! The Dock icon follows what Handset is doing: the robot's antennae are grey while it waits for a phone,
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

/// Show the icon for `state`. Cheap to call often: it only does anything when the state changes.
#[cfg(target_os = "macos")]
pub fn set(state: State) {
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    use std::sync::Mutex;

    static CURRENT: Mutex<Option<State>> = Mutex::new(None);
    let mut current = CURRENT.lock().unwrap();
    if *current == Some(state) {
        return;
    }
    // AppKit objects belong to the main thread, which is where the event loop runs
    let Some(mtm) = objc2::MainThreadMarker::new() else { return };
    let data = NSData::with_bytes(icon(state));
    let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else { return };
    // SAFETY: called on the main thread with a valid image; AppKit retains it
    unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    *current = Some(state);
    if std::env::var_os("HANDSET_DEBUG").is_some() {
        eprintln!("dock icon: {state:?}");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set(_state: State) {}

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
