//! Control window: every parameter as a live slider, plus a computer-keyboard fallback.

use crate::audio::Audio;
use crate::midi;
use crate::presets;
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
    preset: PresetState,
}

/// What the current scroll gesture is driving. Decided when the gesture starts
/// and kept until it ends, so scrolling the list never grabs a slider it passes
/// over, and adjusting a slider never scrolls the list.
#[derive(Clone, Copy, PartialEq)]
enum WheelTarget {
    Param(ParamId),
    List,
}

/// The preset row: what's loaded, the name being typed, and any pending confirmation.
#[derive(Default)]
struct PresetState {
    names: Vec<String>,
    /// The preset last loaded or saved, and its values (to show unsaved changes and revert).
    current: Option<(String, ParamValues)>,
    /// The name box.
    name: String,
    confirm: Option<Confirm>,
    message: Option<Message>,
}

enum Confirm {
    Overwrite(String),
    Delete(String),
}

struct Message {
    text: String,
    detail: Vec<String>,
    error: bool,
}

impl Message {
    fn ok(text: String) -> Option<Self> {
        Some(Self { text, detail: Vec::new(), error: false })
    }
    fn err(text: String) -> Option<Self> {
        Some(Self { text, detail: Vec::new(), error: true })
    }
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
        preset: Option<&str>,
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
            preset: PresetState { names: presets::list(), ..Default::default() },
        };
        app.connect_midi(midi_wanted);
        app.load_preset(preset.unwrap_or(presets::DEFAULT));
        if preset.is_none() {
            app.preset.message = None; // starting on the defaults needs no announcement
        }
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
        // While typing (a preset name, a value), keys type; releases still pass so nothing sticks.
        let typing = ui.ctx().text_edit_focused();
        for ev in events {
            if let egui::Event::Key { key, pressed, repeat: false, .. } = ev {
                if typing && pressed {
                    continue;
                }
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

    fn load_preset(&mut self, name: &str) {
        let st = &mut self.preset;
        st.confirm = None;
        match presets::load(name) {
            Ok((p, warnings)) => {
                self.params = p;
                self.shared.store(&p);
                st.current = Some((name.to_string(), p));
                // The built-in can't be saved over, so don't offer its name for saving.
                st.name = if name == presets::DEFAULT { String::new() } else { name.to_string() };
                st.message = Some(if warnings.is_empty() {
                    Message { text: format!("Loaded '{name}'"), detail: Vec::new(), error: false }
                } else {
                    let n = warnings.len();
                    let s = if n == 1 { "" } else { "s" };
                    Message { text: format!("Loaded '{name}' with {n} warning{s} (hover)"), detail: warnings, error: true }
                });
            }
            Err(e) => st.message = Message::err(e),
        }
    }

    fn save_preset(&mut self, name: &str) {
        let st = &mut self.preset;
        st.confirm = None;
        match presets::save(name, &self.params) {
            Ok(()) => {
                st.current = Some((name.to_string(), self.params));
                st.names = presets::list();
                st.message = Message::ok(format!("Saved '{name}'"));
            }
            Err(e) => st.message = Message::err(e),
        }
    }

    fn preset_row(&mut self, ui: &mut egui::Ui) {
        let modified = self.preset.current.as_ref().is_some_and(|(_, p)| *p != self.params);
        let current_name = self.preset.current.as_ref().map(|(n, _)| n.clone());
        let mut pick = None;
        ui.horizontal(|ui| {
            ui.label("Preset:");
            let shown = match &current_name {
                Some(n) if modified => format!("{n} (modified)"),
                Some(n) => n.clone(),
                None => "none loaded".into(),
            };
            let combo = egui::ComboBox::from_id_salt("preset").selected_text(shown).width(200.0).show_ui(ui, |ui| {
                for name in &self.preset.names {
                    let label = if name == presets::DEFAULT { "Default (built in)" } else { name.as_str() };
                    if ui.selectable_label(current_name.as_deref() == Some(name), label).clicked() {
                        pick = Some(name.clone());
                    }
                    if name == presets::DEFAULT {
                        ui.separator();
                        if self.preset.names.len() == 1 {
                            ui.weak("no saved presets yet");
                        }
                    }
                }
            });
            // Pick up files added or removed outside the app each time the list opens.
            if combo.response.clicked() {
                self.preset.names = presets::list();
            }

            match self.preset.confirm.take() {
                Some(Confirm::Overwrite(name)) => {
                    ui.label(format!("'{name}' exists. Overwrite it?"));
                    if ui.button("Overwrite").clicked() {
                        self.save_preset(&name);
                    } else if !ui.button("Cancel").clicked() {
                        self.preset.confirm = Some(Confirm::Overwrite(name));
                    }
                }
                Some(Confirm::Delete(name)) => {
                    ui.label(format!("Delete '{name}'?"));
                    if ui.button("Delete").clicked() {
                        self.preset.message = match presets::delete(&name) {
                            Ok(()) => {
                                self.preset.current = None;
                                Message::ok(format!("Deleted '{name}'"))
                            }
                            Err(e) => Message::err(e),
                        };
                        self.preset.names = presets::list();
                    } else if !ui.button("Cancel").clicked() {
                        self.preset.confirm = Some(Confirm::Delete(name));
                    }
                }
                None => {
                    let edit = ui.add(egui::TextEdit::singleline(&mut self.preset.name).hint_text("name").desired_width(150.0));
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Save").on_hover_text("Save every setting under this name").clicked() || enter {
                        match presets::check_name(&self.preset.name).map(str::to_string) {
                            Err(e) => self.preset.message = Message::err(e),
                            Ok(name) if presets::exists(&name) && current_name.as_deref() != Some(&name) => {
                                self.preset.confirm = Some(Confirm::Overwrite(name));
                            }
                            Ok(name) => self.save_preset(&name),
                        }
                    }
                    if ui.add_enabled(modified, egui::Button::new("Revert")).on_hover_text("Back to the preset as saved").clicked()
                        && let Some((_, p)) = self.preset.current
                    {
                        self.params = p;
                        self.shared.store(&p);
                        self.preset.message = None;
                    }
                    let deletable = current_name.as_deref().is_some_and(|n| n != presets::DEFAULT);
                    if ui.add_enabled(deletable, egui::Button::new("Delete…")).clicked()
                        && let Some(name) = current_name.clone()
                    {
                        self.preset.confirm = Some(Confirm::Delete(name));
                    }
                }
            }

            if let Some(m) = &self.preset.message {
                let color = if m.error { ui.visuals().warn_fg_color } else { ui.visuals().weak_text_color() };
                let r = ui.colored_label(color, &m.text);
                if !m.detail.is_empty() {
                    r.on_hover_text(m.detail.join("\n"));
                }
            }
        });
        if let Some(name) = pick {
            self.load_preset(&name);
        }
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
                let mut slider = egui::Slider::new(&mut v, s.min..=s.max).logarithmic(s.log);
                if s.unit == "key" {
                    slider = slider
                        .custom_formatter(|k, _| format!("{} · {k:.1}", note_name(k)))
                        .custom_parser(|t| t.rsplit(['·', ' ']).next()?.trim().parse().ok());
                }
                let r = ui.add(slider).on_hover_text(s.help);
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
        if changed {
            self.shared.store(&self.params);
        }
    }
}

/// MIDI key number as a note name, e.g. 60 → "C4".
fn note_name(key: f64) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let k = key.round() as i32;
    format!("{}{}", NAMES[k.rem_euclid(12) as usize], k.div_euclid(12) - 1)
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
            self.preset_row(ui);
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
