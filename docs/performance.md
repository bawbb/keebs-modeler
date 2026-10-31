# Performance ideas

A running list of places the engine could be made cheaper, noted as we find them. **None of these are done.** The desktop build stays unconstrained (see [hardware-daisy-seed.md](hardware-daisy-seed.md)). This list is for when we port. Every idea here changes the sound to some degree, so compare each one by ear against the full model before keeping it.

Add an entry whenever a feature introduces a hot loop, a per-mode cost, or a memory cost worth revisiting.

## Where the time goes now (2026-10-31)

- Cost is the number of modes summed over sounding voices. Each mode costs about 5 multiply-adds per sample in `ModalString::tick`, plus about 3 more in `predict_strike` while the hammer is in contact.
- Modes per key with every partial up to 20 kHz at default inharmonicity: A0 ≈ 221, C2 ≈ 147, C3 ≈ 89, C4 ≈ 52, C5 ≈ 28, C6 ≈ 14, C7 ≈ 7, C8 ≈ 4. **The bass dominates.**
- `bench`: 29 sustained voices take ≈ 5–6% of one desktop core.
- Memory: each voice preallocates `MAX_MODES` (1024) × 10 `f64` arrays ≈ 80 KB. At 32 voices that is ≈ 2.6 MB, which is already more than the Daisy's ~1 MB of internal SRAM.

## Mode budget

- **Lower the frequency ceiling per key.** Bass partials above about 8–10 kHz are faint and decay fast (`HfDamping`), so a per-key ceiling or a global "brilliance" budget would roughly halve the bass mode count.
- **Skip inaudible modes at configure time.** Drop modes whose `exc × out_w` is negligible. That covers modes near a strike-point node or a listen-point node, and modes filtered out by tilt or odd/even.
- **Retire modes as they decay.** Track each mode's amplitude coarsely, once per block, and stop ticking a mode once it is far below the voice's peak. High partials die in tens of ms while the fundamental rings for seconds, so most of a long note's life needs only a few modes. The mode arrays would need compacting, or an "active count" with the modes sorted by decay rate.
- **Share a mode pool across voices** instead of giving each voice `MAX_MODES`. This bounds total CPU and memory regardless of chord shape, and lets voice stealing work on modes rather than whole notes.
- **Multirate.** Run low modes at a decimated rate, e.g. fs/4 for modes under fs/10, and upsample their sum. This suits bass notes, where most modes are low.

## Per-sample loop

- **f32 and SIMD.** The arrays are already structure-of-arrays, so the loop should vectorize 4–8 wide. Check that decay stays stable in f32 for long T60s, since `r` gets very close to 1.
- **Merge `predict_strike` into `tick`** so contact samples make one pass over the modes, not two. The prediction for the next sample can be accumulated from the new `q` values during the current tick.
- **Fold `exc` into the hammer path only.** After contact ends (`force == 0`) the `exc[i] * force` term is dead weight. A second, contact-free loop saves a multiply-add and a memory stream per mode.

## Hammer

- **The felt law calls `powf` once or twice per Newton iteration.** That's fine on desktop, and contact lasts only 1–4 ms. On hardware, use a LUT, or restrict the exponent to a few values with a fast pow.

## Control rate

- **`configure()` evaluates sin/cos/exp/powf per mode, for every voice, on every parameter change.** On hardware, spread it across blocks, or use recurrences: `sin(nπx)` via Chebyshev, and the tilt factor via `log2(n)` tables.
- **`set_damping` calls `exp` per mode per block while the damper moves.** A first-order approximation or a per-mode `damp_w`-scaled table would be cheaper.

## Voices

- **Raise the silence threshold.** It is currently −80 dB relative to the voice's peak, over 8 blocks. Freeing voices sooner is the cheapest win.
- **A voice cap with quietest-voice stealing** already exists. On hardware the cap will be lower, so steal by mode cost, not voice count.
