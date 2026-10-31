//! How much CPU a dense, sustained chord costs.
use keebs_core::{Engine, Event};
use std::time::Instant;
fn main() {
    let sr = 48_000.0;
    for (label, keys) in [("10 bass notes", (21u8..41).step_by(2).collect::<Vec<_>>()), ("10 mid notes", (55u8..75).step_by(2).collect()), ("32 notes spread", (21u8..108).step_by(3).take(32).collect())] {
        let mut e = Engine::new(sr, 32);
        e.handle(Event::Sustain(1.0));
        for &k in &keys { e.handle(Event::NoteOn { key: k, velocity: 100 }); }
        let mut buf = vec![0.0f32; 128];
        let blocks = (sr as usize) / 128; // 1 s of audio
        let t = Instant::now();
        for _ in 0..blocks { e.render(&mut buf); }
        let secs = t.elapsed().as_secs_f64();
        println!("{label:<16} {:>2} voices: {:.1}% of one core", e.active_voices(), secs * 100.0);
    }
}
