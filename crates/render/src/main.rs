//! Render struck notes to a WAV file.
//!
//!   render [key=value ...]
//!
//! Options: note=60  vel=100 (or vel=20,60,100,127 for a sweep)  hold=2.0 (s per strike)
//!          gap=0.5 (s after release)  pedal=0..1  sr=48000  out=tmp/out.wav
//! Any engine parameter by key, e.g. strike_pos=0.5 hardness=3 inharm=8. `render help` lists them.

use keebs_core::{Engine, Event, PARAMS, ParamValues, params};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!("render [key=value ...]\n");
    eprintln!("  note=60  vel=100 | vel=20,60,100,127  hold=2.0  gap=0.5  pedal=0..1  sr=48000  out=tmp/out.wav\n");
    eprintln!("Engine parameters:");
    for s in &PARAMS {
        eprintln!("  {:<12} {:>7} .. {:<7} default {:<6} {}  — {}", s.key, s.min, s.max, s.default, s.unit, s.help);
    }
}

fn run() -> Result<(), String> {
    let mut note: u8 = 60;
    let mut vels: Vec<u8> = vec![100];
    let mut hold = 2.0f32;
    let mut gap = 0.5f32;
    let mut pedal = 0.0f32;
    let mut sr = 48_000u32;
    let mut out = String::from("tmp/out.wav");
    let mut p = ParamValues::default();

    for arg in std::env::args().skip(1) {
        if arg == "help" || arg == "--help" || arg == "-h" {
            usage();
            return Ok(());
        }
        let (k, v) = arg.split_once('=').ok_or_else(|| format!("expected key=value, got `{arg}`"))?;
        let num = |v: &str| v.parse::<f32>().map_err(|_| format!("`{k}`: not a number: {v}"));
        match k {
            "note" => note = num(v)? as u8,
            "vel" => {
                vels = v.split(',').map(|x| num(x).map(|n| n.clamp(1.0, 127.0) as u8)).collect::<Result<_, _>>()?
            }
            "hold" => hold = num(v)?,
            "gap" => gap = num(v)?,
            "pedal" => pedal = num(v)?.clamp(0.0, 1.0),
            "sr" => sr = num(v)? as u32,
            "out" => out = v.to_string(),
            _ => {
                let s = params::find(k).ok_or_else(|| format!("unknown option `{k}` (try `render help`)"))?;
                p.set(s.id, num(v)?);
            }
        }
    }

    let mut engine = Engine::new(sr as f32, 8);
    engine.set_params(&p);
    engine.handle(Event::Sustain(pedal));

    let samples = |secs: f32| (secs * sr as f32) as usize;
    let mut audio = Vec::new();
    for &velocity in &vels {
        engine.handle(Event::NoteOn { key: note, velocity });
        let start = audio.len();
        audio.resize(start + samples(hold), 0.0);
        engine.render(&mut audio[start..]);
        if let Some(v) = engine.voice_for_key(note) {
            let h = v.hammer();
            println!("  vel {velocity:>3}: contact {:.2} ms, peak force {:.1} N, compression {:.2} mm, {} modes",
                h.contact_time * 1e3, h.peak_force, h.peak_compression * 1e3, v.mode_count());
        }
        engine.handle(Event::NoteOff { key: note });
        let start = audio.len();
        audio.resize(start + samples(gap), 0.0);
        engine.render(&mut audio[start..]);
    }

    let spec = hound::WavSpec { channels: 1, sample_rate: sr, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    if let Some(dir) = std::path::Path::new(&out).parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let mut w = hound::WavWriter::create(&out, spec).map_err(|e| e.to_string())?;
    for s in &audio {
        w.write_sample(*s).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())?;

    let peak = audio.iter().fold(0f32, |m, s| m.max(s.abs()));
    println!("wrote {out}: note {note}, vel {vels:?}, {:.1}s, peak {peak:.3} ({:.1} dBFS)",
        audio.len() as f32 / sr as f32, 20.0 * peak.log10());
    Ok(())
}
