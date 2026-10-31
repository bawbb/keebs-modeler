//! State shared between the UI, MIDI, and audio threads, all lock-free on the audio side.

use keebs_core::params::PARAM_COUNT;
use keebs_core::{Event, ParamValues};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};

/// Parameters written by the UI, read by the audio thread when `version` changes.
pub struct SharedParams {
    values: [AtomicU32; PARAM_COUNT],
    version: AtomicU64,
}

impl SharedParams {
    pub fn new(p: &ParamValues) -> Self {
        Self {
            values: std::array::from_fn(|i| AtomicU32::new(p.0[i].to_bits())),
            version: AtomicU64::new(0),
        }
    }

    pub fn load(&self) -> ParamValues {
        ParamValues(std::array::from_fn(|i| f32::from_bits(self.values[i].load(Relaxed))))
    }

    pub fn store(&self, p: &ParamValues) {
        for (a, v) in self.values.iter().zip(p.0.iter()) {
            a.store(v.to_bits(), Relaxed);
        }
        self.version.fetch_add(1, Relaxed);
    }

    pub fn version(&self) -> u64 {
        self.version.load(Relaxed)
    }
}

/// Numbers the audio thread reports back for display.
#[derive(Default)]
pub struct Stats {
    pub voices: AtomicU32,
    /// Output peak since the UI last read it (f32 bits).
    pub peak: AtomicU32,
    /// Fraction of the buffer period spent rendering (f32 bits).
    pub load: AtomicU32,
    pub last_velocity: AtomicU32,
    pub sustain: AtomicU32,
    /// Set by the audio error callback when the system output device changed or vanished.
    pub device_changed: AtomicBool,
    pub device_lost: AtomicBool,
}

impl Stats {
    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Relaxed))
    }

    pub fn load(&self) -> f32 {
        f32::from_bits(self.load.load(Relaxed))
    }
}

/// Events from MIDI and the UI into the audio thread. Producers lock (never the audio thread).
pub type EventTx = Arc<Mutex<rtrb::Producer<Event>>>;

pub fn send(tx: &EventTx, ev: Event) {
    if let Ok(mut p) = tx.lock() {
        let _ = p.push(ev);
    }
}
