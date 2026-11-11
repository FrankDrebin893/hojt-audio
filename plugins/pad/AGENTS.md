# Shimmer Pad

Loaded when working in `plugins/pad`. The suite-wide rules are in the root `AGENTS.md`.

A slow ambient pad. MIDI in, stereo out, twelve voices. Ported from `prototypes/jsfx`.

- `src/lib.rs` - Plugin entry point, parameters. Reads them once per block into `PadSettings`
- `src/pad.rs` - `ShimmerPad`: voices (three detuned saws and noise through a low-pass that
  opens with the swell), chorus, the hall (`suite_common::dsp::Hall`), the shimmer
- `src/editor.rs` - egui GUI: plum pedal, twelve knobs, Chorus, Shimmer and Hold footswitches

How it behaves:

- A note that is played again takes the voice it has; a thirteenth takes the quietest voice
  that is let go, or the quietest of all
- The filters are set every 32 samples counted across host blocks, so the output does not
  depend on the host's block size
- Shimmer reads the hall's tail back an octave up and feeds it into the hall again
- The two footswitches fade their effect in and out in 20 ms; off is the same as the knob at 0
- All notes off lets go of every key; all sound off stops voices and tails at once
- The mod wheel (CC 1, any channel; `set_wheel`, not a parameter, so no id) opens the filter by
  up to two octaves (`WHEEL_OCTAVES`, the cutoff times `2^(2 * wheel)`, still held under 0.4 times
  the rate) and lifts the shimmer level by up to 0.3 (`WHEEL_SHIMMER`, never past the knob's own
  top of 0.6). The Shimmer switch stays the master: off, the wheel adds none. The wheel is followed
  over about 30 ms, one step per control tick, so a fast move does not zipper. At 0 the sound is
  bit-for-bit what it was; the knobs do not move on screen. Reset keeps the wheel where it is
- The sustain pedal (CC 64, any channel, 0.5 and up is down; `pedal_cc` in `lib.rs`, not a
  parameter) and the Hold switch (`hold`) are one state, `pedal_cc || hold`, passed to
  `ShimmerPad::set_sustain` once per block (the switch) and at each CC 64 event (sample-accurate).
  Down: `note_off` sets `Voice::pedal` and leaves `held` up, so the voice stays at full; up: every
  voice with `pedal` is let go with the Release time. A key played again clears `pedal` (the voice
  it has, held by the finger, and by the pedal again when it is let go); a pedalled voice counts as
  held for stealing. A key still held at pedal-up stays. All notes off clears both; Reset leaves the
  pedal state where it is, like the wheel. Half pedalling is not modelled; an inverted pedal is not handled

| Parameter (id) | Range | Default | Description |
|----------------|-------|---------|-------------|
| Volume (`volume`) | -40 to +12 dB | -6 dB | Output level |
| Attack (`attack`) | 0.01-6 s | 1.5 s | Time to swell to full level |
| Release (`release`) | 0.05-10 s | 3.5 s | Time to fall by 40 dB once the key is let go |
| Cutoff (`cutoff`) | 150-8000 Hz | 1600 Hz | Where the filter of a middle C stands once swelled |
| Dry (`dry`) | 0-100 % | 45 % | Level of the pad itself |
| Detune (`detune`) | 0-30 ct | 9 ct | Distance of the outer saws from the middle one |
| Breath (`breath`) | 0-100 % | 18 % | Noise in each voice |
| Motion (`motion`) | 0-100 % | 35 % | How far the filters wander |
| Chorus (`chorus_on`), Chorus Depth (`chorus`) | on / off, 0-100 % | on, 60 % | |
| Reverb (`reverb`) | 0-100 % | 60 % | Level of the hall |
| Reverb Decay (`reverb_decay`) | 1-20 s | 7 s | Time for the tail to fall by 60 dB |
| Shimmer (`shimmer_on`), Shimmer Level (`shimmer`) | on / off, 0-60 % | on, 30 % | How much of the tail comes back an octave up |
| Hold (`hold`) | on / off | off | Keeps every key that is let go sounding, as a sustain pedal does |

Tests sit next to the code (`pad.rs`): tuning at several rates, envelopes, voice stealing, the effects and their switches, bounded output with everything up. Two ignored tests are tools for working on the sound:

```bash
# Level, tuning and brightness per note, and the cost
cargo test -p pad --release pad_report -- --ignored --nocapture

# WAV files in target/renders: pad.wav, and pad-sustain.wav (a progression changed with the pedal)
cargo test -p pad --release render_wavs -- --ignored
cargo test -p pad --release render_sustain_wav -- --ignored
```
