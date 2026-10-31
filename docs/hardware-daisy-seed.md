# Hardware: Daisy Seed

Notes on the eventual embedded target.

## Board

- STM32H750, ARM Cortex-M7 @ 480 MHz, hardware FPU.
- 64 MB external SDRAM (slow relative to internal SRAM), ~1 MB internal SRAM.
- At 48 kHz: ~10,000 CPU cycles per sample for everything.

## Rust support

- [`daisy-embassy`](https://docs.rs/crate/daisy-embassy/latest) — Embassy-based async audio for Seed 1.1 / 1.2 / Seed3 / Patch SM. Builds on `stm32h7xx-hal`, `daisy_bsp`, `zlosynth/daisy`, `libdaisy-rust`.

## Stance (2026-10-31)

Desktop first, with no compute budget. Hardware must not limit the sound while we're finding it. The port comes later: find cheaper approximations of the finished sound and compare them by ear.

## Habits kept anyway (free, and needed for desktop real-time too)

- DSP core in its own crate, separate from the app shell (`cpal` audio, `midir` MIDI).
- No allocation or locking in the audio path.

## Deferred

- `no_std`, `f32`-only, fixed buffers, mode/voice budgets: handle these at port time.
- Rough guess (verify by benchmark): a 2-pole resonator is a handful of multiply-adds per sample, so a few hundred to ~1000 modes total may be realistic on the M7 if state lives in internal SRAM.
