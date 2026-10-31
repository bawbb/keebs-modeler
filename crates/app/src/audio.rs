//! Audio output: runs the engine inside the device callback.

use crate::shared::{SharedParams, Stats};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use keebs_core::{Engine, Event};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

const MAX_VOICES: usize = 32;

/// Room for events between the UI/MIDI threads and the audio thread.
const EVENT_QUEUE: usize = 1024;

pub struct Audio {
    _stream: cpal::Stream,
    /// Name of the device actually playing.
    pub device: String,
    /// `None` = auto (follows the system default output).
    pub choice: Option<String>,
    pub sample_rate: u32,
    pub buffer: String,
}

fn device_name(d: &cpal::Device) -> Option<String> {
    d.description().ok().map(|d| d.name().to_string())
}

/// Name of the current system default output device.
pub fn default_device_name() -> Option<String> {
    device_name(&cpal::default_host().default_output_device()?)
}

/// Names of every output device.
pub fn output_device_names() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|ds| ds.filter_map(|d| device_name(&d)).collect())
        .unwrap_or_default()
}

/// Open an output stream running a fresh engine. `choice` names a device, or
/// `None` for auto (the system default, followed as it changes). Returns the
/// producer side of the stream's event queue.
pub fn start(
    choice: Option<&str>,
    params: Arc<SharedParams>,
    stats: Arc<Stats>,
    buffer_frames: u32,
) -> Result<(Audio, rtrb::Producer<Event>), String> {
    let host = cpal::default_host();
    let device = match choice {
        None => host.default_output_device().ok_or("no audio output device")?,
        Some(name) => host
            .output_devices()
            .map_err(|e| e.to_string())?
            .find(|d| device_name(d).as_deref() == Some(name))
            .ok_or_else(|| format!("output device `{name}` not found"))?,
    };
    let name = device_name(&device).unwrap_or_else(|| "unknown".into());
    let (tx, rx) = rtrb::RingBuffer::new(EVENT_QUEUE);
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    let format = supported.sample_format();
    let mut config: cpal::StreamConfig = supported.config();
    let buffer = match supported.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } if (*min..=*max).contains(&buffer_frames) => {
            config.buffer_size = cpal::BufferSize::Fixed(buffer_frames);
            format!("{buffer_frames} frames")
        }
        _ => "device default".into(),
    };

    use cpal::SampleFormat as F;
    let stream = match format {
        F::F32 => build::<f32>(&device, &config, params, stats.clone(), rx),
        F::F64 => build::<f64>(&device, &config, params, stats.clone(), rx),
        F::I16 => build::<i16>(&device, &config, params, stats.clone(), rx),
        F::I32 => build::<i32>(&device, &config, params, stats.clone(), rx),
        F::U16 => build::<u16>(&device, &config, params, stats.clone(), rx),
        other => return Err(format!("unsupported sample format {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    stats.device_lost.store(false, Relaxed);
    stats.device_changed.store(false, Relaxed);
    let audio = Audio {
        _stream: stream,
        device: name,
        choice: choice.map(str::to_string),
        sample_rate: config.sample_rate,
        buffer,
    };
    Ok((audio, tx))
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    params: Arc<SharedParams>,
    stats: Arc<Stats>,
    mut rx: rtrb::Consumer<Event>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let sr = config.sample_rate as f32;
    let mut engine = Engine::new(sr, MAX_VOICES);
    engine.set_params(&params.load());
    let mut seen_version = params.version();
    let mut mono = vec![0.0f32; 4096];
    let err_stats = stats.clone();

    device
        .build_output_stream(
            *config,
            move |data: &mut [T], _| {
                let started = Instant::now();
                while let Ok(ev) = rx.pop() {
                    match ev {
                        Event::NoteOn { velocity, .. } if velocity > 0 => {
                            stats.last_velocity.store(velocity as u32, Relaxed)
                        }
                        Event::Sustain(s) => stats.sustain.store(s.to_bits(), Relaxed),
                        _ => {}
                    }
                    engine.handle(ev);
                }
                let v = params.version();
                if v != seen_version {
                    seen_version = v;
                    engine.set_params(&params.load());
                }

                let frames = data.len() / channels;
                let mut peak = 0f32;
                let mut done = 0;
                while done < frames {
                    let n = (frames - done).min(mono.len());
                    let buf = &mut mono[..n];
                    engine.render(buf);
                    for (i, s) in buf.iter().enumerate() {
                        peak = peak.max(s.abs());
                        let frame = &mut data[(done + i) * channels..(done + i + 1) * channels];
                        frame.fill(T::from_sample(*s));
                    }
                    done += n;
                }

                let period = frames as f32 / sr;
                let load = started.elapsed().as_secs_f32() / period.max(1e-6);
                stats.load.store(load.to_bits(), Relaxed);
                stats.voices.store(engine.active_voices() as u32, Relaxed);
                let prev = f32::from_bits(stats.peak.load(Relaxed));
                if peak > prev {
                    stats.peak.store(peak.to_bits(), Relaxed);
                }
            },
            // A stream on the default device follows the system output by itself;
            // we only need to tell the UI so it can update the device name.
            move |e| match e.kind() {
                cpal::ErrorKind::DeviceChanged => {
                    err_stats.device_lost.store(false, Relaxed);
                    err_stats.device_changed.store(true, Relaxed);
                }
                cpal::ErrorKind::DeviceNotAvailable => err_stats.device_lost.store(true, Relaxed),
                _ => eprintln!("audio error: {e}"),
            },
            None,
        )
        .map_err(|e| e.to_string())
}
