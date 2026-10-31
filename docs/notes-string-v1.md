# Notes: first string model (2026-10-31)

What was built and what we learned getting it to behave.

## Model

- **String**: modal, one 2-pole resonator per partial up to 20 kHz (≈300 modes at C2, ≈4 at C8). Impulse-invariant. Mode frequencies `n·f1·√((1+Bn²)/(1+B))` keep the fundamental on pitch as stiffness changes.
- **Hammer**: mass + felt `F = Fref·(c + α·dc/dt)`, `c = (compression/1 mm)^p`.
- **Output**: bridge force (∝ n·q_n), times per-partial audibility weights (tilt, odd/even, listen point). The weights change what you *hear* without changing how the string moves.
- **Damper**: extra uniform decay, smoothed over ~8 ms. Pedal value scales it, so half-pedal works.
- Re-striking a ringing key hits the moving string (state kept).

## Things that bit us

1. **Explicit hammer–string coupling blows up in the treble.** Light strings + stiff felt put the contact resonance above Nyquist. Fix: solve the contact implicitly each sample. The string's strike-point response is `y_free + compliance·F`; solve the scalar felt equation with a bracketed Newton. Stable at any hardness.
2. **Lingering hammer re-contacts choke soft notes.** The hammer is now released as soon as it rebounds.
3. **Soft treble notes were smothered (−50 dB).** When contact time is several periods long, the string just follows the hammer and barely rings afterwards. Real-ish hammer values do this. We made treble hammers far lighter and harder than real (see `note.rs` anchors). In the top octave only a few partials fit below 20 kHz, so this costs almost nothing in tone.
4. **Felt exponent is the main "soft vs hard" knob.** At p=2.5 the soft/hard brightness gap is small; at p=3.5 (now the default) it's clearly larger. Higher p also widens the treble's loudness range.
5. **Level trim per key** (`level` in anchors) makes a medium strike equally loud on every key.

## Measured (defaults, `examples/survey.rs`)

- Soft → hard ring level: ≈23 dB bass, ≈28 dB mid, ≈33 dB treble.
- Brightness (mean partial number) rises with velocity everywhere. Least in the top octave (few partials).
- CPU: 29 sustained voices spread across the keyboard ≈ 6% of one core (desktop, f64).

## Not yet

- Unisons, sympathetic resonance, soundboard/body, phantom partials, release noise.
- Per-key curves for any tone control (everything is global or a per-key default).
- Parameter smoothing: big slider jumps on sounding notes may click.
