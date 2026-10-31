# Research: Physically Modelled Piano

Notes from initial research (2026-10-31).

## How a piano sound is made, part by part

| Part | What it contributes | Common technique |
|---|---|---|
| Hammer | Velocity → loudness *and* brightness | Mass + nonlinear felt spring, `F = K·x^p`, p ≈ 2–3.5. Felt hysteresis (Stulov). Wave digital filter versions stay stable. |
| Strike point | Comb-filter notches on the partials | Falls out of where the hammer meets the string model |
| String | Pitch, decay | Digital waveguide (delay + loss filter + fractional delay) or modal resonator bank |
| Stiffness / inharmonicity | Stretched partials: `f_n = n·f0·√(1 + B·n²)` | Allpass dispersion filters in a waveguide loop; exact per-mode frequencies in a modal model |
| Unisons | Beating, two-stage decay | 2–3 detuned strings coupled at the bridge |
| Longitudinal motion | Phantom partials, bass "growl" when loud | Bank's simplified modal model; forced response (phantom partials) matters more perceptually than free longitudinal modes |
| Soundboard | Most of the body and colour | Convolution with an IR, feedback delay network, or parallel 2nd-order filters |
| Sympathetic resonance | Undamped strings ringing along | Keep string models running, couple via the bridge |
| Dampers / pedals / noise | Release, half-pedal, mechanical noise | Damper filters, noise samples |

## Modelling approaches

- **Digital waveguide** — cheap per string, sympathetic resonance is natural. Accurate dispersion needs high-order allpasses; hammer–string loop is fiddly.
- **Modal** — direct per-partial control, exact inharmonicity. Bank/Zambon/Fontana (2010) ran a full-polyphony real-time piano this way, including longitudinal motion.
- **Finite difference** — most physically faithful, very expensive. OpenPiano uses this; still alpha, no soundboard, one string per note.
- **Commuted** — fold soundboard into the excitation. Cheap, but conflicts with nonlinear hammer and sympathetic resonance; largely superseded.

## Existing open-source work

- [OpenPiano](https://github.com/michele-perrone/OpenPiano) — JUCE, finite difference. Useful list of open problems.
- [Piano Forte](https://chilloutwithbeats.com/en/tesserato-pianoforte-intro/) — free, very CPU-heavy.
- [OpenWurli](https://github.com/hal0zer0/openwurli), [EP-MK1](https://github.com/MikeMorenoDSP/EP-MK1) — electric pianos; good structural references.

## Key sources

- Bank et al., *Physically Informed Signal Processing Methods for Piano Sound Synthesis* (EURASIP 2003) — https://asp-eurasipjournals.springeropen.com/articles/10.1155/S1110865703304093
- Välimäki et al., *Discrete-time modelling of musical instruments* (2006) — https://www.ece.uvic.ca/~bctill/papers/numacoust/V%c3%a4lim%c3%a4ki_etal_2006.pdf
- Bank, *Physics-Based Sound Synthesis of the Piano* (thesis) — https://home.mit.bme.hu/~bank/thesis/thesabs.html
- Bank, Zambon, Fontana, *A Modal-Based Real-Time Piano Synthesizer* — https://home.mit.bme.hu/~bank/publist/taslp-piano/index.html
- Bank, *Generation of longitudinal vibrations in piano strings* — https://home.mit.bme.hu/~bank/publist/jasa-longitud/index.html
- Bank & Chabassier, *Model-based digital pianos: from physics to sound synthesis* — https://core.ac.uk/download/162998423.pdf
- J.O. Smith, *Physical Audio Signal Processing* — dispersion filters, stiff strings, nonlinear strings — https://ccrma.stanford.edu/~jos/pasp/
- Rauhala & Välimäki, *Dispersion Modeling in Waveguide Piano Synthesis Using Tunable Allpass Filters* (DAFx 2006) — https://dafx.de/paper-archive/details/n4Qv1ScD_VbmqShsq0ZVQQ
- Kleimola, *Dispersion modulation using allpass filters* (DAFx 2008) — https://dafx.de/paper-archive/details/7DtE9e8mjdCgASzIGbEBLg
