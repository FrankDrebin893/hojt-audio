# Bass Synth

Loaded when working in `plugins/bass`. The suite-wide rules are in the root `AGENTS.md`.

A plucked string bass, one note at a time. MIDI in, the same signal on both outputs. Two
voices that share the string: Metal (picked; clean lows under a distorted band of mids) and
Groove (finger; a tone filter and a little tube drive). Ported from `prototypes/jsfx`, and since
#130 no longer like the scripts in the string, the plucks and the pickup.

- `src/lib.rs` - Plugin entry point, parameters. Reads them once per block into `BassSettings`
- `src/bass.rs` - `Bass`: the string (a delay line that feeds itself through a low-pass and
  holds how fast the string moves), the two plucks, the pickup, the sine under the string, the
  two output stages; `Voice` and its settings
- `src/editor.rs` - egui GUI: cobalt pedal, eight knobs, one footswitch per voice

How it behaves:

- A voice footswitch also sets the knobs to that voice's own settings (`Voice::settings`),
  like a preset. A host that changes Voice itself leaves the knobs alone
- Slide at 0 ms plucks every note. Above 0, a note that starts while another is held slides
  to it without a new pluck, in the Slide time
- Tone is the low-pass behind the string on Groove, and behind the drive on Metal
- Letting go of the key damps the string (70 ms on Metal, 120 ms on Groove), and so does all
  notes off. All sound off stops it at once
- A note that is ringing keeps the string it was plucked with when the voice changes; what
  stands behind the string changes at once. Sustain reaches the ringing note
- The string: each note's loop low-pass is worked out from two T60s, the fundamental's
  (Sustain) and the top's at 1.5 kHz (Brightness, 0.15 s at 0 % to 0.8 s at 100 %, shorter
  for a softer note), so a low note loses its top as fast as a high one. The loop's gain
  takes out what the low-pass loses at the note, and its length what the low-pass adds there
- The plucks are written into the line as the string's velocity: two levels either side of
  where it is plucked (the pick at 0.18 of the string from the bridge, the finger at 0.12),
  their edges rounded by how sharp the touch is (velocity and Brightness). Metal adds a little
  noise for the pick's click. A new pluck stops what was ringing; the step it makes in the
  output fades out over 1.5 ms
- The pickup reads the line at two points a fifth of the string apart (a comb, which takes out
  every fifth harmonic), and a resonant low-pass at 2.8 kHz follows it
- Sub is a sine that starts in phase with what the pickup reads of the fundamental, at a level
  against it (4 times it at 100 %), and dies with T60s of 0.85 of the string's, held and let go
- Metal splits the lows from the mids at 120 Hz with a low-pass and what it leaves, which sum
  to the input
- Notes are within 1 cent from 44.1 to 192 kHz (`bass_report`)
- `lib.rs` takes every default from `BassSettings::default`, which is Metal's settings

| Parameter (id) | Range | Metal | Groove | Description |
|----------------|-------|-------|--------|-------------|
| Voice (`voice`) | Metal / Groove | | | Pluck and output stage |
| Volume (`volume`) | -30 to +12 dB | -2 dB | -3 dB | Output level |
| Drive (`drive`) | 0-100 % | 50 % | 15 % | Distortion of the mids (Metal), tube drive (Groove) |
| Brightness (`brightness`) | 0-100 % | 55 % | 35 % | How hard the string is plucked and how long its top rings |
| Sub (`sub`) | 0-100 % | 25 % | 30 % | Level of the sine under the string |
| Tone (`tone`) | 300-4000 Hz | 3200 Hz | 1100 Hz | Low-pass |
| Sustain (`sustain`) | 0.5-8 s | 3 s | 3 s | Time for a held note to fall by 60 dB |
| Slide (`slide`) | 0-300 ms | 0 ms | 60 ms | Slide between overlapping notes; 0 plucks every note |
| Transpose (`transpose`) | -24 to +24 st | 0 | 0 | |

Tests sit next to the code (`bass.rs`): tuning at several rates, envelopes, the two voices, bounded output with everything up, and the tone: the top dies before the fundamental, the same note sounds the same, Groove has harmonics, the sine does not take over a held note, and a new pluck does not click. Two ignored tests are tools for working on the sound:

```bash
# Level, tuning and brightness per note; from the string alone the T60 of the fundamental and of
# the partial nearest 1.5 kHz and the levels of harmonics 2 and 3; and the cost
cargo test -p bass --release bass_report -- --ignored --nocapture

# WAV files in target/renders: bass_metal.wav, bass_groove.wav, and a walking, a reggae and a
# funk line on each voice (bass_<line>_<voice>.wav)
cargo test -p bass --release render_wavs -- --ignored
```
