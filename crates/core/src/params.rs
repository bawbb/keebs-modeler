//! Player-facing parameters. One table drives the UI, the renderer, and the engine.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum ParamId {
    StrikePos,
    HammerHardness,
    FeltExponent,
    FeltHysteresis,
    HammerMass,
    MaxHammerSpeed,
    VelocityCurve,
    Inharmonicity,
    Sustain,
    HfDamping,
    Tilt,
    OddEven,
    ListenMix,
    ListenPos,
    DamperRelease,
    MasterGain,
}

pub const PARAM_COUNT: usize = 16;

#[derive(Clone, Copy, Debug)]
pub struct ParamSpec {
    pub id: ParamId,
    /// Stable machine name, used on the command line.
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    /// Display on a logarithmic slider.
    pub log: bool,
    pub help: &'static str,
}

#[allow(clippy::too_many_arguments)]
const fn p(
    id: ParamId,
    key: &'static str,
    label: &'static str,
    group: &'static str,
    min: f32,
    max: f32,
    default: f32,
    unit: &'static str,
    log: bool,
    help: &'static str,
) -> ParamSpec {
    ParamSpec { id, key, label, group, min, max, default, unit, log, help }
}

use ParamId::*;

pub const PARAMS: [ParamSpec; PARAM_COUNT] = [
    p(StrikePos, "strike_pos", "Strike position", "Strike", 0.02, 0.5, 0.12, "of length", false,
      "Where the hammer hits. Partials whose number times this is a whole number are silenced (1/2 kills evens)."),
    p(HammerHardness, "hardness", "Felt hardness", "Hammer", 0.05, 20.0, 1.0, "x", true,
      "Felt stiffness relative to the key's default. Harder = brighter, shorter contact."),
    p(FeltExponent, "felt_exp", "Felt nonlinearity", "Hammer", 1.0, 5.0, 3.5, "", false,
      "How much the felt stiffens as it compresses. Higher = bigger timbre gap between soft and hard playing."),
    p(FeltHysteresis, "hysteresis", "Felt hysteresis", "Hammer", 0.0, 1.0, 0.2, "", false,
      "Energy lost in the felt during contact. Softens and rounds the attack."),
    p(HammerMass, "hammer_mass", "Hammer mass", "Hammer", 0.1, 10.0, 1.0, "x", true,
      "Hammer mass relative to the key's default. Heavier = longer contact, darker, fuller."),
    p(MaxHammerSpeed, "max_speed", "Max hammer speed", "Touch", 0.5, 12.0, 4.0, "m/s", true,
      "Hammer speed at full velocity. Sets how far toward 'hard' the top of the keyboard's range reaches."),
    p(VelocityCurve, "vel_curve", "Velocity curve", "Touch", 0.25, 4.0, 1.4, "exp", true,
      "Maps key velocity to hammer speed. >1 gives more room at the soft end."),
    p(Inharmonicity, "inharm", "Inharmonicity", "String", 0.0, 20.0, 1.0, "x", false,
      "String stiffness relative to the key's default. Stretches overtones sharp; 0 = perfectly harmonic."),
    p(Sustain, "sustain", "Sustain", "String", 0.05, 5.0, 1.0, "x", true,
      "How long the fundamental rings, relative to the key's default."),
    p(HfDamping, "hf_damp", "High-partial damping", "String", 0.0, 1.0, 0.25, "", false,
      "How much faster high partials die than low ones. 0 = all ring equally."),
    p(Tilt, "tilt", "Partial tilt", "Audibility", -12.0, 12.0, 0.0, "dB/oct", false,
      "Boost or cut higher partials in what you hear, without changing how the string moves."),
    p(OddEven, "odd_even", "Odd / even", "Audibility", -1.0, 1.0, 0.0, "", false,
      "-1 = even partials only, +1 = odd partials only."),
    p(ListenMix, "listen_mix", "Listen-point mix", "Audibility", 0.0, 1.0, 0.0, "", false,
      "Blend from hearing the bridge (0) to hearing a single point on the string (1)."),
    p(ListenPos, "listen_pos", "Listen point", "Audibility", 0.01, 0.5, 0.3, "of length", false,
      "Where on the string the listen point sits. Adds its own pattern of silenced partials."),
    p(DamperRelease, "damper", "Damper release", "Damper", 0.03, 3.0, 0.25, "s", true,
      "How long a note takes to die after the key is released (no pedal)."),
    p(MasterGain, "gain", "Master gain", "Output", -36.0, 12.0, -6.0, "dB", false,
      "Output level before the soft limiter."),
];

pub fn spec(id: ParamId) -> &'static ParamSpec {
    &PARAMS[id as usize]
}

pub fn find(key: &str) -> Option<&'static ParamSpec> {
    PARAMS.iter().find(|s| s.key == key)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamValues(pub [f32; PARAM_COUNT]);

impl Default for ParamValues {
    fn default() -> Self {
        let mut v = [0.0; PARAM_COUNT];
        for s in &PARAMS {
            v[s.id as usize] = s.default;
        }
        Self(v)
    }
}

impl ParamValues {
    pub fn get(&self, id: ParamId) -> f32 {
        self.0[id as usize]
    }

    pub fn set(&mut self, id: ParamId, value: f32) {
        let s = spec(id);
        self.0[id as usize] = value.clamp(s.min, s.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_indexed_by_id_and_defaults_in_range() {
        for (i, s) in PARAMS.iter().enumerate() {
            assert_eq!(s.id as usize, i, "{} out of order", s.key);
            assert!(s.min <= s.default && s.default <= s.max, "{} default out of range", s.key);
        }
    }
}
