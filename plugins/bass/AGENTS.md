# Bass Synth

Loaded when working in `plugins/bass`. The suite-wide rules are in the root `AGENTS.md`.

A plucked string bass, one note at a time. MIDI in, the same signal on both outputs. Two
voices that share the string: Metal (picked; clean lows under a distorted band of mids) and
Groove (finger; a tone filter and a little tube drive). Ported from `prototypes/jsfx`, and since
#130 no longer like the scripts in the string, the plucks and the pickup.

- `src/lib.rs` - Plugin entry point, parameters. Reads them once per block into `BassSettings`
- `src/bass.rs` - `Bass`: the string (a delay line that feeds itself through a low-pass and
  holds how fast the string moves), the two plucks, the pickup, the sine under the string, the
  two output stages, the cabinet behind them; `Voice` and its settings
- `src/editor.rs` - egui GUI: cobalt pedal, nine knobs, one footswitch per voice

How it behaves:

- A voice footswitch also sets the knobs to that voice's own settings (`Voice::settings`),
  like a preset, all but Bend, which belongs to the player's keyboard as the wheel does. A host
  that changes Voice itself leaves the knobs alone
- The pitch-bend wheel (`MidiPitchBend`, 8192 exactly centre) bends whatever note is sounding
  by up to Bend semitones either way. The bend is smoothed (5 ms) and divides the loop's length,
  sliding or not, so a slide and a bend add up; the sine under the string follows. The loop's
  gain (and what its low-pass adds to the length, at the bent pitch) follows the bend once a
  block when it has moved 1 cent or settled, so Sustain is the same T60 on a bent note; the
  low-pass itself stays the one the note was plucked with. A pluck starts at the bent pitch; all sound off and a host's
  reset put the wheel back at centre. At centre the string is exactly as it was without it
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
  where it is plucked (the pick at 0.18 of the open string from the bridge, the finger at 0.12),
  their edges rounded by how sharp the touch is (velocity and Brightness). Metal adds a little
  noise for the pick's click. No two plucks are the same: each moves where it is plucked (up to
  ±0.015 of the string), how round its edges are (±15 %) and the click's level (±25 %), the
  first two each to the other side of the last pluck, from the voice's own `Noise` (a fixed
  seed, so renders and tests repeat). Velocity is untouched
- A pluck starts sharp by the square of its velocity (6 cents at full velocity) and falls to
  pitch with a 60 ms time constant; the tuning is measured after it has settled
- A new pluck on the same note keeps a quarter of what was ringing under it (a pick lands on a
  moving string); on another note it stops it. The step it makes in the output fades out over
  1.5 ms
- The pickup reads the line at two points 0.2 of the open string apart (a comb: on an open
  string it takes out every fifth harmonic), and a resonant low-pass at 2.8 kHz follows it
- The pick, the finger and the pickup are fixed distances from the bridge, as on an instrument:
  a note is played on the highest open string (E1 28, A1 33, D2 38, G2 43) at or below what it
  sounds (with Transpose), and on fret `n` each is `2^(n/12)` times its part of the open string,
  up to 0.3 of the length that rings. So the comb takes out another harmonic on each fret (the
  4th on the 4th fret of the A), and a note up the neck is rounder. Below the low E it is open
- Sub is a sine that starts in phase with what the pickup reads of the fundamental, at a level
  against it (1.5 times it at 100 %), and dies with T60s of 0.85 of the string's, held and let go
- Metal splits the lows from the mids at 120 Hz with a low-pass and what it leaves, which sum
  to the input. The drive's gain follows the note down from the pluck (it falls by 60 dB in a
  quarter of Sustain, or of the release once let go), and three quarters of what it loses is
  made up after it: the mids break up on the attack and clean up as the note rings, so the top
  dies with the string and the fundamental rings as long at the output as on the string
- A cabinet behind both output stages, fixed: a lift of 5 dB at 350 Hz for the note's body, a
  presence rise of 7 dB at 1.6 kHz and a low-pass at 4.5 kHz where a bass speaker stops
  (`CABINET_*`)
- Notes are within 1 cent from 44.1 to 192 kHz (`bass_report`)
- `lib.rs` takes every default from `BassSettings::default`, which is Metal's settings

| Parameter (id) | Range | Metal | Groove | Description |
|----------------|-------|-------|--------|-------------|
| Voice (`voice`) | Metal / Groove | | | Pluck and output stage |
| Volume (`volume`) | -30 to +12 dB | -2 dB | -3 dB | Output level |
| Drive (`drive`) | 0-100 % | 40 % | 15 % | Distortion of the mids (Metal), tube drive (Groove) |
| Brightness (`brightness`) | 0-100 % | 55 % | 50 % | How hard the string is plucked and how long its top rings |
| Sub (`sub`) | 0-100 % | 15 % | 15 % | Level of the sine under the string |
| Tone (`tone`) | 300-4000 Hz | 2600 Hz | 1400 Hz | Low-pass |
| Sustain (`sustain`) | 0.5-8 s | 3 s | 3 s | Time for a held note to fall by 60 dB |
| Slide (`slide`) | 0-300 ms | 0 ms | 60 ms | Slide between overlapping notes; 0 plucks every note |
| Transpose (`transpose`) | -24 to +24 st | 0 | 0 | |
| Bend (`bend`) | 1-12 st | 2 | 2 | How far the pitch-bend wheel goes at its end; no voice sets it |

Tests sit next to the code (`bass.rs`): tuning at several rates, envelopes, the two voices, the pitch-bend wheel (its range, back to centre, a pluck and a slide while bent, no zipper, Sustain on a bent note), bounded output with everything up and the wheel at both ends, and the tone: the top dies before the fundamental, the same note sounds the same while two plucks in a row are not copies, a hard pluck starts sharp and settles, Metal's drive lets the note die, Groove has harmonics, the sine does not take over a held note and sits under the string (what it adds to the fundamental, the fundamental's share of the harmonics' power and their centroid, at 50 ms), the pickup's notch moves with the fret, and a new pluck does not click. Two ignored tests are tools for working on the sound:

```bash
# Level, tuning and brightness per note; with Sub at 0, from what the pickup reads before the
# output stage ("string"), the T60 of the fundamental and of the partial nearest 1.5 kHz and the
# levels of harmonics 2 and 3, and the same two T60s at the output ("out"); at velocity 0.8 with
# the voice's settings, what the sine adds to the fundamental at 50 ms, the fundamental's share
# of the power of the harmonics up to 6 kHz, their power-weighted centroid and the 5th against
# the fundamental; and the cost
cargo test -p bass --release bass_report -- --ignored --nocapture

# WAV files in target/renders: bass_metal.wav, bass_groove.wav, and a walking, a reggae and a
# funk line on each voice (bass_<line>_<voice>.wav)
cargo test -p bass --release render_wavs -- --ignored
```
