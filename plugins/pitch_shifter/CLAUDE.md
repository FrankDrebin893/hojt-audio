# Pitch Shifter

Loaded when working in `plugins/pitch_shifter`. The suite-wide rules are in the root `CLAUDE.md`.

Shift guitar audio by semitones (-12 to +12), chords as well as single notes, with a fixed latency
of about 11 ms for live playing. A spectral shifter that waits only for the newest few
milliseconds of each frame.

- `src/lib.rs` - Plugin entry point, parameters, nih-plug integration
- DSP: `crates/suite_common/src/dsp/pitch.rs` (`suite_common::dsp::pitch::PitchShifter`: the dry
  signal, the blend, the switch to and from a shift of nothing, the step down to the engine's rate)
  and `crates/suite_common/src/dsp/pitch/spectral.rs` (the engine). Shared with the Guitar Amp's
  pitch pedal, with its tests and the two tools below
- `src/editor.rs` - egui GUI: orange pedal, four knobs, bypass footswitch (`test_the_controls_do_not_touch` keeps them apart)

Algorithm (the engine's module comment has it in full):

1. The engine runs at 44.1 to 48 kHz: at a higher rate the input is stepped down by a whole
   number and the output up again (0.25 ms of filters)
2. Every hop (128 samples, 2.7 ms) the last 4096 samples (85 ms) are windowed and transformed. The
   window is lopsided: it rises over the frame and falls over the last half chunk, so the frame
   resolves the partials of a chord while the output only waits for the chunk (the latency)
3. A tracker (a symmetric window over the same samples, every 512 samples) finds the steady
   partials, with their frequencies read from the phase
4. Each tracked partial is fitted to the frame as a sinusoid, taken out, and put back at its
   shifted frequency. What is left (attacks, noise, bends, what the tracker has not caught yet)
   is cut into regions around its peaks and each region is moved as it is
5. Each partial and region carries a rotation on from frame to frame, so the chunks join. The
   newest chunk of the inverse transform is overlap-added to the output
6. 0 semitones: the engine rests and the input passes through bit-exactly, 2 samples late

Each stereo channel has an engine of its own.

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| Semitones | -12 to +12 | 0 | Pitch shift amount |
| Max Latency | 2-50 ms | 15 ms | The lag is the largest of 5.3, 10.7, 21 and 43 ms (at 48 kHz) that fits under it; under 5.3 ms it is 5.3. Longer is cleaner |
| Smoothness | 0.5-10 ms | 2 ms | Crossfade between the input and the shifted signal when a shift starts or ends (Bypass, 0 semitones) or Max Latency takes another step |
| Blend | 0-100 % | 100 % | All dry to all shifted, mixed linearly. Smoothed per sample (20 ms) |
| Bypass | on / off | off | The footswitch. Bypass is a shift of 0 semitones, so it switches without a click |

Blend's dry part is the input 2 samples late, which is also what a shift of 0 is, so at a shift
of 0 (and in Bypass) the two parts are one signal and the output stays bit-exact at any Blend.
The shifted part trails the dry one by the latency: at octaves and fifths that is what a pitch
pedal does, at +-1 and +-2 semitones it can be heard as a short doubling. That is left as it is.

The delay is fixed by Max Latency and does not depend on the note. At the default it is 10.7 ms
(48 kHz), where single strings come out about 45 dB clean and full chords 22 to 38 dB;
5.3 ms costs single notes about 10 dB, and chords little. It costs 3 to 4 % of a core per channel
at any setting, in a frame every 2.7 to 10.7 ms (about a tenth of a 64-sample block at 48 kHz).

Why not the splicing shifter this replaced (variable-rate playback with waveform-matched splices,
until 0.3.0): a splice needs a period to land on. A string's partials are not whole multiples,
so every splice of a ringing note left a small step, at the rate of the splices (a buzz, 20 to
35 dB down), and the notes of an equal-tempered chord share no period under 50 ms, so a chord
was mostly artifacts (5 to 10 dB down) at the default. The measurements are on the issue.

Its CLAP ID and VST3 class ID predate the suite and are kept as they were.

Measure DSP changes with `quality_report` before judging them by ear.

Pitch shifter tests are in `crates/suite_common/src/dsp/pitch.rs` and cover:
- Bit-exact passthrough at 0 semitones, and with Bypass, at any Blend
- Blend: 100 % unchanged from the shifted output, 0 % the dry input, 50 % a two-note harmony,
  no click while it turns
- Artifact level of shifted sines and harmonic tones (`artifact_ratio_db`: energy that is not at
  the expected shifted frequencies), and of ringing strings and equal-tempered chords of them
  (`fading_artifact_ratio_db`, which lets every partial decay)
- Latency: a note starts one latency late whatever its pitch, and the latency stays within the setting
- Level of notes, chords and noise, stereo, no clicks under parameter changes, 96 and 192 kHz
- Reset and silence

Two ignored tests are tools for working on the sound:

```bash
# Table of artifact levels and latency per note and shift. Compare before/after a DSP change.
cargo test -p suite_common --release quality_report -- --ignored --nocapture

# WAV files in target/renders for listening (notes, a chord, power chords, a long note).
# Optional env vars: PITCH_SHIFTER_INPUT_WAV=<recording>, PITCH_SHIFTER_LATENCY_MS=<ms>
cargo test -p suite_common --release render_wavs -- --ignored
```

Three more tools score the shifter on recorded playing, which the made-up strings above cannot
stand in for (`scripts/pitch-bench/`):

```bash
# A WAV through the shifter: in, out, semitones, max latency in ms, rate factor (4 = in the amp)
cargo run -p suite_common --release --example pitch_render -- in.wav out.wav -4 15 4

# The pedal in front of a driven rig (compressor, drive, two amps): AMP_INPUT_WAV, AMP_OUTPUT_WAV,
# PITCH_SEMITONES (0 = off), PITCH_LATENCY_MS
cargo test -p amp --release render_pitch_rig -- --ignored

# Every take of a folder, alone and through the rig, scored against the true shifted signal
# (the take played slower): tone, junk, shape, attack, level, lag. Run it on two checkouts with
# the same output folder and two names to compare engines
scripts/pitch-bench/run.sh <takes> <out> -4 15 <name>
```

`shape` is what decides how the pedal sounds in front of a drive or an amp: a clipper answers to
the waveform, not to the spectrum, so a shifter that keeps every partial but lets their phases
drift apart sounds unlike a detuned guitar once it is distorted. Judge a change by the rig's
table as well as by the pedal's own.
