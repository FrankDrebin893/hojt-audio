# The pedals: pitch shifter, EQ, wah, fuzz, compressor and cost

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

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
  up or down lags the input by a fixed time whatever the note, the largest of 5.3, 10.7, 21 and
  43 ms that fits under Tracking (a plucked 110 Hz string starts 10 ms later with Tracking
  15 ms, 5.5 ms with 2 ms, 40 ms with 50 ms; the dry part of Blend is not delayed). Nothing is
  reported to the host. Flat to 0.0 dB: the note that comes out is as loud as the amp playing it
  itself. The shifter works at 44.1 to 48 kHz whatever the oversampled rate (it steps down and
  up again), so the guitar comes out of it band-limited to 21 kHz. Cost: 3 to 5 % of a core
  over the amp (shift 0: nothing), in a frame every 2.7 ms that takes about a tenth of a
  64-sample block at 48 kHz and half of one at 192 kHz; the report is `pitch_report`
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
