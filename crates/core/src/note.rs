//! Per-key physical defaults. Loosely piano-like starting points, not measurements.
//! Hammer values are per string (a real hammer is shared across 2–3 unison strings).

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
    /// Inharmonicity coefficient B.
    pub b: f64,
    /// T60 of the fundamental, s.
    pub t60: f64,
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
    mu: f64,
    b: f64,
    t60: f64,
    hammer_mass: f64,
    hammer_fref: f64,
    /// Linear gain.
    level: f64,
    /// Linear gain on top of `level` for equal perceived loudness (A-weighted; C4 = 1).
    balance: f64,
}

// `level` matches peak level per key; `balance` then matches perceived loudness
// at a medium strike (bass energy sits low, where hearing is weak). Re-measure
// both with the survey example after changing anything else here.
// Treble hammers are much lighter and harder than a real piano's so soft notes
// up top still ring instead of being smothered by a long contact.
const ANCHORS: [Anchor; 6] = [
    Anchor { key: 21.0, length: 1.95, mu: 0.070, b: 2.0e-4, t60: 25.0, hammer_mass: 0.0110, hammer_fref: 100.0, level: 0.437, balance: 2.541 },
    Anchor { key: 36.0, length: 1.60, mu: 0.018, b: 1.5e-4, t60: 18.0, hammer_mass: 0.0049, hammer_fref: 126.0, level: 0.724, balance: 2.042 },
    Anchor { key: 60.0, length: 0.62, mu: 0.0063, b: 4.0e-4, t60: 10.0, hammer_mass: 0.0030, hammer_fref: 142.0, level: 1.189, balance: 1.0 },
    Anchor { key: 84.0, length: 0.18, mu: 0.0050, b: 3.0e-3, t60: 3.0, hammer_mass: 0.0007, hammer_fref: 1000.0, level: 1.905, balance: 0.525 },
    Anchor { key: 96.0, length: 0.095, mu: 0.0045, b: 7.7e-3, t60: 1.55, hammer_mass: 0.00015, hammer_fref: 5000.0, level: 3.199, balance: 0.733 },
    Anchor { key: 108.0, length: 0.05, mu: 0.0040, b: 2.0e-2, t60: 0.8, hammer_mass: 0.00004, hammer_fref: 30000.0, level: 7.762, balance: 0.966 },
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

impl NoteDefaults {
    pub fn for_key(key: u8) -> Self {
        let k = key as f64;
        let f0 = key_frequency(key);
        let length = interp(k, |a| a.length);
        let mu = interp(k, |a| a.mu);
        Self {
            f0,
            length,
            mu,
            tension: 4.0 * length * length * f0 * f0 * mu,
            b: interp(k, |a| a.b),
            t60: interp(k, |a| a.t60),
            hammer_mass: interp(k, |a| a.hammer_mass),
            hammer_fref: interp(k, |a| a.hammer_fref),
            level: interp(k, |a| a.level),
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
    fn a4_is_440() {
        assert!((key_frequency(69) - 440.0).abs() < 1e-9);
    }
}
