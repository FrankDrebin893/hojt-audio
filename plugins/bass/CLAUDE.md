# Bass Synth

Loaded when working in `plugins/bass`. The suite-wide rules are in the root `CLAUDE.md`.

A plucked string bass, one note at a time. MIDI in, the same signal on both outputs. Two
voices that share the string: Metal (picked; clean lows under a distorted band of mids) and
Groove (finger; a tone filter and a little tube drive). Ported from `prototypes/jsfx`, and since
#130 no longer like the scripts in the string, the plucks and the pickup.

- `src/lib.rs` - Plugin entry point, parameters. Reads them once per block into `BassSettings`
- `src/bass.rs` - `Bass`: the string (a delay line that feeds itself through a low-pass and a
  chain of all-passes, and holds how fast the string moves), the two plucks, the pickup, the
  sine under the string, the two output stages, the cabinet behind them; `Voice` and its settings
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
- The string is stiff: four first-order all-passes in the loop (`stiffness_coeff`) make partial
  `n` sit at about `n f0 sqrt(1 + B n^2)`. B is 1.5e-4 on the open E (41.2 Hz, the 10th partial
  13 cents sharp, the 5th 3.6) and falls with the square of the pitch (the 10th of the A is 7
  cents sharp, of the G 3); the chain is fitted at the 10th partial, and above 800 Hz there is no
  chain. The all-passes have no gain, and the loop's length takes out what they add at the
  fundamental (at the bent pitch too), so the fundamental is where it was.
  Tuning is measured on the fundamental's spectral peak, since a stiff string no longer repeats
  exactly. The pickup is a part of the whole way round (`ring`: the line, the low-pass and the
  all-passes), so its comb stays where it was
- The plucks are written into the line as the string's velocity: two levels either side of
  where it is plucked (the pick at 0.18 of the open string from the bridge, the finger at 0.12),
  their edges rounded by how sharp the touch is (velocity and Brightness). Metal adds a little
  noise for the pick's click. No two plucks are the same: each moves where it is plucked (up to
  ±0.015 of the string), how round its edges are (±15 %) and the click's level (±25 %), the
  first two each to the other side of the last pluck, from the voice's own `Noise` (a fixed
  seed, so renders and tests repeat). Velocity is untouched
- The string has two planes: a second loop (its own line, low-pass state, all-pass states and gain)
  written and read with the first. It is plucked a quarter of a period later at 0.15 of the
  first's level, rings 1.2 times as long and runs `PLANE_BEAT_HZ` (0.4 Hz, in Hz not in cents, so
  that the fundamental beats at every pitch) sharp of it, so a held note breathes and its level
  from 1 to 3 s is not a straight line in dB. The quarter period makes the two start a quarter of
  a beat apart, so the level ripples about where it would be instead of falling first (Sustain
  still reads right). The pickup reads both. The first plane is tuned `PLANE_FLAT_HZ` flat, which
  takes back what the second pulls the note up by, so notes stay within 1 cent. The sine follows
  the first plane. The second plane's pluck is never random: only `noise` draws where a pluck
  lands, and `touch_noise` draws what the hand and the finger make, so that a longer scrape
  does not move the jitter of the plucks after it. `STRING_GAIN` went from 0.7 to 0.62: a level trim
  for the second plane, which adds about 1.3 dB to the first, so the voices stay about as loud
- Tuning: the second plane pulls the spectral peak of the sum up by a fixed 0.03 to 0.04 Hz, so
  `PLANE_FLAT_HZ` (0.03) takes it back; the amplitude-weighted 0.052 Hz is too much (low E 1.4
  cents flat). Milestone 3 moved three tests: the sine test's centroid factor is 1.4, not 1.5
  (Groove's G, note 43, is at 224 to 226 Hz against 233 before the planes, 1.45 times; the quarter
  period puts the planes' 2nd harmonics against each other, which lifts the fundamental's share);
  Metal against Groove's brightness is 1.7 times, not 2 (it was 2.14: Groove's finger knock is
  brightness); and the hard-pluck test measures the first plane alone (`line_other` silenced),
  since the 0.4 Hz beat biases a 20 ms period read on the low E
- The cost of one note is held by `test_one_note_costs_at_most_2_percent_of_a_core`, which runs
  only optimised (`scripts/check.sh` and `ci.yml` run `cargo test -p amp -p bass --release`)
- The hand: Metal's pick scrapes (noise of 2.5 ms that swells over a fifth of it and dies, in
  the pluck), Groove's finger lands with a knock (an impulse through a resonance at 1.5 kHz, with
  a little noise, a few ms) and a small thump (one period of a sine of 3 ms), all scaled by the
  velocity. Letting go of a held note makes a tick (smoothed, differenced noise that dies in
  4 ms) before the cabinet, 30 to 45 dB under the note's peak; none from a note that is not held
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
| Brightness (`brightness`) | 0-100 % | 60 % | 54 % | How hard the string is plucked and how long its top rings |
| Sub (`sub`) | 0-100 % | 15 % | 15 % | Level of the sine under the string |
| Tone (`tone`) | 300-4000 Hz | 2600 Hz | 1400 Hz | Low-pass |
| Sustain (`sustain`) | 0.5-8 s | 3 s | 3 s | Time for a held note to fall by 60 dB |
| Slide (`slide`) | 0-300 ms | 0 ms | 60 ms | Slide between overlapping notes; 0 plucks every note |
| Transpose (`transpose`) | -24 to +24 st | 0 | 0 | |
| Bend (`bend`) | 1-12 st | 2 | 2 | How far the pitch-bend wheel goes at its end; no voice sets it |

Tests sit next to the code (`bass/tests.rs`): tuning at several rates, envelopes, the two voices, the pitch-bend wheel (its range, back to centre, a pluck and a slide while bent, no zipper, Sustain on a bent note), bounded output with everything up and the wheel at both ends, and the tone: the top dies before the fundamental, the same note sounds the same while two plucks in a row are not copies, a hard pluck starts sharp and settles, Metal's drive lets the note die, Groove has harmonics, the sine does not take over a held note and sits under the string (what it adds to the fundamental, the fundamental's share of the harmonics' power and their centroid, at 50 ms), the pickup's notch moves with the fret, the string is stiff (the low E's 10th partial 8 to 30 cents sharp, the 5th less, and every note within 1 cent of its pitch from 44.1 to 192 kHz), a held note's level from 1 to 3 s is 0.5 to 3 dB off a straight line, Groove has more energy above 1 kHz in its first 5 ms than before its finger attack, letting go of a note makes a sound 30 to 45 dB under its peak, and a new pluck does not click. Two ignored tests are tools for working on the sound:

```bash
# Level, tuning and brightness per note; with Sub at 0, from what the pickup reads before the
# output stage ("string"), the T60 of the fundamental and of the partial nearest 1.5 kHz and the
# levels of harmonics 2 and 3, and the same two T60s at the output ("out"); at velocity 0.8 with
# the voice's settings, what the sine adds to the fundamental at 50 ms, the fundamental's share
# of the power of the harmonics up to 6 kHz, their power-weighted centroid and the 5th against
# the fundamental; how far the level from 1 to 3 s is off a straight line, the energy above 1 kHz in
# the first 5 ms and the sound of letting go; how many cents sharp the 5th and 10th partials of
# the string run; and the cost (at most 2 % of a core)
cargo test -p bass --release bass_report -- --ignored --nocapture

# WAV files in target/renders: bass_metal.wav, bass_groove.wav, and a walking, a reggae and a
# funk line on each voice (bass_<line>_<voice>.wav)
cargo test -p bass --release render_wavs -- --ignored
```
