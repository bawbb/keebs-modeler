//! One key: a string, its hammer, and its damper.

use crate::hammer::{Hammer, HammerSettings};
use crate::note::NoteDefaults;
use crate::params::{ParamId, ParamValues};
use crate::string::{ModalString, T60_TO_SIGMA};

/// Damper felt reaches full effect over roughly this long, s.
const DAMPER_SMOOTH: f64 = 0.008;
/// A voice is silent once its block peak falls this far below its loudest (−80 dB).
const SILENCE_REL: f64 = 1e-4;
/// Consecutive silent blocks before the voice is freed.
const SILENT_BLOCKS: u32 = 8;
/// Slowest hammer, m/s (so velocity 1 still makes a sound).
const MIN_SPEED: f64 = 0.05;

pub struct Voice {
    pub key: Option<u8>,
    nd: NoteDefaults,
    string: ModalString,
    hammer: Hammer,
    sample_rate: f64,
    /// 0 = damper off the string, 1 = fully on.
    damper_target: f64,
    damper: f64,
    damper_sigma: f64,
    damper_coeff: f64,
    silent_blocks: u32,
    /// Peak of the last processed block.
    pub peak: f64,
    /// Loudest block peak since the last strike.
    max_peak: f64,
}

impl Voice {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            key: None,
            nd: NoteDefaults::for_key(60),
            string: ModalString::new(),
            hammer: Hammer::default(),
            sample_rate,
            damper_target: 0.0,
            damper: 0.0,
            damper_sigma: 0.0,
            damper_coeff: 1.0 - (-1.0 / (DAMPER_SMOOTH * sample_rate)).exp(),
            silent_blocks: 0,
            peak: 0.0,
            max_peak: 0.0,
        }
    }

    pub fn is_free(&self) -> bool {
        self.key.is_none()
    }

    /// Take this voice for `key` (silencing whatever it held).
    pub fn assign(&mut self, key: u8, p: &ParamValues) {
        self.key = Some(key);
        self.nd = NoteDefaults::for_key(key);
        self.string.reset();
        self.configure(p);
        self.hammer = Hammer::default();
        self.damper = 0.0;
        self.damper_target = 0.0;
        self.silent_blocks = 0;
        self.peak = 0.0;
        self.max_peak = 0.0;
    }

    pub fn configure(&mut self, p: &ParamValues) {
        self.string.configure(&self.nd, p, self.sample_rate);
        self.damper_sigma = T60_TO_SIGMA / p.get(ParamId::DamperRelease) as f64;
    }

    pub fn hammer(&self) -> &Hammer {
        &self.hammer
    }

    pub fn mode_count(&self) -> usize {
        self.string.mode_count()
    }

    /// Strike with MIDI velocity 1..=127. The string keeps ringing if it already is.
    pub fn strike(&mut self, velocity: u8, p: &ParamValues) {
        let v = (velocity.clamp(1, 127) as f64) / 127.0;
        let speed = (p.get(ParamId::MaxHammerSpeed) as f64 * v.powf(p.get(ParamId::VelocityCurve) as f64))
            .max(MIN_SPEED);
        let settings = HammerSettings {
            mass: self.nd.hammer_mass * p.get(ParamId::HammerMass) as f64,
            fref: self.nd.hammer_fref * p.get(ParamId::HammerHardness) as f64,
            exponent: p.get(ParamId::FeltExponent) as f64,
            hysteresis: p.get(ParamId::FeltHysteresis) as f64,
        };
        self.hammer.strike(settings, speed, self.string.y_strike());
        self.damper_target = 0.0;
        self.silent_blocks = 0;
        self.max_peak = 0.0;
    }

    /// 0 = undamped, 1 = fully damped.
    pub fn set_damper(&mut self, amount: f64) {
        self.damper_target = amount.clamp(0.0, 1.0);
    }

    /// Add this voice's output into `out`. Frees the voice once it's silent.
    pub fn process(&mut self, out: &mut [f32]) {
        if self.key.is_none() {
            return;
        }
        let dt = 1.0 / self.sample_rate;
        let mut peak: f64 = 0.0;
        for o in out.iter_mut() {
            self.damper += (self.damper_target - self.damper) * self.damper_coeff;
            let damp = if self.damper > 1e-6 { (-self.damper * self.damper_sigma * dt).exp() } else { 1.0 };
            let force = if self.hammer.is_active() {
                let y_free = self.string.predict_strike(damp);
                self.hammer.tick(y_free, self.string.compliance(), dt)
            } else {
                0.0
            };
            let s = self.string.tick(force, damp);
            peak = peak.max(s.abs());
            *o += s as f32;
        }
        self.peak = peak;

        if !peak.is_finite() {
            // Blew up (extreme settings). Silence rather than scream.
            self.string.reset();
            self.key = None;
            return;
        }
        self.max_peak = self.max_peak.max(peak);
        if peak < SILENCE_REL * self.max_peak && !self.hammer.is_active() {
            self.silent_blocks += 1;
            if self.silent_blocks >= SILENT_BLOCKS {
                self.key = None;
            }
        } else {
            self.silent_blocks = 0;
        }
    }
}
