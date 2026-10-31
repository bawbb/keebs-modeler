//! Audio output: runs the engine inside the device callback.

use crate::shared::{SharedParams, Stats};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use keebs_core::{Engine, Event};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

const MAX_VOICES: usize = 32;

pub struct Audio {
    _stream: cpal::Stream,
    pub device: String,
    pub sample_rate: u32,
    pub buffer: String,
}

pub fn start(
    params: Arc<SharedParams>,
    stats: Arc<Stats>,
    rx: rtrb::Consumer<Event>,
    buffer_frames: u32,
) -> Result<Audio, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no audio output device")?;
    let name = device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| "unknown".into());
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
        F::F32 => build::<f32>(&device, &config, params, stats, rx),
        F::F64 => build::<f64>(&device, &config, params, stats, rx),
        F::I16 => build::<i16>(&device, &config, params, stats, rx),
        F::I32 => build::<i32>(&device, &config, params, stats, rx),
        F::U16 => build::<u16>(&device, &config, params, stats, rx),
        other => return Err(format!("unsupported sample format {other:?}")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(Audio { _stream: stream, device: name, sample_rate: config.sample_rate, buffer })
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
            |e| eprintln!("audio error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}
