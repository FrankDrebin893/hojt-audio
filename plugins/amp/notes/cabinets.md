# Cabinets: built-in and the player's own

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

- Besides its own cabinet an amp can play through any of the fourteen built-in cabinets
  (`SPEAKERS` in `amp/model.rs`: `1X10 SMALL`, `1X12 OPEN`, `2X12 OPEN`, `4X10 CRISP`,
  `4X12 MID`, `4X12 TIGHT`, `1X15 DEEP`, `1X8 TINY`, `2X12 PUNCH`, `4X12 HEAVY`, and the clear ones of the cabinets work that followed the ten: `1X12 CLEAR`, `1X10 SNAP`, `2X12 CHIME` and `FULL RANGE`, a woofer and a tweeter for clean and acoustic sounds, the one that is no guitar speaker; designs of their own, not the amps'), picked on the
  lane's `Cabinet` stepper after `OWN` (the second amp's has no file, only its own amp's
  cabinet and these). A built-in cabinet is stored as `speaker:<id>` in the cabinet choice;
  the ids are frozen (a test holds them), the order may change. One this build does not have
  plays the amp's own and is shown with `?`. Picking the cabinet that already plays changes
  nothing, to the bit. The chain designs them all when the sample rate is set
- Besides a built-in cabinet an amp can play through a WAV impulse response from
  `Documents/Hojt Audio/Cabinets` (or the folder in `HOJT_CABINETS_DIR`), picked on the cabinet.
  Mono or the left channel of stereo, 8/16/24/32-bit integer or 32/64-bit float, any rate; resampled,
  trimmed so the sound arrives within 0.2 ms, cut to 40 ms, levelled like the built-in ones.
  Latency stays the same
- The cabinet choice is a persisted file name or built-in id, not a parameter and not part of
  a preset. It stays when another amp is picked. A missing or unusable file plays the amp's own
  cabinet
- Files are read on a background task (`initialize` only asks for one, which the first block hands to the host's background thread), never on the audio thread; the taps
  reach it through `CabStage` without allocating, freeing or waiting
- The folder is created the first time the stepper is used, not by loading the plugin
- A 40 ms cabinet adds about 0.1 % of a core against the amp's own at any rate. Its late part is
  convolved in FFT partitions whose work is spread over the samples of a block: no added latency,
  the worst block as with the amp's own cabinet, output independent of the host's block size
- A swap to a long cabinet of the player's own starts its crossfade once its late part is there:
  about the length of the response after a short or built-in cabinet, two blocks after another
  long one. The cabinet before plays on meanwhile

- The built-in cabinets are designed at 48 kHz and resampled for every other rate (within
  0.05 dB of each other from 60 Hz to 10 kHz). A rate is designed once and kept
