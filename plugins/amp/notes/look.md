# The look of the rig

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

## Look: the rig

The style guide (`crates/suite_common/src/ui/STYLE.md`) applies; these are the rules for a window of several pedals, which only the amp has.

- **Rigs:** a plugin made of several components (the Guitar Amp) is the one exception to one
  pedal per plugin. Its window is a `rig`: a `strip` across the top, a shallow enclosure in the
  plugin's paint that carries the jack captions, model number, logo and art (no name band), with a wing at each end for the plugin's own controls; and under it one line in signal
  order: the gate, the pedals in front of the amp, the stack (two lanes, a head
  with its cabinet each, and the plate where they meet, `STACK_WIDTH` wide), the pedals behind the cabinet. The pedals stand on the floor beside the
  cabinet, as tall as it. Each mini pedal has a paint of its own. On a strip, a head and mini
  pedals the switches are `small_footswitch` and the LEDs `rig_led` (an unlit `led` disappears
  on dark paint), beside or above the switch. A mini pedal holds two knobs of radius 24 side by
  side, three of radius 17 in a triangle, (the EQ) four in two rows, or (the Wah) the rocker with two knobs of radius 14 beside it. Everything else in this guide applies
- **The strip's wings:** the left one holds what comes in and what is picked (input meter,
  Bank, Preset, Save; the tuner display while tuning), the right one what goes out (Output, the
  output meter, Clip, On, Tuner). Its jack captions read `In` at the left and `Stereo Out` at
  the right, arrows left to right: the signal runs along the window. Nothing in a wing reaches
  the band (a test holds it)
- **The stack is two lanes** (#177): the first amp's head with the cabinet beside it, and under
  it the second amp's, as tall as the pedals together (`LANE_HEIGHT`). Each lane has a head
  that looks as its amp (below), a nameplate, six dials of radius 15, and a line under the dials
  with the lane's name (`Amp 1`, `Amp 2`) and, on the second lane, its `On` switch with a
  `rig_led`. The cabinet beside it (`LANE_CAB_WIDTH`, 210 wide since #446) looks as the cabinet
  that plays through that amp and has no control plate; its choice is a nameplate at its foot
  (a `nameplate_stepper_sized` on brushed chrome: the name, a chrome step button each side), and
  a double-click on the first lane's cabinet above the nameplate opens the cabinets folder, its
  hover text naming the folder. Both input jacks take the guitar (a cable links
  them), and both lanes end at a black plate, `MIX_WIDTH` wide, through a coupler each. The
  plate holds, from the top: `Blend` over a `crossfader` (`ui::crossfader`: `Amp 1` at the
  top of the slot, `Amp 2` at the bottom, the value on label tape under it), the Mic and Res
  knobs (radius 17), the `Mic Type` stepper, the output jack with its `Out` caption, at the
  height of the pedals' plugs, and the Cab switch with its LED and caption. Mic, Res, Mic Type
  and Cab act on both amps' cabinets. The first lane's cabinet nameplate goes through the amp's
  own, the built-in cabinets and the files, as the Speaker stepper did; the second's through its
  own and the built-in ones (`cabinet2`, never a file). Nothing of the lanes is dimmed while
  the second amp is off: its lamp and its switch say so
- **Head and cabinet:** each amp has a head of its own (`looks.rs`, drawn by `ui::amp_head`),
  which changes the moment another amp is picked:

  | Amp | Covering | Grille | Panel, print | Knobs | Piping |
  |-----|----------|--------|--------------|-------|--------|
  | Klar | ivory, smooth | wheat cloth with a fine gold thread | brushed chrome, ink | chrome skirt | none |
  | Glød | lacquered tweed | oxblood-brown cloth, a gold stripe across it | dark wood, cream | cream chicken-head | none |
  | Brøl | black levant | salt-and-pepper basketweave | brushed brass, ink | gold top | white |
  | Torden | black bronco | perforated steel | black anodised, cream, one red line | black fluted | red |
  | Sang | deep blue smooth | salt-and-pepper cloth | brushed brass, ink | black chicken-head | silver |
  | Drøn | dark green bronco | black felt | dark wood, cream | gold top | none |

  The amp is picked on a `nameplate_stepper` in the middle of the grille: its name on a plate
  of the panel's metal, a chrome button each side. The panel holds, left to right, the input
  jack the cable from the pedals goes into with its `In` caption, the six dials as
  `panel_knob`s (radius 18, caption above, value tape under, in the panel's print and the amp's
  cap), and the jewel pilot lamp, lit while the amp is On. Tests hold every scale, caption and
  tape on the panel and apart, and the nameplate's buttons on the grille under the handle.

  The cabinet (`ui::cabinet`) looks as the cabinet that plays, and changes the moment another
  is picked (`looks::cab_look`, from the cabinet choice, the amp and what the loader says of
  the choice):

  | What plays | Covering | Grille | Speakers | Piping, corners |
  |------------|----------|--------|----------|-----------------|
  | `OWN`, Klar | ivory, smooth | wheat cloth | one 12" | none |
  | `OWN`, Glød | lacquered tweed | oxblood-brown cloth | two 12" | none |
  | `OWN`, Brøl | black levant | basketweave | four 12" | white piping, chrome corners |
  | `OWN`, Torden | black bronco | perforated steel | four 12" | steel corners |
  | `OWN`, Sang | deep blue smooth | salt-and-pepper cloth | two 12" | silver piping |
  | `OWN`, Drøn | dark green bronco | black felt | four 12" | none |
  | `1X10 SMALL` | tan, smooth | wheat cloth | one 10", set high | none |
  | `1X12 OPEN` | cream levant | silver cloth | one 12" | none |
  | `2X12 OPEN` | grey levant | silver cloth | two 12" | none |
  | `4X10 CRISP` | black, smooth | silver cloth | four 10" | none |
  | `4X12 MID` | black levant | basketweave | four 12" | white piping |
  | `4X12 TIGHT` | black bronco | perforated steel | four 12" | none |
  | `1X15 DEEP` | navy levant | black felt | one 15" | none |
  | `1X8 TINY` | walnut tweed | faded brown cloth | one 8", set high | none |
  | `2X12 PUNCH` | green bronco | basketweave | two 12" | none |
  | `4X12 HEAVY` | black, smooth | dark felt | four 12" | none |
  | `1X12 CLEAR` | pale blue, smooth | salt-and-pepper cloth | one 12" | none |
  | `1X10 SNAP` | red levant | salt-and-pepper cloth | one 10", set high | none |
  | `2X12 CHIME` | pale tweed | silver cloth | two 12" | none |
  | `FULL RANGE` | grey, smooth | perforated steel | one 12" | none |
  | a WAV file | charcoal, smooth | grey felt | one 12", a shadow behind the felt | none |

  A choice shown with `?` (missing, or a built-in cabinet of another build) or a file shown
  with `!` (unusable) plays the amp's own cabinet and shows its look. Switching the cabinet off
  does not change the look: its LED says so. A new built-in cabinet needs a look in
  `SPEAKER_CABS` (a test walks `SPEAKERS`), and no two built-in looks share covering, grille
  and speakers. Every cabinet is the same size (a lane's, `LANE_CAB_WIDTH` wide and
  as tall as the lane); its controls are not on it but on its lane's head and the shared plate
  (above)
- **Pedal art:** every mini pedal has its own `PAINT_*` and its own `Motif`, all on one theme
  per rig (the Guitar Amp: northern weather and nature). Two coats and a print: a large motif
  in darker tones of the pedal's paint (`theme::shade`) that may run under the controls, with
  no fine detail under a caption, and a few `SILK` lines (2 px) and dots only in the clear
  paint beside the controls. The art keeps 5 px of plain paint to the edge of the face. No
  letters, numbers or cream triangles in it, and nothing that moves. A new paint goes in
  `PEDAL_PAINTS`, where tests hold it to cream print reading on it and to being clearly apart
  from the others; a new motif is drawn in `art.rs` for the layout of its pedal
- **The signal line** (#270, replacing "equal pedals, one row, 3204 px wide"): every block of the
  chain is a pedal `PEDAL_WIDTH` (180 px) wide and `GAP` (27 px) from the next, in signal order
  from the left: the gate that is always first, the blocks in front of the amps, the stack
  (`STACK_WIDTH` wide, 790 since #446), the blocks behind the cabinets. The line is as wide as its blocks (3501
  px for the gate, twelve pedals and the stack; it was 3411 before the cabinets grew by 90. No saved
  value holds a width, so a project from before opens as that line) and is never stretched. A block draws from its own parameters where it
  stands on its pedal's slot and from its slot's `slot<n>_*` parameters otherwise (`editor::Src`:
  a slot's knob is told in the terms of the pedal's own parameter, at the same position), each
  kind with its own paint, art and size (a pair, a trio, a diagonal pair, the EQ's four, the
  wah's rocker, the delay's Sync); a duplicate looks as the original. A pedal with two knobs is
  either a pair side by side (radius 24) or on a diagonal (`SLIM_LED`, `SLIM_KNOBS`,
  `SLIM_SWITCH`): the LED where a trio's upper left knob is, the two radius-17 knobs upper right
  and lower left, as far from the middle as a pair's. The narrow motifs (`art.rs`, `NARROW_ART`)
  stand in the middle of the wider face and their paint runs out to both edges. In a row the
  LEDs and switches line up
- **The window scales and scrolls the line** (#270, replacing "fixed window size"): the layer is
  the line's width (not under `STRIP_MIN_WIDTH`, 2600) by 584, and the window's zoom is the one
  that fits it wide and high, between 0.5 (`MIN_ZOOM`, the least that reads) and 1.0. Where
  0.5 does not fit the line the window shows part of it and the wheel over the line or the bar
  along the foot (`ui::scroll_bar`) scrolls it; the strip stays and stretches with the window,
  its right end and middle following the edges. No proportions are locked on the corner drag.
  Nothing wider than 8192 px is made: the strip's lit surface is one picture whose middle is
  stretched (`raster::paint_box_wide`). Tests hold the zoom, the scroll, the strip at its
  narrowest and widest, and snapshots show the line short, small, large, sixteen blocks and
  scrolled
- **The Wah's rocker:** the one control that is neither a knob nor a switch, `ui::rocker` (a
  variant of the crossfader: a tall slot with a ribbed chrome treadle, ticks both sides, the
  heel at the bottom and the toe at the top, drag the cap or the slot, Shift for fine,
  double-click for the default). It is the pedal's middle, with Bite over Sens in a column on
  the left, the LED, the rocker's value on tape and the Auto push button (a `chrome_button`
  with a `rig_led` above and a caption under, like the Delay's Sync) on the right. Toe, Heel
  and the rocker's caption (Rocker, and Rest while Auto is on) are `silk_label`s; the tape says
  the value as the parameter does. The look was picked out of three on #242 (a treadle in
  perspective, this slider, a face dial)
- **A line that can be rearranged:** a block is moved by dragging it by its title, across the amps too (the gate and the stack are fixed; a block keeps its slot, and its values with it)
  (`title_grip`); while it is held it is drawn `lifted` and last, the others glide to make
  room, and it lands in the nearest place it may have. No arrows, menus or lists. A pedal
  that cannot be moved has no grip
- **The signal between the pieces** is drawn on the bench: a `coupler` between two pedals that
  stand side by side, a `patch_cable` from the last pedal in front of the amp into the head's
  input jack and one from the cabinet's output jack to the first pedal behind it. They follow
  a pedal that is dragged; a pedal that is held has no couplers. The stack never leaves its
  place among the pedals, and glides with it when a pedal changes sides. Objects on the bench,
  not print: silk triangles still mean signal direction at the jacks only
- **An order is never a parameter.** Keep it in a `#[persist]` field, as ids, like a preset
  or a file choice
- **The strip is slim** (`ui::slim_strip`, 64 px, picked as A "Signal Line" on #261): the enclosure
  with the jack sockets and their captions, the model number and the logo, no name band and no
  screws. What is on it stands on one line (`STRIP_Y`), in groups: from the left the input
  (jack, meter with its caption over it, the Input gain beside it), the preset display; in the
  middle the metronome, tuner and settings icons; from the right the output (knob, meter with
  its caption, Clip lamp), the model number and the logo, the output jack. Its knobs are
  `bare_param_knob`s with no caption (the meter's caption names the pair, the name and value
  show as a hint). **On is not on it:** the `bypass` parameter and `make_bypass()` stay for the
  host, nothing in the editor sets it (a rules test holds that). Tests hold that nothing on the
  strip overlaps, that all of it is on the face, and that the plates open under it. The input
  gain moved here from the Gate pedal, which is a pair of knobs now
- **Level meters:** a signal level (the amp's input and output) is a `level_meter_wide`, a row
  of lamps on label tape with a `silk_label` caption over it, on the strip. Same atomics and repaint rule as the signal lamps
- **The Delay's Sync:** the one on/off of the rig that is not a stomp switch. A second
  `small_footswitch` beside the On switch runs off the face, so Sync is a `chrome_button` with
  a `rig_led` above it and its `silk_label` above that, right of the Mix knob where the Gate
  has its lamp (#145). While it is lit the Time knob, in the same place, picks Delay Note and
  its tape shows the note
- **Signal lamps:** a light that follows the sound and not a switch (gate open, output clipping)
  is a `signal_lamp`, with a `silk_label` of size 13 centred 18 px under it, placed beside the
  control it belongs to. An LED above or beside a switch only ever says that the switch is on.
  Lamps are read from atomics the audio thread writes once per block; the editor asks for a
  repaint only when one changes, 20 times a second at most
- **The sound bank's LED display:** the preset display and the foot's name display are built-in
  LED displays (`suite_common::ui::led`): flat dark glass with a crisp white Plex Mono readout,
  no dot grid. It is `LED_HEIGHT` high and `led_width(chars)` wide: the strip's holds 26
  characters (two are the marker's), the foot's 18. The
  text is capitals from the left, so a name being typed grows to the right, with the `_` caret
  after it; what does not fit is cut. The font has every letter a name can hold. The
  tuner's display is the same glass
- **Pick one of many (presets):** the preset display is one click target that opens the preset
  plate (`drop_panel_with`: the banks and presets, a foot with the name display, Folder and Save).
  There are no step arrows on it. A trailing `*` on the display means a control was moved since
  loading. Names have at most 15 characters and are our own words. While the tuner is on the
  display is the tuner's and opens nothing
- **Plates and the sheet** (#259, replacing "a list that drops open is for the preset banks and
  nothing else"): configuration that is not on show at all times is a **hardware plate** that
  opens from an icon or a display on the strip and drops under it (`ui::plate`; `drop_panel_with`
  for the preset plate), or the effects **sheet** that rises from below (built with the signal
  line, #272). No other pop-ups, no dropdowns, and no widget of egui's own. A plate keeps the
  amp's plate look: dark with a chrome edge, label tape rows, steppers, lit lamps; its title is
  a `silk_label`. **One is open at a time:** `View.plate` is one field, so opening one closes
  the other (a test clicks through them). It closes on a click outside it (the icon or display
  that opens it is not outside: its click closes it, as Escape does), and while a name is typed on
  it escape drops the name first. It is drawn after everything else in the window, in a
  foreground layer, so nothing behind it answers a click while it is open, and it is kept inside
  the window. The four plates: presets (under the display, a row of columns of tape rows
  that show seven rows at a time with a chrome bar and the wheel), the metronome's (tempo, tap,
  sound, meter, click out, volume, Sync; its switch with a lamp that flashes on the beat is on
  the strip beside the icon), the looper's (a footswitch in a ring of lamps, Stop, Clear, Level; see DESIGN.md, Looper) and the settings' (the cabinet files, picked as on the head, and in
  the standalone program the machine's audio with Apply and Cancel: the audio's choice opens with
  the plate and is let go when it closes). Icons are `ui::icon_switch`: a drawing, and where the
  thing can be on or off a lamp and a chrome switch; the tuner's is its switch
- **The effects bank's cards are miniature pedals** (#445): one row of twelve, each card a plate
  round the real pedal at half size (`editor::paint_miniature` in `ui::miniature`: paint, art, title,
  LED, knobs at mid travel and footswitch; no captions or value tapes, no Sync, no Auto, the Wah's
  rocker a bare slot) with the count under it; a pedal not in the chain is dimmed. `SHEET_HEIGHT` is
  250, which leaves the tops of the pedals and the gaps' plus in view above it. The ghost under a
  dragged card is the same miniature
- **Typed text** is taken in two places only: the name of a new preset bank, on its row of the
  preset plate, and the name of a new preset, on the plate's foot after Save
  (`type_on_tape`, 24 letters, the last 15 shown). Both are `type_into`: the letters on the tape or display with a `_` after them,
  enter to keep it and escape to drop it. Nothing else in the suite is typed, and egui's text widget is never drawn.
  Nothing ever waits for the keyboard: a second click on the row (or a second press of Save) keeps the name as it stands,
  and an empty name is one the player did not give, so the mouse alone is enough
- **Asking for the keyboard.** An editor is a child window of the host's, and on Windows
  nothing gives it the keyboard focus by itself: a window that never asks hears no letters at
  all. The row or tape that takes a name asks egui for the keyboard (`ask_for_keyboard`, on a widget
  that is focusable, or egui takes the focus back in the same frame:
  `request_focus` and a `widget_info` that says it is a field with text), which is what
  egui-baseview's `windows_keyboard_workaround` watches for before it takes the focus for the
  window; the workspace turns that feature on (see the root `Cargo.toml`). Nothing else in the
  suite asks, so a knob or a footswitch never takes the keyboard from the host. The window
  keeps it until the player clicks the host again, which is the price of typing there.
  That is still not enough in REAPER, which takes a key press for a shortcut of its own unless
  the focused window answers `WM_GETDLGCODE` with `DLGC_WANTALLKEYS` (baseview's never does,
  and nih-plug's `IPlugView::on_key_down` says not implemented): `ask_for_keyboard` also calls
  `ui::host_keys::want_all_keys`, which puts that answer in front of the window (#132). The
  standalone does not show the difference, only a host does; try it in REAPER with "Send all
  keyboard input to plug-in" off (its default)
- **Steppers on the plate** (`Mic Type`): the
  `silk_label` caption above the tape, and the buttons clear of the next knob, caption or LED.
  The amp's is the nameplate on its head and each lane's cabinet's the nameplate at its foot,
  which need no caption: they say the name
- **Save (a preset):** a `chrome_button` (a stepper's push button with a dot in its cap) right
  of the preset stepper, its `silk_label` caption on the stepper's caption line. It writes the
  sound into the bank showing and the stepper's tape says `SAVED` (or `NOT SAVED`) for 1.5 s.
  Every save takes its name on the stepper's tape first, which starts as the name of the preset
  loaded out of the bank showing: kept as it is, that preset is written over. Renaming later is
  also done in the folder
- **A preset choice is never a parameter.** The editor sets the real parameters through the
  `ParamSetter` and keeps only the index in a `#[persist]` field
- **Pick a file (cabinets):** files are never picked in a dialog. They live in one folder and are
  stepped through in name order with the two chrome buttons of the cabinet's nameplate (no caption:
  the plate says the name), after the built-in
  choices: the amp's own (`OWN`), then the built-in cabinets. On the first lane's cabinet nameplate
  the name shows at most 10 characters (print size 15 on a 128 px plate): no extension, capitals, long names as the first 5 and last 4
  with `~` between. `!` in front of the name of a file that cannot be used, `?` in front of the name of one
  that is not there. Stepping on to `OWN` past where the files would be, with none in the folder,
  says `NO FILES` on the plate for 1.5 s. Hovering the
  cabinet (above its nameplate) names the folder; a double-click on it opens the folder
- **A file choice is never a parameter.** Keep the file name (not the path) in a `#[persist]` field
- **Metronome:** one cluster on the strip between Save and Output, its controls closer
  (`METRONOME_GAP`) than the strip's others (`STRIP_GAP`), left to right: a `small_footswitch`
  with `rig_led` and its `Metro` caption like On and Tuner, a `beat_lamp` (amber on a beat, cream
  on the first of the bar, lit 90 ms) with its `Beat` caption, the tempo on an LED display of three
  characters with a chrome button at each end (also dragged up and down, 3 points a beat per
  minute and a quarter of that with shift, scrolled, or double-clicked for 120), the Tap
  `chrome_button`, a `stepper` for the sound over two side by side, the time signature and the click's output (`Click out`: Mixed or Separate, on a 70 point tape that Separate fills), the Volume knob
  (radius 20) and Sync, a `chrome_button` with a `rig_led` above it. Captions are on the preset
  captions' line. The window is drawn again every 20 ms while it runs (the lamp follows the
  sound) and not at all when off. The cluster is one control of the strip's row, wider by the click's output (the window is not: `STRIP_GAP` went from 43 to 32.5 points, and the gap holding the standalone program's Audio button, `AUDIO_GAP`, stayed 43); a test holds
  its parts clear of each other and of their neighbours
- **Tuner:** a `small_footswitch` with `rig_led` and a `silk_label` at the end of the strip's
  right wing, beside the On switch. While it is on, a
  `tuner_display` (an LED display, `ui::led`) under its own caption takes the place of the
  preset display: the note name in dots at the left, a needle of 17 lamps, flat to the left
  and sharp to the right, the tall middle lamp lit alone within 2 cents. All in the display's
  amber; no numbers or other colours. The editor repaints continuously only while the tuner is on
