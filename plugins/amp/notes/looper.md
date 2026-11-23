# The looper

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

- **The looper (`looper.rs`, #280)** sits in `process` after the chain and before the click, so a loop
  is what the player heard, never has the click in it and is not run through the amp again; Bypass
  and the Output knob are therefore already in it, and the tuner silences it (playback only). States:
  Empty, Recording, Playing, Overdubbing, Stopped. The buffer (60 s of stereo, 40 s above 96 kHz) is
  allocated in `set_sample_rate` and nowhere else; a change of rate empties the loop, the same rate
  leaves it, and `reset` leaves it too (a host that stops and starts does not take a performance away,
  and the loop runs on its own sample count, not the host's transport). A loop is counted in samples; the
  recorded start is faded in and its end out over 5 ms, so the seam is silent and there is no click.
  Empty it costs three atomic reads a block (the looper alone, not the amp: 0.02 us a block of 64; playing 0.55 us,
  overdubbing 0.64 us, under 0.05 % of a core; the amp's own `amp_report` is unchanged: `cargo test -p amp --release looper_report -- --ignored --nocapture`) and
  leaves the sound bit-identical (`lib.rs` tests).
  Closing rounds the length to whole bars of the metronome where it is on (`Metronome::bar_samples`,
  from the host's tempo too), at least one, cut to the buffer; the rest of a bar rounded up is
  silence, a recording shorter than 10 ms is thrown away. Overdub adds through `soft_limit` (quiet
  passes add exactly, a loud one cannot run away) with a 5 ms ramp in and out. Stop mutes the
  loop but it keeps counting, so Resume comes back in time; Clear fades out over 5 ms and then
  is Empty. Commands (`press`, `stop`, `clear`) are atomics taken at the start of the next block, so a
  press lands to the block, not the sample. `looper_level` is the one parameter (not in presets);
  the buttons are not parameters. The plate (`ui::ring`: a footswitch in a ring of 32 lamps, red fill
  while recording against `LooperControl::capacity`, a head going round while playing, Stop, Clear, Level) and
  the strip's looper icon with its lamp and switch are built; the window asks to be drawn 30 times a second
  only while the plate is open and the loop records or plays, and 20 times while it records with the plate shut.
  With the metronome on, the first press of an Empty looper does not start at once but waits for the
  metronome's next downbeat (`Metronome::samples_to_downbeat`, worked out ahead of the block the way
  `render` will place it, without moving the clock; `LooperSettings::downbeat_samples`): the sound
  passes untouched until that sample, which is the first of the loop. It is counted in samples, so
  it starts on the beat to the sample, at the host's tempo and position too when the click follows
  them. Nothing waits when the click is off, just switched on (its downbeat is the first sample), or
  follows a host that is stopped (no downbeat is coming): a record starts at the press. While it
  waits the state is still Empty (the plate shows nothing yet, `Looper::is_idle` is false so the
  plugin stays awake), and a second Record press or Clear takes the press back. There is no count-in.
  The loop does not follow the host's transport (decided on #280)
- **Bars (#450, `looper_bars`):** Free, 1, 2, 4 or 8 bars of the click. `lib.rs` hands the looper
  `LooperSettings::length_samples` = bars times `bar_samples`, only while the click is on (click off
  reads Free). The length and the bar are **fixed when the recording starts** (`Looper::fixed`), so a tempo
  change while recording does not move them; longer than the buffer it is the whole bars that fit,
  and a bar longer than the buffer is Free. When the recording reaches the length it closes itself
  with exactly that length (`close` with the bar, the same fade as a press) and the loop plays from
  its first sample on the very next one, in the same block, with no press. A press before that
  closes early, rounded to whole bars of the bar it started in. The ring fills against that length
  (`LooperControl::fills_against`, published as `goal` while recording), so it is a full circle at the
  close; Free fills against the buffer as before
- **Click while playing (`looper_click`):** Always (as it was) or Rec only. With Rec only `lib.rs` sets
  `MetronomeSettings::hushed` while the looper is Playing, Overdubbing or Stopped (read after the
  looper's block): the click's clock runs on and only its voice is faded over 5 ms (`Metronome::gate`,
  1.0 and bit-identical when never hushed), so it is back on the beat after Clear. The loop never holds the click either way
- The plate (500 x 264) carries a click row under the ring: the metronome's own on-switch and tempo
  (`metronome_on`, `metronome_bpm`, the tempo display is `tempo_display` shared with the metronome's plate), the Click
  stepper and the Bars stepper; the last two are dimmed (hover text says why) while the click is off
