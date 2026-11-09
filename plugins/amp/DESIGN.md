# Guitar Amp: why it is the way it is

The reasons behind choices that the code alone does not explain. What it does is in
`AGENTS.md` (or `CLAUDE.md`) next to this file; what is open is the `plugin:amp` issues. Dates are 2026.

## Targets (set with Rasmus)

- Torden: modern tight metal, tight low end, fast attack, clear in drop tunings, meant to be
  boosted by the drive. Klar: glassy, lots of headroom, slightly scooped. Brøl: mid-forward
  stack crunch that cleans up with picking dynamics
- Presets: good defaults first, a browser later. Cabinets: our own at first, a loader later
- The window is one rig: a strip over one line in signal order, the half stack (head on cabinet)
  among the pedals, each pedal with its own paint and art (#96)
- The strip's print is 13 points or more (7 px at the default zoom 0.55) and its meters are
  lit green, amber (-10, -5 dBFS) and red (0), with the unlit state visible. The strip's knobs
  keep no value readout (`bare_param_knob`; the hover hint and a double click say it). At zoom
  0.4 the print is about 5 px: the least that reads is the window at 0.55, so the window does
  not open smaller. The plus on the strip is #272's
- The cabinet looks as the cabinet that plays (#100), but its box keeps one size for all
  twelve looks (a lane's, since #177): the pedals stand beside the stack as tall as it, and
  the cables, the controls and every click stay where they are whatever is picked. A real 1x10 and 4x12 differ in size; here only the speakers behind
  the grille do. What a missing or unusable choice shows is the amp's own cabinet, because that is
  what plays. The cabinet is not dimmed while switched off: nothing else in the rig changes
  look when off, and the LED on its plate says it

## Signal and numerics

- Amp and cabinet are mono (a stereo input is averaged); delay and reverb make stereo
- Clippers are antialiased (first-order antiderivative, f64) on top of oversampling; plain 4x
  measured about -40 dB aliasing at full gain, this gives -87 dB or better. Second order only
  where it pays (Torden's second and third stages, the drive); everywhere cost twice as much
  for the same figures. With the drive on, a stage that would be hit by square waves clips at
  twice the rate instead (`finer_when_driven`)
- Oversampling is minimum-phase IIR half-bands: 4x at 44.1 and 48 kHz, 2x from 88.2 kHz
- Biquads are f64: at 4 x 192 kHz a 100 Hz filter sits too close to the unit circle for f32
- A 1e-20 constant in every feedback path keeps denormals away (tests run without
  flush-to-zero)
- A preamp stage takes its operating point from the previous sample, which breaks the chain
  of divisions and saves a quarter of the preamp's time. The preamp runs one stage over the
  whole block and then the next (same output, a third less time)
- Two samples at a time through the clippers as an SSE2 pair, bit-identical. Rejected: block
  passes for auto-vectorising (slower), plain pairs of numbers in release, an own logarithm
  (not bit-identical; issue #22)
- nih-plug does not smooth parameters; `AmpChain` does, per 32-sample tick counted across host
  blocks, so tests that drive the chain behave like the plugin and the output does not depend
  on block size
- Latency target under 1 ms, nothing reported to the host (0.1 to 0.3 ms)

## Amps and levels

- Amps are matched by loudness (LUFS) for a guitar at -24 dBFS RMS, not by RMS: a clean amp
  follows the guitar dB for dB and a clipping one hardly does, so they can only meet at one
  level. Brøl and Torden are themselves 3.3 LU apart at -30, so "within 3 LU from -30 to -18"
  cannot be met; held is 3 LU at the reference and 5 LU 6 dB either side
- Klar's power stage holds peaks with its own curve (`clean_to`: straight up to a knee, then
  bending), because the old curve bent from the start and gave -36 dB of distortion on a held
  note at the reference level (now -61)
- Brøl below Gain 5 is made up in front of its power stage so Gain 0 is clean on pick attacks
  and the safety clip is not what holds the peaks
- Switching amp fades out over 4.5 ms in front of the cabinet, switches at the silent sample,
  fades in; the cabinet crossfades over 10 ms. Two amps never run side by side
  while switching within a slot (double the oversampled cost); the one exception is the second
  amp (#177), a slot of its own that runs beside the first only while it is switched on, so a
  heavy and a clean amp can be mixed, and costs nothing while off. The six dials are shared
  parameters; each amp's own positions are kept by the editor (`amp-dials` and `amp2-dials`,
  atomics because nih-plug restores state on the audio thread)
- Cost figures are from a fast core; cores 2 to 9 of the development machine take 1.33 times
  as long, so the reports pin their thread

## Cabinet

- Designed at 48 kHz and resampled for other rates, so the sound matches across rates; the
  response is normalised to 0 dB average power from 100 Hz to 4 kHz
- The speaker resonance is a biquad behind a short IR (5 to 10 ms), fitted to the old 20 ms
  response within 0.4 dB from 60 Hz to 8 kHz. It is not applied to the player's own cabinets
- Mic is a shelf pair and Resonance a bell after the IR, skipped at 5.0 so the default is
  bit-identical
- More cabinets than amps: ten built in (`SPEAKERS`), picked
  on the same Cabinet stepper as files (each lane's, since #177), between `OWN` and the files.
  Not a parameter, like the file: one stepper, one stored choice. Each is a `CabModel` like the amps' own, so it
  is designed, levelled and resampled alike and the dials work on its own frequencies.
  They are not the amps' own: those were tuned with their amps and lie within 1 to 2 dB of
  each other per octave band, too close to be heard as a choice. Every two built-in ones
  are at least 3 dB apart in some octave band of a guitar (a test holds it;
  `speaker_report` prints how far), and the amps' own stay as they were
- No file dialog (the style guide forbids pop-ups): files in one folder, stepped through. The
  folder is found through the Windows known-folder call by a hand-written FFI declaration (it
  follows OneDrive), no crate. The choice is a persisted file name, not a parameter, not in
  presets, and stays when the amp changes
- Player cabinets are cut to 40 ms at every rate, trimmed to arrive within 0.2 ms, levelled
  like the built-in ones. Long ones run as a zero-latency two-stage convolver (head of two
  blocks direct, the rest in FFT partitions, spectra made off the audio thread); a tail at a
  decimated rate was measured and cannot beat it
- Taps reach the audio thread through one pre-allocated buffer with an atomic state; a swap to
  a long cabinet waits for its tail (the old one plays on)

## Pedals

- Gate: hard, 3 dB hysteresis, 0.5 ms opening ramp, 40 ms hold, no lookahead, its detector
  reads before the input gain. Preset gates: -54 dB is the highest threshold that leaves palm
  mutes at -30 dBFS alone; release, not threshold, sets how tight the rests are
- Drive: 720 Hz high-pass into a soft clipper with the dry signal added back, in the
  oversampled region before the preamp
- Fuzz (#11): a slim pedal with Fuzz and Tone only. Its level is matched inside instead of a
  third knob (a slim pedal holds two), so it is as loud as off at any setting. The window grew
  by one slim pedal rather than the rows changing. Gain of 26 to 56 dB into the asymmetric
  clipper of the drive (second-order antialiased), whose own average is fed back against its
  bias: hard playing moves the operating point and a dying note breaks up. Behind the cabinet
  it oversamples itself, since nothing else there does. What held its aliasing at -70 dB was
  not the clipper but a square wave's harmonics in the decimator's transition band (20 to
  28 kHz at 48 kHz), folding back just under the host's half rate: a fourth-order low-pass at
  6 kHz in front of the decimator took it to -92 dB. Clipping at twice the rate again was
  measured and gained nothing, at twice the cost
- Delay and reverb are additive (the dry signal is never scaled, so mix 0 and off are
  bit-transparent). The delay's two lines feed each other so the left-right offset does not
  grow; its odd repeats lose 3 dB in mono by design (#36). Bypass cuts the tails in 10 ms
- The chain reports `KeepAlive` while a tail rings, since `Tail(n)` needs the length ahead of
  time. VST3 hosts read the tail once, so the amp, pad and bells set `VST3_INFINITE_TAIL` in
  our nih-plug fork: a host that suspends silent plugins never suspends these (about 2 % of a
  core on silence)
- A mono host layout gets the left effect channel only: the sum combs by up to 6.7 dB
- The order is saved as ids, never a parameter. A new order fades out in 4.5 ms, swaps at the
  silent sample, fades in; a pedal that changes sides starts from rest

## Pitch shifter

- #179: the Pitch Shifter plugin's algorithm as a pedal, so the rig is self-contained for drop
  tunings. It moved to `suite_common::dsp::pitch` (tests and reports with it) so both use one
  copy. It stands in front of the amp only, first by default: pitch tracking works best on the
  raw guitar, and the gate stays first (it listens to the guitar as it arrives). It runs in the
  oversampled region like the other front pedals; a splice at 4x costs under 50 us, which a
  block at 192 kHz absorbs. A third pedal of three knobs is a wide one, so the window grew
  by one pedal (2747 px, the strip's gaps with it) and the persisted editor size key became
  `editor-state-rig6`, as before. Its paint is the Pitch Shifter plugin's orange (`PAINT_ORANGE`,
  added to `PEDAL_PAINTS`): the amp's other pedals leave no room between the ochre and the
  ember for a darker orange at the distance the paint test asks for. Its art is the midnight sun on the sea
- Not in the factory presets: they switch it off, as they do every pedal they do not use

## EQ

- #215: a free-standing tone control, because the amps' and cabinets' dials cannot fix a thin
  single-coil or a boomy bridge pickup or a room. Three simple bands and not a parametric:
  Low and High shelves, Mid a bell with a sweepable centre (Freq); the dials are 0 to 10 around
  a flat 5, like the amp's. Behind the cabinet by default (where a room or a speaker is put
  right), draggable in front of the amp (to cut mud before high gain). Shelves with Butterworth
  slope (Q 0.707), the bell Q 0.9: wide enough for a tone control, not a notch. Dials glide
  over 20 ms rather than snapping, and a band that is flat is not run at all, so the pedal at
  rest changes no bit
- Look: four knobs make a pedal of the equal width with its knobs in two rows (radius 17); Low
  over High on the left (the shelves), Mid over Freq on the right (the bell and its centre), the
  LED between the upper two where a trio has it. Its paint is the spare brown (`PAINT_BARK`)
  and its art basalt columns of different heights, the bars of an equaliser. The window grew
  by one pedal (3204 px, `editor-state-rig8`) and the strip's gaps with it; the print of the
  amp's art was regenerated for the new width. Not in the factory presets: they switch it off
- Its knobs are radius 15 (a trio's are 17) so the two rows have room between their tapes and
  ticks and above the switch; the columns are hexagonal prisms (a light top and two faces,
  the left darker) at staggered heights, kept under the lower captions and no white print
  in them, so nothing reads as a mark of the interface
- Rejected: Freq and Q per band (a real parametric: too much for the pedal's job); a pedal
  of its own paint (the spare brown was there); dials shown in dB and kHz (the amp's dials are
  0 to 10 and the host sees the same)

## Tuner

- Muting is the behaviour: the amp's input and its output fade in 10 ms so tails are silent
  and a tuned string does not end up in the delay. `filter_state` drops `tuner` from a loaded
  state so a project never opens muted. Bypass wins while tuning (the dry signal is heard)
- Detector at about 5.5 kHz: normalised difference function for the rough period, a harmonic
  check against octave errors and chords, then the phase advance of the first three harmonics
  for the exact pitch (interpolating the difference function cannot reach a cent on high notes
  at that rate). The median of three readings is shown
- A note is shown only after a pluck (a 25 ms frame 12 dB over the quieter of the two before
  it; 6 dB to change notes). 25 ms is 1.5 periods of 60 Hz and 1.25 of 50 Hz: with 10 ms
  frames hum swung 5 dB from frame to frame. Hum that sets in at once must also fall 3 dB from
  0.2 s to 1 s, or the note is let go (#18; what remains is #44)

## Metronome

- #181: a click for practice, added to the output after the chain (not before Output as the
  spec's first words had it: the Output knob would then turn the click with the amp, and the
  safety clip would act on it; after the chain the click has its own Volume and never
  distorts). Bypass leaves it alone so a player can practise with the amp off; the tuner
  silences it. It is not a pedal and not in presets: a preset is a sound, a click is the room
- One clock for both modes: a count of samples to the next beat in `f64`. Following the host,
  each block is placed afresh from the host's position (exact), so a loop or a jump is followed
  and no state can disagree with the host. Rejected: a phase locked loop on the host's position
  (needless, positions are exact) and a clock in beats (a tempo change would shift the beats
  already placed)
- 6/8 clicks eighths (six to the bar) with one accent: simple and what a player expects of a
  click; no medium accent on the fourth
- The tempo is whole bpm in the display and by tap; the parameter is a float so a host can
  automate between. The Sync button is shown in the standalone program too: nih-plug's
  standalone has a transport (120 bpm, running) that it follows
- The cluster sits between Save and Output so the strip's order reads: what is picked, the
  practice click, what goes out. The strip's gaps shrank (the window did not grow) and the
  cluster's parts stand closer than the strip's others, so it reads as one thing

- The click's own signal (follows #181: "unaffected by the currently running preset"): presets
  already never touch the click's settings, so what was left was that the click shared the main
  output with the amp. It now can have an output of its own, `metronome_out`: Mixed (default,
  nothing changes) or Separate, on an auxiliary stereo output `Metronome` that the DAW routes by
  itself, with the main output the amp alone. Decided without asking: a switch and not a
  second plugin or always-on aux, because a host that cannot use aux outputs keeps working;
  Separate falls back to mixing where there is no usable such output (always in the standalone program, which has no aux output and whose Click out stepper is left out), because
  a click you cannot hear is worse than one in the wrong place. Not a channel pair of the
  main output: a host would have to route the amp's own pair to the same place. The stepper
  stands under the sound beside the meter, because the column had no room for a third row
  without growing the strip; the strip's gaps gave up 10.5 points each instead

## Wah

- #240, #243: a wah on the board, in front of the amp only (`wah_pedal` is the rocker, 0 heel to
  10 toe). Look picked on #242 out of three: a treadle drawn in perspective, a slider with a
  chrome cap, and one very large knob; the owner chose the slider. It is a variant of the
  crossfader (`ui::rocker`): the treadle is wider than a fader's cap and ribbed, the toe is at
  the top and the heel at the bottom, as the foot would have it, and the ticks are the same
  every tenth. Not a perspective treadle (the closest to a real wah, the most to draw, the only
  one that could move by itself in Auto and so cost repaints) and not a big knob
- The pedal's own paint is brass (`PAINT_BRASS`), the nearest hue to the pick's mock-up that is
  at least 45 apart from every paint of the board and holds cream print; the art is a waterfall
  (`Motif::Foss`) at the foot, left of the switch, so the middle (Heel) and the corner (Auto)
  stay bare. Bite over Sens in a column on the left at radius 14, smaller than a trio's, as the
  slot takes the middle; on the right the lamp, the rocker's value on tape and the Auto push
  button (the Delay's Sync idiom) with its lamp above it and its caption under it
- Auto is a push button, not a stepper or a second switch: it is one of two modes of one
  pedal, the footswitch stays the pedal's On. The rocker's caption says Rest while Auto is on,
  since the cap is then where the pedal comes back to and not where it is; the cap does not move
  by itself (a moving cap would need a repaint every frame, against the style guide's rule)
- The window grew by one pedal (3411 px: the gap between pedals is the same 27 px, so the
  window grew by a pedal and a gap, `editor-state-rig11`); the strip's wide gaps with it, the
  print of the amp's art regenerated for the new width. The default zoom is still 0.55, so the
  opening size on a 1920 px screen is 1876 x 358. Not in the factory presets: they switch it off

## Look

- Two lanes, not a tower (#177, picked by the owner out of three: two heads on one cabinet, one
  head with a flip, two lanes): each amp is a lane of its own, a head with the cabinet it plays
  through beside it, because the player mixes any amp with any cabinet in each branch, and the
  lanes meet where the signal does, on a black plate with the Blend as a crossfader, the one
  control that is a balance between two paths. The window grew 250 px for it
  (`editor-state-rig7`) and the strip's gaps with it, the print of its art regenerated for the new
  width. The dials went to radius 15 and the cabinet to 120 px to fit one lane beside the plate. The
  cabinet's Speaker and Mic Type, which stood on its plate, moved: the cabinet's choice to its
  lane's line under the dials, the microphone to the shared plate. The second amp's cabinet is
  its own or a built-in one: a file is the first amp's only
- The plate has three zones, ruled apart. Blend leads: a silk frame of its own, the longest
  slot and the biggest cap in the rig, large Amp 1 and Amp 2 at its ends and its value on a tape
  beside the slot, so it cannot be read as Amp 2's. Under it a frame, Cabinets, for what both
  cabinets share: Mic, Res, Mic Type and the Cab switch, whose LED says whether both cabinets
  are in the signal. Last, under a rule, the plate's one output jack, its own zone, so the
  cable that leaves is the plate's. Mic and Res are radius 15, the lane dials' size, not the 19 of a
  secondary row in a frame: the plate is 148 px wide and two 19s with their ticks do not fit
  inside a frame. All four screws are on, as `mounted_plate` puts them
- Amp 1 has no switch and Amp 2 has On and a lamp: the first amp is always in the signal (its
  lamp is always lit, the rig's one exception to a footswitch per path), and Amp 2 with its
  switch is what Blend balances it against
- A lane's captions (Cabinet, In) are 15 px, as large as the head's panel lets them be

- The strip has 50 px small footswitches with the LED beside the switch; its band tilts -1.5
  degrees, pedals -4. The amp is picked with a stepper, not a switch per amp: three switches
  left no room for a fourth amp. Output stays on the strip next to the output meter and Clip:
  it is the plugin's level, not the amp's (#97)
- Each amp has a head of its own, so the amp that plays is seen at a glance as on a real
  stage, and stepping through them shows what changes before it is heard. The looks are in
  `looks.rs`, apart from `amp/model.rs`: tuning a sound never touches a look, and the other way
  round. They are all our own: no maker's logo, script or plate shape. The amp is picked on a
  nameplate on the grille, which replaces the Amp stepper of #97: the name is the control. The
  panel's one line of knobs left no room for full scales at radius 19 with the jack and the
  pilot lamp beside them, so the dials are radius 18, and a scale is measured by where its
  ticks end, not by a circle round it (#99)
- The window's size is persisted under a key of its own (`editor-state-rig12`); it is renamed
  whenever the default size changes, so a project saved before opens at the new size. The board is
  drawn in 3411 x 584 points on `ui::rig_layer()` and scaled by a layer transform (`0.4` to `1.0`,
  opening sized to the screen up to the full `3411 x 584` or `1876 x 321` px on a `1920 x 1080` screen) inside `ResizableWindow` with the `3411 : 584` proportions
  locked (#227). The slim strip (#267) made it 66 points lower than the 650 it was: the pedals and
  the stack kept their size
- The strip is slim (64 px, #261 direction A "Signal Line"): the controls that were on it all
  at once are behind icons and plates (#258, #259): presets, the metronome and the settings open
  as hardware plates under the strip, one at a time; the Output knob's caption is over its meter
  (the pair is named once, as the input's is), where the pick has it over the knob, because the
  slim strip has no room for a caption over a knob with its scale
- The gate's `OPEN` lamp is its own small lamp: a half-lit pedal LED was built and rejected
  because at 30 % the lens is about the colour of the paint
- Rigs use `rig_led` (black socket, darker unlit lens): a plain unlit `led` disappears on dark paint

## Not yet exercised

Some of this was only run on synthetic signals or in the standalone window; #21 is the check in
a real DAW: mouse clicks on the stepper, saving and restoring the preset index and the cabinet
file in a project, the background task in VST3/CLAP hosts, macOS and Linux folders, a
OneDrive-redirected Documents folder, `non_automatable` and the tuner state filter, the
repaint rate of lamps and tuner, and host undo (#16).
