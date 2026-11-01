//! Survey loudness, brightness, and hammer contact across keys and velocities.
//!   cargo run --release -p keebs-core --example survey [preset=<name>] [param=value ...]
use keebs_core::{Engine, Event, ParamId, ParamValues, note::NoteDefaults, params, string::mode_frequency};
use std::f64::consts::PI;

const SR: f64 = 48_000.0;

/// Power at frequency f over a window (single DFT bin, Hann-windowed).
fn power(x: &[f32], f: f64) -> f64 {
    let n = x.len() as f64;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, s) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * PI * i as f64 / n).cos();
        let ph = 2.0 * PI * f * i as f64 / SR;
        re += *s as f64 * w * ph.cos();
        im += *s as f64 * w * ph.sin();
    }
    re * re + im * im
}

/// A-weighting gain (linear power) at f: roughly how loud the ear finds it at moderate levels.
fn a_weight(f: f64) -> f64 {
    let f2 = f * f;
    let ra = 12194f64.powi(2) * f2 * f2
        / ((f2 + 20.6f64.powi(2)) * ((f2 + 107.7f64.powi(2)) * (f2 + 737.9f64.powi(2))).sqrt() * (f2 + 12194f64.powi(2)));
    (ra * 1.2589).powi(2) // +2 dB so 1 kHz is 0 dB
}

/// `preset=<name or path>`: a file path, or a name saved by the player under `presets/`.
fn load_preset(arg: &str) -> ParamValues {
    let path = if std::path::Path::new(arg).is_file() { arg.into() } else { format!("{}/../../presets/{arg}.preset", env!("CARGO_MANIFEST_DIR")) };
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("preset `{path}`: {e}"));
    let (p, warnings) = keebs_core::preset::from_text(&text);
    for w in warnings { eprintln!("preset `{path}`: {w}"); }
    p
}

fn main() {
    let mut p = std::env::args().find_map(|a| a.strip_prefix("preset=").map(load_preset)).unwrap_or_default();
    p.set(ParamId::MasterGain, -36.0);
    for arg in std::env::args().skip(1) {
        let (k, v) = arg.split_once('=').expect("key=value");
        if k == "preset" { continue; }
        p.set(params::find(k).expect("unknown param").id, v.parse().expect("number"));
    }
    let gain = 10f64.powf(-p.get(ParamId::MasterGain) as f64 / 20.0);
    let keys: Vec<u8> = std::env::var("KEYS").ok()
        .map(|k| k.split(',').map(|x| x.parse().unwrap()).collect())
        .unwrap_or(vec![21, 28, 36, 48, 60, 72, 84, 90, 96, 102, 108]);
    println!("key  vel  peak_dB  rms_dB  heard_dB  brightness(mean partial #)  contact_ms");
    for key in keys {
        for vel in [20u8, 64, 127] {
            let mut e = Engine::new(SR as f32, 2);
            e.set_params(&p);
            e.handle(Event::NoteOn { key, velocity: vel });
            let mut out = vec![0.0f32; (0.15 * SR) as usize];
            e.render(&mut out);
            for s in out.iter_mut() { *s = ((*s as f64).atanh() * gain) as f32; }
            // Measure the free ring after the hammer has left (5 ms on).
            let ring = &out[(0.005 * SR) as usize..];
            let peak = ring.iter().fold(0f64, |m, s| m.max(s.abs() as f64));
            let rms = (ring.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / ring.len() as f64).sqrt();
            let nd = NoteDefaults::new(key, &p);
            let b = nd.b * p.get(ParamId::Inharmonicity) as f64;
            // Brightness: power-weighted mean partial number over the first 40 partials.
            let pw: Vec<(f64, f64)> = (1..=40).map(|n| (n as f64, mode_frequency(n, nd.f0, b)))
                .filter(|(_, f)| *f < 20_000.0).map(|(n, f)| (n, power(ring, f))).collect();
            let centroid = pw.iter().map(|(n, p)| n * p).sum::<f64>() / pw.iter().map(|(_, p)| p).sum::<f64>();
            // Perceived level: A-weighted sum of every partial's power (relative; compare across keys).
            let heard: f64 = (1..).map(|n| mode_frequency(n, nd.f0, b)).take_while(|f| *f < 16_000.0).take(300)
                .map(|f| power(ring, f) * a_weight(f)).sum();
            let heard_db = 10.0 * (heard / (ring.len() as f64).powi(2)).log10();
            let contact = e.voice_for_key(key).map(|v| v.hammer().contact_time * 1e3).unwrap_or(f64::NAN);
            println!("{key:>3}  {vel:>3}  {:7.1}  {:6.1}  {:8.1}  {:7.2}  {:6.2}", 20.0 * peak.log10(), 20.0 * rms.log10(), heard_db, centroid, contact);
        }
    }
}
