# Notes: continuous pedal and damper (2026-10-31)

## How it works

- **Pedal curve**: pedal position → damper lift through a *bite point* (where dampers start to leave the strings) and a *half-pedal width* (travel from first lift to fully clear), with a smoothstep between.
- **Gap model**: a lifted damper floats a gap above the string. The gap grows from 0 (resting) to "everything fits" as the damper lifts; *half-pedal depth* (dB) sets how small the gap is partway up. Contact strength = fraction of a sine cycle the string spends beyond the gap, `(2/π)·acos(gap/amplitude)`. Amplitude is a peak follower on the voice output.
  - Result: loud notes are pulled down to about the gap level, then ring on. Notes already quieter than the gap are untouched.
- **Damper tone**: per-mode damping weight `(1 − 0.75·tone)·(f_n/f_1)^tone`. At tone 1 the fundamental lingers about 4x longer and high partials go much faster, leaving a muffled tail.
- Damping is folded into each mode's coefficients once per 64-sample block, so the per-sample loop costs nothing extra.

## Measured (C3, `examples/pedal.rs`, dB 1 s after release)

| pedal | lift | loud (vel 110) | soft (vel 40) |
|---|---|---|---|
| 0.0 | 0 | silent | silent |
| 0.3 | 0.22 | −49 | −49 |
| 0.4 | 0.50 | −31 | −30 |
| 0.5 | 0.78 | −15 | −25 (untouched) |
| ≥0.6 | ~1 | −12 | −25 |

## Ideas not done yet

- Damper strength across the keyboard (weaker bass dampers, none in the top octave and a half). Belongs with keyboard-range curves.
- Damper landing speed following how fast the pedal moves; damper contact noise.
- Amplitude is measured from the *heard* output, so audibility controls (tilt, listen point) slightly change how the damper behaves. Could switch to measuring string motion directly.
