# String Model Choice

Which string technique best serves the spec's priorities (expression, character controls, hardware later) — not physical accuracy.

## Recommendation: modal string + nonlinear hammer

A modal string is a bank of decaying resonators, one per partial, each with its own frequency, decay, and gain. Almost every control in `spec/tone.md` becomes a direct per-mode parameter:

| Control | In a modal model |
|---|---|
| Strike position | Per-mode excitation weight `sin(n·π·β)` (β = strike point / length). Notches where `n·β` is an integer. Real pianos use β ≈ 1/7–1/9. |
| Harmonic audibility | Any weighting curve on top — strike position is just one preset curve. Can be non-physical (odd-only, single-partial solo, spectral tilt, moving notches). |
| Inharmonicity | Mode frequencies set exactly from `B`; can also be bent arbitrarily. |
| High vs low decay | Per-mode decay times, from a curve. |
| Wound ↔ steel blend | Interpolate the parameter sets (B, decay curves, impedance/hammer response, longitudinal content). |
| Unison count / detune | Extra mode sets per note; a fractional count is a fading-in gain. |

Why it fits:
- **Seamless blends** — everything is a continuous number, so the bass/tenor break and string-count transitions can be smooth curves across the keyboard.
- **Graceful hardware scaling** — drop high or inaudible modes to fit a CPU budget; quality degrades smoothly.
- **Touch** — hammer contact force is computed from string displacement at the strike point (sum of modes × mode shape), so velocity-dependent brightness comes from the nonlinear felt, not from a filter.

Costs / open questions:
- Bass notes need many modes (100+ below ~100 Hz to reach high frequencies); treble needs few.
- Unison coupling (two-stage decay) and sympathetic resonance are less natural than with waveguides. Approximations exist (Bank 2010).
- Phantom partials need extra modelling (Bank's simplified longitudinal model).

## Alternative: digital waveguide

Cheaper for very many partials, natural sympathetic resonance and unison coupling. Strike-position comb still easy. Per-partial audibility and arbitrary inharmonicity are indirect (filter design). A hybrid (modal for character, waveguide for resonance network) is possible later.

## First milestone

One note: a modal string struck by a nonlinear hammer, driven by MIDI velocity, with live strike-position, inharmonicity, and decay controls. Render offline to WAV for quick listening, and play it in real time.
