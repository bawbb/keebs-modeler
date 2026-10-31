# AGENTS.md

An expressive, physically modelled struck-string instrument played from a MIDI keyboard. Rust workspace.

## Priorities

- **Expressiveness over accuracy.** Responsiveness, soft↔hard contrast, and playable character controls matter; faithful piano reproduction does not. Non-physical settings are welcome when musical.
- **Desktop first, no compute budget.** Hardware (Daisy Seed class) is a later port of a finished sound — never limit the sound for it now.

## Repo layout and conventions

- `spec/` — **only the *what***, never the *how*. Concise, one file per part of the vision. No technique names, crates, or algorithms.
- `docs/` — research, decisions, and notes learned along the way (the *how* and *why*). Add a note when something non-obvious is learned; index it in `docs/README.md`.
- `tmp/` — gitignored scratch output (renders, demos). Put generated files here, not in the repo or system temp.
- `crates/core` — the sound engine. No I/O. `examples/` hold measurement tools.
- `crates/app` — standalone player (`cpal` audio, `midir` MIDI, `eframe`/egui UI).
- `crates/render` — offline WAV renderer (defaults to `tmp/out.wav`).

## Commands

```sh
cargo test -p keebs-core --release                 # engine tests
cargo clippy --workspace --release                 # keep at zero warnings
cargo run --release -p keebs                       # play (all MIDI inputs, auto audio out)
cargo run --release -p keebs -- --list             # MIDI inputs + audio outputs
cargo run --release -p keebs-render -- help        # render options + every parameter
cargo run --release -p keebs-core --example survey [param=value ...]  # loudness/brightness/contact per key & velocity
cargo run --release -p keebs-core --example pedal  [param=value ...]  # decay after release at pedal positions
cargo run --release -p keebs-core --example bench                     # CPU cost of dense chords
```

## Verifying sound changes

You can't listen, so **measure**: use `survey`, `pedal`, and `bench`, and add a core test for any behaviour that matters (e.g. "harder is brighter", "half-pedal spares quiet notes"). Render WAVs to `tmp/` for the user to audition. Report numbers, and say plainly that it hasn't been heard.

## Engine rules

- **Parameters** live in one table (`core/src/params.rs`) that drives the UI, renderer, and engine. To add one: add the `ParamId` variant and the `PARAMS` entry **in the same position** (a test enforces order), bump `PARAM_COUNT`, give it a `help` string written for a player.
- **Audio thread**: no allocation, no locks, no I/O. Mode vectors are preallocated to `MAX_MODES`; `configure()` may run mid-note and must keep ringing state. Events reach the audio thread through an `rtrb` ring buffer (producers lock; the audio consumer never does).
- Internal DSP is `f64`.
- **Hammer–string contact is solved implicitly** each sample (bracketed Newton on the felt law). Explicit coupling blows up on light treble strings — don't revert it.
- **Treble hammers are deliberately much lighter/harder than real** (`core/src/note.rs` anchors); otherwise soft treble notes are smothered. Per-key `level` matches peak level and `balance` then matches *perceived* (A-weighted) loudness at a medium strike — re-check both with `survey` (`heard_dB` column) after changing anchors.
- **Damping** is folded into each mode's coefficients once per 64-sample block (`ModalString::set_damping`), so the per-sample loop stays lean.
- Per-key defaults are log-interpolated between anchors in `note.rs`.
- **Performance ideas** go in `docs/performance.md` as you notice them (new hot loops, per-mode costs, memory). Note them; don't apply them — desktop stays unconstrained.

## UI (egui 0.37)

- API differs from older egui: `App::ui(&mut self, ui, frame)`, `egui::Panel::top/bottom(..).show(ui, ..)`, `CentralPanel::default().show(ui, ..)`.
- Slider rows: name column + full-width track + value box. Mouse-wheel adjust and double-click reset apply **only to the value box**; a scroll gesture latches to a value box or to the list at its start (see `WheelLatch`).
- Audio output: "Auto" follows the system default (cpal reroutes and reports `DeviceChanged`); a named device rebuilds the stream and swaps in a new event producer.

## Commits

Commit on your own once the user has confirmed a feature's work has been tested/reviewed (e.g. they've played it and are happy) — don't commit untested or unreviewed work. Group by topic (e.g. engine vs UI), one commit per feature. **Never push on your own** — commits stay local until the user says it's a good time to push to `origin` (GitHub, SSH).
