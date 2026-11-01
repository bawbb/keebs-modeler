//! Per-key physical defaults. Loosely piano-like starting points, not measurements.
//! Hammer values are per string (a real hammer is shared across 2–3 unison strings).
//!
//! Strings are built, not just specified: a steel core of some diameter, plus
//! (on wound strings) copper winding that adds mass but no stiffness. Mass,
//! tension, and inharmonicity all follow from that, so making a string heavier
//! or stiffer changes how the hammer, the decay, and the loudness behave.

use crate::params::{ParamId, ParamValues};
use std::f64::consts::PI;

/// Steel density, kg/m³.
const STEEL_DENSITY: f64 = 7850.0;
/// Steel Young's modulus, Pa.
const STEEL_E: f64 = 2.0e11;

#[derive(Clone, Copy, Debug)]
pub struct NoteDefaults {
    /// Fundamental frequency, Hz (equal temperament, A4 = 440).
    pub f0: f64,
    /// Speaking length, m.
    pub length: f64,
    /// Linear mass density, kg/m.
    pub mu: f64,
    /// Tension, N (derived from f0, length, mu).
    pub tension: f64,
    /// Inharmonicity coefficient B (from the core's stiffness).
    pub b: f64,
    /// T60 of the fundamental, s.
    pub t60: f64,
    /// How wound this string is, 0 = plain steel, 1 = fully wound.
    pub wound: f64,
    /// Effective hammer mass per string, kg.
    pub hammer_mass: f64,
    /// Felt force at 1 mm compression, N.
    pub hammer_fref: f64,
    /// Output trim so a medium strike peaks equally on every key.
    pub level: f64,
    /// Further gain so a medium strike sounds equally loud to the ear (scaled by Key balance).
    pub balance: f64,
}

/// Values at a few keys; everything between is log-interpolated.
struct Anchor {
    key: f64,
    length: f64,
    /// Steel core diameter, mm.
    core_mm: f64,
    /// Winding mass relative to the core's, when the string is wound.
    winding: f64,
    t60: f64,
    hammer_mass: f64,
    hammer_fref: f64,
    /// Linear gain.
    level: f64,
    /// Linear gain on top of `level` for equal perceived loudness (A-weighted; C4 = 1).
    balance: f64,
}

// Cores and windings reproduce the earlier hand-tuned mass and inharmonicity at
// the anchors (bass: core from B, winding from mass; treble: plain steel).
// Treble windings only matter if the wound/plain break is moved up. The first
// plain strings above the break (C3 anchor) get a thick core so they aren't
// lighter and quieter than the wound strings below.
// `level` matches peak level per key; `balance` then matches perceived loudness
// at a medium strike (bass energy sits low, where hearing is weak). Re-measure
// both with the survey example after changing anything else here.
// Treble hammers are much lighter and harder than a real piano's so soft notes
// up top still ring instead of being smothered by a long contact.
const ANCHORS: [Anchor; 7] = [
    Anchor { key: 21.0, length: 1.95, core_mm: 1.586, winding: 3.516, t60: 25.0, hammer_mass: 0.0110, hammer_fref: 100.0, level: 0.437, balance: 2.541 },
    Anchor { key: 36.0, length: 1.60, core_mm: 1.330, winding: 0.652, t60: 18.0, hammer_mass: 0.0049, hammer_fref: 126.0, level: 0.724, balance: 2.042 },
    Anchor { key: 48.0, length: 0.996, core_mm: 1.314, winding: 0.5, t60: 13.42, hammer_mass: 0.00383, hammer_fref: 133.8, level: 0.928, balance: 1.429 },
    Anchor { key: 60.0, length: 0.62, core_mm: 1.011, winding: 0.4, t60: 10.0, hammer_mass: 0.0030, hammer_fref: 142.0, level: 1.189, balance: 1.0 },
    Anchor { key: 84.0, length: 0.18, core_mm: 0.901, winding: 0.4, t60: 3.0, hammer_mass: 0.0007, hammer_fref: 1000.0, level: 1.905, balance: 0.525 },
    Anchor { key: 96.0, length: 0.095, core_mm: 0.854, winding: 0.4, t60: 1.55, hammer_mass: 0.00015, hammer_fref: 5000.0, level: 3.199, balance: 0.733 },
    Anchor { key: 108.0, length: 0.05, core_mm: 0.805, winding: 0.4, t60: 0.8, hammer_mass: 0.00004, hammer_fref: 30000.0, level: 7.762, balance: 0.966 },
];

pub fn key_frequency(key: u8) -> f64 {
    440.0 * 2f64.powf((key as f64 - 69.0) / 12.0)
}

/// Log-linear interpolation between anchors, clamped at the ends.
fn interp(key: f64, pick: impl Fn(&Anchor) -> f64) -> f64 {
    let k = key.clamp(ANCHORS[0].key, ANCHORS[ANCHORS.len() - 1].key);
    for w in ANCHORS.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if k <= b.key {
            let t = (k - a.key) / (b.key - a.key);
            return (pick(a).ln() * (1.0 - t) + pick(b).ln() * t).exp();
        }
    }
    pick(&ANCHORS[ANCHORS.len() - 1])
}

/// How wound `key` is (1 = wound, 0 = plain): half-wound at `brk + 0.5`, spread over `blend` keys.
pub fn woundness(key: f64, brk: f64, blend: f64) -> f64 {
    if blend <= 0.0 {
        return if key <= brk { 1.0 } else { 0.0 };
    }
    let x = ((brk + 0.5 - key) / blend + 0.5).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Mass per length and core diameter of `key`'s string under `p`.
fn construction(key: f64, p: &ParamValues) -> (f64, f64, f64) {
    let core = interp(key, |a| a.core_mm) * 1e-3 * p.get(ParamId::StringGauge) as f64;
    let wound = woundness(key, p.get(ParamId::WoundBreak) as f64, p.get(ParamId::WoundBlend) as f64);
    let winding = interp(key, |a| a.winding) * p.get(ParamId::WindingWeight) as f64;
    let mu_core = STEEL_DENSITY * PI * core * core / 4.0;
    (mu_core * (1.0 + wound * winding), core, wound)
}

impl NoteDefaults {
    /// Defaults for `key` at default settings.
    pub fn for_key(key: u8) -> Self {
        Self::new(key, &ParamValues::default())
    }

    /// Defaults for `key`, with the string built from `p`'s construction settings.
    pub fn new(key: u8, p: &ParamValues) -> Self {
        let k = key as f64;
        let f0 = key_frequency(key);
        let length = interp(k, |a| a.length);
        let (mu, core, wound) = construction(k, p);
        let tension = 4.0 * length * length * f0 * f0 * mu;
        // Heavier strings hand energy to the bridge faster: shorter ring.
        let (mu_default, _, _) = construction(k, &ParamValues::default());
        // Hammer matching resizes the hammer with the string (keeping the touch)
        // and undoes the loudness change (output is roughly proportional to mass).
        let matched = (mu / mu_default).powf(p.get(ParamId::HammerMatching) as f64);
        Self {
            f0,
            length,
            mu,
            tension,
            b: PI.powi(3) * STEEL_E * core.powi(4) / (64.0 * tension * length * length),
            t60: interp(k, |a| a.t60) * mu_default / mu,
            wound,
            hammer_mass: interp(k, |a| a.hammer_mass) * matched,
            hammer_fref: interp(k, |a| a.hammer_fref),
            level: interp(k, |a| a.level) / matched,
            balance: interp(k, |a| a.balance),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tensions_are_plausible_across_the_keyboard() {
        for key in 21..=108 {
            let t = NoteDefaults::for_key(key).tension;
            assert!((300.0..2500.0).contains(&t), "key {key}: tension {t} N");
        }
    }

    #[test]
    fn anchors_keep_their_tuned_mass_and_stiffness() {
        for (key, mu, b) in [(21, 0.070, 2.0e-4), (36, 0.018, 1.5e-4), (60, 0.0063, 4.0e-4), (84, 0.005, 3.0e-3)] {
            let nd = NoteDefaults::for_key(key);
            assert!((nd.mu / mu - 1.0).abs() < 0.02, "key {key}: mu {}", nd.mu);
            assert!((nd.b / b - 1.0).abs() < 0.1, "key {key}: B {}", nd.b);
        }
    }

    #[test]
    fn winding_adds_mass_but_not_stiffness() {
        let mut bare = ParamValues::default();
        bare.set(ParamId::WindingWeight, 0.0);
        let (wound, plain) = (NoteDefaults::for_key(28), NoteDefaults::new(28, &bare));
        assert!(wound.mu > 2.0 * plain.mu);
        assert!(wound.b < 0.5 * plain.b, "same core, more tension: less inharmonic");
        assert!(wound.t60 < plain.t60);
    }

    #[test]
    fn a4_is_440() {
        assert!((key_frequency(69) - 440.0).abs() < 1e-9);
    }
}
