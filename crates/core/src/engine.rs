//! Voices, keys, pedal, and the output stage.

use crate::params::{ParamId, ParamValues};
use crate::voice::Voice;

/// Voices are processed in chunks of this many samples.
const CHUNK: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    NoteOn { key: u8, velocity: u8 },
    NoteOff { key: u8 },
    /// 0 = up, 1 = fully down. Values between are half-pedal.
    Sustain(f32),
}

pub struct Engine {
    voices: Vec<Voice>,
    held: [bool; 128],
    sustain: f64,
    params: ParamValues,
    gain: f32,
    scratch: [f32; CHUNK],
}

impl Engine {
    pub fn new(sample_rate: f32, max_voices: usize) -> Self {
        let params = ParamValues::default();
        Self {
            voices: (0..max_voices).map(|_| Voice::new(sample_rate as f64)).collect(),
            held: [false; 128],
            sustain: 0.0,
            gain: db_to_gain(params.get(ParamId::MasterGain)),
            params,
            scratch: [0.0; CHUNK],
        }
    }

    pub fn params(&self) -> &ParamValues {
        &self.params
    }

    /// Apply new parameters to every sounding voice, keeping them ringing.
    pub fn set_params(&mut self, p: &ParamValues) {
        self.params = *p;
        self.gain = db_to_gain(p.get(ParamId::MasterGain));
        for v in self.voices.iter_mut().filter(|v| !v.is_free()) {
            v.configure(p);
        }
    }

    pub fn voice_for_key(&self, key: u8) -> Option<&Voice> {
        self.voices.iter().find(|v| v.key == Some(key))
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| !v.is_free()).count()
    }

    pub fn handle(&mut self, ev: Event) {
        match ev {
            Event::NoteOn { key, velocity: 0 } => self.note_off(key),
            Event::NoteOn { key, velocity } => self.note_on(key, velocity),
            Event::NoteOff { key } => self.note_off(key),
            Event::Sustain(s) => {
                self.sustain = (s as f64).clamp(0.0, 1.0);
                let damper = 1.0 - self.sustain;
                for v in self.voices.iter_mut() {
                    if let Some(k) = v.key
                        && !self.held[k as usize]
                    {
                        v.set_damper(damper);
                    }
                }
            }
        }
    }

    fn note_on(&mut self, key: u8, velocity: u8) {
        let key = key.min(127);
        self.held[key as usize] = true;
        let idx = match self.voices.iter().position(|v| v.key == Some(key)) {
            Some(i) => i,
            None => {
                let i = self
                    .voices
                    .iter()
                    .position(Voice::is_free)
                    .unwrap_or_else(|| self.quietest_voice());
                self.voices[i].assign(key, &self.params);
                i
            }
        };
        self.voices[idx].strike(velocity, &self.params);
    }

    fn note_off(&mut self, key: u8) {
        let key = key.min(127);
        self.held[key as usize] = false;
        let damper = 1.0 - self.sustain;
        for v in self.voices.iter_mut().filter(|v| v.key == Some(key)) {
            v.set_damper(damper);
        }
    }

    fn quietest_voice(&self) -> usize {
        self.voices
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.peak.total_cmp(&b.1.peak))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Render mono audio, overwriting `out`.
    pub fn render(&mut self, out: &mut [f32]) {
        for chunk in out.chunks_mut(CHUNK) {
            let buf = &mut self.scratch[..chunk.len()];
            buf.fill(0.0);
            for v in self.voices.iter_mut() {
                v.process(buf);
            }
            for (o, s) in chunk.iter_mut().zip(buf.iter()) {
                *o = (s * self.gain).tanh();
            }
        }
    }
}

fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}
