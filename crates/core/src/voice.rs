//! One key: a string, its hammer, and its damper.
//!
//! The damper floats a gap above the string that depends on how far it is
//! lifted. Whenever the string swings wider than the gap it rubs the felt, for
//! the fraction of each cycle it spends beyond the gap. So a partly lifted
//! damper pulls loud notes down until they fit inside the gap, then lets them
//! ring: real half-pedalling.

use crate::hammer::{Hammer, HammerSettings};
use crate::note::NoteDefaults;
use crate::params::{ParamId, ParamValues};
use crate::string::{ModalString, T60_TO_SIGMA};
use std::f64::consts::FRAC_2_PI;

/// Damper felt moves between positions over roughly this long, s.
const DAMPER_SMOOTH: f64 = 0.008;
/// Amplitude follower release, s (longer than the lowest note's period).
const ENVELOPE_RELEASE: f64 = 0.05;
/// Output amplitude a hard strike rings at; the half-pedal gap is relative to this.
const GAP_REF: f64 = 1.0;
/// A voice is silent once its block peak falls this far below its loudest (−80 dB).
const SILENCE_REL: f64 = 1e-4;
/// How long a voice must stay silent before it is freed, s.
const SILENT_TIME: f64 = 0.01;
/// ...and at least this many periods of its fundamental: a low string's output
/// is a train of pulses with near-silent gaps up to a period long.
const SILENT_PERIODS: f64 = 2.0;
/// Slowest hammer, m/s (so velocity 1 still makes a sound).
const MIN_SPEED: f64 = 0.05;

pub struct Voice {
    pub key: Option<u8>,
    nd: NoteDefaults,
    string: ModalString,
    hammer: Hammer,
    sample_rate: f64,
    /// Damper lift: 0 = resting on the string, 1 = fully clear.
    lift_target: f64,
    lift: f64,
    /// Damper decay rate at full contact, 1/s.
    damper_sigma: f64,
    half_pedal_depth_db: f64,
    /// Recent output amplitude.
    envelope: f64,
    envelope_coeff: f64,
    /// Samples of continuous silence so far.
    silent_samples: f64,
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
            lift_target: 1.0,
            lift: 1.0,
            damper_sigma: 0.0,
            half_pedal_depth_db: 42.0,
            envelope: 0.0,
            envelope_coeff: (-1.0 / (ENVELOPE_RELEASE * sample_rate)).exp(),
            silent_samples: 0.0,
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
        self.lift = 1.0;
        self.lift_target = 1.0;
        self.envelope = 0.0;
        self.silent_samples = 0.0;
        self.peak = 0.0;
        self.max_peak = 0.0;
    }

    pub fn configure(&mut self, p: &ParamValues) {
        self.string.configure(&self.nd, p, self.sample_rate);
        self.damper_sigma = T60_TO_SIGMA / p.get(ParamId::DamperRelease) as f64;
        self.half_pedal_depth_db = p.get(ParamId::HalfPedalDepth) as f64;
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
        // Pressing the key lifts its damper before the hammer arrives.
        self.lift = 1.0;
        self.lift_target = 1.0;
        self.silent_samples = 0.0;
        self.max_peak = 0.0;
    }

    /// Damper lift: 0 = resting on the string, 1 = fully clear.
    pub fn set_lift(&mut self, lift: f64) {
        self.lift_target = lift.clamp(0.0, 1.0);
    }

    /// Fraction of full damper contact right now (0..1).
    fn contact(&self) -> f64 {
        if self.lift >= 0.999 {
            return 0.0;
        }
        // Gap grows from nothing (resting) to "everything fits" as the damper lifts.
        let gap = GAP_REF * self.lift * 10f64.powf(-self.half_pedal_depth_db * (1.0 - self.lift) / 20.0);
        if self.envelope <= gap {
            0.0
        } else {
            // Fraction of a sine cycle spent beyond ±gap.
            FRAC_2_PI * (gap / self.envelope).acos()
        }
    }

    /// Add this voice's output into `out`. Frees the voice once it's silent.
    pub fn process(&mut self, out: &mut [f32]) {
        if self.key.is_none() {
            return;
        }
        let dt = 1.0 / self.sample_rate;
        let n = out.len() as f64;
        self.lift += (self.lift_target - self.lift) * (1.0 - (-n * dt / DAMPER_SMOOTH).exp());
        self.string.set_damping(self.contact() * self.damper_sigma);

        let mut peak: f64 = 0.0;
        for o in out.iter_mut() {
            let force = if self.hammer.is_active() {
                let y_free = self.string.predict_strike();
                self.hammer.tick(y_free, self.string.compliance(), dt)
            } else {
                0.0
            };
            let s = self.string.tick(force);
            self.envelope = s.abs().max(self.envelope * self.envelope_coeff);
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
            self.silent_samples += n;
            if self.silent_samples * dt >= SILENT_TIME.max(SILENT_PERIODS / self.nd.f0) {
                self.key = None;
            }
        } else {
            self.silent_samples = 0.0;
        }
    }
}
