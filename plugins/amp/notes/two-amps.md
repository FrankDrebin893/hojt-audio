# Two amps

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

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
