//! keebs: play the modelled string from a MIDI keyboard.
//!
//!   keebs [--midi <name>] [--buffer <frames>] [--list]

mod audio;
mod midi;
mod shared;
mod ui;

use keebs_core::ParamValues;
use shared::{SharedParams, Stats};
use std::sync::{Arc, Mutex};

fn main() -> Result<(), String> {
    let mut midi_wanted = String::new();
    let mut buffer = 128u32;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--midi" => midi_wanted = args.next().ok_or("--midi needs a name")?,
            "--buffer" => buffer = args.next().and_then(|b| b.parse().ok()).ok_or("--buffer needs a number")?,
            "--list" => {
                println!("MIDI inputs:");
                for p in midi::port_names() {
                    println!("  {p}");
                }
                println!("Audio outputs:");
                let default = audio::default_device_name();
                for d in audio::output_device_names() {
                    let mark = if Some(&d) == default.as_ref() { "  (system default)" } else { "" };
                    println!("  {d}{mark}");
                }
                return Ok(());
            }
            _ => return Err(format!("unknown argument `{a}`\nusage: keebs [--midi <name>] [--buffer <frames>] [--list]")),
        }
    }

    let params = Arc::new(SharedParams::new(&ParamValues::default()));
    let stats = Arc::new(Stats::default());
    let (audio, tx) = audio::start(None, params.clone(), stats.clone(), buffer)?;
    let tx = Arc::new(Mutex::new(tx));

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([980.0, 700.0]).with_min_inner_size([720.0, 400.0]).with_title("keebs"),
        ..Default::default()
    };
    eframe::run_native(
        "keebs",
        options,
        Box::new(move |_| Ok(Box::new(ui::App::new(params, stats, tx, audio, buffer, &midi_wanted)))),
    )
    .map_err(|e| e.to_string())
}
