//! One mirroring session: the scrcpy server, the video and audio threads, local playback.

use crate::audio;
use crate::control::Control;
use crate::server;
use crate::stream::{self, Slot, UserEvent};
use std::sync::{Arc, Mutex};
use winit::event_loop::EventLoopProxy;

/// What is being mirrored.
enum Source {
    Android(server::Session),
    #[cfg(target_os = "macos")]
    Ios(crate::ios::Capture),
}

/// Keeps the session alive: dropping it kills the server process and removes the adb forward (or, for
/// an iOS device, stops the capture).
pub struct Mirror {
    source: Source,
    /// The playback stream is not `Send`, so it stays on the thread that made it.
    _playback: Option<audio::Playback>,
}

impl Mirror {
    pub fn start(
        serial: &str,
        opts: &server::Options,
        mute: bool,
        proxy: EventLoopProxy<UserEvent>,
        slot: Arc<Mutex<Slot>>,
        audio_sink: audio::RecSink,
        sync: Arc<Mutex<crate::clipboard::Sync>>,
    ) -> Result<(Mirror, Control), String> {
        let session = server::start(serial, opts).map_err(|e| format!("Could not start mirroring: {e}"))?;
        eprintln!("connected: {}", session.device_name);
        if let Some(addr) = crate::connect::retry_addr(serial) {
            crate::recent::remember(&addr, &session.device_name);
        }

        stream::spawn(session.video.try_clone().unwrap(), slot, proxy);

        // The server writes device messages (the phone's clipboard...) on the control socket: read them so
        // clipboard text reaches the Mac and the server never blocks.
        crate::clipboard::spawn_reader(session.control.try_clone().unwrap(), sync);

        // Audio: play it here (unless --mute) and give recordings a copy.
        let playback = (session.audio.is_some() && !mute)
            .then(|| audio::Playback::new().map_err(|e| eprintln!("no local playback: {e}")).ok())
            .flatten();
        if let Some(a) = &session.audio {
            audio::spawn(a.try_clone().unwrap(), playback.as_ref().map(|p| p.ring()), audio_sink);
        }

        let control = Control::new(session.control.try_clone().unwrap());
        Ok((Mirror { source: Source::Android(session), _playback: playback }, control))
    }

    /// Mirror an iOS device over USB, view only.
    #[cfg(target_os = "macos")]
    pub fn start_ios(
        device: &crate::ios::Device,
        with_audio: bool,
        mute: bool,
        proxy: EventLoopProxy<UserEvent>,
        slot: Arc<Mutex<Slot>>,
        audio_sink: audio::RecSink,
    ) -> Result<Mirror, String> {
        let playback = (with_audio && !mute)
            .then(|| audio::Playback::new().map_err(|e| eprintln!("no local playback: {e}")).ok())
            .flatten();
        let capture = crate::ios::Capture::start(
            device,
            with_audio,
            playback.as_ref().map(|p| p.ring()),
            audio_sink,
            slot,
            proxy,
        )?;
        eprintln!("connected: {} (iOS, view only)", capture.name);
        Ok(Mirror { source: Source::Ios(capture), _playback: playback })
    }

    pub fn with_audio(&self) -> bool {
        match &self.source {
            Source::Android(s) => s.audio.is_some(),
            #[cfg(target_os = "macos")]
            Source::Ios(c) => c.audio,
        }
    }

    /// The adb serial, or the capture device's id for an iOS device.
    pub fn serial(&self) -> &str {
        match &self.source {
            Source::Android(s) => s.serial(),
            #[cfg(target_os = "macos")]
            Source::Ios(c) => &c.uid,
        }
    }

    pub fn device_name(&self) -> &str {
        match &self.source {
            Source::Android(s) => &s.device_name,
            #[cfg(target_os = "macos")]
            Source::Ios(c) => &c.name,
        }
    }

    /// An iOS device: no control, and nothing for adb to wait for when it goes away.
    pub fn is_ios(&self) -> bool {
        match &self.source {
            Source::Android(_) => false,
            #[cfg(target_os = "macos")]
            Source::Ios(_) => true,
        }
    }
}
