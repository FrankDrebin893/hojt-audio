# Reports and renders

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

Amp tests sit next to the code. The ones for the whole chain are in `chain/tests.rs`, the editor's in `editor/tests.rs`. These ignored
tests are the tools for working on the sound:

```bash
# The clean amps against the rest: distortion of a held note per Gain, crest of clean playing,
# bite, air, lows, loudness, what two notes played together make between them (chord blur) at the
# reference level and 6 dB hotter, and peaks
cargo test -p amp --release clean_report -- --ignored --nocapture

# Levels, distortion, aliasing, tightness, gate, drive, cabinet, effects, latency and CPU time.
# Run before and after a DSP change and compare.
cargo test -p amp --release amp_report -- --ignored --nocapture

# How loud each amp is heard (LUFS), its peaks and crest per Gain, for guitars recorded from
# -36 to -12 dBFS RMS, and the distortion of a held note at those levels.
cargo test -p amp --release loudness_report -- --ignored --nocapture

# Træ, the acoustic amp (#400), against Klar: loudness, peaks and crest per Gain, what it adds
# between two held notes per level (the blur of a chord), its response to a quiet sine, aliasing
cargo test -p amp --release trae_report -- --ignored --nocapture

# The four amps of #263 against the others: total preamp gain, loudness, octave bands, the bite
# at 3 to 5 kHz, the fizz above 7 kHz, aliasing, palm mutes. render_voices writes each next to its
# nearest old amp at Gain 3, 5 and 7 as WAV files in target/renders (voices_*.wav)
cargo test -p amp --release voices_report -- --ignored --nocapture
cargo test -p amp --release render_voices -- --ignored

# One number per amp, Gain and pedal that changes with any bit of the output. Save the lines
# before and after a change that must leave a sound as it is, and compare them.
cargo test -p amp --release fingerprint_report -- --ignored --nocapture

# The same for every effect as a block on a slot (#371), on each side, switched off and on again and
# crossed to the other side: one number per kind, side and rate. Save before and after, compare.
cargo test -p amp --release slot_fingerprint_report -- --ignored --nocapture

# Delay and reverb by themselves: echo times, decay times, density, width, cost.
cargo test -p amp --release effects_report -- --ignored --nocapture

# Time per stage of the chain, and level, peak and tightness of every factory preset.
cargo test -p amp --release stage_report -- --ignored --nocapture
cargo test -p amp --release preset_report -- --ignored --nocapture

# How every preset's gate treats ringing notes, quiet playing, palm mutes and hiss.
cargo test -p amp --release preset_gate_report -- --ignored --nocapture

# Compressor, fuzz and modulation pedals by themselves: what they do per setting, and their cost.
cargo test -p amp --release comp_report -- --ignored --nocapture

# Pitch shifter pedal: level of the shifted note, latency per Tracking, and cost, with the pedal
# off and at -12, 0 and +12 semitones.
cargo test -p amp --release pitch_report -- --ignored --nocapture

# EQ pedal: gain per band at the dials' ends and middle, Mid across Freq, the largest step of a
# sweep of the dials against the signal's own, and cost on either side of the amp.
cargo test -p amp --release eq_report -- --ignored --nocapture

# Wah pedal: peak, width and height of the response at the rocker's heel, middle and toe and
# three Bite settings, the level through the sweep, Auto mode's opening and return, and cost.
cargo test -p amp --release wah_report -- --ignored --nocapture
cargo test -p amp --release fuzz_report -- --ignored --nocapture
cargo test -p amp --release mods_report -- --ignored --nocapture

# The metronome's three sounds, in 4/4 and 6/8, at -12 and 0 dB, as WAV files in
# target/renders (amp_metronome_*.wav), with their peaks: to listen to the downbeat and the level
cargo test -p amp --release render_metronome -- --ignored --nocapture

# Tuner: error in cents per note and signal, time to the first reading, drift, cost.
cargo test -p amp --release tuner_report -- --ignored --nocapture

# The player's cabinets: cost per length and rate, latency, what loading does to a response.
cargo test -p amp --release user_cab_report -- --ignored --nocapture

# Long cabinets: direct FIR against the tail, per length and rate; error, evenness, swaps.
cargo test -p amp --release convolver_report -- --ignored --nocapture

# How far apart the built-in cabinets are: octave bands of a guitar through Klar and
# Torden and each one, and the difference of every pair. render_speakers writes the same
# as WAV files in target/renders (amp_cab_*.wav), to hear them side by side.
cargo test -p amp --release speaker_report -- --ignored --nocapture
cargo test -p amp --release render_speakers -- --ignored

# What makes a sound clear or blurred: octave bands of a guitar, and how much of two notes a
# major third apart lies between their harmonics (what a clipper adds to a chord), for each amp
# at Gain 3, 5 and 7 with and without its cabinet, and for Klar, Brøl and Torden through each
# built-in cabinet. A cabinet only re-weights what the amp made: it moves the figure by a few dB.
cargo test -p amp --release clarity_report -- --ignored --nocapture

# The amps' own and the built-in cabinets as 48 kHz WAV files in
# target/renders/cabinets (cab_report prints their levels per frequency).
cargo test -p amp --release export_cabinets -- --ignored --nocapture

# A DI guitar through each amp and setting, as WAV files in target/renders (amp_*.wav).
# Optional: AMP_INPUT_WAV=<recording>. render_effects writes the effects alone (amp_fx_*.wav),
# render_presets every preset (amp_preset_*.wav) and all of them in a row, render_comp and
# render_mods those pedals alone (amp_comp_*.wav, amp_mod_*.wav)
cargo test -p amp --release render_wavs -- --ignored
cargo test -p amp --release render_effects -- --ignored
cargo test -p amp --release render_presets -- --ignored
cargo test -p amp --release render_comp -- --ignored
cargo test -p amp --release render_mods -- --ignored
```
