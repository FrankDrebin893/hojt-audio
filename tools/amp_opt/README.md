# Amp DSP Perceptual Optimizer (`tools/amp_opt`)

Offline, Rust-in-the-loop perceptual audio diagnostic and CMA-ES parameter optimizer for the Guitar Amp (`amp_bench`). Requires only Python 3 + `numpy`.

## 1. Local Data Setup (`bench/data/`, gitignored)

Record mono 48 kHz WAVs (24-bit or 32-bit float), peaking around `-12` to `-6 dBFS`:
- `bench/data/di/clean.wav`, `crunch.wav`, `metal.wav`
- `bench/data/irs/<shared_cab>.wav` (use the **exact same IR** in your reference amp sim with its built-in cab disabled)
- `bench/data/ref/clear_gain5.wav`, `roar_gain5.wav`, `thunder_gain6.wav` (reverb/delay/chorus/gate OFF)

## 2. Diagnose Current DSP vs. Reference

```bash
python tools/amp_opt/optimize.py diagnose \
  --amp thunder \
  --di bench/data/di/metal.wav \
  --ref bench/data/ref/thunder_gain6.wav \
  --ir bench/data/irs/v30.wav \
  --gain 0.6 --bass 0.5 --mid 0.5 --treble 0.5 --presence 0.5 --master 0.5
```

Outputs a 7-band dB error table (`Sub`, `Low`, `Mud`, `Mid`, `Bite`, `Pres`, `Fizz`), envelope/crest-factor gaps, and loudness-matched `01_reference.wav` + `02_baseline.wav` in `bench/runs/`.

## 3. Run CMA-ES Optimization

Create `bench/cases_thunder.json`:
```json
[
  {
    "name": "thunder_gain6",
    "di": "bench/data/di/metal.wav",
    "ref": "bench/data/ref/thunder_gain6.wav",
    "ir": "bench/data/irs/v30.wav",
    "gain": 0.6, "bass": 0.5, "mid": 0.5, "treble": 0.5, "presence": 0.5, "master": 0.5
  }
]
```

Run:
```bash
python tools/amp_opt/optimize.py run --amp thunder --cases bench/cases_thunder.json --evals 120
```

Outputs `best_overrides.txt`, `report.md`, and loudness-matched `01_reference.wav` / `02_before.wav` / `03_after.wav` in `bench/runs/`.
