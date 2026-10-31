# Bass: why it sounded weak, and the levers

Findings from 2026-10-31. Measured with `survey`, not heard.

## Measurements at medium velocity, before changes

| key | peak | RMS | heard (A-weighted) | brightness (mean partial #) | contact |
|---|---|---|---|---|---|
| A0 | −12 | −24.7 | −41.8 | 8.0 | 6.8 ms |
| C2 | −12 | −23.5 | −39.9 | 4.0 | 3.7 ms |
| C4 | −12 | −19.5 | −33.7 | 1.5 | 2.4 ms |
| C6 | −12 | −19.3 | −28.1 | 1.2 | 0.8 ms |

- **The level trim matched peaks, but a bass note is a pulse train with a high peak-to-average ratio.** RMS was 5 dB below the mid range.
- **Bass energy sits in partials 1–8, mostly under 300 Hz, where hearing is weak.** Perceived loudness ran 8 dB under C4 and 14 dB under C6.
- **Fix:** a per-key `balance` gain (anchors in `note.rs`) matches A-weighted loudness at a medium strike. The *Key balance* parameter scales it: 0 = old peak matching, 1 = equal to the ear.
  - Headroom cost: at full velocity the low bass now peaks around +4 dB before master gain, so loud bass chords lean on the limiter.

## Hammer levers in the bass

- **Mass is the tone lever.** Hammer mass ×0.5 raises A0's brightness 8 → 10–11 and cuts contact to ~3 ms, for 1–2 dB less perceived level.
- **Hardness barely matters.** The heavy string's resistance, not the felt, sets how long the hammer stays on.
- **Strike position near 1/8 notches partials 8 and 16–17**, which sit right where bass character lives.

## Bug found: soft bass notes cut off

- The bridge output of a low string is a train of pulses one period apart (36 ms at A0), with near silence between them when the hammer pulse is narrow.
- The silence detector freed voices after ~11 ms of quiet, so soft or light-hammer bass notes could die in the first gap.
- It now waits at least two fundamental periods (`SILENT_PERIODS` in `voice.rs`).
