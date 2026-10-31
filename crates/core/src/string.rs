//! A string as a bank of decaying resonators, one per partial.
//!
//! Displacement is `y(x, t) = Σ q_n(t) · sin(nπx/L)`. Each mode obeys
//! `q'' + 2σ q' + ω² q = (2 / μL) · F · sin(nπ x_strike / L)`, discretised with
//! impulse invariance. What we hear is a weighted sum of the modes (the bridge
//! force, reshaped by the audibility controls).

use crate::note::NoteDefaults;
use crate::params::{ParamId, ParamValues};
use std::f64::consts::{PI, SQRT_2};

pub const MAX_MODES: usize = 1024;
/// Highest partial frequency we bother with, Hz.
const MAX_MODE_HZ: f64 = 20_000.0;
/// High-partial damping at 1.0: σ added per Hz² (≈ T60 of 0.15 s at 5 kHz).
const HF_DAMPING_K: f64 = 1.84e-6;
/// Scales bridge force to roughly ±1 output for a hard strike.
const OUTPUT_NORM: f64 = 0.05;

pub const T60_TO_SIGMA: f64 = 6.907_755_278_982_137; // ln(1000)

/// Frequency of partial `n` (1-based) for a stiff string whose fundamental is `f1`.
pub fn mode_frequency(n: usize, f1: f64, b: f64) -> f64 {
    let n = n as f64;
    n * f1 * ((1.0 + b * n * n) / (1.0 + b)).sqrt()
}

#[derive(Default)]
pub struct ModalString {
    // Per-mode coefficients, undamped.
    a1: Vec<f64>,
    a2: Vec<f64>,
    /// Coefficients with the current damper applied (what `tick` uses).
    a1_eff: Vec<f64>,
    a2_eff: Vec<f64>,
    /// How strongly the damper acts on each mode, relative to the base rate.
    damp_w: Vec<f64>,
    /// Damper decay rate currently folded into `a1_eff`/`a2_eff`, 1/s.
    damper_sigma: f64,
    dt: f64,
    /// Hammer force → mode gain.
    exc: Vec<f64>,
    /// Mode shape at the strike point (for the hammer's view of the string).
    shape: Vec<f64>,
    /// Mode → output gain.
    out_w: Vec<f64>,
    // Per-mode state.
    q1: Vec<f64>,
    q2: Vec<f64>,
    /// String displacement at the strike point after the last tick.
    y_strike: f64,
    /// How far the strike point moves in one sample per newton applied.
    compliance: f64,
}

impl ModalString {
    pub fn new() -> Self {
        let v = || Vec::with_capacity(MAX_MODES);
        Self {
            a1: v(),
            a2: v(),
            a1_eff: v(),
            a2_eff: v(),
            damp_w: v(),
            damper_sigma: 0.0,
            dt: 1.0 / 48_000.0,
            exc: v(),
            shape: v(),
            out_w: v(),
            q1: v(),
            q2: v(),
            y_strike: 0.0,
            compliance: 0.0,
        }
    }

    pub fn mode_count(&self) -> usize {
        self.a1.len()
    }

    /// Recompute coefficients. Ringing state is kept, so this is safe mid-note.
    /// Never allocates beyond `MAX_MODES` capacity.
    pub fn configure(&mut self, nd: &NoteDefaults, p: &ParamValues, sample_rate: f64) {
        let dt = 1.0 / sample_rate;
        self.dt = dt;
        let b = nd.b * p.get(ParamId::Inharmonicity) as f64;
        let damper_tone = p.get(ParamId::DamperTone) as f64;
        let f1 = nd.f0;
        let sigma1 = T60_TO_SIGMA / (nd.t60 * p.get(ParamId::Sustain) as f64);
        let hf = HF_DAMPING_K * p.get(ParamId::HfDamping) as f64;
        let strike = p.get(ParamId::StrikePos) as f64;
        let tilt_db_oct = p.get(ParamId::Tilt) as f64;
        let odd_even = p.get(ParamId::OddEven) as f64;
        let listen_mix = p.get(ParamId::ListenMix) as f64;
        let listen_pos = p.get(ParamId::ListenPos) as f64;
        let force_to_mode = 2.0 / (nd.mu * nd.length);
        let level = nd.level * nd.balance.powf(p.get(ParamId::KeyBalance) as f64);
        let bridge = nd.tension * PI / nd.length * OUTPUT_NORM * level;
        let f_limit = MAX_MODE_HZ.min(0.45 * sample_rate);

        let mut count = 0;
        while count < MAX_MODES && mode_frequency(count + 1, f1, b) < f_limit {
            count += 1;
        }
        for v in [
            &mut self.a1,
            &mut self.a2,
            &mut self.a1_eff,
            &mut self.a2_eff,
            &mut self.damp_w,
            &mut self.exc,
            &mut self.shape,
            &mut self.out_w,
        ] {
            v.resize(count, 0.0);
        }
        // New modes start silent; dropped modes are simply discarded.
        self.q1.resize(count, 0.0);
        self.q2.resize(count, 0.0);

        for i in 0..count {
            let n = i + 1;
            let nf = n as f64;
            let f = mode_frequency(n, f1, b);
            let omega = 2.0 * PI * f;
            let sigma = sigma1 + hf * (f * f - f1 * f1);
            let omega_d = (omega * omega - sigma * sigma).max(1.0).sqrt();
            let r = (-sigma * dt).exp();
            self.a1[i] = 2.0 * r * (omega_d * dt).cos();
            self.a2[i] = r * r;
            // Tone 0: every partial damped alike. Tone 1: the fundamental lingers
            // ~4x longer and high partials go much faster.
            self.damp_w[i] = (1.0 - 0.75 * damper_tone) * (f / f1).powf(damper_tone);
            let shape = (nf * PI * strike).sin();
            self.shape[i] = shape;
            self.exc[i] = dt * r * (omega_d * dt).sin() / omega_d * force_to_mode * shape;

            let tilt = 10f64.powf(tilt_db_oct * nf.log2() / 20.0);
            let parity = if n % 2 == 1 {
                1.0 - (-odd_even).max(0.0)
            } else {
                1.0 - odd_even.max(0.0)
            };
            let listen = (1.0 - listen_mix) + listen_mix * SQRT_2 * (nf * PI * listen_pos).sin();
            self.out_w[i] = bridge * nf * tilt * parity * listen;
        }
        self.compliance = self.exc.iter().zip(&self.shape).map(|(e, s)| e * s).sum();
        let sigma = self.damper_sigma;
        self.damper_sigma = f64::NAN; // force a re-apply
        self.set_damping(sigma);
    }

    /// Apply a damper decay rate (1/s, scaled per mode by the damper tone).
    /// Cheap to call once per block; no-op if unchanged.
    pub fn set_damping(&mut self, sigma: f64) {
        if sigma == self.damper_sigma {
            return;
        }
        self.damper_sigma = sigma;
        if sigma <= 0.0 {
            self.a1_eff.copy_from_slice(&self.a1);
            self.a2_eff.copy_from_slice(&self.a2);
            return;
        }
        for i in 0..self.a1.len() {
            let d = (-sigma * self.damp_w[i] * self.dt).exp();
            self.a1_eff[i] = self.a1[i] * d;
            self.a2_eff[i] = self.a2[i] * d * d;
        }
    }

    pub fn reset(&mut self) {
        self.q1.iter_mut().for_each(|q| *q = 0.0);
        self.q2.iter_mut().for_each(|q| *q = 0.0);
        self.y_strike = 0.0;
    }

    pub fn y_strike(&self) -> f64 {
        self.y_strike
    }

    pub fn compliance(&self) -> f64 {
        self.compliance
    }

    /// Where the strike point would be next sample with no hammer force.
    #[inline]
    pub fn predict_strike(&self) -> f64 {
        let n = self.a1.len();
        let (a1, a2, shape) = (&self.a1_eff[..n], &self.a2_eff[..n], &self.shape[..n]);
        let (q1, q2) = (&self.q1[..n], &self.q2[..n]);
        (0..n).map(|i| shape[i] * (a1[i] * q1[i] - a2[i] * q2[i])).sum()
    }

    /// Advance one sample with hammer force `force` (N). Returns the output sample.
    #[inline]
    pub fn tick(&mut self, force: f64) -> f64 {
        let mut y = 0.0;
        let mut out = 0.0;
        let n = self.a1.len();
        let (a1, a2, exc, shape, w) =
            (&self.a1_eff[..n], &self.a2_eff[..n], &self.exc[..n], &self.shape[..n], &self.out_w[..n]);
        let (q1, q2) = (&mut self.q1[..n], &mut self.q2[..n]);
        for i in 0..n {
            let q = a1[i] * q1[i] - a2[i] * q2[i] + exc[i] * force;
            q2[i] = q1[i];
            q1[i] = q;
            y += shape[i] * q;
            out += w[i] * q;
        }
        self.y_strike = y;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fundamental_stays_put_and_partials_stretch() {
        let f1 = 100.0;
        assert!((mode_frequency(1, f1, 1e-3) - f1).abs() < 1e-9);
        assert!((mode_frequency(10, f1, 0.0) - 1000.0).abs() < 1e-9);
        assert!(mode_frequency(10, f1, 1e-3) > 1000.0);
    }

    #[test]
    fn striking_at_half_silences_even_partials() {
        let nd = NoteDefaults::for_key(48);
        let mut p = ParamValues::default();
        p.set(ParamId::StrikePos, 0.5);
        let mut s = ModalString::new();
        s.configure(&nd, &p, 48_000.0);
        for (i, e) in s.exc.iter().enumerate() {
            if (i + 1) % 2 == 0 {
                assert!(e.abs() < 1e-12 * s.exc[0].abs(), "partial {} excited", i + 1);
            }
        }
    }
}
