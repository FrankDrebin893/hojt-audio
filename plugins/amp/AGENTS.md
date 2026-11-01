# Guitar Amp

Loaded when working in `plugins/amp`. The suite-wide rules are in the root `AGENTS.md`; the
reasons behind the amp's design are in `DESIGN.md` next to this file; open work is the
`plugin:amp` issues.

A guitar amp with a full signal chain, all our own designs: own names, no real makers' names,
trademarks or circuit names anywhere (code, comments, docs, commit messages). Six amps, each
with its own cabinet: Klar (clean, glassy), Brøl (mid-forward crunch), Torden (modern tight high
gain), Glød (warm edge of breakup, between Klar and Brøl), Sang (a smooth, compressed singing
lead) and Drøn (a big, loose, dark amp with the low end let through). They share the six dials.

- `src/lib.rs` - Plugin entry point, parameters. Reads them once per block into `AmpSettings`
- `src/chain.rs` - `AmpChain`: the whole chain, dial smoothing, bypass crossfade, amp switching
- `src/gate.rs`, `src/drive.rs` - Noise gate; overdrive pedal
- `src/fuzz.rs` - Fuzz pedal: clipper with a bias that moves with the playing, Tone, its level
  matched inside
- `src/amp/` - `model.rs` (`Amp` and one `AmpModel` of constants per amp; `SPEAKERS`, the
  built-in cabinets), `preamp.rs`,
  `tonestack.rs`, `poweramp.rs`
- `src/cab.rs` - Cabinet: impulse response designed in code at 48 kHz and resampled for other
  rates, direct FIR; a player's response of 641 taps or more keeps its first two blocks as a
  direct FIR and runs the rest through the `Tail`; Mic and Resonance filters behind it (on the frequencies of the built-in cabinet that plays, or on `NEUTRAL_CAB` while a file is picked)
- `src/user_cab.rs` - The player's own cabinets: folder, WAV reading, resampler, trim, cut, level
- `src/cab_stage.rs` - `CabStage` (lock-free hand-off of a cabinet to the audio thread) and
  `CabLoader` (reads the chosen file on a background task)
- `src/delay.rs`, `src/reverb.rs` - Stereo delay; stereo reverb (8-line feedback delay network)
- `src/comp.rs` - Compressor pedal
- `src/pitch.rs` - Pitch shifter pedal: `suite_common::dsp::pitch::PitchShifter` (the Pitch Shifter
  plugin's algorithm) as a stage in front of the amp, with its switch fade
- `src/mods/` - `chorus.rs`, `flanger.rs`, `phaser.rs`, `tremolo.rs`, with `lfo.rs` and `line.rs`
  (the modulated delay line of chorus and flanger)
- `src/order.rs` - `ChainOrder`: which pedals stand in front of the amp and which behind the
  cabinet, in what order; the rules, and `StoredOrder` (saved as `chain-order`)
- `src/dsp/` - `filters.rs` (f64 biquads), `oversample.rs` (minimum-phase IIR half-bands),
  `shaper.rs` (antialiased clippers), `resample.rs` (windowed-sinc resampler for cabinets),
  `pair.rs` (two samples' divisions and square roots as one SSE2 pair), `fft.rs` (radix-2 real
  FFT), `partitioned.rs` (`Tail`: the late part of a long impulse response in FFT partitions)
- `src/presets.rs` - Factory presets as plain data, and `preset_report`
- `src/user_presets.rs` - The player's own presets: one text file per preset in
  `Documents/Hojt Audio/Presets` (or `HOJT_PRESETS_DIR`), and the banks they are kept in,
  which are the factory presets, that folder itself and the folders the player made in it:
  read, written, listed and stepped through
- `src/dial_memory.rs` - Each amp's six dial positions, kept by the editor and saved with the
  project (`amp-dials`)
- `src/lamps.rs` - Gate-open and clip lamps and the input and output level meters: what the
  chain tells the editor, and how long the editor shows it
- `src/tuner.rs` - Tuner: pitch detector on the mono input, `TunerReading` (one atomic shared
  with the editor), `Note`
- `src/editor.rs` - egui GUI (2747 x 650 window): the oxblood strip across the top with the
  Bank and Preset displays, the list they drop open, or the tuner display in its left wing and
  Output, meters, Clip, On and Tuner in its right; the stack (the head of the amp that is
  picked, with its nameplate and dials, the cabinet with its control plate); each of the eleven
  pedals in its own paint with its own art
- `src/looks.rs` - What each amp's head looks like: a `Look` (its `HeadStyle` and knob cap) per
  amp, in the order of `Amp::ALL`. The sound stays in `amp/model.rs`
- `src/board.rs` - The line under the strip in the chain's order: the pedals and the stack,
  the couplers and patch cables between them, and dragging a pedal to another place

Signal flow:

1. Input (a stereo input is averaged to mono), input gain, gate. The gate's detector reads the
   input before the input gain; no lookahead. The tuner, when on, hears the same input in front
   of the input gain and the gate
2. One oversampled region: the pedals in front of the amp in their order, preamp stages, tone
   stack, power amp. 4x at 44.1 and 48 kHz, 2x from 88.2 kHz on. The clippers are antialiased
   (antiderivative method; second order where the gain is highest)
3. Cabinet: a short impulse response (5 to 10 ms) with the speaker resonance as a filter behind
   it, then the Mic and Resonance dials, DC blocker
4. Two channels from here: the pedals behind the cabinet in their order. Delay and reverb add
   wet to an untouched dry signal, smooth their own settings, ring out when switched off and
   cost nothing once idle. What each gives out is rounded off under full scale (`soft_limit`,
   left alone below 0.7)
5. Output level, safety clip

How it behaves:

- Amps differ only by constants in `amp/model.rs`. Tune there, never with `match amp` in the DSP
- Dials are read every 32 samples counted across host blocks, so the output does not depend on
  the host's block size
- Switching amp fades the amp out over 4.5 ms, reconfigures it at silence and fades it back in
- Bypass crossfades to the untouched input in 10 ms and drops the effects' tails
- With every pedal off and the cabinet dials at 5.0 the chain is bit-identical to the amp alone
- Latency is 0.1 to 0.3 ms and nothing is reported to the host
- A mono host layout gets the left channel of the effects
- `process` returns `KeepAlive` while a delay or reverb tail rings. VST3 hosts are told the tail
  is infinite from the start (`VST3_INFINITE_TAIL`), as are those of the pad and the bells
- Tuner on gives the amp silence and turns its output down in 10 ms (exactly silent, tails
  included); off brings it back as fast. Bypass still passes the input while tuning. The tuner
  costs nothing when off and under 3 us in its worst 64-sample block when on
- The tuner reads 48 Hz to 1420 Hz against A = 440 Hz (`REFERENCE_A_HZ`), a new reading every
  30 ms, none for silence, noise, hum or chords. A note is shown from a pluck on (a rise of
  12 dB within 50 ms; 6 dB to change notes or pluck again), and let go after a second if it
  does not fade like a string. Steady hum (50 or 60 Hz and its 2nd and 3rd harmonics, -70 dBFS
  or more) heard at rest, with nothing struck for 1.5 s, switches on notches for it in front
  of the detector, which pull a string within 2 Hz of them by a few cents; without hum they
  are off and the readings are unchanged. A loaded
  project never opens with the tuner on
- Besides its own cabinet an amp can play through any of the ten built-in cabinets
  (`SPEAKERS` in `amp/model.rs`: `1X10 SMALL`, `1X12 OPEN`, `2X12 OPEN`, `4X10 CRISP`,
  `4X12 MID`, `4X12 TIGHT`, `1X15 DEEP`, `1X8 TINY`, `2X12 PUNCH`, `4X12 HEAVY`; designs of their own, not the amps'), picked on the
  cabinet's control plate after `OWN`. A built-in cabinet is stored as `speaker:<id>` in the cabinet choice;
  the ids are frozen (a test holds them), the order may change. One this build does not have
  plays the amp's own and is shown with `?`. Picking the cabinet that already plays changes
  nothing, to the bit. The chain designs them all when the sample rate is set
- Besides a built-in cabinet an amp can play through a WAV impulse response from
  `Documents/Hojt Audio/Cabinets` (or the folder in `HOJT_CABINETS_DIR`), picked on the cabinet.
  Mono or the left channel of stereo, 8/16/24/32-bit integer or 32/64-bit float, any rate; resampled,
  trimmed so the sound arrives within 0.2 ms, cut to 40 ms, levelled like the built-in ones.
  Latency stays the same
- The cabinet choice is a persisted file name or built-in id, not a parameter and not part of
  a preset. It stays when another amp is picked. A missing or unusable file plays the amp's own
  cabinet
- Files are read on a background task (`initialize` only asks for one, which the first block hands to the host's background thread), never on the audio thread; the taps
  reach it through `CabStage` without allocating, freeing or waiting
- The folder is created the first time the stepper is used, not by loading the plugin
- A 40 ms cabinet adds about 0.1 % of a core against the amp's own at any rate. Its late part is
  convolved in FFT partitions whose work is spread over the samples of a block: no added latency,
  the worst block as with the amp's own cabinet, output independent of the host's block size
- A swap to a long cabinet of the player's own starts its crossfade once its late part is there:
  about the length of the response after a short or built-in cabinet, two blocks after another
  long one. The cabinet before plays on meanwhile
- The clippers work on two samples at a time (`dsp/pair.rs`), the same to the bit as one by one.
  That path is only compiled in optimised x86-64 builds: run `cargo test -p amp --release` too
  when touching the clippers
- `hound` is the amp's one dependency besides nih-plug; it reads the WAV files
- Each amp remembers its own six dials: the nameplate on the head stores them under the amp that
  is left and sets them to what the amp that is picked had (defaults if never left). The dials
  are still six shared parameters. A host that changes Amp itself leaves the dials alone; a
  preset's dials become its amp's memory
- An amp that is switched to starts at the dials' target, at the silent sample of the switch
- A preset load (factory or the player's) and an amp switch are told to the host as one edit
  (`presets::set_together`): every gesture begun, then every value set, then every gesture
  ended. REAPER makes that one undo point, and one undo puts the whole sound back, the preset
  display's name and the pedal order with it (checked by hand in REAPER, #16). A host that
  keeps one undo step per gesture still makes one per parameter
- The Gate pedal's `OPEN` lamp is lit while the gate is on and lets the guitar through; `CLIP`
  beside Output is lit for 150 ms after the output went over the safety clip's knee
- Two level meters on the strip (`level_meter`): `INPUT` in the left wing shows the input before
  the input gain, `OUTPUT` by the clip lamp the output in front of the safety clip. They take a
  peak at once and fall 60 dB a second; the window is redrawn for them only while they move,
  30 times a second at most
- The delay's tail is cut at -72 dBFS, and repeats under -60 dBFS fade faster, so a 1000 ms
  delay at full feedback goes idle in about 37 s. Neither effect at its maximum pushes the
  output into the safety clip
- While the drive pedal is on, the second stage of Brøl and of Torden clips at twice the rate
  (`StageModel.finer_when_driven`) and Klar's power stage turns itself down under a heavy load
  (`PowerModel.driven_sag`), so no setting of the pedal aliases above -70 dB. With the pedal
  off the output is the same to the bit
- The amps are matched by loudness (BS.1770, `test_util::loudness_lufs`), not RMS, for a guitar
  recorded at -30 dBFS RMS (`REFERENCE_DI_DB`), its peaks around -12 dBFS: every factory
  preset is heard at -13 LUFS within 2 LU (Kobber -15.4), leads highest, as loud as the drums
  and the bass (#131). A clean
  amp follows the guitar dB for dB and one that clips hardly does, so they can only meet at
  one level: 6 dB quieter the clean settings are 2 to 4 LU under the others, 6 dB hotter as
  much over. The Input knob is what moves a guitar to that level
- Klar's power stage holds its peaks (`PowerModel.clean_to`): straight up to a knee, so a note
  held at -30 dBFS RMS has no distortion (under -60 dB), and rounding off the attacks that go
  over. Chords from a guitar 6 dB over the reference stay under the safety clip's knee on
  every preset; 12 dB over (-18 dBFS RMS) only on most of the presets that clip
  (`presets::HOLD_HOT_GUITAR`), and the others touch it: the Clip lamp says to turn Input
  down. Brøl below Gain 5 is made up in front of
  its power stage for the same reason
- The built-in cabinets are designed at 48 kHz and resampled for every other rate (within
  0.05 dB of each other from 60 Hz to 10 kHz). A rate is designed once and kept
- `lib.rs` takes every parameter default and range from `AmpSettings::default` and the limits in
  the DSP modules; a test pins the values
- A preset holds everything except Bypass, Input and Output, so every preset must fit at
  Output 0 dB (a test checks peak and level of each). It holds the pedal order too. New
  presets go at the end: a project remembers its preset by its number
- The player's own presets (`user_presets.rs`) are `.preset` text files in
  `Documents/Hojt Audio/Presets` (or the folder in `HOJT_PRESETS_DIR`), one `id = value` line
  per parameter by its frozen id, then `order` and `cabinet`. They hold what a factory preset
  holds and the microphone and cabinet too. A file is read forgivingly: unknown lines are
  skipped and what it does not name gets its default. A test holds every parameter to being
  in a preset or left alone with Bypass, Input, Output and the Tuner
- Every preset is in a bank, and the Bank display on the strip says which one is showing:
  `FACTORY` (the built-in presets), `MINE` (the files loose in the presets folder, where every
  preset was kept before there were banks) or one of the folders the player made in it, in
  name order. Only the folders directly in the presets folder are banks, and what is deeper is
  left alone. The bank showing is persisted as `preset-bank`: empty for the factory bank, `/`
  for the presets folder itself and `/<folder>` for a folder in it, checked like a file name so
  it is never a path and never `..`. A project saved before there were banks has a preset of
  the player's and no bank, and opens on `MINE`
- The Preset display's buttons step through the bank showing and come round in it without ever
  leaving it; in a bank with nothing in it they say `NO PRESETS` for 1.5 s. The tape of either
  display drops the list open: the banks in one column and the presets of the bank clicked in
  the other. The bank showing changes when a preset is picked out of another bank, when the
  Bank buttons step and when a bank is made; the sound then stays as it stands, and with
  nothing loaded out of the bank the display falls back to the name of the preset the sound
  came from
- `+ NEW BANK`, at the end of the banks, makes one. Clicked, its tape takes a name; enter keeps
  it, escape drops it, and a second click on the tape keeps it as it stands, so a bank can be
  made with the mouse alone. A bank nobody named is `Bank 1`, `Bank 2` and so on, the first
  that is free, renamed in the folder afterwards. A name is filtered like a file name, and one
  that already stands as a folder is simply that bank, with everything still in it
- Save writes into the bank showing and always asks for a name: it makes the Preset display's
  tape take one, which starts as the name of the preset loaded out of the bank showing (empty
  with none loaded). Enter or a second press of Save keeps it, escape drops it and writes
  nothing, stepping or opening a list drops it too. The name kept as it stands writes over the
  preset loaded; any other is the file name, `<name>.preset`, with a number added when it is
  taken. A name with nothing left once the characters a file name cannot hold are taken out is
  one nobody gave, and is `<name> 2.preset` (the next free number) named after the preset the
  sound came from, as before. Nothing is written into `FACTORY`: a sound saved there goes into `MINE`, which is the
  bank showing from then on. The file name of the one loaded last is persisted as `user-preset`
  (empty after a factory preset, and after the bank changed); renaming, moving from one bank to
  another, copying and deleting is done in the folder, which a double-click on the Preset
  caption opens — the folder of the bank showing
- The pedal order (`order.rs`): Gate always first, amp and cabinet one block in the middle.
  Drive and Pitch only in front of the amp; Delay and Reverb only behind the cabinet; Fuzz, Comp,
  Chorus, Phaser, Flanger and Tremolo on either side. Default: Pitch, Fuzz, Comp, Drive, Phaser, amp,
  Chorus, Flanger, Tremolo, Delay, Reverb, which with the new pedals off is the chain as it
  was, to the bit. A project saved before a pedal existed gets it where the default has it
- In front of the amp a pedal runs on one channel at the oversampled rate, behind the cabinet
  on two at the host's rate (chorus, flanger and phaser a quarter turn apart left and right).
  Either way without latency (but for the pitch shifter, below). The fuzz is the one exception behind the cabinet: it raises the
  rate itself for its clipping (as the amp does: 4x, 2x from 88.2 kHz), which delays it by
  the oversampler's fraction of a millisecond while it is on
- The pitch shifter (`pitch.rs`, the algorithm of the Pitch Shifter plugin in
  `suite_common::dsp::pitch`) stands in front of the amp only, on one channel at the amp's
  oversampled rate, and is first in front of the amp by default (the gate stays in front of
  everything: it listens to the guitar as it arrives). Its buffers (about 1 MB) are made with
  the chain and cleared when the pedal wakes, never resized: the `no_alloc` tests hold it.
  Off it is skipped and the signal is bit-identical; switching crossfades in 20 ms, and a
  pedal that has faded out is idle (`is_idle`, so it never keeps the host awake). On it adds
  latency: a shift of 0 passes the input 2 samples of the oversampled rate late, and a shift
  up or down lags the input by about a pitch period of the note, never more than Tracking (a
  plucked 110 Hz string starts 4 ms later at -12, 7 ms at +12 with Tracking 15 ms, 25 ms at
  +12 with 50 ms; the dry part of Blend is not delayed). Nothing is reported to the host. Flat
  to 0.0 dB: the note that comes out is as loud as the amp playing it itself. Above 192 kHz
  of oversampled rate the buffer is shorter than 50 ms (42 ms at a host's 192 kHz). Cost: about
  0.4 to 4 % of a core over the amp at 48 kHz (shift 0: under 0.5 %), with a splice every
  period costing up to a tenth of a block; the report is `pitch_report`
- The order is saved with the project (`chain-order`, a list of pedal ids with `amp` between
  the sides), not a parameter and not automatable. The chain reads it with the dials: the
  output fades to silence in 4.5 ms, the pedals change places, it fades back. A pedal that
  changes sides starts from rest. A list that is damaged or from another build is put right,
  never refused
- The pedals were matched for a guitar at -24 dBFS RMS (`PEDAL_DI_DB`), the amps' reference
  before #131. The compressor's Level at 5 is as loud as the pedal off (within 0.3 LU at that
  level); Sustain 10 adds at most 20 dB to what is quiet, hiss included
- The fuzz has no Level knob: it is as loud as the pedal off for a guitar at `PEDAL_DI_DB`,
  within 0.3 LU at any Fuzz and Tone (within 1 LU held by a test). Everything that is
  not a harmonic is at least 80 dB under the note at any setting and rate (about -92 dB at
  44.1 kHz flat out), and flat out in front of any amp at full gain at most -72 dB, as with
  the drive. It costs about 0.6 % of a core at 48 kHz in front of the amp and 1.2 % behind
  the cabinet, where it runs two channels and its own oversampling
- Cost with everything on but the fuzz (Torden; the fuzz is the line above): about 2.0 % of
  a core at 48 kHz, 2.6 % at 96 kHz, 5.5 % at 192 kHz, on a fast core. The cores of a machine differ (1.33 times on the development
  machine): the reports pin their thread (`test_util::pin_thread`)

| Parameter (id) | Range | Default | Description |
|----------------|-------|---------|-------------|
| Bypass (`bypass`) | on / off | off | The On switch on the strip |
| Input (`in_gain`) | -24 to +24 dB | 0 dB | Gain in front of everything except the gate's detector |
| Gate (`gate_on`) | on / off | on | Noise gate on the input |
| Gate Threshold (`gate_thresh`) | -80 to -20 dB | -60 dB | Input level that opens the gate (closes 3 dB lower) |
| Gate Release (`gate_release`) | 20-500 ms | 100 ms | Time to close after a 40 ms hold |
| Comp (`comp_on`) | on / off | off | Compressor pedal |
| Comp Sustain (`comp_sustain`) | 0.0-10.0 | 5.0 | How much it evens out: threshold and ratio together |
| Comp Level (`comp_level`) | 0.0-10.0 | 5.0 | The pedal's output, -12 to +12 dB around as loud as off |
| Drive (`drive_on`) | on / off | off | Overdrive pedal in front of the amp |
| Drive Gain (`drive_gain`) | 0.0-10.0 | 3.0 | Amount of clipping in the pedal |
| Drive Tone (`drive_tone`) | 0.0-10.0 | 5.0 | The pedal's top end, 1.5 to 7 kHz |
| Drive Level (`drive_level`) | 0.0-10.0 | 5.0 | The pedal's output, -20 to +20 dB into the amp |
| Fuzz (`fuzz_on`) | on / off | off | Fuzz pedal, either side of the amp |
| Fuzz Amount (`fuzz_fuzz`) | 0.0-10.0 | 5.0 | Gain into the clipper, 26 to 56 dB |
| Pitch (`pitch_on`) | on / off | off | Pitch shifter pedal, in front of the amp only |
| Pitch Semitones (`pitch_semitones`) | -12 to +12 | 0 | Whole semitones: the guitar down an octave to up one. Shown `-5`, `0`, `+7`; double-click returns to 0 |
| Pitch Blend (`pitch_blend`) | 0-100 % | 100 % | The guitar as it comes in (0) to the shifted guitar alone (100). The dry part is not delayed |
| Pitch Tracking (`pitch_latency`) | 2-50 ms | 15 ms | The most the shifted guitar may lag behind the input: the longest period that splices cleanly (a low E is 12 ms, a power chord on it about 24 ms) |
| Fuzz Tone (`fuzz_tone`) | 0.0-10.0 | 5.0 | Dark (a low-pass at 650 Hz) to bright (a high-pass at 1.1 kHz), the mids scooped in between |
| Amp (`amp`) | Klar / Brøl / Torden / Glød / Sang / Drøn | Brøl | Amp and its cabinet. Variant ids `klar`, `brol`, `torden`, `glod`, `sang`, `dron`. New amps go at the end: a project remembers its amp by its number |
| Gain, Bass, Mid, Treble, Presence, Master (`gain`, `bass`, `mid`, `treble`, `presence`, `master`) | 0.0-10.0 | 5.0 | The amp's dials |
| Chorus, Phaser, Flanger, Tremolo (`chorus_on`, `phaser_on`, `flanger_on`, `trem_on`) | on / off | off | Modulation pedals |
| Chorus Rate (`chorus_rate`) | 0.1-6 Hz | 0.8 Hz | Speed of the sweep, 7 to 12 ms |
| Phaser Rate (`phaser_rate`) | 0.1-8 Hz | 0.5 Hz | Speed of the sweep, 200 Hz to 1.6 kHz |
| Flanger Rate (`flanger_rate`) | 0.05-5 Hz | 0.3 Hz | Speed of the sweep, 0.5 to 6 ms |
| Tremolo Rate (`trem_rate`) | 0.5-12 Hz | 5 Hz | Speed of the volume wobble |
| Depth (`chorus_depth`, `phaser_depth`, `flanger_depth`, `trem_depth`) | 0-100 % | 50 % | Width of the sweep; for chorus and flanger also the level of the voice |
| Cabinet (`cab_on`) | on / off | on | Off passes the amp at about the same level, for a cabinet elsewhere |
| Cab Mic (`cab_mic`) | 0.0-10.0 | 5.0 | Darker to brighter microphone position |
| Cab Mic Type (`cab_mic_type`) | Dynamic, Ribbon, Condenser, Close, Air | Dynamic | Which microphone hears the cabinet (`MicType` in `cab.rs`): Dynamic leaves it as designed; Close is dark with the lows up, Air bright with the lows let go. New ones go at the end, a project stores the position |
| Cab Resonance (`cab_res`) | 0.0-10.0 | 5.0 | Less or more low thump |
| Delay (`delay_on`) | on / off | off | Stereo delay behind the cabinet |
| Delay Time (`delay_time`) | 20-1000 ms | 350 ms | Time between repeats |
| Delay Feedback (`delay_feedback`) | 0-90 % | 35 % | Level of each repeat against the one before |
| Delay Mix (`delay_mix`) | 0-100 % | 25 % | Level of the repeats |
| Delay Sync (`delay_sync`) | on / off | off | The repeats follow the song's tempo: the Time knob picks Delay Note, and the time is `60000 / bpm` times the note's beats, kept to 20-1000 ms. With no tempo from the host, the Time knob's ms. Factory presets set it off |
| Delay Note (`delay_note`) | 1/4, 1/8 dotted, 1/8, 1/8 triplet, 1/16 | 1/8 dotted | The note value while Sync is on (`suite_common::dsp::NoteValue`). Variant ids `quarter`, `dotted_eighth`, `eighth`, `eighth_triplet`, `sixteenth`; a new one goes at the end |
| Reverb (`reverb_on`) | on / off | off | Stereo reverb behind the delay |
| Reverb Decay (`reverb_decay`) | 0.3-6.0 s | 1.5 s | Time for the tail to fall by 60 dB |
| Reverb Mix (`reverb_mix`) | 0-100 % | 20 % | Level of the tail |
| Output (`out_level`) | -30 to +6 dB | 0 dB | Output level, in front of the safety clip |
| Tuner (`tuner`) | on / off | off | Silent tuning. Not automatable, not in presets, not changed when a project is loaded |

Dials are stored 0.0-1.0 and shown as 0.0-10.0.

Amp tests sit next to the code. The ones for the whole chain are in `chain.rs`. These ignored
tests are the tools for working on the sound:

```bash
# Levels, distortion, aliasing, tightness, gate, drive, cabinet, effects, latency and CPU time.
# Run before and after a DSP change and compare.
cargo test -p amp --release amp_report -- --ignored --nocapture

# How loud each amp is heard (LUFS), its peaks and crest per Gain, for guitars recorded from
# -36 to -12 dBFS RMS, and the distortion of a held note at those levels.
cargo test -p amp --release loudness_report -- --ignored --nocapture

# One number per amp, Gain and pedal that changes with any bit of the output. Save the lines
# before and after a change that must leave a sound as it is, and compare them.
cargo test -p amp --release fingerprint_report -- --ignored --nocapture

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
cargo test -p amp --release fuzz_report -- --ignored --nocapture
cargo test -p amp --release mods_report -- --ignored --nocapture

# Tuner: error in cents per note and signal, time to the first reading, drift, cost.
cargo test -p amp --release tuner_report -- --ignored --nocapture

# The player's cabinets: cost per length and rate, latency, what loading does to a response.
cargo test -p amp --release user_cab_report -- --ignored --nocapture

# Long cabinets: direct FIR against the tail, per length and rate; error, evenness, swaps.
cargo test -p amp --release convolver_report -- --ignored --nocapture

# How far apart the ten built-in cabinets are: octave bands of a guitar through Klar and
# Torden and each one, and the difference of every pair. render_speakers writes the same
# as WAV files in target/renders (amp_cab_*.wav), to hear them side by side.
cargo test -p amp --release speaker_report -- --ignored --nocapture
cargo test -p amp --release render_speakers -- --ignored

# The amps' own and the ten built-in cabinets as 48 kHz WAV files in
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

## Look: the rig

The style guide (`crates/suite_common/src/ui/STYLE.md`) applies; these are the rules for a window of several pedals, which only the amp has.

- **Rigs:** a plugin made of several components (the Guitar Amp) is the one exception to one
  pedal per plugin. Its window is a `rig`: a `strip` across the top, a shallow enclosure in the
  plugin's paint that carries the jack captions, model number, logo and art (no name band), with a wing at each end for the plugin's own controls; and under it one line in signal
  order: the gate, the pedals in front of the amp, the stack (the head standing on the cabinet,
  `STACK_WIDTH` wide), the pedals behind the cabinet. The pedals stand on the floor beside the
  cabinet, as tall as it. Each mini pedal has a paint of its own. On a strip, a head and mini
  pedals the switches are `small_footswitch` and the LEDs `rig_led` (an unlit `led` disappears
  on dark paint), beside or above the switch. A mini pedal holds two knobs of radius 24 side by
  side or three of radius 17 in a triangle. Everything else in this guide applies
- **The strip's wings:** the left one holds what comes in and what is picked (input meter,
  Bank, Preset, Save; the tuner display while tuning), the right one what goes out (Output, the
  output meter, Clip, On, Tuner). Its jack captions read `In` at the left and `Stereo Out` at
  the right, arrows left to right: the signal runs along the window. Nothing in a wing reaches
  the band (a test holds it)
- **Head and cabinet:** each amp has a head of its own (`looks.rs`, drawn by `ui::amp_head`),
  which changes the moment another amp is picked:

  | Amp | Covering | Grille | Panel, print | Knobs | Piping |
  |-----|----------|--------|--------------|-------|--------|
  | Klar | ivory, smooth | wheat cloth with a fine gold thread | brushed chrome, ink | chrome skirt | none |
  | Glød | lacquered tweed | oxblood-brown cloth, a gold stripe across it | dark wood, cream | cream chicken-head | none |
  | Brøl | black levant | salt-and-pepper basketweave | brushed brass, ink | gold top | white |
  | Torden | black bronco | perforated steel | black anodised, cream, one red line | black fluted | red |
  | Sang | deep blue smooth | salt-and-pepper cloth | brushed brass, ink | black chicken-head | silver |
  | Drøn | dark green bronco | black felt | dark wood, cream | gold top | none |

  The amp is picked on a `nameplate_stepper` in the middle of the grille: its name on a plate
  of the panel's metal, a chrome button each side. The panel holds, left to right, the input
  jack the cable from the pedals goes into with its `In` caption, the six dials as
  `panel_knob`s (radius 18, caption above, value tape under, in the panel's print and the amp's
  cap), and the jewel pilot lamp, lit while the amp is On. Tests hold every scale, caption and
  tape on the panel and apart, and the nameplate's buttons on the grille under the handle.

  The cabinet (`ui::cabinet`) looks as the cabinet that plays, and changes the moment another
  is picked (`looks::cab_look`, from the cabinet choice, the amp and what the loader says of
  the choice):

  | What plays | Covering | Grille | Speakers | Piping, corners |
  |------------|----------|--------|----------|-----------------|
  | `OWN`, Klar | ivory, smooth | wheat cloth | one 12" | none |
  | `OWN`, Glød | lacquered tweed | oxblood-brown cloth | two 12" | none |
  | `OWN`, Brøl | black levant | basketweave | four 12" | white piping, chrome corners |
  | `OWN`, Torden | black bronco | perforated steel | four 12" | steel corners |
  | `OWN`, Sang | deep blue smooth | salt-and-pepper cloth | two 12" | silver piping |
  | `OWN`, Drøn | dark green bronco | black felt | four 12" | none |
  | `1X10 SMALL` | tan, smooth | wheat cloth | one 10", set high | none |
  | `1X12 OPEN` | cream levant | silver cloth | one 12" | none |
  | `2X12 OPEN` | grey levant | silver cloth | two 12" | none |
  | `4X10 CRISP` | black, smooth | silver cloth | four 10" | none |
  | `4X12 MID` | black levant | basketweave | four 12" | white piping |
  | `4X12 TIGHT` | black bronco | perforated steel | four 12" | none |
  | `1X15 DEEP` | navy levant | black felt | one 15" | none |
  | `1X8 TINY` | walnut tweed | faded brown cloth | one 8", set high | none |
  | `2X12 PUNCH` | green bronco | basketweave | two 12" | none |
  | `4X12 HEAVY` | black, smooth | dark felt | four 12" | none |
  | a WAV file | charcoal, smooth | grey felt | one 12", a shadow behind the felt | none |

  A choice shown with `?` (missing, or a built-in cabinet of another build) or a file shown
  with `!` (unusable) plays the amp's own cabinet and shows its look. Switching the cabinet off
  does not change the look: its LED says so. A new built-in cabinet needs a look in
  `SPEAKER_CABS` (a test walks `SPEAKERS`), and no two built-in looks share covering, grille
  and speakers. Every cabinet is the same size with the same black control plate across its
  top (`CAB_PLATE_HEIGHT`, cream print): the `Speaker` stepper over the `Mic Type` stepper, the
  Mic and Res knobs (radius 17), and its switch with LED and caption above it. Its output jack
  is under the plate, at the height of the pedals' plugs, on a small plate of the same black
  with its `Out` caption, so the caption reads on every cloth
- **Pedal art:** every mini pedal has its own `PAINT_*` and its own `Motif`, all on one theme
  per rig (the Guitar Amp: northern weather and nature). Two coats and a print: a large motif
  in darker tones of the pedal's paint (`theme::shade`) that may run under the controls, with
  no fine detail under a caption, and a few `SILK` lines (2 px) and dots only in the clear
  paint beside the controls. The art keeps 5 px of plain paint to the edge of the face. No
  letters, numbers or cream triangles in it, and nothing that moves. A new paint goes in
  `PEDAL_PAINTS`, where tests hold it to cream print reading on it and to being clearly apart
  from the others; a new motif is drawn in `art.rs` for the layout of its pedal
- **Equal pedals:** every pedal of the row is `PEDAL_WIDTH` (180 px) wide (eleven of them and the stack: the window is 2747 px wide); only the stack has its
  own width. The gap between neighbours is what is left of the window after the margin at each
  end (`GAP` in `board.rs`, a test holds it even and every pedal as wide as the next). A pedal
  with two knobs is either a pair side by side (radius 24) or on a diagonal (`SLIM_LED`,
  `SLIM_KNOBS`, `SLIM_SWITCH`): the LED where a trio's upper left knob is, the two radius-17
  knobs upper right and lower left, as far from the middle as a pair's. The narrow motifs
  (`art.rs`, `NARROW_ART`) stand in the middle of the wider face and their paint runs out to
  both edges. In a row the LEDs and switches line up
- **A board that can be rearranged:** a pedal is moved by dragging it by its title
  (`title_grip`); while it is held it is drawn `lifted` and last, the others glide to make
  room, and it lands in the nearest place it may have. No arrows, menus or lists. A pedal
  that cannot be moved has no grip
- **The signal between the pieces** is drawn on the bench: a `coupler` between two pedals that
  stand side by side, a `patch_cable` from the last pedal in front of the amp into the head's
  input jack and one from the cabinet's output jack to the first pedal behind it. They follow
  a pedal that is dragged; a pedal that is held has no couplers. The stack never leaves its
  place among the pedals, and glides with it when a pedal changes sides. Objects on the bench,
  not print: silk triangles still mean signal direction at the jacks only
- **An order is never a parameter.** Keep it in a `#[persist]` field, as ids, like a preset
  or a file choice
- **The strip's row:** with no name band, the strip's controls stand in one row across it with
  the same gap between neighbours (`STRIP_GAP`): the input meter, the Bank and Preset displays,
  Save, the output knob, the output meter, the Clip lamp, On and Tuner. Each one's place is
  worked from the end of the one before it, and a test holds the gaps even, the controls clear
  of each other and of the edges, and the right end a little further from its edge (the logo
  stands there)
- **Level meters:** a signal level (the amp's input and output) is a `level_meter`, a column
  of lamps on label tape with a `silk_label` caption under it, in a wing of the strip. Same atomics and repaint rule as the signal lamps
- **The Delay's Sync:** the one on/off of the rig that is not a stomp switch. A second
  `small_footswitch` beside the On switch runs off the face, so Sync is a `chrome_button` with
  a `rig_led` above it and its `silk_label` above that, right of the Mix knob where the Gate
  has its lamp (#145). While it is lit the Time knob, in the same place, picks Delay Note and
  its tape shows the note
- **Signal lamps:** a light that follows the sound and not a switch (gate open, output clipping)
  is a `signal_lamp`, with a `silk_label` of size 13 centred 18 px under it, placed beside the
  control it belongs to. An LED above or beside a switch only ever says that the switch is on.
  Lamps are read from atomics the audio thread writes once per block; the editor asks for a
  repaint only when one changes, 20 times a second at most
- **The sound bank's LED display:** the Bank and Preset displays are built-in LED displays
  (`suite_common::ui::led`): a dark recessed window with a dot-matrix readout in amber, the
  unlit dots faintly there, lit pixel by pixel like the rest of `raster` (no image files). It
  is `LED_HEIGHT` high and `led_width(chars)` wide: Bank holds 10 characters, Preset 16. The
  text is capitals from the left, so a name being typed grows to the right, with the `_` caret
  after it; what does not fit is cut. A character without a 5 x 7 glyph of its own is drawn as
  a box, and a test holds that every letter, digit and sign a name can hold has one. The
  tuner's display is the same window
- **Pick one of many (presets):** a `led_stepper` (a stepper on an LED display) with a
  `silk_label` caption above it, in the left
  wing of the strip. Step arrows are
  engraved in chrome buttons; silk triangles on the paint mean signal direction at the jacks only.
  Names have at most 15 characters and are our own words. A trailing `*` on the display means a
  control was moved since loading. The bank the presets come from is picked the same way, on a
  `led_stepper` of 10 characters left of the preset display, its `Bank` caption on the preset
  caption's line and its window on the preset window's line. While the tuner is on neither is drawn
- **A list that drops open** is for the preset banks and nothing else, where stepping through
  every preset one at a time is too slow: everywhere else a choice is stepped through, with no
  dropdowns, lists or pop-ups. It is hardware, not a menu. `drop_panel` drops a dark plate with
  a chrome edge from under the window of a `led_stepper`, its columns under cream `display`
  captions and every row a strip of label tape in `mono` capitals. The row in use carries a lit
  lamp at its left, the row under the pointer lightens, and a column of more than seven rows
  shows seven, with a chrome bar at its right and the wheel to reach the others. It is drawn
  after everything else in the window, in a foreground layer, so nothing behind it answers a
  click while it is open, and it closes on a pick, on a click outside it or on escape. No bar to
  drag, no icons, and no widget of egui's own
- **Typed text** is taken in two places only: the name of a new preset bank, on its row of that
  list, and the name of a new preset, on the Preset display after Save
  (`type_on_tape`, 24 letters, the last 15 shown). Both are `type_into`: the letters on the tape or display with a `_` after them,
  enter to keep it and escape to drop it. Nothing else in the suite is typed, and egui's text widget is never drawn.
  Nothing ever waits for the keyboard: a second click on the row (or a second press of Save) keeps the name as it stands,
  and an empty name is one the player did not give, so the mouse alone is enough
- **Asking for the keyboard.** An editor is a child window of the host's, and on Windows
  nothing gives it the keyboard focus by itself: a window that never asks hears no letters at
  all. The row or tape that takes a name asks egui for the keyboard (`ask_for_keyboard`, on a widget
  that is focusable, or egui takes the focus back in the same frame:
  `request_focus` and a `widget_info` that says it is a field with text), which is what
  egui-baseview's `windows_keyboard_workaround` watches for before it takes the focus for the
  window; the workspace turns that feature on (see the root `Cargo.toml`). Nothing else in the
  suite asks, so a knob or a footswitch never takes the keyboard from the host. The window
  keeps it until the player clicks the host again, which is the price of typing there.
  That is still not enough in REAPER, which takes a key press for a shortcut of its own unless
  the focused window answers `WM_GETDLGCODE` with `DLGC_WANTALLKEYS` (baseview's never does,
  and nih-plug's `IPlugView::on_key_down` says not implemented): `ask_for_keyboard` also calls
  `ui::host_keys::want_all_keys`, which puts that answer in front of the window (#132). The
  standalone does not show the difference, only a host does; try it in REAPER with "Send all
  keyboard input to plug-in" off (its default)
- **Steppers on the cabinet** (the speaker and the microphone): the `silk_label` caption
  above the tape, and the buttons clear of the next knob, caption or LED. The amp's is the
  nameplate on its head, which needs no caption: it says the amp's name
- **Save (a preset):** a `chrome_button` (a stepper's push button with a dot in its cap) right
  of the preset stepper, its `silk_label` caption on the stepper's caption line. It writes the
  sound into the bank showing and the stepper's tape says `SAVED` (or `NOT SAVED`) for 1.5 s.
  Every save takes its name on the stepper's tape first, which starts as the name of the preset
  loaded out of the bank showing: kept as it is, that preset is written over. Renaming later is
  also done in the folder
- **A preset choice is never a parameter.** The editor sets the real parameters through the
  `ParamSetter` and keeps only the index in a `#[persist]` field
- **Pick a file (cabinets):** files are never picked in a dialog. They live in one folder and are
  stepped through in name order with a `stepper` under a `silk_label` caption, after the built-in
  choices: the amp's own (`OWN`), then the built-in cabinets. On the cabinet's control plate
  the tape is 100 px wide and shows at most 10 characters: no extension, capitals, long names as the first 5 and last 4
  with `~` between. `!` in front of the name of a file that cannot be used, `?` in front of the name of one
  that is not there. Stepping on to `OWN` past where the files would be, with none in the folder,
  says `NO FILES` for 1.5 s. Hovering the
  `Speaker` caption names the folder; a double-click on it opens the folder
- **A file choice is never a parameter.** Keep the file name (not the path) in a `#[persist]` field
- **Tuner:** a `small_footswitch` with `rig_led` and a `silk_label` at the end of the strip's
  right wing, beside the On switch. While it is on, a
  `tuner_display` (an LED display, `ui::led`) under its own caption takes the place of the
  preset display: the note name in dots at the left, a needle of 17 lamps, flat to the left
  and sharp to the right, the tall middle lamp lit alone within 2 cents. All in the display's
  amber; no numbers or other colours. The editor repaints continuously only while the tuner is on
