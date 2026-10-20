---
name: Piano MIDI Visualization
description: A Windows desktop piano companion whose window is an instrument's front panel — brushed scarlet aluminium, a black OLED window, real ivory keys.
colors:
  panel-scarlet: "#A8131C"
  oled-black: "#000000"
  well-deep: "#060606"
  rubber-raised: "#1C1C1C"
  rubber-hover: "#2E2E2E"
  rubber-line: "#3E3E3E"
  bezel: "#0C0C0C"
  felt: "#111111"
  disabled-fill: "#141414"
  ivory-key: "#FBF8F0"
  legend-primary: "#FFFFFF"
  legend-pressed: "#F4F2EE"
  legend-secondary: "#F0EEEC"
  legend-tertiary: "#DCDAD8"
  legend-muted: "#D0CECC"
  legend-faint: "#8E8C8A"
  legend-disabled: "#7C7A78"
  ink: "#0A0A0A"
  amber: "#F0C06A"
  amber-pale: "#E9C47E"
  amber-strong: "#EDA73C"
  amber-deep: "#6E4F1C"
  amber-border: "#8A6A30"
  amber-wash: "#3A2E1B"
  amber-fill: "#2A2215"
  amber-ink: "#1C1609"
  held-blue: "#4FA3FF"
  target-magenta: "#E83E9C"
  hit-green: "#8BE36B"
  miss-red: "#FF5252"
  record-red: "#F04A3E"
  record-idle: "#9C4A42"
  right-hand-teal: "#3FC1B0"
  left-hand-violet: "#B39DDB"
  halo-low: "#5DFF7A"
  halo-mid: "#FFC23A"
  halo-high: "#FF4A3A"
  halo-off: "#2A1214"
  danger-red: "#C42B1C"
  danger-text: "#F0736A"
typography:
  display:
    fontFamily: "Segoe UI"
    fontSize: "72px"
    fontWeight: 900
  headline:
    fontFamily: "Segoe UI"
    fontSize: "18px"
    fontWeight: 600
  body:
    fontFamily: "Segoe UI"
    fontSize: "12px"
    fontWeight: 400
  body-readout:
    fontFamily: "Segoe UI"
    fontSize: "14px"
    fontWeight: 400
  legend:
    fontFamily: "Segoe UI"
    fontSize: "12px"
    fontWeight: 700
  data:
    fontFamily: "Cascadia Mono, Consolas"
    fontSize: "18px"
    fontWeight: 400
  data-small:
    fontFamily: "Cascadia Mono, Consolas"
    fontSize: "11px"
    fontWeight: 400
rounded:
  control: "3px"
  tile: "4px"
  card: "6px"
  oled-inner: "6px"
  oled-bezel: "10px"
  pill: "11px"
spacing:
  hairline: "2px"
  xs: "4px"
  sm: "6px"
  md: "8px"
  lg: "12px"
  xl: "16px"
components:
  button:
    backgroundColor: "{colors.rubber-raised}"
    textColor: "{colors.legend-secondary}"
    rounded: "{rounded.control}"
    padding: "3px 10px"
  button-hover:
    backgroundColor: "{colors.rubber-hover}"
    textColor: "{colors.legend-primary}"
  button-disabled:
    backgroundColor: "{colors.disabled-fill}"
    textColor: "{colors.legend-disabled}"
  button-primary:
    backgroundColor: "{colors.legend-pressed}"
    textColor: "{colors.ink}"
  toolbar-toggle-on:
    backgroundColor: "{colors.legend-pressed}"
    textColor: "{colors.ink}"
  chord-tile:
    backgroundColor: "{colors.rubber-raised}"
    textColor: "{colors.legend-tertiary}"
    rounded: "{rounded.tile}"
    height: "22px"
  chord-tile-played:
    backgroundColor: "{colors.legend-primary}"
    textColor: "{colors.ink}"
  oled-window:
    backgroundColor: "{colors.oled-black}"
    rounded: "{rounded.oled-inner}"
  halo-knob:
    width: "104px"
    height: "104px"
---

<!-- Provenance: recorded from the shipped artifact on 2026-10-20 (commit e7ec2fd and the halo-knob commit after it, branch feature/app-design-concepts-1467dc). Direction: concept K, "Scarlet anodised", picked by the user from three rounds of hardware-display concepts (docs/design-concepts/Scarlet.dc.html; the live canvas is linked from docs/design-concepts/README.md). It replaced a phosphor-scope trial (commits 30a6ba0, 08255c7), which replaced "piano character" (shipped 2026-09-28). The Impeccable sidecar, .impeccable/design.json, still describes piano character and needs regenerating. Product truth lives in PRODUCT.md and is not repeated here. -->

# Design System: Piano MIDI Visualization

Platform note: this is a native WPF (.NET 10) app. The normative source is `src/PianoMidiVisualizationApp/Themes/Dark.xaml`: `Color.*` primitives, `Brush.*` role brushes built only from them, `Size.Readout.*`, and the control templates. The code-behind renderers (`PianoKeyboardControl`, `FallingNotesControl`, `TakeTimeline`, `CircleOfFifthsControl`, `GrandStaffControl`, `HaloKnob`) resolve those brushes by key and must keep doing so.

Token slugs above map to `Color.*` keys, several under names kept from piano character so every renderer still resolves: `panel-scarlet` is `Color.Lacquer.Chrome`, `oled-black` is `Color.Lacquer.Stage`, `well-deep` is `Color.Lacquer.Deep`, `rubber-*` are `Color.Lacquer.Raised/Hover/Line`, `legend-*` are `Color.Ivory.Primary…Disabled`, `ink` is `Color.Ink`, `held-blue` is `Color.Blue`, `target-magenta` is `Color.Target`. The bezel, felt, disabled fill and halo colours are set directly on their `Brush.*` keys.

## Overview

**Creative North Star: "The Front Panel"**

The window is a piece of hardware. The chrome (title bar, sidebar, toolbar, status bar) is one sheet of brushed scarlet aluminium with white legends. Set into it is a black OLED window that holds everything you read: the grand staff, the chord name, the circle of fifths and the chord strip. Below, behind a felt strip, sit real ivory and ebony keys. Beside the window, two halo knobs set the volume and the click.

The OLED is where the eye goes: true black, white type, amber for the key you're in, blue for what you're playing. The panel around it is loud in colour but quiet in content, with legends and controls only. Controls are black rubber keys on the red, and a switched-on control is a white key with black ink.

Density stays instrument-tight: 12px body, 22px tiles, 3px corners, a 900x420 minimum window where every row is budgeted, and the readout and knobs shrink rather than clip when the window is short.

**Key Characteristics:**
- One scarlet panel, brushed, shared by every chrome surface without seams.
- One black OLED window with a bezel holds the readout; song practice hides it so the lane meets the keys edge to edge.
- Real keys: a held key goes down and lights blue along its back edge.
- White legends that clear 4.5:1 on the scarlet, and so clear it easily on black.
- On means white: a lit toggle, the primary action and the played chord tile are white with black ink.
- Every other hue keeps one job (amber key, blue playing, magenta asking, teal/violet hands, green hit, red miss/record); the halo knobs are the one place a green-to-red level scale appears.

## Colors

A scarlet panel, a black display, white legends, one amber, and the single-job signal hues.

### Primary
- **Panel Scarlet** (`panel-scarlet`): the anodised aluminium of every chrome surface. `Brush.Window`, `Brush.TitleBar` and `Brush.Surface` are each a tiled `DrawingBrush`: the scarlet with a 1px light and a 1px dark hairline every 4px, tiled in absolute units, so panels that meet show one continuous grain. They are three copies of one definition, because a `<StaticResource x:Key>` alias defined in the merged `Dark.xaml` is not found from the window.
- **OLED Black** (`oled-black`): the display. The readout window, the settings drawer, cards laid over the stage, the keyboard's backing.

### Secondary
- **Key-Centre Amber** (`amber` family): the selected key and whatever is waiting on you, unchanged from piano character. The tonic is a full amber keytop; in-key white keys take a warm wash and an amber front lip; the circle's selected key is a Deep Amber wedge; the status chip and first-run step go amber while something isn't ready.
- **Held-Key Blue** (`held-blue`): you are playing. The back edge of a held key, the circle's chord-root outline, your recorded notes, a learned piano control firing.
- **Asking Magenta** (`target-magenta`): the app asking for a key: hint outlines and dots, the hovered chord tile's outline, the chord wait mode holds for.

### Tertiary
- **Right-Hand Teal**, **Left-Hand Violet**: your hands in a song; accompaniment is warm neutrals.
- **Hit Green** and **Miss Red**: you hit it, you missed it. **Record Red** / **Record Idle**: recording.
- **Halo Low / Mid / High** (`halo-low`, `halo-mid`, `halo-high`) and **Halo Off**: the knobs' level LEDs, rising green, amber near the top, red at the top, as a meter reads. Used on the halos only.
- **Close Red** (`danger-red`) and **Danger Text** (`danger-text`): the title bar's close hover and a remove glyph's hover.

### Neutral
- **Well Deep** (`well-deep`): song lane, take timeline, MIDI log, the circle's minor ring.
- **Rubber Raised / Hover / Line** (`rubber-*`): controls, tiles and inputs at rest; under the pointer; borders and the strongest fill. Black rubber keys whether they sit on the panel or in the OLED.
- **Bezel** (`bezel`) with a 25%-white bottom lip (`Brush.Oled.Lip`): the frame round the OLED.
- **Felt** (`felt`): the strip behind the keys.
- **Disabled Fill** (`disabled-fill`): a control that can't be used. Never the panel brush, which would make it read as a lit red button.
- **Keytop Ivory** (`ivory-key`): white keytops and the playhead.
- **Legend Primary → Muted** (`legend-*`): text from the chord name down to hints. Muted (#D0CECC) is the floor for words: 4.8:1 on the scarlet.
- **Legend Faint** (`legend-faint`): marks only (staff lines, the metronome's dot, scroll thumbs). Never words.
- **Ink** (`ink`): text on a white key: a switched-on toggle, the primary action, the played chord tile.

### Named Rules
**The One Panel Rule.** Every chrome surface is the same brushed scarlet. Separation inside the chrome comes from black rubber controls and legends, never from a second red.

**The Window Holds the Reading Rule.** Whatever you read mid-phrase (chord, numeral, notes, staff, circle, chord strip) sits in the black OLED window. The panel carries controls and labels only.

**The On Means White Rule.** A switched-on toggle, the one primary action and the played chord tile are white keys with black ink. Amber never means "on"; blue never means "on".

**The One Job Per Hue Rule.** Amber is the key centre and what waits on you; blue is you playing; magenta is the app asking; teal and violet are your hands; green is hit; red is miss and recording. The halo knobs' green-to-red scale is the single exception, a level meter, and stays on the halos.

**The Dead Key Rule.** Disabled controls are a near-black fill with grey legends. Red on this panel always reads as live.

## Typography

**Body Font:** Segoe UI (WPF's default, never overridden at the window)
**Data Font:** Cascadia Mono, falling back to Consolas on Windows 10 (`Font.Data`)
**Icon Font:** Segoe MDL2 Assets (`Font.Icon`); music glyphs come from Segoe UI Symbol

### Hierarchy
- **Display** (Segoe UI Black 900, 72px, `Size.Readout.Name`): the chord name, white on the OLED. The only display-size text.
- **Headline** (SemiBold 18px, `Size.Readout.Function`): the Roman numeral leading the voicing line.
- **Body** (Regular 12px; 14px for the voicing, `Size.Readout.Voicing`): controls, menus, tiles, lists.
- **Legend** (Bold 12px): the knob names, VOLUME and CLICK, printed on the panel.
- **Data** (Cascadia Mono 18px notes, 14px intervals, 11px knob values, the MIDI log and clocks): pitches, intervals and numbers that change.
- **Key labels** (set in `PianoKeyboardControl`, Segoe UI): Medium at rest, SemiBold when held, as in piano character.

### Named Rules
**The Data Holds Still Rule.** Monospace is for pitches, intervals, levels, the MIDI log and clocks, so digits don't shift as they change.

**The Shrink Not Clip Rule.** The readout sits in a scale-down-only Viewbox, as do the knobs, so a short window shrinks them rather than clipping the chord name.

## Layout

A single fixed-structure window, minimum 900x420, default 1400x620. Rows: a 32px title bar on the panel; the stage; then the recorder, MIDI log, AI chat and status bar as collapsible rows. The stage's left column has two columns of its own: the OLED window (star width) and the halo knobs (Auto, so it takes no room when they hide). The OLED window spans the stage's first three rows (practice toolbar, readout, chord strip), inset 16px from the panel edges with a 6px bezel. The practice toolbar and the keyboard span both columns below it. The chord-progression sidebar is a fixed 220px column on the panel.

Song practice hides the OLED window and the knobs, and drops the felt strip, so the falling-notes lane runs edge to edge and its bottom is the keys' top edge. Zen mode keeps the window and the keys and hides the knobs.

Spacing stays small and even, in 2 / 4 / 6 / 8 / 12 / 16px steps.

## Elevation & Depth

The panel is flat aluminium. Depth comes from objects set into it or standing proud of it:
- **The OLED window**: a black bezel (10px corners) holding the black screen (6px corners), with a light lip along its bottom edge.
- **The keys**: piano character's ivory gradients and ebony bevel with their drop shadows (keytop: direction 270, depth 1, blur 2, 15%; ebony: direction 315, depth 3, blur 5, 50%). A held key shades darker from its top and lights a blue back edge (the first ~5% of its gradient), the way a key-light strip does; a pedal-held key keeps a paler blue edge.
- **The halo knobs**: a dark recess with a faint rim, the LED ring inside it, and a black cap with a white pointer.
- **Piano echo**: the transient pill over the stage, the only floating chrome.

### Named Rules
**The Set-In Rule.** A surface either sits in the panel (the OLED window, the knob recesses) or is a black key on it. There are no shadows on panels, tiles or inputs.

## Shapes

Controls, inputs and menus 3px; chord tiles and the focus ring 4px; cards 6px; the OLED screen 6px inside a 10px bezel; pills half their height. White keys round only their front corners (4px), black keys 2px. Knobs are circles.

## Components

### Buttons
Black rubber keys with white legends.
- **Default:** Rubber Raised with Legend Secondary text; hover lifts to Rubber Hover with Legend Primary; pressed sinks to OLED Black; disabled is Disabled Fill with Legend Disabled.
- **Primary** (`Style.PrimaryButton`, one per view): a white key with Ink, SemiBold.
- **Toolbar toggle:** a black key while off, a white key with Ink while on.
- **Glyph buttons:** Segoe MDL2 glyphs in Legend Muted, brightening to Primary; only removing something turns Danger Text on hover. The close button hovers Windows' red.
- **Focus:** a 1.5px Legend Secondary ring, 2px outside the control.

### OLED Window (signature)
A `Border` in the stage grid behind the readout and chord strip: Bezel fill, 6px padding, 10px corners, a lit bottom lip, and an inner OLED Black screen with 6px corners. Not hit-testable, so everything drawn over it still takes clicks. Hidden during song practice.

### Halo Knob (signature, `HaloKnob`)
A `RangeBase` drawn in code: a 104px recess, 15 level LEDs on a 270° sweep from the bottom left, and a black cap whose white pointer follows the value. LEDs light from the start up to the value, the first nine green, the next three amber, the last three red. Drag up or down (200 DIPs is the full range) or scroll (5% a notch). Never focusable, so Space stays "save chord"; the sliders in Settings set the same values from the keyboard. Two instances beside the OLED: **VOLUME** (`Settings.Volume`) and **CLICK** (`Settings.MetronomeVolume`), each with a bold legend and its value as a percentage below.

### Keyboard (signature)
Piano character's ivory keytops and beveled ebony sharps with its amber key-centre roles and edge marks, unchanged except for the held state: the key shades down and lights a blue back edge. Hints are magenta outlines and dots with a deep keyline. A black felt strip runs above the keys across the panel (hidden in song practice).

### Diatonic Chord Strip (signature)
Black 22px rubber tiles on the OLED's bottom edge, Legend Tertiary text with a white SemiBold numeral. The played tile turns white with Ink for both name and numeral (`Brush.ChordTile.PlayedInk`). A hovered tile takes a magenta outline. Never focusable.

### Chord Readout, Grand Staff, Circle of Fifths
As in piano character, now white on the OLED: chord name in Segoe UI Black, numeral and voicing line, notes and intervals in columns, the save line. The staff draws Faint lines, Tertiary clefs and white notes. The circle's major ring is Rubber Raised and its minor ring Well Deep, with a Deep Amber selected wedge, Amber Fill related keys and a blue root outline.

### Chips, Menus, Inputs, Scrollbars
As in piano character, recoloured by the tokens: the status chip is a pill on the panel that turns amber when something isn't ready; menus and popups are Rubber Raised with a Rubber Line border; inputs are Rubber Raised with a Rubber Line outline; scroll thumbs are Legend Faint.

## Do's and Don'ts

### Do:
- **Do** build every brush from a `Color.*` token in `Dark.xaml` and resolve it by key in code-behind renderers.
- **Do** put anything read mid-phrase inside the OLED window, and controls and legends on the panel.
- **Do** mark "on" and the single primary action with a white key and Ink.
- **Do** give disabled controls the Disabled Fill, never the panel brush.
- **Do** keep words at Legend Muted or brighter on the scarlet; Legend Faint draws marks only.
- **Do** make stage controls (chord tiles, setup steps, metronome buttons, the halo knobs) non-focusable so Space stays "save chord", and keep a keyboard route to the same setting elsewhere.
- **Do** put colours for stateful controls in Style setters, never local attributes, so triggers can paint.

### Don't:
- **Don't** add a second red to the chrome, or a gradient across the panel that makes tiles band.
- **Don't** use amber as decoration or to mean "on".
- **Don't** borrow blue for selection or "on"; blue means you are playing.
- **Don't** use the halos' green-to-red scale anywhere but the halos.
- **Don't** alias a brush with `<StaticResource x:Key>` inside `Dark.xaml`; it is not found from the window.
- **Don't** use Unicode look-alike glyphs (✕, ↻, □) for icons; use Segoe MDL2 Assets codes or a drawn path.

### Open ideas (not shipped; not rules)
K's button grid and progression LEDs on the panel (the sidebar does that job), and a halo knob for tempo.
