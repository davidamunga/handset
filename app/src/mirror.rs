//! One mirroring session: the scrcpy server, the video and audio threads, local playback.

use crate::audio;
use crate::control::Control;
use crate::server;
use crate::stream::{self, Slot, UserEvent};
use std::sync::{Arc, Mutex};
use winit::event_loop::EventLoopProxy;

/// Keeps the session alive: dropping it kills the server process and removes the adb forward.
pub struct Mirror {
    pub session: server::Session,
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
        Ok((Mirror { session, _playback: playback }, control))
    }

    pub fn with_audio(&self) -> bool {
        self.session.audio.is_some()
    }
}
