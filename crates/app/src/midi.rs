//! MIDI input: raw bytes → engine events.

use crate::shared::{EventTx, send};
use midir::{MidiInput, MidiInputConnection};

pub fn port_names() -> Vec<String> {
    let Ok(input) = MidiInput::new("keebs-list") else { return vec![] };
    input.ports().iter().filter_map(|p| input.port_name(p).ok()).collect()
}

/// Connect to every port whose name contains `wanted` (case-insensitive);
/// an empty `wanted` means all ports.
pub fn connect(wanted: &str, tx: EventTx) -> Result<(Vec<MidiInputConnection<()>>, String), String> {
    let wanted_lc = wanted.to_lowercase();
    let names: Vec<String> = port_names()
        .into_iter()
        .filter(|n| wanted_lc.is_empty() || n.to_lowercase().contains(&wanted_lc))
        .collect();
    if names.is_empty() {
        return Err(if wanted.is_empty() { "no MIDI inputs found".into() } else { format!("no MIDI input matching `{wanted}`") });
    }
    let mut conns = Vec::new();
    let mut connected = Vec::new();
    for name in &names {
        match connect_one(name, tx.clone()) {
            Ok(c) => {
                conns.push(c);
                connected.push(name.clone());
            }
            Err(e) => eprintln!("MIDI: couldn't open {name}: {e}"),
        }
    }
    if conns.is_empty() {
        return Err("couldn't open any MIDI input".into());
    }
    let label = if wanted.is_empty() { format!("all ({})", connected.join(", ")) } else { connected.join(", ") };
    Ok((conns, label))
}

/// Open the port named exactly `name`.
fn connect_one(name: &str, tx: EventTx) -> Result<MidiInputConnection<()>, String> {
    let mut input = MidiInput::new("keebs").map_err(|e| e.to_string())?;
    input.ignore(midir::Ignore::All);
    let port = input
        .ports()
        .into_iter()
        .find(|p| input.port_name(p).ok().as_deref() == Some(name))
        .ok_or_else(|| format!("{name} disappeared"))?;
    input
        .connect(&port, "keebs-in", move |_, bytes, _| {
            if let Some(ev) = keebs_core::midi::parse(bytes) {
                send(&tx, ev);
            }
        }, ())
        .map_err(|e| e.to_string())
}
