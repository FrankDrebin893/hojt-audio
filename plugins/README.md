# Plugins

- **Pitch Shifter** (`plugins/pitch_shifter`) - Real-time guitar pitch shifter (effect): semitones -12 to +12 by variable-rate playback with waveform-matched splices. Bypass is a shift of 0 semitones. Its CLAP ID and VST3 class ID predate the suite and are kept as they were. Details, algorithm and tools: `plugins/pitch_shifter/AGENTS.md`.
- **Drum Synth** (`plugins/drums`) - Fully synthesized drums (no sample files), MIDI in, stereo out, General MIDI map; kits Rock, Jazz, Metal, Groove (new kits go at the end: a project remembers its kit by its number). Details, note map, parameters and tools: `plugins/drums/AGENTS.md`.
- **Bass Synth** (`plugins/bass`) - Plucked string bass, one note at a time, voices Metal (picked) and Groove (finger). Details: `plugins/bass/AGENTS.md`.
- **Shimmer Pad** (`plugins/pad`) - Slow ambient pad, twelve voices, with a chorus and a shimmering hall. Details: `plugins/pad/AGENTS.md`.
- **Glass Bells** (`plugins/bells`) - Soft glassy bells with a ping-pong delay and a hall, twelve voices; note-off does nothing. Details: `plugins/bells/AGENTS.md`.
- **Guitar Amp** (`plugins/amp`) - A guitar amp with a full signal chain, all our own designs: own names, no real makers' names, trademarks or circuit names anywhere (code, comments, docs, commit messages; `plugins/amp/tests/rules.rs` holds this against a list kept as hashes). Four amps (Klar clean, Brøl crunch, Torden high gain, Glød edge of breakup), a gate, compressor, overdrive, fuzz, four modulation pedals, delay and reverb, a cabinet (built-in or the player's own WAV), a tuner, factory presets and a pedal order the player sets. Its signal flow, parameter table and report tools are in `plugins/amp/AGENTS.md`, read when working there.

# Adding a plugin

Each plugin keeps DSP in its own module(s), separate from `lib.rs` (plugin entry point, parameters, nih-plug integration) and `editor.rs` (egui GUI).

1. Create `plugins/<name>/` with a `Cargo.toml` that uses the workspace dependencies (copy `plugins/drums/Cargo.toml`)
2. Add a `[<name>]` entry to `bundler.toml`
3. Use `suite_common::VENDOR`. In the editor, call `ui::install` in the build closure and draw with
   `ui::pedal` and a `PedalStyle` (own paint colour in `ui/theme.rs`, model number, ornament). Look and
   style rules: `crates/suite_common/src/ui/STYLE.md`
4. Pick a unique `VST3_CLASS_ID` (16 bytes) and `CLAP_ID`; DAW projects reference plugins by these
5. Give it `no_alloc` tests (see `AGENTS.md`, Code Conventions) and an `ids` test (see Versioning in `AGENTS.md`)
