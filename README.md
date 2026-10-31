# keebs-modeler

An expressive, physically modelled struck-string instrument played from a MIDI keyboard.

- [`spec/`](spec) — what we're building
- [`docs/`](docs) — research and notes along the way

## Run

```sh
cargo run --release -p keebs                 # listens to every MIDI input
cargo run --release -p keebs -- --midi "name" # just one (substring match)
cargo run --release -p keebs -- --list        # list MIDI inputs and audio outputs
cargo run --release -p keebs -- --buffer 64   # request a smaller audio buffer
```

No controller? Computer keys A–K play, Z/X shift octave.
Hover a slider to see what it does; scroll over its value to nudge it (hold Option for fine steps); double-click the value to reset it.

## Render to WAV

```sh
cargo run --release -p keebs-render -- note=48 vel=20,64,127 hold=2 out=tmp/sweep.wav
cargo run --release -p keebs-render -- note=48 strike_pos=0.5 inharm=8 out=tmp/odd.wav
cargo run --release -p keebs-render -- help   # every parameter
```

Renders and other scratch output go in `tmp/` (gitignored; default output is `tmp/out.wav`).

## Crates

| Crate | |
|---|---|
| `crates/core` | The sound engine (no I/O). `examples/` measure tone (`survey`), pedal behaviour (`pedal`), and CPU (`bench`). |
| `crates/app` | Standalone player: `cpal` audio, `midir` MIDI, `eframe` controls. |
| `crates/render` | Offline renderer for listening and comparing settings. |
