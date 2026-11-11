# Guitar Amp

Loaded when working in `plugins/amp`. The suite-wide rules are in the root `AGENTS.md`; the
reasons behind the amp's design are in `DESIGN.md` next to this file; open work is the
`plugin:amp` issues.

A guitar amp with a full signal chain, all our own designs: own names, no real makers' names,
trademarks or circuit names anywhere (code, comments, docs, commit messages). Ten amps, each
with its own cabinet: Klar (clean, glassy), Brøl (mid-forward crunch), Torden (modern tight high
gain), Glød (warm edge of breakup, between Klar and Brøl), Sang (a smooth, compressed singing
lead), Drøn (a big, loose, dark amp with the low end let through), and, from #263, Knald (thick,
warm, slightly woolly mid-gain), Gnist (its bright, chimey cousin), Skarp (crisp, tight modern high
gain with a bite at 3 kHz and deep sag) and Hvas (dense, hot-rodded high gain, looser low end). They
share the six dials. New amps go at the end of `Amp`: ids are frozen (`test_amp_ids_are_frozen`).

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
- `src/eq.rs` - EQ pedal: Low and High shelves, Mid bell with a sweepable centre (`Eq`, `EqSettings`)
- `src/wah.rs` - Wah pedal: a resonant band-pass (`Wah`, `WahSettings`, `WahMode`), the rocker by
  hand or by the playing (Auto)
- `src/pitch.rs` - Pitch shifter pedal: `suite_common::dsp::pitch::PitchShifter` (the Pitch Shifter
  plugin's algorithm) as a stage in front of the amp, with its switch fade
- `src/mods/` - `chorus.rs`, `flanger.rs`, `phaser.rs`, `tremolo.rs`, with `lfo.rs` and `line.rs`
  (the modulated delay line of chorus and flanger)
- `src/order.rs` - `ChainOrder`: which of the twelve pedals stand in front of the amp and which
  behind the cabinet, in what order, and the rules of where each may stand (none now: every
  pedal stands on either side, #268). It is the view of the chain the board draws and a project
  saves for an older build; the audio reads `layout.rs`
- `src/slots.rs` - The chain model (#252, #266): `Chain` (up to 16 blocks, `EffectKind` and a slot
  each, in two zones, and the amp lanes, the first of which cannot be removed), `StoredChain`
  (the chain as the editor sets it and the audio reads it, lock-free, saved as `chain`; also the
  `Memory` of the values of blocks taken out, by kind), `LegacyOrder` (the older `chain-order`) and
  the 16 `SlotParams` (`slot<n>_on`, `slot<n>_k1` to `_k5`: K = 5, the delay's knobs). Rules:
  a block on the slot of its own pedal (`Chain::is_home`: slot n of `Pedal::ALL`) keeps its values
  in that pedal's parameters, which hosts and old projects know, so automation of them keeps
  working; any other block (a duplicate, anything on slots 13 to 16) keeps them in its slot's
  parameters, which the audio reads (`layout.rs`, `extras.rs`, #268). At most `MAX_PER_KIND` = 2 blocks of a kind (`AddError::TooMany`). A
  state with no `chain` (every project and preset from before) opens as the twelve pedals in the
  order of its `chain-order`; with both, `chain` wins (keys are read in alphabetical order, so
  `LegacyOrder` skips the `chain-order` that follows a `chain`). Slot ids are frozen
- `src/layout.rs` - `Layout`: the line as the audio plays it (#268), made from the chain by
  `Layout::of`: `Voice::Home(pedal)` for a block on its pedal's own slot (the pedal plays from its
  own parameters, as it always did) and `Voice::Slot { kind, slot, rank }` for any other. A pedal
  the chain does not hold is not in it, so it is not played, whatever its switch says
- `src/extras.rs` - The effects that play from a slot's parameters (#268): `BlockSettings` (what
  a block is told, one variant a kind), `Unit` (one effect, made for either side as the pedals
  are) and `Pool` (`MAX_PER_KIND` units of each of the 13 kinds, made with the chain by
  `AmpChain::set_sample_rate`, so putting a block in the chain allocates nothing). Units are
  assigned by the layout (`Pool::follow`): one that is put in or moves to the other side starts
  from rest
- `src/dsp/` - `filters.rs` (f64 biquads), `oversample.rs` (minimum-phase IIR half-bands),
  `shaper.rs` (antialiased clippers), `resample.rs` (windowed-sinc resampler for cabinets),
  `pair.rs` (two samples' divisions and square roots as one SSE2 pair), `fft.rs` (radix-2 real
  FFT), `partitioned.rs` (`Tail`: the late part of a long impulse response in FFT partitions)
- `src/presets.rs` - Factory presets as plain data, and `preset_report`
- `src/user_presets.rs` - The player's own presets: one text file per preset in
  `Documents/Hojt Audio/Presets` (or `HOJT_PRESETS_DIR`), and the banks they are kept in,
  which are the factory presets, that folder itself and the folders the player made in it:
  read, written, listed and stepped through
- `src/second.rs` - The second amp (#177): `Second` (its preamp, tone stack, power amp and
  cabinet, its dial smoothing, its switch and amp-switch fades), `SecondSettings`, `Timing` and
  `BLEND_LAW`. `chain.rs` runs it and mixes it in
- `src/dial_memory.rs` - Each amp's six dial positions, kept by the editor and saved with the
  project (`amp-dials` for the first amp, `amp2-dials` for the second)
- `src/lamps.rs` - Gate-open and clip lamps and the input and output level meters: what the
  chain tells the editor, and how long the editor shows it
- `src/tuner.rs` - Tuner: pitch detector on the mono input, `TunerReading` (one atomic shared
  with the editor), `Note`
- `src/metronome.rs` - Metronome: the click on a clock of its own or the host's (`Metronome`,
  `MetronomeSettings`), its three sounds, `BeatReading` (what the editor's lamp reads) and `TapTempo`
- `src/looper.rs` - Looper (#280): records the finished sound, plays it back and overdubs (`Looper`,
  `LooperSettings`, `LooperControl`, `State`). Milestone 1 of 3: the audio and its commands; no plate yet
- `src/standalone.rs` - The standalone program's settings (`Prefs`, saved as
  `amp-standalone.json` in the system's settings folder or `HOJT_CONFIG_DIR`), the `Host` trait
  `main.rs` implements with cpal to list devices and restart, and the Audio panel's choices. The
  panel itself is the rows of the settings plate in `editor.rs` (`settings_plate`). nih-plug's host opens its streams once from its
  command line, so Apply saves and starts the program again; no host is installed in a DAW and
  the editor then shows no Audio button. Eight rows: Driver, Input, Input channels, Output,
  Output channels, Buffer, Sample rate, Test tone. Channels are the amp's stereo pair (the first
  layout) out of the device's, counted from 1 (`--input-channel`/`--output-channel` of the fork's
  host; a device that changes puts them back to 1; `Host::channels` says how many the device
  has). The test tone is live, not part of Apply: `Host::set_test_tone` ->
  `nih_standalone_set_test_tone` (the fork's host puts a 440 Hz sine, 0.1 of full scale, over the
  main output) and `AudioChoice::close` turns it off. ASIO is the cargo feature `asio`
  (`amp/asio` -> `nih_plug/asio` -> cpal's): Windows only, and the Steinberg ASIO SDK has to be
  at `CPAL_ASIO_DIR` at build time. No CI job builds it (`check_mirrors_ci` wants every cargo
  step of CI in `check.sh`, which cannot reach the SDK): build it by hand on Windows,
  `cargo run -p amp --features asio`. Notes from trying it on a Focusrite: the driver takes the buffer size set in its own
  control panel (the fork brings the size asked for into the driver's range), the panel's
  device list and channel counts are read once before the streams open (`AsioSnapshot` in
  `main.rs`: asking a running ASIO driver hangs the window), and Apply starts the program again
  from a new process and exits at once, since closing the window with ASIO streams open hangs
  WASAPI's shared mode hands the output callback more frames than the period asked for (528 against 512)
  and the count can change; the fork's cpal backend walks such a callback in chunks of at most the period (#250).
  The ASIO path is untouched by that
- The exe carries the HØJT badge (`assets/amp.ico`, `assets/amp.rc`, `build.rs`) and
  `give_window_the_icon` in `main.rs` sets it on the window, which the framework makes without one
- `src/editor.rs` - egui GUI (the signal line in a layer 584 high and as wide as the chain, 3411 for today's twelve pedals; opened scaled to the screen up to 3411 x 584 or 1876 x 321 on a 1920 x 1080 screen, resizable freely from 1301 x 293 by its corner handle, scaled to fit between 0.5 and 1.0 and scrolled sideways below that, `Fit`): the slim oxblood strip
  across the top (64 px) with the input (meter and Input gain), the preset display that opens
  the bank and preset plate, or the tuner display in its place, the metronome, tuner and
  settings icons in the middle, and the output (knob, meter, Clip) with the model and logo at
  its right; the plates that open under it (`View.plate`: presets, metronome, settings); the stack (two lanes, one per amp: its head
  with nameplate and dials, and the cabinet it plays through beside it; the plate where they meet
  with the Blend fader); each of the thirteen pedals in its own paint with its own art
- `src/looks.rs` - What each amp's head looks like: a `Look` (its `HeadStyle` and knob cap) per
  amp, in the order of `Amp::ALL`. The sound stays in `amp/model.rs`
- `src/board.rs` - The signal line under the strip, from the chain model (`Line`): the gate, the
  blocks in front of the amps, the stack, the blocks behind, each a `Piece`, the couplers and
  patch cables between them, and dragging a block by its title to another place (`Chain::move_block`), the plus in each gap (hover) that opens the bank, the dashed place a dragged card opens, the cross on a block's corner that takes it out, and the two dashed sides of an empty chain (#272)
- `src/bank.rs` - The effects bank (#272): the sheet that rises from the bottom of the window over the line
  (`ui::sheet`; `Sheet` is held by the editor's `View` as `effects`: one open at most, and opening it closes
  a plate and the other way round). A card for each of twelve effects (the Gate is the line's first piece and
  has none) with how many of it are in the chain, any number of times. A click on a card adds it in the gap the
  bank was opened from (and moves that gap on, so the next card follows it); a card dragged out opens a place in
  the nearest gap of the line (`Line::drop_target`) and lands there when it is let go over the line, over the
  sheet it goes back; the cross on a block (`Pedals::take_out`, `StoredChain::take_out`) opens the bank with a
  note that its settings are kept and an Undo. What plays: the audio plays the pedal on its own slot, so a
  second block of a kind, or one off its pedal's slot, is drawn, kept and saved but silent until #268's last
  piece; taking out a pedal on its own slot switches its parameter off (`Chain::order` puts the pedal back, so
  only its switch keeps it silent) and putting it in again brings the values and the switch back
  (kept by kind, `Memory`). The plus under the amps (`Chain::add_amp`) shows while the chain has fewer lanes than `MAX_AMPS` (two: the chain is limited to two amps, by the engineer's decision on #269 after four amps cost 7.8 to 9.0 % of a core at Gain 10, over the 8 % limit); the stack always draws both lanes, so today it only shows for a chain saved with one, and amp lanes have no cross yet (amp 1 never)

Signal flow:

1. Input (a stereo input is averaged to mono), input gain, gate. The gate's detector reads the
   input before the input gain; no lookahead. The tuner, when on, hears the same input in front
   of the input gain and the gate
2. One oversampled region: the pedals in front of the amp in their order, preamp stages, tone
   stack, power amp. 4x at 44.1 and 48 kHz, 2x from 88.2 kHz on. The clippers are antialiased
   (antiderivative method; second order where the gain is highest)
3. Cabinet: a short impulse response (5 to 10 ms) with the speaker resonance as a filter behind
   it, then the Mic and Resonance dials, DC blocker
4. Two channels from here: the pedals behind the cabinet in their order. (A delay or reverb
   dragged in front of the amp runs in step 2 instead, on one channel.) Delay and reverb add
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
- The metronome (`metronome.rs`) is added to the output after the whole chain, in `process` of
  `lib.rs`: the amp never distorts it, and Output and Bypass leave it alone. The tuner silences
  it (a click already sounding rings out, a few ms). Off, it adds nothing and the output is
  bit-identical. Its clock is a count of samples to the next beat kept as an `f64`, so a tempo
  that is not a whole number of samples per beat never drifts and the clicks fall on the
  sample whatever the host's block size. A tempo change is taken at the next beat; switching
  it on starts on the first sample, on the downbeat. Beats are quarter notes, in 6/8 eighths
  (six to the bar). With Sync on and a tempo from the host it follows the host's tempo and
  position every block: silent while the host is stopped, a jump or a loop is followed, bar
  position from the position in the song. With no tempo it uses its own. Sounds: Woodblock,
  Beep, Click, each higher and louder (1.0 against 0.6) on the downbeat. It keeps the plugin
  awake (`KeepAlive`) while on, and while Metro and Sync are both on (the tuner not muting it) even with the host stopped (a host
  may otherwise stop calling a plugin that is waiting for the song). A click still ringing is cut
  when a synced host stops. The seven `metronome_*` parameters are not in presets
- **The looper (`looper.rs`, #280)** sits in `process` after the chain and before the click, so a loop
  is what the player heard, never has the click in it and is not run through the amp again; Bypass
  and the Output knob are therefore already in it, and the tuner silences it (playback only). States:
  Empty, Recording, Playing, Overdubbing, Stopped. The buffer (60 s of stereo, 40 s above 96 kHz) is
  allocated in `set_sample_rate` and nowhere else; a change of rate empties the loop, the same rate
  leaves it, and `reset` leaves it too (a host that stops and starts does not take a performance away,
  and the loop runs on its own sample count, not the host's transport). A loop is counted in samples; the
  recorded start is faded in and its end out over 5 ms, so the seam is silent and there is no click.
  Empty it costs three atomic reads a block (0.02 us a block of 64; playing 0.55 us, overdubbing 0.64 us,
  under 0.05 % of a core: `cargo test -p amp --release looper_report -- --ignored --nocapture`) and
  leaves the sound bit-identical (`lib.rs` tests).
  Closing rounds the length to whole bars of the metronome where it is on (`Metronome::bar_samples`,
  from the host's tempo too), at least one, cut to the buffer; the rest of a bar rounded up is
  silence, a recording shorter than 10 ms is thrown away. Overdub adds through `soft_limit` (quiet
  passes add exactly, a loud one cannot run away) with a 5 ms ramp in and out. Stop mutes the
  loop but it keeps counting, so Resume comes back in time; Clear fades out over 5 ms and then
  is Empty. Commands (`press`, `stop`, `clear`) are atomics taken at the start of the next block, so a
  press lands to the block, not the sample. `looper_level` is the one parameter (not in presets);
  the buttons are not parameters. Not built yet: the plate and the strip's icon (milestone 2, after
  the owner's pick from `/design` explore), and starting a first record on the metronome's next
  downbeat (milestone 3; today a record starts at the press, so a snapped length is right but its
  start is not on the beat)
- **The click's own output (`metronome_out`, follows #181):** both audio layouts carry one more
  stereo output, the auxiliary port named `Metronome` (`CLICK_PORT`). Mixed (the default, so
  existing projects are unchanged) is the click added to the main output as above, bit for bit,
  and the `Metronome` output is written with silence (a host does not clear an output it hands
  over). Separate writes the click, and only the click, to the `Metronome` output
  (`Metronome::process_alone`; the same clock and voice as Mixed, through one `render`), and the
  main output is the amp alone. **In the standalone program, and where the host gives no
  Metronome output (or leaves it off), Separate mixes the click into the main output, as Mixed
  does**, so it is never lost. The standalone program is recognised by `plugin_api()`
  (`PluginApi::Standalone`), not by the buffer: nih-plug's standalone audio backends hand the
  plugin scratch aux buffers for every declared port and throw them away. The Click out
  stepper is not drawn there (`standalone::host()` is `Some`). Tests: `Mixed` equals the old amp-then-click
  path, `Separate` leaves the main equal to the amp alone and the aux equal to the click alone,
  the fallback equals Mixed, and the `no_alloc` tests run with the test host's aux buffers
  (`suite_common::host` now hands a plugin the aux outputs its layout names). The parameter is
  last in `GuitarAmpParams` so every parameter before it keeps its place in a host's list
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
  lane's `Cabinet` stepper after `OWN` (the second amp's has no file, only its own amp's
  cabinet and these). A built-in cabinet is stored as `speaker:<id>` in the cabinet choice;
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
  Output 0 dB (a test checks peak and level of each). It holds the chain too (#271, decision
  #260): loading a preset replaces the whole chain (blocks, order, amp lanes), saving writes it.
  A factory preset is the twelve pedals on their own slots in its order (`Preset::chain`, which
  plays as `ChainOrder` did: the preset test holds the layout and the settings to what they were), so
  every factory preset and every file from before loads as it did. What a load takes out is kept
  by effect kind in the chain's `Memory` (`StoredChain::replace`: a kind the new chain lacks, the
  first block of it in the line, read before any value is set; a kind that stays has its values
  replaced, which is not taking it out), and per amp by `dial_memory`, so the player can come
  back to it by putting the effect in again or picking the amp. New presets go at the end: a
  project remembers its preset by its number
- The player's own presets (`user_presets.rs`) are `.preset` text files in
  `Documents/Hojt Audio/Presets` (or the folder in `HOJT_PRESETS_DIR`), one `id = value` line
  per parameter by its frozen id, then `order`, `chain`, a `slot<n>` line for each block of the
  chain that is not on its own pedal's slot, and `cabinet`. They hold what a factory preset
  holds and the microphone and cabinet too. `chain` is `Chain::ids` (the project's form: `amps=2`,
  `kind@slot` for each block, `amp` between the zones); a block on its pedal's slot has its values
  in that pedal's lines, any other (a second delay, a gate) in `slot<n> = <on>:<k1>,..,<k5>`
  (`SlotValues::to_text`, knobs a kind does not use at 0), and a block with no line starts as a
  slot does (off, knobs at the middle). `order` stays as the twelve-pedal view for a build from
  before the chain. A file with no `chain` line is the twelve pedals in its `order`, with the
  amp lanes of a project from before. The preset shows as edited when the chain, a block's
  switch or a knob of a block off its pedal's slot has moved A file is read forgivingly: unknown lines are
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
- The preset display on the strip says the bank and the preset (`FACTORY · INIT`) and is one
  click target, with no step arrows: it opens the preset plate, the banks in one column and the
  presets of the bank clicked in the other, with the foot (Save, Folder) under them. The bank
  showing changes when a preset is picked out of another bank and when a bank is made; the
  sound then stays as it stands, and with nothing loaded out of the bank the display falls back
  to the name of the preset the sound came from
- `+ NEW BANK`, at the end of the banks, makes one. Clicked, its tape takes a name; enter keeps
  it, escape drops it (the plate stays open), and a second click on the tape keeps it as it stands, so a bank can be
  made with the mouse alone. A bank nobody named is `Bank 1`, `Bank 2` and so on, the first
  that is free, renamed in the folder afterwards. A name is filtered like a file name, and one
  that already stands as a folder is simply that bank, with everything still in it
- Save, in the foot of the preset plate, writes into the bank showing and always asks for a name:
  it makes the foot's display take one, which starts as the name of the preset loaded out of
  the bank showing (empty with none loaded). Enter or a second press of Save keeps it, escape
  drops it and writes nothing, closing the plate drops it too. A name may hold spaces (#264):
  `type_into` takes a space that arrives as text, and one that arrives only as the space bar's
  key press (a host that keeps the key for play and stop), and a test holds both. The name kept as it stands writes over the
  preset loaded; any other is the file name, `<name>.preset`, with a number added when it is
  taken. A name with nothing left once the characters a file name cannot hold are taken out is
  one nobody gave, and is `<name> 2.preset` (the next free number) named after the preset the
  sound came from, as before. Nothing is written into `FACTORY`: a sound saved there goes into `MINE`, which is the
  bank showing from then on. The file name of the one loaded last is persisted as `user-preset`
  (empty after a factory preset, and after the bank changed); renaming, moving from one bank to
  another, copying and deleting is done in the folder, which the foot's Folder button opens
  (hovered it says where) — the folder of the bank showing
- The pedal order (`order.rs`, held by the chain model `slots.rs`): Gate always first, amp and cabinet one block in the middle.
  Every pedal stands on either side of the amp (#268: Delay and Reverb in front, Drive, Pitch and
  Wah behind the cabinet, below); the gate is the one fixed place. Default: Pitch, Wah, Fuzz, Comp, Drive, Phaser, amp,
  Chorus, Flanger, Tremolo, EQ, Delay, Reverb, which with the new pedals off is the chain as it
  was, to the bit. A project saved before a pedal existed gets it where the default has it (the
  wah behind the pitch shifter, wherever the project put that). `Pedal::ALL` only ever grows at the
  end: the order is saved as a number of four bits a pedal too
- **Delay and reverb in front of the amp** (#268, `AmpChain::front_delay`, `front_reverb`): one
  more `Delay` and `Reverb` made for the oversampled rate, so a pedal that changes sides
  allocates nothing (about 3 MB of lines for the delay at 192 kHz). Only the instance of the side
  the pedal stands on runs; the other is reset when the pedal crosses (`set_up_pedal`), so
  a tail does not come back. In front the effect is mono: the signal goes in on both channels of
  the stereo effect and the mean of the two comes out (`mono_through_stereo`), which leaves the
  dry signal as it was and gives the amp the repeats and the tail as a centred listener hears
  them, and the amp plays them. Behind the cabinet nothing changed, to the bit
  (`fingerprint_report` before and after). No latency in either place. `zone_report` prints the
  level, latency and cost of each effect on each side, `render_zones` writes them for listening
- **Slot parameters** (#268, `layout.rs`, `extras.rs`, `slots.rs`): a block on a slot that is
  not its pedal's own (`Chain::is_home`), a second Delay, a Gate block, plays from
  `slot<n>_on` and `slot<n>_k<m>`. `GuitarAmpParams::slot_settings` turns the slot's 0..1 values
  into the kind's settings with the own parameter's range (`preview_plain` of the pedal's own
  knob `m`: the same order as `pedal_params`), so a value that moves between the pedal's own
  parameters and a slot's (`Memory`) means the same in both; a test holds the order, another that
  a slot plays as the pedal does, to the bit, on each side. The pedals keep their instances in
  `AmpChain` and read their own parameters: their sound did not change (`fingerprint_report`
  before and after, identical). The others are `extras::Pool`: `MAX_PER_KIND` (2) units of each
  kind, each with an instance for either side where the sides need one (as the pedals do; about
  11 to 12 MB in all, mostly the delays and reverbs: `pool_memory`), made in `set_sample_rate`. A unit is
  assigned to a block by kind and `rank` (the block's place among the blocks of its kind that
  are on slots of their own, by slot number), moves when the layout changes (the output fades
  as for any change of order, `follow_order`) and starts from rest on the side it comes in
  on. Told at the same reads as the dials (`tell_pool`), so a switch falls on the same sample
  whatever the block size. A drive on a slot that is on in front of the amp pushes it as the
  pedal does (`AmpChain::driven`). A pedal absent from the chain is put to rest and not run.
  `slot_report` prints, for each kind and side, whether it is the pedal's sound, its level,
  latency and cost
- **Drive, wah and pitch shifter behind the cabinet** (#268, `AmpChain::back_drive`,
  `back_wah`, `back_pitch`): made for the host's rate next to the ones in front, so a pedal that
  changes sides allocates nothing (the pitch shifter's buffers, about 1 MB, once a channel).
  Only the instance of the side the pedal stands on runs; the other is reset when it crosses,
  from rest. The wah and the pitch shifter are one instance a channel with the same settings,
  so a mono signal comes out the same on both sides; after a stereo effect (a chorus in front
  of them) each channel follows its own signal, which is what a stereo pedal pair would do. The
  drive raises the rate itself, like the fuzz (`drive::BackDrive`: two `Drive` and two
  oversamplers, 4x, 2x from 88.2 kHz) and crossfades to the untouched signal at the host's rate,
  so off it does not even pass the oversampler and leaves the signal alone, to the bit. A drive
  pedal behind the cabinet does not push the amp (`set_driven` is only for the pedal in front:
  `AmpChain::follow_drive_side`), and the second amp follows the same switch. In front of the amp
  nothing changed, to the bit (`fingerprint_report` before and after, with a pitch and a wah
  row added). The wah's and the pitch shifter's latency is what it is in front; the drive's is
  the oversampler's fraction of a millisecond, like the fuzz's. `zone_report` and `render_zones`
  cover these too
- In front of the amp a pedal runs on one channel at the oversampled rate, behind the cabinet
  on two at the host's rate (chorus, flanger and phaser a quarter turn apart left and right).
  Either way without latency (but for the pitch shifter, below). The fuzz is the one exception behind the cabinet: it raises the
  rate itself for its clipping (as the amp does: 4x, 2x from 88.2 kHz), which delays it by
  the oversampler's fraction of a millisecond while it is on
- The pitch shifter (`pitch.rs`, the algorithm of the Pitch Shifter plugin in
  `suite_common::dsp::pitch`) stands on either side of the amp (behind the cabinet since #268, one
  instance a channel at the host's rate); in front it is on one channel at the amp's
  oversampled rate, and it is first in front of the amp by default (the gate stays in front of
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
- The EQ (`eq.rs`) is three biquads of `dsp/filters.rs` in f64, each gliding to a new setting
  over 20 ms so a dial turned while playing does not click: Low a shelf at 100 Hz, Mid a bell of
  Q 0.9 centred 200 Hz to 5 kHz on a log scale (Freq 5 is 1 kHz), High a shelf at 4 kHz, each
  -12 to +12 dB with the dial's 5 flat (a dial within 0.01 dB of flat is flat). A flat band is
  skipped, so with all three flat the signal passes untouched, to the bit; a band that has
  just become flat runs on a few samples until its state is empty. On either side of the amp
  (one channel at the oversampled rate, or two at the host's), without latency; switching
  crossfades in 10 ms and an off, faded-out pedal is idle. Nothing in it allocates (`no_alloc`
  holds it). Cost (`eq_report`, the pedal by itself, median of a block of 64 host samples as a
  share of the time it lasts): with all three bands flat 0.01 to 0.1 % of a core; with every
  band moved 0.06 to 0.11 % at 48 kHz (behind the cabinet / in front of the amp) and 0.25 to
  0.26 % at 192 kHz, 99th percentile under 0.4 %. `is_idle` is only for the tests: the chain's
  own `is_idle` leaves the EQ out, since a filter's ring is a few milliseconds and keeps no tail
- The wah (`wah.rs`, #240) stands on either side of the amp (behind the cabinet since #268, one
  instance a channel at the host's rate); in front it is on one channel at the oversampled
  rate, and is second there by default, ahead of the fuzz and the drive. A resonant band-pass: one
  state-variable filter in the topology-preserving form (f64, stable at any sweep speed and from
  44.1 kHz to the 4x rate of a host at 96 kHz and over), its centre by `wah_pedal` on a log
  scale from 350 Hz (0, heel) to 2.2 kHz (10, toe), its Q by `wah_q` ("Bite") from 3 to 9 on a log
  scale. A band-pass has no low end and loses level, so the output adds 12 % of the same
  filter's low-pass, and a makeup gain (`MAKEUP_ROCKER_DB` across the rocker, `MAKEUP_BITE_DB`
  across Bite) holds the level through the whole sweep within 3 dB of the pedal off for a guitar
  at `PEDAL_DI_DB` (a test holds it, for riffs and for clean playing). The rocker and Bite are
  worked out once per 32 samples of the stream (whatever the blocks) and followed in 3 ms (`mods::Dial`), the switch crossfades in
  10 ms (`mods::Fade`), and an off, faded-out pedal is idle and costs nothing. Pedal mode: the
  rocker is `wah_pedal`, which a host automates (a controller or an expression pedal mapped to
  it; there is no MIDI input of its own, #240). Auto mode: an envelope follower on the input
  (attack 5 ms, release 150 ms, on the rectified signal) moves the centre up from the rocker's
  resting position by `wah_sens` of the sweep at the level `ENV_FULL` (-14 dBFS) and in
  proportion below it: a note at -12 dBFS opens it 75 % of the way in 16 ms and it is back
  in about 280 ms at Sens 5 (360 ms at Sens 9). The wah has no latency to report (the
  filter's own delay is a few samples). Nothing in it allocates (`no_alloc` holds it). Cost
  (`wah_report`, the pedal by itself in front of the amp at 192 kHz, median of a block of 64
  host samples): 0.15 % of a core in Pedal mode, 0.17 % in Auto, 99th percentile under 0.2 %. Factory presets
  switch it off with the rocker and Bite at 5 and Pedal mode; it is not part of a sound. It has
  no pedal on the board yet (#242, #243): `board.rs` leaves it out and keeps it where the
  order has it, and the editor shows nothing of it
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
| Drive (`drive_on`) | on / off | off | Overdrive pedal, in front of the amp or behind the cabinet |
| Drive Gain (`drive_gain`) | 0.0-10.0 | 3.0 | Amount of clipping in the pedal |
| Drive Tone (`drive_tone`) | 0.0-10.0 | 5.0 | The pedal's top end, 1.5 to 7 kHz |
| Drive Level (`drive_level`) | 0.0-10.0 | 5.0 | The pedal's output, -20 to +20 dB into the amp |
| Fuzz (`fuzz_on`) | on / off | off | Fuzz pedal, either side of the amp |
| Fuzz Amount (`fuzz_fuzz`) | 0.0-10.0 | 5.0 | Gain into the clipper, 26 to 56 dB |
| Pitch (`pitch_on`) | on / off | off | Pitch shifter pedal, in front of the amp or behind the cabinet |
| Pitch Semitones (`pitch_semitones`) | -12 to +12 | 0 | Whole semitones: the guitar down an octave to up one. Shown `-5`, `0`, `+7`; double-click returns to 0 |
| Pitch Blend (`pitch_blend`) | 0-100 % | 100 % | The guitar as it comes in (0) to the shifted guitar alone (100). The dry part is not delayed |
| Pitch Tracking (`pitch_latency`) | 2-50 ms | 15 ms | The most the shifted guitar may lag behind the input: the longest period that splices cleanly (a low E is 12 ms, a power chord on it about 24 ms) |
| Fuzz Tone (`fuzz_tone`) | 0.0-10.0 | 5.0 | Dark (a low-pass at 650 Hz) to bright (a high-pass at 1.1 kHz), the mids scooped in between |
| Amp (`amp`) | Klar / Brøl / Torden / Glød / Sang / Drøn | Brøl | Amp and its cabinet. Variant ids `klar`, `brol`, `torden`, `glod`, `sang`, `dron`. New amps go at the end: a project remembers its amp by its number |
| Gain, Bass, Mid, Treble, Presence, Master (`gain`, `bass`, `mid`, `treble`, `presence`, `master`) | 0.0-10.0 | 5.0 | The amp's dials |
| Amp 2 (`amp2_on`) | on / off | off | The second amp, beside the first behind the same front pedals. Off, it is not run at all |
| Amp 2 Model (`amp2`) | the same six, same variant ids as `amp` | Klar | The second amp's own choice |
| Amp 2 Gain, Bass, Mid, Treble, Presence, Master (`amp2_gain`, `amp2_bass`, `amp2_mid`, `amp2_treble`, `amp2_presence`, `amp2_master`) | 0.0-10.0 | 5.0 | The second amp's dials |
| Amp Blend (`amp_blend`) | 0.0-10.0 | 5.0 | 0 is the first amp alone, 10 the second alone. Not heard while Amp 2 is off |
| EQ (`eq_on`) | on / off | off | EQ pedal, either side of the amp |
| EQ Low, Mid, High (`eq_low`, `eq_mid`, `eq_high`) | 0.0-10.0 | 5.0 | -12 to +12 dB, 5 is flat: Low and High shelves at 100 Hz and 4 kHz, Mid a bell |
| EQ Freq (`eq_freq`) | 0.0-10.0 | 5.0 | The centre of Mid, 200 Hz to 5 kHz on a log scale (5 is 1 kHz) |
| Wah (`wah_on`) | on / off | off | Wah pedal, in front of the amp or behind the cabinet |
| Wah Pedal (`wah_pedal`) | 0.0-10.0 | 5.0 | The rocker: 0 is the heel (350 Hz), 10 the toe (2.2 kHz), on a log scale. Map a controller or an expression pedal to it in the host. In Auto mode, where the rocker rests |
| Wah Bite (`wah_q`) | 0.0-10.0 | 5.0 | The resonance, Q 3 to 9: a wider, woofier peak to a narrow, vocal one |
| Wah Mode (`wah_mode`) | Pedal / Auto | Pedal | Pedal: the rocker is where the dial puts it. Auto: the playing opens it up from the resting position |
| Wah Sens (`wah_sens`) | 0.0-10.0 | 5.0 | Auto only: how far a loud note opens the pedal, as a share of the sweep |
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
| Delay (`delay_on`) | on / off | off | Stereo delay behind the cabinet, or mono in front of the amp |
| Delay Time (`delay_time`) | 20-1000 ms | 350 ms | Time between repeats |
| Delay Feedback (`delay_feedback`) | 0-90 % | 35 % | Level of each repeat against the one before |
| Delay Mix (`delay_mix`) | 0-100 % | 25 % | Level of the repeats |
| Delay Sync (`delay_sync`) | on / off | off | The repeats follow the song's tempo: the Time knob picks Delay Note, and the time is `60000 / bpm` times the note's beats, kept to 20-1000 ms. With no tempo from the host, the Time knob's ms. Factory presets set it off |
| Delay Note (`delay_note`) | 1/4, 1/8 dotted, 1/8, 1/8 triplet, 1/16 | 1/8 dotted | The note value while Sync is on (`suite_common::dsp::NoteValue`). Variant ids `quarter`, `dotted_eighth`, `eighth`, `eighth_triplet`, `sixteenth`; a new one goes at the end |
| Reverb (`reverb_on`) | on / off | off | Stereo reverb behind the delay, or mono in front of the amp |
| Reverb Decay (`reverb_decay`) | 0.3-6.0 s | 1.5 s | Time for the tail to fall by 60 dB |
| Reverb Mix (`reverb_mix`) | 0-100 % | 20 % | Level of the tail |
| Output (`out_level`) | -30 to +6 dB | 0 dB | Output level, in front of the safety clip |
| Tuner (`tuner`) | on / off | off | Silent tuning. Not automatable, not in presets, not changed when a project is loaded |
| Looper Level (`looper_level`) | -60 to +6 dB | 0 dB | The loop against the live sound; -60 is silence. Not in presets. The looper's buttons are commands, not parameters |
| Metronome (`metronome_on`) | on / off | off | The click, on top of the finished sound. Not in presets |
| Metronome Tempo (`metronome_bpm`) | 40-300 bpm | 120 | Beats a minute: quarter notes, in 6/8 eighths |
| Metronome Sync (`metronome_sync`) | on / off | off | Follow the host's tempo and position; silent while the host is stopped. With no tempo from the host, the Tempo knob's |
| Metronome Time Signature (`metronome_time_sig`) | 2/4, 3/4, 4/4, 6/8 | 4/4 | Clicks to the bar. Variant ids `two_four`, `three_four`, `four_four`, `six_eight` |
| Metronome Volume (`metronome_volume`) | -60 to 0 dB | -12 dB | The click's level; -60 is silence |
| Metronome Sound (`metronome_sound`) | Woodblock, Beep, Click | Woodblock | Variant ids `woodblock`, `beep`, `click` |
| Metronome Output (`metronome_out`) | Mixed, Separate | Mixed | Where the click is heard: Mixed on the main output; Separate alone on the auxiliary stereo output `Metronome`, the main output the amp alone (mixed in where the host gives no usable such output, always in the standalone program). Variant ids `mixed`, `separate`. Not in presets |

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

# The four amps of #263 against the others: total preamp gain, loudness, octave bands, the bite
# at 3 to 5 kHz, the fizz above 7 kHz, aliasing, palm mutes. render_voices writes each next to its
# nearest old amp at Gain 3, 5 and 7 as WAV files in target/renders (voices_*.wav)
cargo test -p amp --release voices_report -- --ignored --nocapture
cargo test -p amp --release render_voices -- --ignored

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

## Two amps

- **Where it splits** (#177): after the front pedals, so both amps hear the same gate, drive and
  the rest, and the pedals behind the cabinet work on the mix. Each amp is a lane of its own: a
  preamp, tone stack, power amp and cabinet, run in the same oversampled region on a copy of the
  signal. The mix is behind the two cabinets and in front of the DC blocker, so both reach the
  back pedals in phase (one oversampler, one cabinet latency). `ChainOrder` does not know about
  it: the stack is still one place in the order
- **Amp 1 is always in the signal** (it has no switch). Amp 2 has `amp2_on`. With it off its
  stages are not run, the output is bit-identical to the single amp it was, it costs nothing,
  and every factory preset (except "Tung og Klar") and every project from before opens with it off
- **The second slot uses the same stages and the same `AmpModel` constants** as the first (no
  `match amp` in the DSP, `tests/rules.rs`). It has its own dial smoothing and its own
  amp-switch fade (4.5 ms, switching at silence), and its model and dials are read into
  `AmpSettings::second`. `finer_when_driven` and `driven_sag` apply to both
- **Cabinets:** each amp plays through its own. Amp 1's is the amp's own, a built-in one or the
  player's file (`cabinet`); amp 2's is its own amp's or a built-in one (`cabinet2`, persisted,
  and in a user preset file; never a file, which keeps the cabinet loader single). Mic,
  Resonance, Mic Type and the Cab switch act on both
- **Blend law:** the first amp's share is `(1 - blend)^0.6` and the second's `blend^0.6`
  (`second::BLEND_LAW`), exactly 1 and 0 at the ends, so Blend 0 equals amp 1 alone and Blend 1
  amp 2 alone, to the bit (the other path still runs). Linear dipped 2.8 LU in the middle for
  Torden + Klar (a heavy and a clean amp are largely not the same signal, so their powers add);
  equal power (0.5) is flat for them but bulges 3 dB for two similar amps and put Brøl + Glød
  over the safety clip's knee. Two identical amps therefore add up to +2.4 dB at the middle.
  `blend_report` (`cargo test -p amp --release blend_report -- --ignored --nocapture`) prints
  the loudness of five pairs at five Blends and holds the law to it: every pair at 0.5 within 2
  LU of -13 LUFS, Torden + Klar within 1 LU across 0.2 to 0.8, every pair under the knee
- **Switching amp 2** (on or off, or its model) clicks no more than switching amp 1; tuner and
  bypass act on the whole rig. Dial memory: `amp2-dials` keeps amp 2's dials per amp, as
  `amp-dials` does for amp 1
- **Cost** (`stage_report`, Torden Gain 10 and everything on, plus Klar at Gain 10 at Blend 0.5
  as amp 2, measured in #203; `whole, 2` is both amps on): `whole` 3.0 to 3.5 % of a core at
  48 kHz, both amps on 4.0 to 5.1 % at 48 kHz, 6.2 to 6.5 % at 96 kHz and 14.2 % at 192 kHz,
  within the 8 % limit at 48 kHz. The spec's estimate for amp 2 alone was 2.8 % at 48 kHz; the
  run-to-run spread is as large as the difference
- **Not done, on purpose:** phase invert on amp 2, stereo (amp 1 left, amp 2 right), separate
  effect routing per amp (a split in front of the pedals). The first two only if a report or
  a listen shows a need; the third is a future issue

## Look: the rig

The style guide (`crates/suite_common/src/ui/STYLE.md`) applies; these are the rules for a window of several pedals, which only the amp has.

- **Rigs:** a plugin made of several components (the Guitar Amp) is the one exception to one
  pedal per plugin. Its window is a `rig`: a `strip` across the top, a shallow enclosure in the
  plugin's paint that carries the jack captions, model number, logo and art (no name band), with a wing at each end for the plugin's own controls; and under it one line in signal
  order: the gate, the pedals in front of the amp, the stack (two lanes, a head
  with its cabinet each, and the plate where they meet, `STACK_WIDTH` wide), the pedals behind the cabinet. The pedals stand on the floor beside the
  cabinet, as tall as it. Each mini pedal has a paint of its own. On a strip, a head and mini
  pedals the switches are `small_footswitch` and the LEDs `rig_led` (an unlit `led` disappears
  on dark paint), beside or above the switch. A mini pedal holds two knobs of radius 24 side by
  side, three of radius 17 in a triangle, (the EQ) four in two rows, or (the Wah) the rocker with two knobs of radius 14 beside it. Everything else in this guide applies
- **The strip's wings:** the left one holds what comes in and what is picked (input meter,
  Bank, Preset, Save; the tuner display while tuning), the right one what goes out (Output, the
  output meter, Clip, On, Tuner). Its jack captions read `In` at the left and `Stereo Out` at
  the right, arrows left to right: the signal runs along the window. Nothing in a wing reaches
  the band (a test holds it)
- **The stack is two lanes** (#177): the first amp's head with the cabinet beside it, and under
  it the second amp's, as tall as the pedals together (`LANE_HEIGHT`). Each lane has a head
  that looks as its amp (below), a nameplate, six dials of radius 15, and a line under the dials
  with the lane's name (`Amp 1`, `Amp 2`), the `Cabinet` stepper of its own cabinet and, on the
  second lane, its `On` switch with a `rig_led`. The cabinet beside it looks as the cabinet that
  plays through that amp and has no plate. Both input jacks take the guitar (a cable links
  them), and both lanes end at a black plate, `MIX_WIDTH` wide, through a coupler each. The
  plate holds, from the top: `Blend` over a `crossfader` (`ui::crossfader`: `Amp 1` at the
  top of the slot, `Amp 2` at the bottom, the value on label tape under it), the Mic and Res
  knobs (radius 17), the `Mic Type` stepper, the output jack with its `Out` caption, at the
  height of the pedals' plugs, and the Cab switch with its LED and caption. Mic, Res, Mic Type
  and Cab act on both amps' cabinets. The first lane's `Cabinet` stepper goes through the amp's
  own, the built-in cabinets and the files, as the Speaker stepper did; the second's through its
  own and the built-in ones (`cabinet2`, never a file). Nothing of the lanes is dimmed while
  the second amp is off: its lamp and its switch say so
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
  and speakers. Every cabinet is the same size (a lane's, `LANE_CAB_WIDTH` wide and
  as tall as the lane); its controls are not on it but on its lane's head and the shared plate
  (above)
- **Pedal art:** every mini pedal has its own `PAINT_*` and its own `Motif`, all on one theme
  per rig (the Guitar Amp: northern weather and nature). Two coats and a print: a large motif
  in darker tones of the pedal's paint (`theme::shade`) that may run under the controls, with
  no fine detail under a caption, and a few `SILK` lines (2 px) and dots only in the clear
  paint beside the controls. The art keeps 5 px of plain paint to the edge of the face. No
  letters, numbers or cream triangles in it, and nothing that moves. A new paint goes in
  `PEDAL_PAINTS`, where tests hold it to cream print reading on it and to being clearly apart
  from the others; a new motif is drawn in `art.rs` for the layout of its pedal
- **The signal line** (#270, replacing "equal pedals, one row, 3204 px wide"): every block of the
  chain is a pedal `PEDAL_WIDTH` (180 px) wide and `GAP` (27 px) from the next, in signal order
  from the left: the gate that is always first, the blocks in front of the amps, the stack
  (`STACK_WIDTH` wide), the blocks behind the cabinets. The line is as wide as its blocks (3411
  px for the gate, twelve pedals and the stack, as it always was: a project from before is that
  line, to the pixel) and is never stretched. A block draws from its own parameters where it
  stands on its pedal's slot and from its slot's `slot<n>_*` parameters otherwise (`editor::Src`:
  a slot's knob is told in the terms of the pedal's own parameter, at the same position), each
  kind with its own paint, art and size (a pair, a trio, a diagonal pair, the EQ's four, the
  wah's rocker, the delay's Sync); a duplicate looks as the original. A pedal with two knobs is
  either a pair side by side (radius 24) or on a diagonal (`SLIM_LED`, `SLIM_KNOBS`,
  `SLIM_SWITCH`): the LED where a trio's upper left knob is, the two radius-17 knobs upper right
  and lower left, as far from the middle as a pair's. The narrow motifs (`art.rs`, `NARROW_ART`)
  stand in the middle of the wider face and their paint runs out to both edges. In a row the
  LEDs and switches line up
- **The window scales and scrolls the line** (#270, replacing "fixed window size"): the layer is
  the line's width (not under `STRIP_MIN_WIDTH`, 2600) by 584, and the window's zoom is the one
  that fits it wide and high, between 0.5 (`MIN_ZOOM`, the least that reads) and 1.0. Where
  0.5 does not fit the line the window shows part of it and the wheel over the line or the bar
  along the foot (`ui::scroll_bar`) scrolls it; the strip stays and stretches with the window,
  its right end and middle following the edges. No proportions are locked on the corner drag.
  Nothing wider than 8192 px is made: the strip's lit surface is one picture whose middle is
  stretched (`raster::paint_box_wide`). Tests hold the zoom, the scroll, the strip at its
  narrowest and widest, and snapshots show the line short, small, large, sixteen blocks and
  scrolled
- **The Wah's rocker:** the one control that is neither a knob nor a switch, `ui::rocker` (a
  variant of the crossfader: a tall slot with a ribbed chrome treadle, ticks both sides, the
  heel at the bottom and the toe at the top, drag the cap or the slot, Shift for fine,
  double-click for the default). It is the pedal's middle, with Bite over Sens in a column on
  the left, the LED, the rocker's value on tape and the Auto push button (a `chrome_button`
  with a `rig_led` above and a caption under, like the Delay's Sync) on the right. Toe, Heel
  and the rocker's caption (Rocker, and Rest while Auto is on) are `silk_label`s; the tape says
  the value as the parameter does. The look was picked out of three on #242 (a treadle in
  perspective, this slider, a face dial)
- **A line that can be rearranged:** a block is moved by dragging it by its title, across the amps too (the gate and the stack are fixed; a block keeps its slot, and its values with it)
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
- **The strip is slim** (`ui::slim_strip`, 64 px, picked as A "Signal Line" on #261): the enclosure
  with the jack sockets and their captions, the model number and the logo, no name band and no
  screws. What is on it stands on one line (`STRIP_Y`), in groups: from the left the input
  (jack, meter with its caption over it, the Input gain beside it), the preset display; in the
  middle the metronome, tuner and settings icons; from the right the output (knob, meter with
  its caption, Clip lamp), the model number and the logo, the output jack. Its knobs are
  `bare_param_knob`s with no caption (the meter's caption names the pair, the name and value
  show as a hint). **On is not on it:** the `bypass` parameter and `make_bypass()` stay for the
  host, nothing in the editor sets it (a rules test holds that). Tests hold that nothing on the
  strip overlaps, that all of it is on the face, and that the plates open under it. The input
  gain moved here from the Gate pedal, which is a pair of knobs now
- **Level meters:** a signal level (the amp's input and output) is a `level_meter_wide`, a row
  of lamps on label tape with a `silk_label` caption over it, on the strip. Same atomics and repaint rule as the signal lamps
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
- **The sound bank's LED display:** the preset display and the foot's name display are built-in
  LED displays (`suite_common::ui::led`): flat dark glass with a crisp white Plex Mono readout,
  no dot grid. It is `LED_HEIGHT` high and `led_width(chars)` wide: the strip's holds 26
  characters (two are the marker's), the foot's 18. The
  text is capitals from the left, so a name being typed grows to the right, with the `_` caret
  after it; what does not fit is cut. The font has every letter a name can hold. The
  tuner's display is the same glass
- **Pick one of many (presets):** the preset display is one click target that opens the preset
  plate (`drop_panel_with`: the banks and presets, a foot with the name display, Folder and Save).
  There are no step arrows on it. A trailing `*` on the display means a control was moved since
  loading. Names have at most 15 characters and are our own words. While the tuner is on the
  display is the tuner's and opens nothing
- **Plates and the sheet** (#259, replacing "a list that drops open is for the preset banks and
  nothing else"): configuration that is not on show at all times is a **hardware plate** that
  opens from an icon or a display on the strip and drops under it (`ui::plate`; `drop_panel_with`
  for the preset plate), or the effects **sheet** that rises from below (built with the signal
  line, #272). No other pop-ups, no dropdowns, and no widget of egui's own. A plate keeps the
  amp's plate look: dark with a chrome edge, label tape rows, steppers, lit lamps; its title is
  a `silk_label`. **One is open at a time:** `View.plate` is one field, so opening one closes
  the other (a test clicks through them). It closes on a click outside it (the icon or display
  that opens it is not outside: its click closes it, as Escape does), and while a name is typed on
  it escape drops the name first. It is drawn after everything else in the window, in a
  foreground layer, so nothing behind it answers a click while it is open, and it is kept inside
  the window. The three plates: presets (under the display, a row of columns of tape rows
  that show seven rows at a time with a chrome bar and the wheel), the metronome's (tempo, tap,
  sound, meter, click out, volume, Sync; its switch with a lamp that flashes on the beat is on
  the strip beside the icon) and the settings' (the cabinet files, picked as on the head, and in
  the standalone program the machine's audio with Apply and Cancel: the audio's choice opens with
  the plate and is let go when it closes). Icons are `ui::icon_switch`: a drawing, and where the
  thing can be on or off a lamp and a chrome switch; the tuner's is its switch
- **Typed text** is taken in two places only: the name of a new preset bank, on its row of the
  preset plate, and the name of a new preset, on the plate's foot after Save
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
- **Steppers on the lanes and the plate** (each lane's `Cabinet` and the plate's `Mic Type`): the
  `silk_label` caption above the tape, and the buttons clear of the next knob, caption or LED.
  The amp's is the nameplate on its head, which needs no caption: it says the amp's name
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
  choices: the amp's own (`OWN`), then the built-in cabinets. On the first lane's line under
  the dials the tape is 90 px wide and shows at most 10 characters: no extension, capitals, long names as the first 5 and last 4
  with `~` between. `!` in front of the name of a file that cannot be used, `?` in front of the name of one
  that is not there. Stepping on to `OWN` past where the files would be, with none in the folder,
  says `NO FILES` for 1.5 s. Hovering the
  `Cabinet` caption names the folder; a double-click on it opens the folder
- **A file choice is never a parameter.** Keep the file name (not the path) in a `#[persist]` field
- **Metronome:** one cluster on the strip between Save and Output, its controls closer
  (`METRONOME_GAP`) than the strip's others (`STRIP_GAP`), left to right: a `small_footswitch`
  with `rig_led` and its `Metro` caption like On and Tuner, a `beat_lamp` (amber on a beat, cream
  on the first of the bar, lit 90 ms) with its `Beat` caption, the tempo on an LED display of three
  characters with a chrome button at each end (also dragged up and down, 3 points a beat per
  minute and a quarter of that with shift, scrolled, or double-clicked for 120), the Tap
  `chrome_button`, a `stepper` for the sound over two side by side, the time signature and the click's output (`Click out`: Mixed or Separate, on a 70 point tape that Separate fills), the Volume knob
  (radius 20) and Sync, a `chrome_button` with a `rig_led` above it. Captions are on the preset
  captions' line. The window is drawn again every 20 ms while it runs (the lamp follows the
  sound) and not at all when off. The cluster is one control of the strip's row, wider by the click's output (the window is not: `STRIP_GAP` went from 43 to 32.5 points, and the gap holding the standalone program's Audio button, `AUDIO_GAP`, stayed 43); a test holds
  its parts clear of each other and of their neighbours
- **Tuner:** a `small_footswitch` with `rig_led` and a `silk_label` at the end of the strip's
  right wing, beside the On switch. While it is on, a
  `tuner_display` (an LED display, `ui::led`) under its own caption takes the place of the
  preset display: the note name in dots at the left, a needle of 17 lamps, flat to the left
  and sharp to the right, the tall middle lamp lit alone within 2 cents. All in the display's
  amber; no numbers or other colours. The editor repaints continuously only while the tuner is on
