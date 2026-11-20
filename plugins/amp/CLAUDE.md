# Guitar Amp

Loaded when working in `plugins/amp`. The suite-wide rules are in the root `CLAUDE.md`; the
reasons behind the amp's design are in `DESIGN.md` next to this file; open work is the
`plugin:amp` issues.

A guitar amp with a full signal chain, all our own designs: own names, no real makers' names,
trademarks or circuit names anywhere (code, comments, docs, commit messages). Twelve amps, each
with its own cabinet: Klar (clean, glassy), Brøl (mid-forward crunch), Torden (modern tight high
gain), Glød (warm edge of breakup, between Klar and Brøl), Sang (a smooth, compressed singing
lead), Drøn (a big, loose, dark amp with the low end let through), and, from #263, Knald (thick,
warm, slightly woolly mid-gain), Gnist (its bright, chimey cousin), Skarp (crisp, tight modern high
gain with a bite at 3 kHz and deep sag) and Hvas (dense, hot-rodded high gain, looser low end),
Træ (#400: an electric guitar made to sound acoustic, its "cabinet" the body of a guitar open to
9 kHz, nothing clipping at the reference level up to Gain 7.5; `trae_report` and
`chain/tests/acoustic.rs`) and Smæld (#413: clean with a snap, bright, tight and quick, whose power
stage keeps two notes played together apart; `clean_report` holds it against the others). They
share the six dials. New amps go at the end of `Amp`: ids are frozen (`test_amp_ids_are_frozen`).

This file is a router: it keeps what every amp change must know and says which topic file to read
before touching which source file. The topic files are in `notes/`, moved unchanged out of this file
(issue 370); a session that opens a file named in the index below reads its topic file first, and a
reviewer reads the topic files for the files in the diff.

## Rules every amp change holds

- Amps differ only by constants in `amp/model.rs`: never a `match amp` in the DSP (`tests/rules.rs`,
  `docs/adr/0003-amp-models-are-constants-not-code.md`)
- A `#[persist]` field is never a parameter, and an order, a preset choice or a file choice is never
  a parameter: keep it in a `#[persist]` field, as ids or a name
- The clippers' two-at-a-time path is only compiled in optimised builds: run
  `cargo test -p amp --release` too when touching them (`scripts/check.sh` runs
  `cargo test -p amp -p bass --profile optimised`)
- Parameter ids, variant ids, `#[persist]` keys, amp and pedal order lists are frozen, and new ones go
  at the end (`ids.txt`, each `ids` test)
- No real makers' names, trademarks or circuit names anywhere (`tests/rules.rs` holds it)
- Nothing allocates on the audio thread (`no_alloc.rs` holds it, for every pedal and both sides)
- A cloud session cannot open a standalone editor; reproduce an egui assertion or a layout problem with the editor's `egui_kittest` harness test (`suite_common::ui::snapshot`), and leave the look of the standalone to a manual-review line in the pull request.

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
- The clippers work on two samples at a time (`dsp/pair.rs`), the same to the bit as one by one.
  That path is only compiled in x86-64 builds without debug assertions (`--release`; the plain
  `cargo test` is optimised but keeps them): run `cargo test -p amp --release` too
  when touching the clippers
- `hound` is the amp's one dependency besides nih-plug; it reads the WAV files

## Notes by file

Read the topic file before you edit, or review a change to, the files in its row. A change that
crosses rows reads every row it touches. `notes/files.md` is the map of every source file, for
finding where something lives.

| Before editing or reviewing | Read |
|-----------------------------|------|
| `looper.rs`, the looper's plate (`looper_plate` in `editor.rs`), `LooperControl` | `notes/looper.md` |
| `metronome.rs`, the `metronome_*` parameters, the click's output (`metronome_out`) | `notes/metronome.md` |
| `tuner.rs`, `lamps.rs`, the level meters and signal lamps | `notes/tuner-and-lamps.md` |
| `cab.rs`, `user_cab.rs`, `cab_stage.rs`, `SPEAKERS` in `amp/model.rs`, the cabinet choice | `notes/cabinets.md` |
| `presets.rs`, `user_presets.rs`, `dial_memory.rs`, banks, saving a preset, switching amp | `notes/presets.md` |
| `slots.rs`, `layout.rs`, `extras.rs`, `effect.rs`, `effects.rs`, `order.rs`, `chain.rs` (pedal sides, order) | `notes/slots-and-effects.md` |
| `pitch.rs`, `eq.rs`, `wah.rs`, `fuzz.rs`, `comp.rs`, `drive.rs`, `delay.rs`, `reverb.rs`, `mods/` | `notes/pedals.md` |
| `amp/` (`preamp.rs`, `tonestack.rs`, `poweramp.rs`), `gate.rs`, loudness matching, a level or a tail | `notes/levels.md` |
| `second.rs`, the second amp, Blend | `notes/two-amps.md` |
| `lib.rs` (parameters), `settings.rs`, any parameter's id, default or range | `notes/parameters.md` |
| `standalone.rs`, `main.rs`, `build.rs`, `assets/amp.ico` | `notes/standalone.md` |
| `editor.rs`, `board.rs`, `bank.rs`, `looks.rs`, `art.rs`, anything drawn | `notes/look.md`, and `DESIGN.md` for why |
| A `*_report` or `render_*` test, a DSP change that is measured before and after | `notes/reports.md` |
| Where a file lives, what a module is for | `notes/files.md` |

`DESIGN.md` next to this file holds the reasons behind the amp's design.
