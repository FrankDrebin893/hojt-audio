# nih-plug is our own fork, pinned to one commit

The plugins are built on nih-plug, but not on upstream: `FrankDrebin893/nih-plug` is upstream plus a few commits we needed, kept on the branches `hojt-vst3-infinite-tail` and `hojt-standalone-audio` and pinned by commit in the root `Cargo.toml`. A bump moves every plugin at once.

What the fork carries, and why upstream was not enough:

- `Vst3Plugin::VST3_INFINITE_TAIL`: VST3 hosts that read the tail length once, before the first block, would see 0 and cut delay and reverb tails (#20).
- `nih_plug_egui` paints a frame only when asked (#23), so an editor does not cost a core (see [0006](0006-editors-paint-on-demand.md)).
- The standalone host (the plugin as a program, without a DAW): `--input-channel` and `--output-channel`, the `asio` feature, one device for ASIO input and output, a capture callback that never waits, the buffer size taken from the driver, only the channels the plugin uses opened, a cpal callback longer than the buffer size processed in chunks for WASAPI shared mode (#180, #250), `nih_standalone_reconfigure` for changing devices, channels, sample rate and period size while it runs (#247), a sample rate that defaults to the output device's own, and `nih_standalone_audio_status()` so an editor can say when the audio is the dummy (#356).

The root `Cargo.toml` also names `egui-baseview` at nih-plug's own revision, only to turn on its `windows_keyboard_workaround`: without it an editor, which is a child window of the host's, never takes keyboard focus on Windows.

## Considered options

- Upstream plus workarounds in the plugins: not possible for the VST3 tail and the repaint, which sit inside the framework.
- Forking `vst3-sys` as well: turned down (#161). Do not propose it again.

## Consequences

To move to a newer upstream, rebase the fork's commits onto it and pin the result. A change to the fork is made in a clone of its own and pinned with `scripts/pin-nih-plug.sh` (`plugins/README.md`, "A change in the nih-plug fork"). The tag `hojt-1` keeps the first commit; never delete it.
