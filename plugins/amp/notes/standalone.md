# The standalone program and the exe's icon

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

- `src/standalone.rs` - The standalone program's settings (`Prefs`, saved as
  `amp-standalone.json` in the system's settings folder or `HOJT_CONFIG_DIR`), the `Host` trait
  `main.rs` implements with cpal to list devices and restart, and the Audio panel's choices. The
  panel itself is the rows of the settings plate in `editor/settings.rs` (`settings_plate`). nih-plug's host opens its streams from its
  command line; with WASAPI (and the other cpal drivers but ASIO) Apply changes the devices, channels, buffer size and sample rate while the program plays (`Host::reconfigure`
  -> the fork's `nih_standalone_reconfigure`: it closes the cpal streams, opens the new ones with the
  same process callback; when the rate or the buffer size is not the one that played, the fork
  initialises the amp again in the gap with the streams closed (`Backend::run`'s `reinit` ->
  `Wrapper::reinitialize`: smoothers set for the rate, `initialize`, `reset`), so the chain, the metronome
  and the looper (a new rate empties its loop) are made for it off the audio thread and `no_alloc`
  still holds; a failure, devices that would not open or an `initialize` that said no, opens the old
  ones again, initialised with their rate and buffer, and the panel shows the
  error, and the panel gives up on an audio that does not answer in `ANSWER_TIMEOUT`, 10 s;
  `AudioChoice::apply`, `poll`). A `None` rate or buffer is what the command line leaves out: the
  output device's own rate, the default 512. ASIO still restarts (its driver cannot be opened again in the same
  process, tried on a Focusrite; `Prefs::changes_live`), and so does a new driver
  (`Prefs::same_driver`): Apply saves and starts the program again, as `Host::restart`; no host is installed in a DAW and
  the editor then shows no Audio button. Eight rows: Driver, Input, Input channels, Output,
  Output channels, Buffer, Sample rate, Test tone. Channels are the amp's stereo pair (the first
  layout) out of the device's, counted from 1 (`--input-channel`/`--output-channel` of the fork's
  host; a device that changes puts them back to 1; `Host::channels` says how many the device
  has). The test tone is live, not part of Apply: `Host::set_test_tone` ->
  `nih_standalone_set_test_tone` (the fork's host puts a 440 Hz sine, 0.1 of full scale, over the
  main output) and `AudioChoice::close` turns it off. ASIO is the cargo feature `asio`
  (the feature `asio` of `amp` turns on nih-plug's `asio`, which turns on cpal's): Windows only, and the Steinberg ASIO SDK has to be
  at `CPAL_ASIO_DIR` at build time. The SDK (2.3.4, GPLv3 alternative of its dual licence, host side
  only) is vendored in `third_party/asiosdk`: `scripts/build-standalones.sh` builds the six
  standalones the installer ships with it on Windows (feature `asio` on each plugin, set by the
  script; `HOJT_NO_ASIO=1` leaves it out), so the Windows job of `release.yml` builds ASIO. Locally,
  `cargo run -p amp --features asio` with `CPAL_ASIO_DIR=third_party/asiosdk` (or another SDK folder). Notes from trying it on a Focusrite: the driver takes the buffer size set in its own
  control panel (the fork brings the size asked for into the driver's range), the panel's
  device list and channel counts are read once before the streams open (`AsioSnapshot` in
  `main.rs`: asking a running ASIO driver hangs the window), and Apply starts the program again
  from a new process and exits at once, since closing the window with ASIO streams open hangs
  WASAPI's shared mode hands the output callback more frames than the period asked for (528 against 512)
  and the count can change; the fork's cpal backend walks such a callback in chunks of at most the period (#250).
  The ASIO path is untouched by that
- The exe carries the HØJT badge (`assets/amp.ico`, `assets/amp.rc`, `build.rs`) and
  `give_window_the_icon` in `main.rs` sets it on the window, which the framework makes without one.
  `amp.ico` is made by `scripts/make-icon.py` (one drawing per size from `site/hojt.svg`'s geometry, the
  small print dropped below 32 px), never scaled from the page's picture; `tests/icon.rs` holds its sizes,
  its transparent surround and its sharp edges. Explorer may show the old icon from its cache after an
  update: `ie4uinit.exe -ClearIconCache`
