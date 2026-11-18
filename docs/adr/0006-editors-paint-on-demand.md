# Editors paint only when something asks for it

An editor's window is painted on input, when the host changes a parameter, in the first second after it opens, and when the editor calls `request_repaint` or `request_repaint_after`. It is not painted every frame: that costs a good part of a core, for a plugin that sits open in a DAW all day. This took a change in the fork (`nih_plug_egui`, #23; see [0001](0001-nih-plug-is-our-own-fork.md)).

## Consequences

Anything an editor shows that changes on its own (meters, lamps, the tuner, a gliding pedal, a file read in the background) must ask for a repaint in the frame that draws the change, and only then. A new moving element that forgets to ask looks frozen. The rule for editors is in `crates/suite_common/src/ui/STYLE.md`.
