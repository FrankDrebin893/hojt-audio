# Releasing

The rules `/release` and `/page` follow. The page is https://hojte.net/hojt-audio/ and the downloads are GitHub releases.
The code is GPL-3.0-or-later, which the VST3 bindings require of a VST3 binary that is handed
out; the HØJT name and logo are kept out of it (`NOTICE.md`).

1. Set the version in `Cargo.toml` (`[workspace.package]`) and run `cargo check --workspace` so
   `Cargo.lock` follows
2. Add a `## <version>` section to `CHANGELOG.md`: it becomes the text of the release
3. For every plugin whose sound changed in that section, run `scripts/site-audio.sh <plugin>`
   (`drums`, `amp`, `pitch_shifter`, `bass`, `pad`, `bells`) and listen to what changed in `site/audio/`. If a plugin's
   look changed, or it is new, run `scripts/screenshots.ps1 <plugin>` and look at what it took
4. Run `/page release <version>` so the page describes exactly that version: new plugins,
   controls tables, sound examples
5. Commit on a branch `release/<version>` and merge it through a pull request (`main` is
   protected). Then tag the merge commit `v<version>` and push the tag. `release.yml` tests,
   builds for Windows, macOS (Intel and Apple Silicon in one binary, signed ad hoc) and Linux,
   refuses a bundle without its artwork (`cargo xtask check-art`), and publishes the zips

- The zips are named `<BundleName>-<windows|macos|linux>.zip`, without a version: the page
  links to `releases/latest/download/<name>`, so these names never change
- To see the zips without releasing, run the Release workflow by hand (Actions tab) and take
  them from the run's artifacts
- `Cargo.lock` is committed so a release is built from the crates that were tested
- The page uses the plugins' colours (`ui/theme.rs`) and fonts (copies in `site/fonts/`). Body
  text never sits on the paint: cream on orange or teal is too weak at that size
- The page is published from `main` and the downloads are the latest release, so
  `site/audio/` and `site/img/` only change in the commit of a release, or to catch up with the
  release that is out: a branch that changes a sound or a look leaves them alone, or the page
  shows what nobody can download yet
- Sound examples in `site/audio/` are MP3s (`ffmpeg -codec:a libmp3lame -q:a 4`) of files from
  the `render_*` tests, never made by hand. The table in `scripts/site-audio.sh` says which
  render each one is. An example that has not changed comes out the same to the byte. The one
  exception is the Pitch Shifter's four (`pitch-riff*.mp3`): recordings of the plugin in a DAW,
  as played and shifted down, which `scripts/site-audio.sh` leaves alone
- An example can be our own playing through the plugin: the recording or MIDI file goes in
  `site-sources/` and its name in that table. The sound is still made by the script
- Screenshots in `site/img/` are the standalone windows at their own size, taken by
  `scripts/screenshots.ps1` (it builds each one, opens it and captures its inside), never drawn
  or generated. It needs a display at 100 % scaling and stops otherwise
