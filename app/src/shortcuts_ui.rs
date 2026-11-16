//! `handset --shortcuts`: a small window to change the keyboard shortcuts. It is its own process, like the
//! editor, and writes to the settings file; the mirroring window re-reads them when it gets focus.

use crate::shortcuts::{ACTIONS, Combo, Shortcuts};
use eframe::egui;

pub fn run() -> Result<(), String> {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Handset Keyboard Shortcuts")
            .with_inner_size([520.0, 560.0])
            .with_min_inner_size([420.0, 320.0]),
        ..Default::default()
    };
    eframe::run_native("Handset Keyboard Shortcuts", opts, Box::new(|_| Ok(Box::new(Window::new()))))
        .map_err(|e| e.to_string())
}

struct Window {
    shortcuts: Shortcuts,
    /// The action waiting for a key press.
    recording: Option<&'static str>,
    /// What the last change did, for the line under the list.
    note: String,
}

impl Window {
    fn new() -> Self {
        Self { shortcuts: Shortcuts::load(&crate::settings::Settings::load()), recording: None, note: String::new() }
    }

    /// A key pressed while an action waits for one: bind it, or cancel on Escape.
    fn listen(&mut self, ctx: &egui::Context) {
        let Some(id) = self.recording else { return };
        let pressed = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, repeat: false, .. } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        let Some((key, mods)) = pressed else { return };
        if key == egui::Key::Escape {
            self.recording = None;
            self.note.clear();
            return;
        }
        match Combo::from_egui(key, mods) {
            Some(combo) if combo.allowed() => {
                let label = |id: &str| ACTIONS.iter().find(|a| a.id == id).map_or("", |a| a.label);
                self.note = match self.shortcuts.set(id, Some(combo.clone())) {
                    Some(from) => format!("{} now runs {}; {} has none.", combo.words(), label(id), label(from)),
                    None => String::new(),
                };
                self.shortcuts.save();
                self.recording = None;
            }
            // a letter on its own would be typed into the phone
            Some(_) => self.note = "Hold Cmd/Ctrl or Alt with the key (F-keys work alone).".to_string(),
            None => self.note = "That key cannot be used.".to_string(),
        }
    }
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.listen(&ctx);
        egui::Panel::bottom("footer").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Restore Defaults").clicked() {
                    self.shortcuts.reset_all();
                    self.shortcuts.save();
                    self.recording = None;
                    self.note = "Defaults restored.".to_string();
                }
                ui.weak(&self.note);
            });
            ui.add_space(6.0);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Keyboard Shortcuts");
            ui.weak("Click a shortcut, then press the keys. Escape cancels. Use Cmd/Ctrl or Alt, or an F-key, so typing still reaches the phone.");
            ui.add_space(8.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("shortcuts").num_columns(3).spacing([16.0, 8.0]).striped(true).show(ui, |ui| {
                    for a in ACTIONS {
                        ui.label(a.label);
                        let waiting = self.recording == Some(a.id);
                        let text = if waiting {
                            "Press keys\u{2026}".to_string()
                        } else {
                            self.shortcuts.get(a.id).map_or("None".to_string(), |c| c.words())
                        };
                        let button = egui::Button::new(text).min_size(egui::vec2(150.0, 0.0)).selected(waiting);
                        if ui.add(button).clicked() {
                            self.recording = if waiting { None } else { Some(a.id) };
                            self.note.clear();
                        }
                        ui.horizontal(|ui| {
                            let set = self.shortcuts.get(a.id).is_some();
                            if ui.add_enabled(set, egui::Button::new("Clear").small()).clicked() {
                                self.shortcuts.set(a.id, None);
                                self.shortcuts.save();
                                self.recording = None;
                            }
                            let default = self.shortcuts.is_default(a.id);
                            if ui.add_enabled(!default, egui::Button::new("Reset").small()).clicked() {
                                self.shortcuts.reset(a.id);
                                self.shortcuts.save();
                                self.recording = None;
                            }
                        });
                        ui.end_row();
                    }
                });
            });
        });
    }
}
