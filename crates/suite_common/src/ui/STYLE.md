# Look and style

Every plugin is drawn as a painted stompbox. Shapes, print and art are vector, drawn with egui's painter in
`crates/suite_common/src/ui/`. Lit surfaces (enclosure, knobs, screws) are rendered per pixel by a small
software renderer, `raster.rs`, and cached as textures; there are no image files.

- `theme.rs` - Colours, the three embedded fonts (`install`), text helpers. One paint colour per plugin,
  one per mini pedal of a rig (`PEDAL_PAINTS`), and `shade` for darker tones of a paint
- `widgets.rs` - `param_knob` (fluted knob, name, value on label tape; `param_knob_with` for another
  `KnobCap` and print colour), `panel_knob` (a knob on an amp head's panel: name above, value tape under,
  where there is room for one line of controls; `panel_knob_rect` is the room it takes),
  `nameplate_stepper` (a name on a plate with a chrome step button each side),
  `footswitch`, `led`,
  `small_footswitch`, `rig_led` and `param_switch` (the switch and LED of a rig, bound to an on/off
  parameter), `stepper` (label tape with a chrome button at each end, for picking one of many),
  `menu_stepper` (one whose tape drops a list open instead of stepping on) and `led_stepper` (the
  same on an LED display) with `drop_panel`
  (the plate of tape rows that list, the amp's preset banks, is picked from) and `type_into`
  (a name typed on a row of it, the one place in the suite that takes typed text),
  `chrome_button` (one such button alone, for an action such as Save),
  `cents_meter` (a row of lamps on label tape), `level_meter` (a column of ten lamps on label tape for a signal level, the
  lowest lit from -45 dBFS, the top one at full scale), `value_knob` and `value_switch` (a knob
  and a rig switch for a plain value that is not a parameter),
  `signal_lamp` (a small lamp with no switch, for what the sound is doing),
  `silk_frame`, `silk_label` (`print_label` for a caption in `INK` on metal), and `badge` (the HØJT logo: the Ø is a knob, its slash the pointer)
- `frame.rs` - `pedal`: enclosure, screws, jack captions, tilted name band, model number, logo.
  A plugin describes itself with a `PedalStyle`. For a window with several enclosures: `rig`
  (the bare bench), `strip` (shallow enclosure with the jack captions, model number and logo of a pedal,
  and no name band) and `mini_pedal` (small enclosure with only a title), each
  mini pedal described by a `MiniStyle`; `coupler` and `patch_cable` carry the signal between them
- `raster.rs` - Per-pixel lit surfaces (signed distance field + one fixed light): `box_image`, `knob_image`
  (a `KnobCap`: `Fluted`, the pedals' knob, `ChickenHead`, `CreamChickenHead`, `GoldTop`, `ChromeSkirt`), `screw_image`, and for the
  half stack `tolex_image` (`Tolex`), `grille_image` (`Grille`, cloth or mesh with the speakers behind it) and
  `plate_image` (`Plate`); `paint_*` cache them as textures. To look at them without a window:
  `RASTER_PREVIEW=out.raw cargo test -p suite_common --release preview -- --ignored`
- `artwork.rs` - Loads the PNGs of `assets/art/`, covers the top face with one, cuts it to the corners.
  Keep the middle of each picture clear for the controls. The pictures are compiled into each plugin, and
  `cargo xtask check-art` fails a bundle that does not carry its pictures: a plugin without its art was
  built from other sources than this checkout's. Made by `scripts/make-art.py`, a two-ink screen print
  (halftone, misregistration, paper dropout), or replaced by a painted or generated PNG at the same path
- `hardware.rs` - Lit footswitch, indicator light, label tape, the paper of the name band
- `led.rs` - The built-in LED display: a dark recessed window, a 5 x 7 dot-matrix readout in amber with
  the unlit dots faintly showing, lit pixel by pixel and cached (`led_display`, `led_width`,
  `LED_HEIGHT`, and the `tuner_display` on it). Readouts are capitals from the left; a glyph is
  added in `glyph`, and a test holds that every character a name can hold has one
- `art.rs` - `Motif`: what is painted on a mini pedal under its controls
- `amp.rs` - The half stack of a rig: `amp_head` (handle, tolex, grille with piping and a stripe if it has
  them, panel with a printed line if it has one, jewel pilot lamp lit or not; `head_faces` says where its faces go)
  and `cabinet` (tolex, chrome or steel `Corners` if it has them, piping if it has it, control plate, grille
  with the `Speakers` behind it, set high or in the middle; `cab_faces` says where its faces go), each described
  by a `HeadStyle` or `CabStyle` and returning the faces the controls go on. `mounted_plate` is a plate with a
  screw in each corner, for a small plate of the caller's (the cabinet's jack plate)

Controls are placed at fixed positions relative to the window's top left corner. Knobs: drag up and
down, Shift for fine steps, double-click for the default.

An editor's update runs with every frame, but the window is painted only on input, when the host
changes a parameter, in the first second after it opens, and when the editor asks with
`request_repaint` (or `request_repaint_after`). Anything an editor shows that changes on its own
(meters, lamps, the tuner, a gliding pedal, a file read in the background) asks for a repaint when
it changes, in the frame that draws it, and only then: painting every frame costs a good part of a core

## Style guide

Every plugin, existing and new, follows these. Copy `plugins/pitch_shifter/src/editor.rs` (few
controls, portrait) or `plugins/drums/src/editor.rs` (many controls, landscape) as the starting point.

- **One pedal per plugin.** The whole window is one enclosure from `ui::pedal`. No panels, tabs,
  menus or default egui widgets on top of it
- **Paint:** each plugin has its own saturated paint colour, a `PAINT_*` constant in `theme.rs`.
  Taken: orange (Pitch Shifter), teal (Drum Synth), oxblood red (Guitar Amp), cobalt (Bass Synth),
  plum (Shimmer Pad), pine green (Glass Bells). A new one must be clearly different from those
  and dark enough for cream print to read on it; it goes in `PLUGIN_PAINTS`, where a test holds it to both
- **Print on the paint is always `SILK` (cream).** Black is only for knobs, label tape and the logo plate.
  The one exception is the half stack of a rig (`amp.rs`): head and cabinet are covered, not painted (tolex,
  grille cloth or mesh, plates instead of powder coat), and print on a plate is the `print` of its style:
  `SILK` on a dark plate, `INK` on metal. Each amp has a head of its own and the cabinet looks as the
  cabinet that plays, all from `plugins/amp/src/looks.rs`
  The one other thing on paint is the second coat of a mini pedal's art, in `shade`s of its own paint.
  `BRAND_ORANGE` is only the pointer in the logo
- **Fonts, by role:** `display` (Bebas Neue) for every caption, in capitals; `model` (Bowlby One SC)
  only for the name on the band; `mono` (IBM Plex Mono) only for values on label tape
- **Top to bottom:** jack captions, knobs, name band, footswitches. Model number bottom left and
  the logo badge bottom right come from `ui::pedal` and stay there
- **Knobs:** every continuous parameter is a `param_knob`. Radius about 48 for the one main control,
  about 30 for normal ones, about 19 for a row of secondary ones inside a `silk_frame` with a title.
  The value is always shown on label tape under the name, formatted by the parameter itself.
  The one exception is an amp head's panel (`panel_knob`): one line of knobs, the name above, the tape under
- **Footswitches:** every on/off or pick-one-of-few parameter is a `footswitch` with an `led` above
  it (lit = on or selected) and, when there are several, a `silk_label` below
- **Name band:** the plugin's plain name (what it does, two short words), with an `Ornament` that
  hints at it. Add a new `Ornament` variant in `frame.rs` for a new plugin
- **Model number:** two letters from the name and a number, like `PS-10`, `DS-2` and `BA-1`.
  It is print, not the version, and does not change with releases
- **Jack captions** say what really goes in and out (`In`, `MIDI In`, `Stereo Out`)
- **Window size:** fixed, sized to the controls. Portrait about 400 wide for up to four controls,
  landscape about 660 wide for more
- **Clean factory paint:** no wear or dirt. Surfaces are lit, not flat: powder-coat flake, bevels, brushed and knurled metal come from `ui/raster.rs`, never from image files
- **New shared pieces go in `suite_common::ui`,** not in a plugin's editor, so every plugin gets them
- **Rigs** (a window of several pedals, as the Guitar Amp has): the rules for the strip, the stack, mini pedals, pedal art, equal pedals, a rearrangeable board, signal lamps, presets, file choices and the tuner are in `plugins/amp/AGENTS.md`, "Look: the rig". Read them before drawing a second rig

After changing an editor, run it standalone (see `AGENTS.md`, Build Commands) and look at it before bundling.
