//! Presets as plain text: one `key = value` line per parameter, using the same
//! keys as the renderer's command line. `#` starts a comment.
//!
//! Reading is forgiving so presets survive the parameter table changing:
//! unknown keys are reported and skipped, missing keys keep their defaults,
//! and out-of-range values are clamped.

use crate::params::{self, PARAMS, ParamValues};

/// The whole parameter set as preset text.
pub fn to_text(p: &ParamValues) -> String {
    let mut out = String::from("# keebs preset\n");
    let mut group = "";
    for s in &PARAMS {
        if s.group != group {
            group = s.group;
            out.push_str(&format!("\n# {group}\n"));
        }
        out.push_str(&format!("{} = {}\n", s.key, p.get(s.id)));
    }
    out
}

/// Parse preset text over the defaults. Returns the values and a warning for
/// each line that was skipped or changed.
pub fn from_text(text: &str) -> (ParamValues, Vec<String>) {
    let mut p = ParamValues::default();
    let mut warnings = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let n = n + 1;
        let Some((k, v)) = line.split_once('=') else {
            warnings.push(format!("line {n}: expected `key = value`"));
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        let Some(s) = params::find(k) else {
            warnings.push(format!("line {n}: unknown parameter `{k}` (skipped)"));
            continue;
        };
        let Ok(x) = v.parse::<f32>() else {
            warnings.push(format!("line {n}: `{k}`: not a number: {v}"));
            continue;
        };
        if !x.is_finite() {
            warnings.push(format!("line {n}: `{k}`: not a finite number"));
            continue;
        }
        p.set(s.id, x);
        if p.get(s.id) != x {
            warnings.push(format!("line {n}: `{k}` = {x} out of range, clamped to {}", p.get(s.id)));
        }
    }
    (p, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParamId;

    #[test]
    fn round_trips_exactly() {
        let mut p = ParamValues::default();
        p.set(ParamId::StrikePos, 0.1234567);
        p.set(ParamId::HammerHardness, 7.0 / 3.0);
        p.set(ParamId::MasterGain, -12.5);
        let (q, warnings) = from_text(&to_text(&p));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(p, q);
    }

    #[test]
    fn tolerates_unknown_missing_and_out_of_range() {
        let (p, warnings) = from_text("# old preset\nhardness = 3\nno_such_param = 1\ngain = 99  # too loud\n");
        assert_eq!(p.get(ParamId::HammerHardness), 3.0);
        assert_eq!(p.get(ParamId::MasterGain), params::spec(ParamId::MasterGain).max);
        assert_eq!(p.get(ParamId::Sustain), params::spec(ParamId::Sustain).default);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }
}
