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
  plugin stays awake), and a second Record press or Clear takes the press back. A count-in (#451, below) is the same wait, longer.
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
  (`metronome_on`, `metronome_bpm`, the tempo display is `tempo_display` shared with the metronome's plate) and the Click
  stepper; the Bars stepper stands under the Level knob and the Count-in stepper under Clear (#451, the plate has no
  room to grow); Click, Bars and Count-in are dimmed (hover text says why) while the click is off
- **Count-in (#451, `looper_count_in`):** Off, 1 or 2 bars. `lib.rs` hands the looper a `CountInPlan`
  (`LooperSettings::count_in`) only while the click is on: `length` is the bars times `bar_samples`, `lead` the
  samples to wait before it begins. A Record press on an Empty looper then arms the existing `armed` countdown
  with `lead + length`, so the recording starts on the sample after the count-in, a downbeat, and the sound passes
  untouched meanwhile. A click that runs by itself (no host tempo followed) is **restarted at the press**
  (`Looper::take_restart`, taken by `process` after the looper's block, then `Metronome::restart`), so the press is
  beat one and `lead` is 0. A click that follows the host keeps the host's clock: `lead` is the wait to the host's next
  downbeat (`samples_to_downbeat`), and with the host stopped there is no count-in (the press records at once, as
  it does with no downbeat coming). A second press or Clear takes it back (nothing restarts the click again).
  The plate reads `LooperControl::count_in` (`(beat, beats a bar)`, beat from 1, zero while still waiting for the
  host's downbeat): the ring lights the beats in amber round the circle once a bar and the words read Count 1 to
  Count n; it asks to be drawn 30 times a second meanwhile. Bars and the count-in go together: count in, record the
  set bars, close, play. The beat is worked out in `publish` from the samples left, so it follows the sample count
- **Three tracks (#452):** track 1 is the loop above (`Looper`'s own buffer, state and `len`); tracks 2 and 3 are
  `Extra`s of its length with a pair of full-size buffers each, so three buffers in all, allocated in `set_sample_rate`
  and nowhere else (about 69 MB at 48 kHz, 92 MB at 96 kHz; `vec![0.0; n]` is zeroed memory the system hands out as it is
  used, so an unused track costs address space, not memory. The fallback, tracks 2 and 3 at 30 s, is not needed and
  not built; `set_sample_rate` is the one place to do it). They have **no position or length of their own**: they read and
  write at track 1's `pos`, so they cannot drift from it, and a change of rate or Clear of track 1 empties them all
  (`empty`, and a fade-out together when playing). A press on track 2 or 3 does nothing while track 1 holds no loop (Empty,
  Recording or clearing); it then waits (`Extra::waiting`, published as `LooperControl::track_waiting`, still `Empty`)
  until `pos` is 0, records exactly one pass (faded in and out over 5 ms at the pass's ends) and is Playing by itself on
  the sample after the last. A press during the pass closes it early (`close_early`: the played part faded out, the rest of
  the loop silent; under 10 ms is thrown away). Each track plays, overdubs (`soft_limit`, 5 ms ramps), mutes and clears on its own;
  the output is the live sound plus the three added and clamped. Commands are per track (`LooperControl::press_track`,
  `stop_track`, `clear_track`; `press`, `stop`, `clear` are track 1's), the states per track (`track_state`).
  `looper_level` is track 1's level, `looper_level_2` and `looper_level_3` the others (same range, not in presets).
  `LooperControl::selected` is the plate's choice of track (the editor's, kept here so it survives the window
  closing). The plate: three tabs on the title's line (lamp colour of the state, number on it, brass ring on the selected
  one); the footswitch, ring, Stop, Clear and Level act on the selected track; the ring of a track beside the first
  fills red as the loop goes round while it records, and a red head goes round while it waits. The click's Rec only counts
  a track that records or waits as recording (`extra_recording`), so the click is heard through the pass. Tests:
  `looper/tracks_tests.rs`, the plate's in `editor/tests.rs`, `no_alloc` in `lib.rs`
