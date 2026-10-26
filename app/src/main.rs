#![cfg_attr(windows, windows_subsystem = "windows")]

mod adb;
mod app;
mod apps;
mod audio;
mod black;
mod capture;
mod clipboard;
mod connect;
mod control;
mod dock;
mod doctor;
mod frame;
mod menu;
mod mirror;
mod overlay;
mod recent;
mod render;
mod server;
mod settings;
mod stream;
mod toolbar;
mod tools;
mod transfer;

use std::sync::{Arc, Mutex};
use winit::event_loop::EventLoop;

fn usage() -> ! {
    eprintln!(
        "usage: handset [--frame <name|path|none>] [--max-size <px>]\n\
         \x20                  [--connect <ip[:port]> | --last | --wifi] [--start-app <package>] [serial]\n\
         \x20      handset --pair <ip:port> <code>\n\
         \x20      handset --doctor\n\
         \x20                  [--audio [--mute] [--gain <dB>] [--no-normalize]] [--no-clipboard] [--no-clipboard-sync] [--record-for <seconds>] [--record-format mp4|gif] [--record-size <px>] [--record-bg #rrggbb] [serial]\n\
         built-in frames: {}\n\
         right-click the phone for the menu; keys: Cmd/Ctrl+S screenshot, Cmd/Ctrl+E record, Cmd/Ctrl+R rotate",
        frame::builtin_names().collect::<Vec<_>>().join(", ")
    );
    std::process::exit(2)
}

fn main() {
    #[cfg(windows)]
    tools::attach_console();
    // what was changed in the menus last time; the command line below overrides it
    let saved = settings::Settings::load();
    let mut frame_spec = match saved.get("frame") {
        Some("none") => None,
        Some(spec) => Some(spec.to_string()),
        None => Some("obsidian".to_string()),
    };
    let mut opts = server::Options::default();
    let mut serial = None;
    let mut cap = capture::Settings::default();
    if let Some(f) = saved.get("format").and_then(capture::Format::parse) {
        cap.format = f;
    }
    cap.normalize = saved.get_bool("normalize").unwrap_or(cap.normalize);
    cap.clipboard = saved.get_bool("clipboard").unwrap_or(cap.clipboard);
    let mut mute = saved.get_bool("mute").unwrap_or(false);
    let mut record_for: Option<f32> = None;
    let mut connect: Option<String> = None;
    let mut wifi = false;
    let mut last = false;
    let mut sync_clipboard = saved.get_bool("sync-clipboard").unwrap_or(true);
    let mut start_app: Option<String> = None;
    // no arguments: use the saved defaults (and `--audio` inside the app bundle)
    let given: Vec<String> = std::env::args().skip(1).collect();
    let mut args = if given.is_empty() { tools::default_args() } else { given }.into_iter();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--frame" => {
                let v = args.next().unwrap_or_else(|| usage());
                frame_spec = (v != "none").then_some(v);
            }
            "--max-size" => opts.max_size = args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage()),
            "--record-format" => {
                cap.format = args.next().and_then(|v| capture::Format::parse(&v)).unwrap_or_else(|| usage())
            }
            "--record-size" => cap.rec_size = args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage()),
            "--record-bg" => cap.rec_bg = args.next().and_then(|v| capture::parse_color(&v)).unwrap_or_else(|| usage()),
            "--audio" => opts.audio = true,
            "--gain" => cap.gain_db = args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage()),
            "--no-normalize" => cap.normalize = false,
            "--no-clipboard" => cap.clipboard = false,
            "--no-clipboard-sync" => sync_clipboard = false,
            "--mute" => mute = true,
            "--record-for" => record_for = Some(args.next().and_then(|v| v.parse().ok()).unwrap_or_else(|| usage())),
            "--connect" => connect = Some(args.next().unwrap_or_else(|| usage())),
            "--wifi" => wifi = true,
            "--last" => last = true,
            "--start-app" => start_app = Some(args.next().unwrap_or_else(|| usage())),
            "--pair" => {
                let addr = args.next().unwrap_or_else(|| usage());
                let code = args.next().unwrap_or_else(|| usage());
                match adb::pair(&addr, &code) {
                    Ok(msg) => {
                        println!("{msg}\nNow run: handset --connect <ip:port> (the address under Wireless debugging)");
                        return;
                    }
                    Err(e) => tools::die(&e.to_string()),
                }
            }
            "--doctor" => std::process::exit(doctor::run()),
            "-h" | "--help" => usage(),
            s if s.starts_with('-') => usage(),
            s => serial = Some(s.to_string()),
        }
    }

    let bezel = frame_spec
        .as_deref()
        .map(|s| frame::load(s).unwrap_or_else(|e| tools::die(&format!("Cannot load the frame: {e}"))));

    let event_loop = EventLoop::<stream::UserEvent>::with_user_event().build().unwrap();
    menu::install(event_loop.create_proxy());
    let slot = Arc::new(Mutex::new(stream::Slot::default()));
    let audio_sink: audio::RecSink = Arc::new(Mutex::new(None));
    let sync = Arc::new(Mutex::new(clipboard::Sync::new(sync_clipboard)));

    // Which phone: `--connect` makes its own; otherwise one that is ready now (the one asked for, or the
    // first). With none, the window opens on the connect screen and waits for a phone.
    if last {
        let recent = recent::load();
        let Some(r) = recent.first() else {
            tools::die("No phone to reconnect to yet. Connect one over Wi-Fi first, then --last will remember it.");
        };
        connect = Some(r.addr.clone());
    }
    let ready: Option<String> = if let Some(addr) = &connect {
        Some(adb::connect(addr).unwrap_or_else(|e| tools::die(&e.to_string())))
    } else {
        adb::devices().ok().and_then(|d| match &serial {
            Some(want) => d.into_iter().find(|s| s == want),
            None => d.into_iter().next(),
        })
    };

    let mut wifi_pending = wifi && connect.is_none();
    let mut mirror = None;
    let mut control = control::Control::disconnected();
    let mut title = "Handset".to_string();
    let mut with_audio = false;
    if let Some(mut serial) = ready {
        if std::mem::take(&mut wifi_pending) {
            serial = adb::wifi(&serial).unwrap_or_else(|e| tools::die(&e.to_string()));
            eprintln!("on Wi-Fi as {serial}: you can unplug the cable");
        }
        let (m, c) = mirror::Mirror::start(
            &serial,
            &opts,
            mute,
            event_loop.create_proxy(),
            slot.clone(),
            audio_sink.clone(),
            sync.clone(),
        )
        .unwrap_or_else(|e| tools::die(&e));
        title = m.session.device_name.clone();
        with_audio = m.with_audio();
        control = c;
        mirror = Some(m);
    }
    let waiting = mirror.is_none();

    let mut app = app::App::new(app::Init {
        bezel,
        title,
        slot,
        control,
        cap,
        audio_sink,
        with_audio,
        frame_spec,
        mirror,
        wait: waiting.then(|| app::Wait { serial: serial.clone(), wifi: wifi_pending }),
        proxy: event_loop.create_proxy(),
        opts,
        mute,
        sync,
        start_app,
        toolbar: saved.get_bool("toolbar").unwrap_or(true),
    });
    app.auto_record = record_for.map(std::time::Duration::from_secs_f32);
    if waiting {
        connect::spawn_poll(event_loop.create_proxy());
    }
    event_loop.run_app(&mut app).unwrap();
    connect::stop_poll();
    // `app` drops here, and with it the mirroring session: that kills the server process and removes
    // the adb forward.
}
