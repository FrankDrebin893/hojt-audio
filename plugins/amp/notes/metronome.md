# The metronome and the click's own output

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

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
