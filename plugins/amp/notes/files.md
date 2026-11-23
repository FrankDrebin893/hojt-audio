# Files of the amp

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

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
- `src/effect.rs`, `src/effects.rs` - One seam for a pedal (#371, milestone 1): `Effect` (the trait: `new`,
  `set_sample_rate`, `set_up`, `reset`, `is_idle`, `tell`, `process_front`, `process_back`, and its
  `Settings`), implemented next to each pedal's own code (a pedal with an instance a side keeps both inside
  its `Effect` type: `DriveEffect`, `PitchEffect`, `WahEffect`, `DelayEffect`, `ReverbEffect`), and the
  `effects!` table of `effects.rs`, one line a pedal, from which `BlockSettings`, `Body` and the dispatch
  (a `match`: no `dyn`, no heap) are made. **To add a pedal** (seven production files, one new; a scratch pedal showed it): its own module with its
  `Effect` impl; one line in the table (its `id`, knob count, how a slot's knobs make its settings, how its own
  parameters do, its `Pedal` and `EffectKind`; the line's position sets its bank card's position); its
  parameters in `lib.rs` (the nih-plug derive needs the fields) and its field in `settings.rs`'s `AmpSettings`;
  in `slots.rs` its `EffectKind` variant, its place in `ALL` and its row in `pedal_params`; in `order.rs` its
  `Pedal` variant, its place in `ALL` and in the default order (these lists are frozen ids and orders, each
  held by its `ids` test, so they stay outside the table); its look in `editor.rs` (two `match`es on the kind,
  and its picture); and its tests, where the helpers of `no_alloc.rs` and `chain/tests.rs` that name every
  kind need one arm each (by design: exhaustive matches, a forgotten pedal fails to compile). `chain.rs`,
  `extras.rs`, `layout.rs` and `bank.rs` stay untouched.
  The table also gives `slots.rs` an effect kind's id, knob count, pedal and settings from a slot
  (`effects::id_of`, `knobs_of`, `pedal_of`, `slot_settings`), `order.rs` a pedal's id and `bank.rs` its
  `CARDS`. From milestone 2 `AmpChain`
  holds its twelve pedals through it too (`homes` and `home_settings`, by `Pedal::index`; a row of the table also
  says how the pedal's settings are read from `AmpSettings`, `home`), and `Voice::Home` and `Voice::Slot` play
  through the same `Body::process_front` and `process_back`. What a pedal on its own parameters does differently
  from the same pedal as a slot's block is in its `Effect` impl, kept as it was and never tidied (`set_up_home`,
  `put_away`, `rest`, `rest_with_amp`, `tell_home`). `chain.rs`, `layout.rs` and `extras.rs` name no pedal type
  and nothing there is a `dyn` or a `Box` (`tests/rules.rs`)
- `src/settings.rs` - `AmpSettings`, what the plugin tells the chain once per block (it was in `chain.rs`)
- `src/extras.rs` - The effects that play from a slot's parameters (#268): `Unit` (one effect of the table,
  made for either side as the pedals are, with where it stands) and `Pool` (`MAX_PER_KIND` units of each of the 13 kinds, made with the chain by
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
  `LooperSettings`, `LooperControl`, `State`). The audio and its commands (milestone 1) and, from
  milestone 2, the plate under the strip's looper icon (`editor/looper.rs`, `looper_plate`, ring picked on #280)

- `src/editor/` - the editor's plates, each a child module of `editor.rs`, moved out of it unchanged (#378): `looper.rs`, `metronome.rs`, `settings.rs`, `mix.rs` (the plate where the two lanes meet), `preset_display.rs` (the preset display and its bank and preset lists; not `presets.rs`, which is the factory presets), and `tests.rs`
- `src/editor.rs` - egui GUI (the signal line in a layer 584 high and as wide as the chain, 3411 for today's twelve pedals; opened scaled to the screen up to 3411 x 584 or 1876 x 321 on a 1920 x 1080 screen, resizable freely from 1301 x 293 by its corner handle, scaled to fit between 0.5 and 1.0 and scrolled sideways below that, `Fit`): the slim oxblood strip
  across the top (64 px) with the input (meter and Input gain), the preset display that opens
  the bank and preset plate, or the tuner display in its place, the metronome, looper, tuner and
  settings icons in the middle, and the output (knob, meter, Clip) with the model and logo at
  its right; the plates that open under it (`View.plate`: presets, metronome, looper, settings); the stack (two lanes, one per amp: its head
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
  has none), one row, each a miniature pedal at half size (`editor::paint_miniature` drawn through
  `ui::miniature`, a scaled layer that takes no clicks; captions, value tapes, Sync and the Wah's
  Auto button are left off it) with how many of it are in the chain under it, any number of times. A click on a card adds it in the gap the
  bank was opened from (and moves that gap on, so the next card follows it); a card dragged out opens a place in
  the nearest gap of the line (`Line::drop_target`) and lands there when it is let go over the line, over the
  sheet it goes back; the cross on a block (`Pedals::take_out`, `StoredChain::take_out`) opens the bank with a
  note that its settings are kept and an Undo. What plays: the audio plays the pedal on its own slot, so a
  second block of a kind, or one off its pedal's slot, is drawn, kept and saved but silent until #268's last
  piece; taking out a pedal on its own slot switches its parameter off (`Chain::order` puts the pedal back, so
  only its switch keeps it silent) and putting it in again brings the values and the switch back
  (kept by kind, `Memory`). The plus under the amps (`Chain::add_amp`) shows while the chain has fewer lanes than `MAX_AMPS` (two: the chain is limited to two amps, by the engineer's decision on #269 after four amps cost 7.8 to 9.0 % of a core at Gain 10, over the 8 % limit); the stack always draws both lanes, so today it only shows for a chain saved with one, and amp lanes have no cross yet (amp 1 never)
