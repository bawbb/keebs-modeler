//! Control window: every parameter as a live slider, plus a computer-keyboard fallback.

use crate::audio::Audio;
use crate::midi;
use crate::shared::{EventTx, SharedParams, Stats, send};
use eframe::egui;
use keebs_core::{Event, PARAMS, ParamValues};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;

/// Computer keys → semitone offset from the base note (piano-style layout).
const KEYMAP: [(egui::Key, u8); 13] = {
    use egui::Key::*;
    [(A, 0), (W, 1), (S, 2), (E, 3), (D, 4), (F, 5), (T, 6), (G, 7), (Y, 8), (H, 9), (U, 10), (J, 11), (K, 12)]
};

pub struct App {
    params: ParamValues,
    shared: Arc<SharedParams>,
    stats: Arc<Stats>,
    tx: EventTx,
    audio: Audio,
    midi: Vec<midir::MidiInputConnection<()>>,
    midi_name: String,
    midi_ports: Vec<String>,
    base_note: u8,
    key_velocity: u8,
    peak_hold: f32,
}

impl App {
    pub fn new(shared: Arc<SharedParams>, stats: Arc<Stats>, tx: EventTx, audio: Audio, midi_wanted: &str) -> Self {
        let mut app = Self {
            params: shared.load(),
            shared,
            stats,
            tx,
            audio,
            midi: Vec::new(),
            midi_name: String::new(),
            midi_ports: midi::port_names(),
            base_note: 60,
            key_velocity: 90,
            peak_hold: 0.0,
        };
        app.connect_midi(midi_wanted);
        app
    }

    fn connect_midi(&mut self, wanted: &str) {
        self.midi.clear(); // drop old connections first
        match midi::connect(wanted, self.tx.clone()) {
            Ok((conns, name)) => {
                self.midi = conns;
                self.midi_name = name;
            }
            Err(e) => self.midi_name = format!("none ({e})"),
        }
    }

    fn computer_keys(&mut self, ui: &egui::Ui) {
        let events = ui.input(|i| i.events.clone());
        for ev in events {
            if let egui::Event::Key { key, pressed, repeat: false, .. } = ev {
                if let Some((_, off)) = KEYMAP.iter().find(|(k, _)| *k == key) {
                    let note = self.base_note.saturating_add(*off).min(127);
                    let ev = if pressed {
                        Event::NoteOn { key: note, velocity: self.key_velocity }
                    } else {
                        Event::NoteOff { key: note }
                    };
                    send(&self.tx, ev);
                } else if pressed && key == egui::Key::Z {
                    self.base_note = self.base_note.saturating_sub(12).max(12);
                } else if pressed && key == egui::Key::X {
                    self.base_note = (self.base_note + 12).min(108);
                }
            }
        }
    }

    fn status(&mut self, ui: &mut egui::Ui) {
        let load = self.stats.load();
        let peak = self.stats.take_peak();
        self.peak_hold = peak.max(self.peak_hold * 0.9);
        let sustain = f32::from_bits(self.stats.sustain.load(Relaxed));
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} · {} Hz · {}", self.audio.device, self.audio.sample_rate, self.audio.buffer));
            ui.separator();
            ui.label(format!("voices {}", self.stats.voices.load(Relaxed)));
            ui.separator();
            ui.label(format!("cpu {:.0}%", load * 100.0));
            ui.separator();
            ui.label(format!("last vel {}", self.stats.last_velocity.load(Relaxed)));
            ui.separator();
            ui.label(format!("pedal {:.0}%", sustain * 100.0));
            ui.separator();
            let db = 20.0 * self.peak_hold.max(1e-6).log10();
            ui.add(egui::ProgressBar::new(((db + 60.0) / 60.0).clamp(0.0, 1.0)).desired_width(120.0).text(format!("{db:.0} dBFS")));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("MIDI:");
            egui::ComboBox::from_id_salt("midi")
                .selected_text(&self.midi_name)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(self.midi_name.starts_with("all"), "All inputs").clicked() {
                        self.connect_midi("");
                    }
                    for name in self.midi_ports.clone() {
                        if ui.selectable_label(self.midi_name == name, &name).clicked() {
                            self.connect_midi(&name);
                        }
                    }
                });
            if ui.button("Rescan").clicked() {
                self.midi_ports = midi::port_names();
            }
        });
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        let mut group = "";
        for s in &PARAMS {
            if s.group != group {
                group = s.group;
                ui.add_space(6.0);
                ui.strong(group);
            }
            let mut v = self.params.get(s.id);
            let slider = egui::Slider::new(&mut v, s.min..=s.max)
                .logarithmic(s.log)
                .text(if s.unit.is_empty() { s.label.to_string() } else { format!("{} ({})", s.label, s.unit) });
            let r = ui.add(slider).on_hover_text(s.help);
            if r.double_clicked() {
                v = s.default;
            }
            if v != self.params.get(s.id) {
                self.params.set(s.id, v);
                changed = true;
            }
        }
        ui.add_space(8.0);
        if ui.button("Reset all to defaults").clicked() {
            self.params = ParamValues::default();
            changed = true;
        }
        if changed {
            self.shared.store(&self.params);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.computer_keys(ui);
        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            self.status(ui);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(format!("Computer keys: A–K play from note {} · Z/X octave · velocity", self.base_note));
                ui.add(egui::Slider::new(&mut self.key_velocity, 1..=127));
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| self.controls(ui));
            ui.small("Hover a slider for what it does · double-click to reset it");
        });
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
}
