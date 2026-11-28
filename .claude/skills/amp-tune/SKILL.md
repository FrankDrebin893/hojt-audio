---
name: amp-tune
description: >-
  Diagnose and optimize Guitar Amp DSP models against local DI and reference WAVs
  using amp_bench and tools/amp_opt (2-stage head/cab CMA-ES + perceptual loss +
  loudness-matched A/B listening bundles). Use when tuning amp sound quality,
  fixing mud/headroom/tightness, or rolling out AmpTopology changes.
---

# Amp DSP Perceptual Tuning (`amp-tune`)

Tune Guitar Amp models (`plugins/amp/src/amp/model.rs`) against real guitar DI and
reference renders instead of relying solely on synthetic sine-wave metrics.

## Prerequisites
- Local gitignored audio in `bench/di/` (dry mono 48 kHz DI WAVs) and `bench/ref/`
  (reference renders with reverb/delay/chorus/gate OFF; ideally both no-cab and cab-ON pairs).
- Build the offline renderer:
  `cargo build -p amp --bin amp_bench --profile optimised`

## Workflow

1. **Diagnose baseline first**
   Run `diagnose` to inspect 7-band dB deltas (`Sub`, `Low`, `Mud`, `Mid`, `Bite`, `Pres`, `Fizz`),
   10 ms `Envelope L1`, and `Crest Gap`:
   ```bash
   python tools/amp_opt/optimize.py diagnose \
     --amp <name> --di bench/di/<di>.wav --ref bench/ref/<ref>.wav [--cab-off] --gain <0..1>
   ```
   Check DI RMS/peak levels before tuning so input-level differences are not misread as DSP faults.

2. **Choose the right `AmpTopology` before optimizing parameters**
   - `CleanAcoustic`: pre-gain tone stack + zero-IMD linear `kneed_clip` headroom + 10 ms smooth cab (`breakup_db` near 0) for note separation.
   - `VintageCrunch`: post-preamp tone stack; use `--voice crunch` (`tight_hz >= 100 Hz`, `mid_centre_db >= -1 dB`, unlocked `power.sag` / `power.drive_db` / `bias_shift`) to keep lows tight and mids fat.
   - `ModernHighGain`: 12 dB/oct 2nd-order tight HPF + post-preamp tone stack + balanced sub resonance for tight palm mutes without losing low-end weight.

3. **Run 2-stage CMA-ES optimization**
   - **Stage A (Amp Head, cab OFF)**:
     `python tools/amp_opt/optimize.py run --amp <name> --cases <head.json> --mode head --voice <default|clean|crunch> --evals 150`
   - **Stage B (Built-in Cabinet, cab ON, locking Stage A head)**:
     `python tools/amp_opt/optimize.py run --amp <name> --cases <cab.json> --mode cab --voice <default|clean|crunch> --base-overrides <stage_a_best_overrides.txt> --evals 150`
   - Use multiple Gain dial cases in `<cases>.json` when tuning across the full Gain sweep.

4. **Bake, verify, and ear-check**
   - Bake winning constants into `plugins/amp/src/amp/model.rs`, keeping each stage's `gain_db` monotonic `[g0, g5, g10]`.
   - Never add real maker/trademark names or per-amp `match amp` branches; verify with:
     `cargo test -p amp --test rules`
   - Rebuild `amp_bench --profile optimised` and run `diagnose` with no `--overrides` to verify baked output.
   - Present the loudness-matched A/B folder in `bench/runs/` (`01_reference.wav` vs `02_baseline.wav`) for ear sign-off. If the ear catches a flaw parameter tuning cannot fix (e.g., chord mud from IMD, loose lows, missing sustain), inspect the envelope/band residuals and add a targeted structural DSP primitive before re-running.
