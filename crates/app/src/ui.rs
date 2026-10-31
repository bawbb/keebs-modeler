//! Control window: every parameter as a live slider, plus a computer-keyboard fallback.

use crate::audio::Audio;
use crate::midi;
use crate::shared::{EventTx, SharedParams, Stats, send};
use eframe::egui;
use keebs_core::{Event, PARAMS, ParamId, ParamValues};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;

/// Computer keys → semitone offset from the base note (piano-style layout).
const KEYMAP: [(egui::Key, u8); 13] = {
    use egui::Key::*;
    [(A, 0), (W, 1), (S, 2), (E, 3), (D, 4), (F, 5), (T, 6), (G, 7), (Y, 8), (H, 9), (U, 10), (J, 11), (K, 12)]
};

/// Width of the parameter-name column in the slider list.
const LABEL_WIDTH: f32 = 230.0;
/// Room for the numeric box a slider draws after its track.
const VALUE_WIDTH: f32 = 80.0;

pub struct App {
    params: ParamValues,
    shared: Arc<SharedParams>,
    stats: Arc<Stats>,
    tx: EventTx,
    audio: Option<Audio>,
    audio_error: Option<String>,
    audio_devices: Vec<String>,
    buffer_frames: u32,
    midi: Vec<midir::MidiInputConnection<()>>,
    midi_name: String,
    midi_ports: Vec<String>,
    base_note: u8,
    key_velocity: u8,
    peak_hold: f32,
    wheel: WheelLatch,
}

/// What the current scroll gesture is driving. Decided when the gesture starts
/// and kept until it ends, so scrolling the list never grabs a slider it passes
/// over, and adjusting a slider never scrolls the list.
#[derive(Clone, Copy, PartialEq)]
enum WheelTarget {
    Param(ParamId),
    List,
}

#[derive(Default)]
struct WheelLatch {
    target: Option<WheelTarget>,
    /// Time of the last scroll input, s.
    last: f64,
    /// Each slider's value box, from the previous frame.
    areas: Vec<(ParamId, egui::Rect)>,
}

impl App {
    pub fn new(
        shared: Arc<SharedParams>,
        stats: Arc<Stats>,
        tx: EventTx,
        audio: Audio,
        buffer_frames: u32,
        midi_wanted: &str,
    ) -> Self {
        let mut app = Self {
            params: shared.load(),
            shared,
            stats,
            tx,
            audio: Some(audio),
            audio_error: None,
            audio_devices: crate::audio::output_device_names(),
            buffer_frames,
            midi: Vec::new(),
            midi_name: String::new(),
            midi_ports: midi::port_names(),
            base_note: 60,
            key_velocity: 90,
            peak_hold: 0.0,
            wheel: WheelLatch::default(),
        };
        app.connect_midi(midi_wanted);
        app
    }

    /// Reopen audio on `choice` (`None` = auto). Sounding notes stop.
    fn switch_audio(&mut self, choice: Option<String>) {
        self.audio = None; // close the old stream before opening a new one
        match crate::audio::start(choice.as_deref(), self.shared.clone(), self.stats.clone(), self.buffer_frames) {
            Ok((audio, producer)) => {
                if let Ok(mut tx) = self.tx.lock() {
                    *tx = producer;
                }
                self.audio = Some(audio);
                self.audio_error = None;
            }
            Err(e) => self.audio_error = Some(e),
        }
    }

    fn audio_row(&mut self, ui: &mut egui::Ui) {
        if self.stats.device_changed.swap(false, Relaxed)
            && let Some(audio) = self.audio.as_mut()
            && audio.choice.is_none()
            && let Some(name) = crate::audio::default_device_name()
        {
            audio.device = name;
        }
        let current = match (&self.audio, &self.audio_error) {
            (_, Some(e)) => format!("none ({e})"),
            (Some(a), None) if a.choice.is_none() => format!("Auto ({})", a.device),
            (Some(a), None) => a.device.clone(),
            (None, None) => "none".into(),
        };
        let choice = self.audio.as_ref().and_then(|a| a.choice.clone());
        let mut pick = None;
        ui.horizontal(|ui| {
            ui.label("Audio out:");
            egui::ComboBox::from_id_salt("audio")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(choice.is_none(), "Auto (follow system output)").clicked() {
                        pick = Some(None);
                    }
                    for name in &self.audio_devices {
                        if ui.selectable_label(choice.as_deref() == Some(name), name).clicked() {
                            pick = Some(Some(name.clone()));
                        }
                    }
                });
            if ui.button("Rescan").clicked() {
                self.audio_devices = crate::audio::output_device_names();
            }
        });
        if let Some(choice) = pick {
            self.switch_audio(choice);
        }
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

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let load = self.stats.load();
        let peak = self.stats.take_peak();
        self.peak_hold = peak.max(self.peak_hold * 0.9);
        let sustain = f32::from_bits(self.stats.sustain.load(Relaxed));
        ui.horizontal(|ui| {
            match &self.audio {
                _ if self.stats.device_lost.load(Relaxed) => ui.label("no output device"),
                Some(a) => ui.label(format!("{} Hz · {}", a.sample_rate, a.buffer)),
                None => ui.label("audio off"),
            };
            ui.separator();
            ui.label(format!("voices {}", self.stats.voices.load(Relaxed)));
            ui.separator();
            ui.label(format!("cpu {:.0}%", load * 100.0));
            ui.separator();
            ui.label(format!("last vel {}", self.stats.last_velocity.load(Relaxed)));
            ui.separator();
            let lift = keebs_core::engine::pedal_lift(sustain, &self.params);
            ui.label(format!("pedal {:.0}% → dampers {:.0}% lifted", sustain * 100.0, lift * 100.0));
            ui.separator();
            let db = 20.0 * self.peak_hold.max(1e-6).log10();
            ui.add(egui::ProgressBar::new(((db + 60.0) / 60.0).clamp(0.0, 1.0)).desired_width(120.0).text(format!("{db:.0} dBFS")));
        });
    }

    fn midi_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
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
        self.wheel.areas.clear();
        let mut changed = false;
        let mut group = "";
        for s in &PARAMS {
            if s.group != group {
                group = s.group;
                ui.add_space(6.0);
                ui.strong(group);
            }
            let mut v = self.params.get(s.id);
            let label = if s.unit.is_empty() { s.label.to_string() } else { format!("{} ({})", s.label, s.unit) };
            let row = ui.horizontal(|ui| {
                let h = ui.spacing().interact_size.y;
                ui.allocate_ui_with_layout(
                    egui::vec2(LABEL_WIDTH, h),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_width(LABEL_WIDTH);
                        ui.label(label).on_hover_text(s.help);
                    },
                );
                // Leave room for the value box the slider draws to its right.
                let track = (ui.available_width() - VALUE_WIDTH).max(80.0);
                ui.spacing_mut().slider_width = track;
                let r = ui.add(egui::Slider::new(&mut v, s.min..=s.max).logarithmic(s.log)).on_hover_text(s.help);
                // The response covers track + value box; keep just the value box.
                let mut value_box = r.rect;
                value_box.min.x += track;
                value_box
            });
            let area = row.inner;
            self.wheel.areas.push((s.id, area));
            if ui.rect_contains_pointer(area)
                && ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary))
            {
                v = s.default;
                // The first click of the double-click opened the box for typing;
                // close it without applying the typed text.
                ui.memory_mut(|m| m.stop_text_input());
            }
            if self.wheel.target == Some(WheelTarget::Param(s.id)) {
                v = wheel_adjust(ui, s, v);
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

/// A pause this long ends a scroll gesture, s.
const WHEEL_GESTURE_GAP: f64 = 0.3;
/// Slider travel per point of scroll at the reference speed, as a fraction of the range.
const WHEEL_STEP: f32 = 0.0006;
/// Scroll speed (points/s) at which the step is exactly `WHEEL_STEP`.
const WHEEL_REF_SPEED: f32 = 500.0;
/// How strongly speed scales the step: slow scrolls get finer, fast ones coarser.
const WHEEL_ACCEL: f32 = 0.9;
/// Holding Option/Alt makes the wheel this much finer.
const WHEEL_FINE: f32 = 0.1;

impl WheelLatch {
    /// Call once per frame before the list is drawn: start or end gestures.
    fn update(&mut self, ui: &egui::Ui) {
        let (delta, now, pointer) = ui.input(|i| (i.smooth_scroll_delta, i.time, i.pointer.hover_pos()));
        if delta == egui::Vec2::ZERO {
            if now - self.last > WHEEL_GESTURE_GAP {
                self.target = None;
            }
            return;
        }
        self.last = now;
        if self.target.is_none() {
            let over = pointer.and_then(|p| self.areas.iter().find(|(_, r)| r.contains(p)));
            self.target = Some(match over {
                Some((id, _)) => WheelTarget::Param(*id),
                None => WheelTarget::List,
            });
        }
    }

    fn adjusting(&self) -> bool {
        matches!(self.target, Some(WheelTarget::Param(_)))
    }
}

/// Nudge `v` by this frame's scroll, in the slider's own (linear or log)
/// scale. Faster scrolling moves proportionally further. Takes the scroll.
fn wheel_adjust(ui: &egui::Ui, s: &keebs_core::ParamSpec, v: f32) -> f32 {
    let (delta, alt, dt) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.alt, i.unstable_dt));
    // Shift turns the wheel horizontal on macOS; accept either axis.
    let amount = delta.x + delta.y;
    if amount == 0.0 {
        return v;
    }
    ui.ctx().input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
    let speed = amount.abs() / dt.clamp(1.0 / 240.0, 0.1);
    let gain = (speed / WHEEL_REF_SPEED).powf(WHEEL_ACCEL).clamp(0.05, 20.0);
    let step = amount * WHEEL_STEP * gain * if alt { WHEEL_FINE } else { 1.0 };
    let (min, max) = (s.min, s.max);
    if s.log && min > 0.0 {
        let t = (v / min).ln() / (max / min).ln();
        min * (max / min).powf((t + step).clamp(0.0, 1.0))
    } else {
        let t = (v - min) / (max - min);
        min + (max - min) * (t + step).clamp(0.0, 1.0)
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.computer_keys(ui);
        egui::Panel::top("top").show(ui, |ui| {
            ui.add_space(4.0);
            self.audio_row(ui);
            self.midi_row(ui);
            ui.horizontal(|ui| {
                ui.label(format!("Computer keys: A–K play from note {} · Z/X octave · velocity", self.base_note));
                ui.add(egui::Slider::new(&mut self.key_velocity, 1..=127));
            });
            ui.small("Hover a control for what it does · scroll over a value to nudge it (hold Option for fine) · double-click a value to reset");
            ui.add_space(4.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.add_space(2.0);
            self.status_bar(ui);
            ui.add_space(2.0);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            self.wheel.update(ui);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .scroll_source(egui::scroll_area::ScrollSource {
                    mouse_wheel: !self.wheel.adjusting(),
                    ..Default::default()
                })
                .show(ui, |ui| self.controls(ui));
        });
        ui.ctx().request_repaint_after(Duration::from_millis(50));
    }
}
