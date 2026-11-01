# String construction: gauge, winding, wound/plain break

Added 2026-10-31. Measured with `survey`, not heard.

## Model

Each key's string is built from a steel core of diameter `d`, plus copper winding on wound strings. The winding adds mass (as a ratio of the core's mass) but no stiffness. From that:

- `mu = ρ_steel·πd²/4 · (1 + wound·winding)`
- `T = 4L²f²·mu`, so for a fixed pitch a heavier string needs more tension
- `B = π³·E·d⁴ / (64·T·L²)`. Stiffness comes from the core only, so winding lowers B by raising T
- `t60 ∝ 1/mu` relative to the default construction. Heavier strings lose energy to the bridge faster

The anchors in `note.rs` were back-solved from the earlier hand-tuned `mu` and `B` values. Bass cores come from B and windings from mass; the treble is plain steel. At default settings the sound stays within 0.8 dB of before.

- The earlier anchors were already physically consistent for plain steel: C4 comes out as a 1.01 mm wire. The bass needed a 1.59 mm core with 3.5× its mass in winding at A0.
- An extra C3 anchor gives the first plain strings above the break a thick core. Without it they came out lighter than the old curve: 2 dB quieter and darker around keys 46–54. Real piano designers thicken these strings for the same reason.

## Wound/plain break

`woundness(key)` is a smoothstep centred at `break + 0.5`, spread over `blend` keys. Blend 0 gives an abrupt break. The break range is 0–128, so its ends really mean all-plain and all-wound even at the widest blend.

## What the controls do (A0, medium strike)

| setting | heard | brightness | contact |
|---|---|---|---|
| default | −33.7 | 8.0 | 6.8 ms |
| winding 2.5 | −29.3 | 9.5 | 2.9 ms |
| winding 0 (bare core) | −45.8 | 3.5 | 11.2 ms |
| gauge 1.6 | −29.3 | 9.2 | 2.7 ms |
| gauge 0.6 | −41.1 | 4.7 | 8.1 ms |

- **With the same hammer, string mass is the bass's brightness and loudness lever.** A heavier string throws the hammer off sooner, which shortens the contact. A light string under a big hammer gets smothered.
- **Hammer matching** blends from that physical behaviour (0) to rescaling the hammer and output level with string mass (1). Output is about proportional to mass, so the level divides by `(mu/mu_default)^matching`. At 1, construction changes only stiffness, ring and winding loss, and loudness holds within 1 dB.
- Bare steel bass rings very long (t60 ×4.5) and is very inharmonic. This is the "plain steel" character, and it is only bright when hammer matching is up.
