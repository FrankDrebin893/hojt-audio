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
- The cabinet looks as the cabinet that plays (#100), but its box keeps one size and its
  control plate one place for all twelve looks: the pedals stand beside it as tall as it, the
  head stands on it, and the cables, the plate's controls and every click stay where they are
  whatever is picked. A real 1x10 and 4x12 differ in size; here only the speakers behind the
  grille do. What a missing or unusable choice shows is the amp's own cabinet, because that is
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
  fades in; the cabinet crossfades over 10 ms. Two amps never run side by side (double the
  oversampled cost). The six dials are shared parameters; each amp's own positions are kept by
  the editor (`amp-dials`, atomics because nih-plug restores state on the audio thread)
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
  on the same Speaker display as files, between `OWN` and the files. Not a parameter, like
  the file: one display, one stored choice. Each is a `CabModel` like the amps' own, so it
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
  Separate falls back to mixing where there is no such output (the standalone program), because
  a click you cannot hear is worse than one in the wrong place. Not a channel pair of the
  main output: a host would have to route the amp's own pair to the same place. The stepper
  stands under the sound beside the meter, because the column had no room for a third row
  without growing the strip; the strip's gaps gave up 10.5 points each instead

## Look

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
- The window's size is persisted under a key of its own (`editor-state-rig5`); it is renamed
  whenever the size changes, so a project saved before opens at the new size
- The gate's `OPEN` lamp is its own small lamp: a half-lit pedal LED was built and rejected
  because at 30 % the lens is about the colour of the paint
- Rigs use `rig_led` (black socket, darker unlit lens): a plain unlit `led` disappears on dark paint

## Not yet exercised

Some of this was only run on synthetic signals or in the standalone window; #21 is the check in
a real DAW: mouse clicks on the stepper, saving and restoring the preset index and the cabinet
file in a project, the background task in VST3/CLAP hosts, macOS and Linux folders, a
OneDrive-redirected Documents folder, `non_automatable` and the tuner state filter, the
repaint rate of lamps and tuner, and host undo (#16).
