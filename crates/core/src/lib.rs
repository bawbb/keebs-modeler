//! keebs-core: the sound engine. No I/O; the app and renderer wrap it.

pub mod engine;
pub mod hammer;
pub mod midi;
pub mod note;
pub mod params;
pub mod string;
pub mod voice;

pub use engine::{Engine, Event};
pub use params::{PARAMS, ParamId, ParamSpec, ParamValues};

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn render(engine: &mut Engine, secs: f32) -> Vec<f32> {
        let mut out = vec![0.0; (secs * SR) as usize];
        engine.render(&mut out);
        out
    }

    fn rms(x: &[f32]) -> f64 {
        (x.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
    }

    /// Crude brightness: RMS of the first difference over RMS of the signal.
    fn brightness(x: &[f32]) -> f64 {
        let d: Vec<f32> = x.windows(2).map(|w| w[1] - w[0]).collect();
        rms(&d) / rms(x)
    }

    fn strike(key: u8, velocity: u8, p: ParamValues) -> Vec<f32> {
        let mut e = Engine::new(SR, 4);
        let mut p = p;
        p.set(ParamId::MasterGain, -24.0); // keep the limiter out of the measurement
        e.set_params(&p);
        e.handle(Event::NoteOn { key, velocity });
        render(&mut e, 0.5)
    }

    #[test]
    fn strike_sounds_and_is_finite() {
        for key in [21, 36, 60, 84, 108] {
            let out = strike(key, 100, ParamValues::default());
            assert!(out.iter().all(|s| s.is_finite()), "key {key} not finite");
            assert!(rms(&out) > 1e-4, "key {key} silent: {}", rms(&out));
        }
    }

    #[test]
    fn harder_is_louder_and_brighter() {
        for key in [36, 60, 84] {
            let soft = strike(key, 20, ParamValues::default());
            let hard = strike(key, 127, ParamValues::default());
            assert!(rms(&hard) > 2.0 * rms(&soft), "key {key} loudness");
            assert!(brightness(&hard) > 1.1 * brightness(&soft), "key {key}: soft {} hard {}",
                brightness(&soft), brightness(&hard));
        }
    }

    #[test]
    fn damper_stops_the_note_and_frees_the_voice() {
        let mut e = Engine::new(SR, 4);
        e.handle(Event::NoteOn { key: 60, velocity: 100 });
        render(&mut e, 0.2);
        e.handle(Event::NoteOff { key: 60 });
        let tail = render(&mut e, 4.0);
        assert!(rms(&tail[tail.len() - 4800..]) < 1e-5);
        assert_eq!(e.active_voices(), 0);
    }

    #[test]
    fn sustain_pedal_keeps_it_ringing() {
        let mut e = Engine::new(SR, 4);
        e.handle(Event::Sustain(1.0));
        e.handle(Event::NoteOn { key: 60, velocity: 100 });
        render(&mut e, 0.2);
        e.handle(Event::NoteOff { key: 60 });
        let tail = render(&mut e, 1.0);
        assert!(rms(&tail[tail.len() - 4800..]) > 1e-3);
    }

    /// Peak level (dB, pre-gain) in a 50 ms window `at` seconds after release.
    fn tail_db(key: u8, velocity: u8, pedal: f32, p: ParamValues, at: f32) -> f64 {
        let mut p = p;
        p.set(ParamId::MasterGain, -36.0);
        let mut e = Engine::new(SR, 2);
        e.set_params(&p);
        e.handle(Event::Sustain(pedal));
        e.handle(Event::NoteOn { key, velocity });
        render(&mut e, 0.2);
        e.handle(Event::NoteOff { key });
        let tail = render(&mut e, at + 0.05);
        let w = &tail[(at * SR) as usize..];
        let pk = w.iter().fold(0f32, |m, s| m.max(s.abs())) as f64 * 10f64.powf(36.0 / 20.0);
        20.0 * pk.max(1e-12).log10()
    }

    #[test]
    fn half_pedal_tames_loud_notes_but_spares_quiet_ones() {
        let p = ParamValues::default();
        let (half, full) = (0.45, 1.0);
        let loud_full = tail_db(48, 120, full, p, 1.0);
        let loud_half = tail_db(48, 120, half, p, 1.0);
        let loud_none = tail_db(48, 120, 0.0, p, 1.0);
        assert!(loud_half < loud_full - 6.0, "loud: half {loud_half:.1} vs full {loud_full:.1}");
        assert!(loud_half > loud_none + 40.0, "half pedal should still sustain: {loud_half:.1}");
        let quiet_full = tail_db(48, 25, full, p, 1.0);
        let quiet_half = tail_db(48, 25, half, p, 1.0);
        assert!((quiet_half - quiet_full).abs() < 1.5, "quiet: half {quiet_half:.1} vs full {quiet_full:.1}");
    }

    #[test]
    fn pedal_below_bite_point_does_nothing() {
        let p = ParamValues::default();
        let bite = p.get(ParamId::PedalBite);
        assert_eq!(engine::pedal_lift(bite * 0.9, &p), 0.0);
        assert!(tail_db(48, 100, bite * 0.9, p, 1.0) < -100.0);
    }

    #[test]
    fn damper_tone_takes_highs_first() {
        let mut dark = ParamValues::default();
        dark.set(ParamId::DamperTone, 1.0);
        dark.set(ParamId::DamperRelease, 1.0);
        let mut even = dark;
        even.set(ParamId::DamperTone, 0.0);
        let tail = |p: ParamValues| {
            let mut e = Engine::new(SR, 2);
            let mut p = p;
            p.set(ParamId::MasterGain, -36.0);
            e.set_params(&p);
            e.handle(Event::NoteOn { key: 48, velocity: 110 });
            render(&mut e, 0.2);
            e.handle(Event::NoteOff { key: 48 });
            render(&mut e, 0.3)[(0.2 * SR) as usize..].to_vec()
        };
        let (b_dark, b_even) = (brightness(&tail(dark)), brightness(&tail(even)));
        assert!(b_dark < 0.8 * b_even, "dark {b_dark:.4} vs even {b_even:.4}");
    }

    #[test]
    fn restrike_keeps_the_string_ringing() {
        let mut e = Engine::new(SR, 4);
        e.handle(Event::NoteOn { key: 60, velocity: 100 });
        render(&mut e, 0.3);
        e.handle(Event::NoteOn { key: 60, velocity: 100 });
        assert_eq!(e.active_voices(), 1);
        let first = render(&mut e, 0.001);
        assert!(rms(&first) > 1e-3, "restrike started from silence");
    }

    #[test]
    fn hard_hammers_on_light_strings_stay_stable() {
        let mut p = ParamValues::default();
        p.set(ParamId::HammerHardness, 20.0);
        p.set(ParamId::MaxHammerSpeed, 12.0);
        p.set(ParamId::MasterGain, -36.0);
        for key in [84, 96, 108, 120] {
            let mut e = Engine::new(SR, 4);
            e.set_params(&p);
            e.handle(Event::NoteOn { key, velocity: 127 });
            let out = render(&mut e, 0.05);
            assert_eq!(e.active_voices(), 1, "key {key} blew up and was reset");
            assert!(out.iter().all(|s| s.abs() < 0.99), "key {key} slammed the limiter");
        }
    }

    #[test]
    fn extreme_settings_stay_finite() {
        let mut p = ParamValues::default();
        for s in &PARAMS {
            p.set(s.id, s.max);
        }
        p.set(ParamId::StrikePos, 0.02);
        for key in [21, 60, 108] {
            let out = strike(key, 127, p);
            assert!(out.iter().all(|s| s.is_finite()), "key {key}");
        }
    }
}
