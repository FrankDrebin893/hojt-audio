# Plugins

- **Pitch Shifter** (`plugins/pitch_shifter`) - Real-time guitar pitch shifter (effect): semitones -12 to +12, chords as well as single notes, by a spectral shifter with a fixed latency of about 11 ms. Bypass is a shift of 0 semitones. Its CLAP ID and VST3 class ID predate the suite and are kept as they were. Details, algorithm and tools: `plugins/pitch_shifter/CLAUDE.md`.
- **Drum Synth** (`plugins/drums`) - Fully synthesized drums (no sample files), MIDI in, stereo out, General MIDI map; kits Rock, Jazz, Metal, Groove (new kits go at the end: a project remembers its kit by its number). Details, note map, parameters and tools: `plugins/drums/CLAUDE.md`.
- **Bass Synth** (`plugins/bass`) - Plucked string bass, one note at a time, voices Metal (picked) and Groove (finger). Details: `plugins/bass/CLAUDE.md`.
- **Shimmer Pad** (`plugins/pad`) - Slow ambient pad, twelve voices, with a chorus and a shimmering hall. Details: `plugins/pad/CLAUDE.md`.
- **Glass Bells** (`plugins/bells`) - Soft glassy bells with a ping-pong delay and a hall, twelve voices; note-off does nothing. Details: `plugins/bells/CLAUDE.md`.
- **Guitar Amp** (`plugins/amp`) - A guitar amp with a full signal chain, all our own designs: own names, no real makers' names, trademarks or circuit names anywhere (code, comments, docs, commit messages; `plugins/amp/tests/rules.rs` holds this against a list kept as hashes). Ten amps (Klar clean, Brøl crunch, Torden high gain, Glød edge of breakup, Sang singing lead, Drøn loose and heavy, Knald thick warm mid-gain, Gnist bright chimey mid-gain, Skarp crisp high gain, Hvas hot-rodded high gain), a gate, pitch shifter, compressor, overdrive, fuzz, four modulation pedals, delay and reverb, a cabinet (built-in or the player's own WAV), an optional second amp with a cabinet of its own and a Blend between the two (a heavy and a clean sound at once, #177), a tuner, factory presets and a pedal order the player sets. Its signal flow and rules are in `plugins/amp/CLAUDE.md`, a router, and the parameter table, report tools and the rest in the topic files of `plugins/amp/notes/` it names, read when working there.

# Adding a plugin

Each plugin keeps DSP in its own module(s), separate from `lib.rs` (plugin entry point, parameters, nih-plug integration) and `editor.rs` (egui GUI).

1. Create `plugins/<name>/` with a `Cargo.toml` that uses the workspace dependencies (copy `plugins/drums/Cargo.toml`)
2. Add a `[<name>]` entry to `bundler.toml`
3. Use `suite_common::VENDOR`. In the editor, call `ui::install` in the build closure and draw with
   `ui::pedal` and a `PedalStyle` (own paint colour in `ui/theme.rs`, model number, ornament). Look and
   style rules: `crates/suite_common/src/ui/STYLE.md`
4. Pick a unique `VST3_CLASS_ID` (16 bytes) and `CLAP_ID`; DAW projects reference plugins by these
5. Give it `no_alloc` tests (see `CLAUDE.md`, Code Conventions) and an `ids` test (see Versioning in `CLAUDE.md`)

## A change in the nih-plug fork

The pinned nih-plug is our fork, `FrankDrebin893/nih-plug` (see CLAUDE.md, Code Conventions). A change to it is made
in a clone of its own, never in the shared checkout (`/home/user/nih-plug` is not usable). From the worktree, each
as a plain command of its own (no `cd ... &&` chains, no `git -C`, which the worktree guard refuses):

1. Clone it to a spelled-out absolute path in the scratchpad: `git clone https://github.com/FrankDrebin893/nih-plug.git <scratchpad>/nih-plug`
   (the scratchpad's full path from the session's environment), then `cd` there and check out the branch the
   change belongs to (`hojt-standalone-audio` for the standalone host).
2. Change it, commit it and test it there: `cargo test --features standalone` in the fork (the tests of the
   crate the change is in).
3. Push the branch. The pin is the branch's tip, so a change on top of it is a new commit.
4. Back in the worktree: `scripts/pin-nih-plug.sh <the 40-hex commit>` replaces the rev in `Cargo.toml` and
   `Cargo.lock` and the short commit CLAUDE.md names, and prints what it changed; then
   `cargo check --workspace --locked` proves the lock still resolves. Add the change to CLAUDE.md's list of what
   the fork carries.

## Versioning

A new build replaces the old one in place: install it over the old bundle and every DAW project
picks it up, with its settings. For that to hold, these never change once a plugin is in use:

- the bundle name in `bundler.toml`
- `NAME`, `VST3_CLASS_ID` and `CLAP_ID` in `plugins/<name>/src/lib.rs`
- the `#[id = "..."]` of every existing parameter (adding parameters is fine; removing or renaming
  an id loses that setting in saved projects)

Each plugin's `ids` test holds these, with the keys of its `#[persist]` fields, against its
`ids.txt` (`suite_common::ids`). A plugin that gains a parameter: run
`UPDATE_IDS=1 cargo test -p <name> ids` and commit `ids.txt`. Never do that for an id that changed.

The version number lives in `Cargo.toml` (`[workspace.package]`) and is what hosts show.

Only to compare two builds side by side in the DAW: temporarily give one of them another bundle
name, `NAME`, `VST3_CLASS_ID` and `CLAP_ID`, and do not commit that.
