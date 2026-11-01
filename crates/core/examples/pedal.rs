//! How a note decays after release at different pedal positions.
//!   cargo run --release -p keebs-core --example pedal [key=48] [vel=110] [preset=<name>] [param=value ...]
use keebs_core::{Engine, Event, ParamValues, engine::pedal_lift, params};

/// `preset=<name or path>`: a file path, or a name saved by the player under `presets/`.
fn load_preset(arg: &str) -> ParamValues {
    let path = if std::path::Path::new(arg).is_file() { arg.into() } else { format!("{}/../../presets/{arg}.preset", env!("CARGO_MANIFEST_DIR")) };
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("preset `{path}`: {e}"));
    let (p, warnings) = keebs_core::preset::from_text(&text);
    for w in warnings { eprintln!("preset `{path}`: {w}"); }
    p
}

fn main() {
    let sr = 48_000.0;
    let (mut key, mut vel) = (48u8, 110u8);
    let mut p = std::env::args().find_map(|a| a.strip_prefix("preset=").map(load_preset)).unwrap_or_default();
    for arg in std::env::args().skip(1) {
        let (k, v) = arg.split_once('=').expect("key=value");
        match k {
            "preset" => {}
            "key" => key = v.parse().unwrap(),
            "vel" => vel = v.parse().unwrap(),
            _ => p.set(params::find(k).expect("unknown param").id, v.parse().unwrap()),
        }
    }
    p.set(keebs_core::ParamId::MasterGain, -36.0);
    let times = [0.05, 0.25, 0.5, 1.0, 2.0, 4.0];
    print!("key {key} vel {vel}\npedal  lift  |");
    for t in times { print!(" {t:>5}s"); }
    println!("   (dB after release)");
    for pedal in [0.0f32, 0.3, 0.4, 0.5, 0.6, 0.7, 1.0] {
        let mut e = Engine::new(sr, 2);
        e.set_params(&p);
        e.handle(Event::Sustain(pedal));
        e.handle(Event::NoteOn { key, velocity: vel });
        let mut hold = vec![0.0f32; (0.2 * sr) as usize];
        e.render(&mut hold);
        e.handle(Event::NoteOff { key });
        let mut tail = vec![0.0f32; (4.1 * sr) as usize];
        e.render(&mut tail);
        print!("{pedal:>5.2}  {:>4.2}  |", pedal_lift(pedal, &p));
        for t in times {
            let i = (t * sr) as usize;
            let w = &tail[i..i + 2400];
            let pk = w.iter().fold(0f32, |m, s| m.max(s.abs())) as f64 * 10f64.powf(36.0 / 20.0);
            print!(" {:>6.1}", 20.0 * pk.max(1e-9).log10());
        }
        println!();
    }
}
