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
