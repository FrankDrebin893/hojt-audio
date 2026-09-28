---
name: Piano MIDI Visualization
description: A Windows desktop piano companion whose window is the piano's case — lacquer recedes, ivory and one amber lead.
colors:
  lacquer-deep: "#0B0A09"
  lacquer-chrome: "#100F0E"
  lacquer-stage: "#1C1A17"
  lacquer-raised: "#282520"
  lacquer-hover: "#34302B"
  lacquer-line: "#3D3934"
  ivory-key: "#FBF8F0"
  ivory-primary: "#F5F0E6"
  ivory-pressed: "#E6DFD2"
  ivory-secondary: "#D2CBBF"
  ivory-tertiary: "#ADA699"
  ivory-muted: "#979084"
  ivory-faint: "#7A736A"
  ivory-disabled: "#58534C"
  amber: "#F0C06A"
  amber-pale: "#E9C47E"
  amber-strong: "#EDA73C"
  amber-deep: "#6E4F1C"
  amber-border: "#8A6A30"
  amber-wash-hover: "#463820"
  amber-wash: "#3A2E1B"
  amber-fill: "#2A2215"
  amber-ink: "#1C1609"
  held-blue: "#50A0EB"
  target-magenta: "#E83E9C"
  hit-green: "#8BE36B"
  miss-red: "#FF5252"
  record-red: "#F04A3E"
  record-idle: "#9C4A42"
  right-hand-teal: "#3FC1B0"
  left-hand-violet: "#B39DDB"
  danger-red: "#C42B1C"
  danger-text: "#F0736A"
typography:
  display:
    fontFamily: "Segoe UI"
    fontSize: "64px"
    fontWeight: 700
  headline:
    fontFamily: "Segoe UI"
    fontSize: "18px"
    fontWeight: 600
  title:
    fontFamily: "Segoe UI"
    fontSize: "12px"
    fontWeight: 600
  body:
    fontFamily: "Segoe UI"
    fontSize: "12px"
    fontWeight: 400
  body-readout:
    fontFamily: "Segoe UI"
    fontSize: "14px"
    fontWeight: 400
  label:
    fontFamily: "Segoe UI"
    fontSize: "11px"
    fontWeight: 400
  data:
    fontFamily: "Consolas"
    fontSize: "18px"
    fontWeight: 400
  data-small:
    fontFamily: "Consolas"
    fontSize: "11px"
    fontWeight: 400
rounded:
  control: "3px"
  tile: "4px"
  card: "6px"
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
    backgroundColor: "{colors.lacquer-raised}"
    textColor: "{colors.ivory-secondary}"
    rounded: "{rounded.control}"
    padding: "3px 10px"
  button-hover:
    backgroundColor: "{colors.lacquer-hover}"
    textColor: "{colors.ivory-primary}"
  button-pressed:
    backgroundColor: "{colors.lacquer-stage}"
  button-disabled:
    backgroundColor: "{colors.lacquer-chrome}"
    textColor: "{colors.ivory-disabled}"
  button-primary:
    backgroundColor: "{colors.ivory-pressed}"
    textColor: "{colors.lacquer-chrome}"
    rounded: "{rounded.control}"
    padding: "3px 10px"
  button-primary-hover:
    backgroundColor: "{colors.ivory-primary}"
  toolbar-toggle:
    backgroundColor: "{colors.lacquer-raised}"
    textColor: "{colors.ivory-secondary}"
    rounded: "{rounded.control}"
    padding: "0 7px"
  toolbar-toggle-on:
    backgroundColor: "{colors.ivory-pressed}"
    textColor: "{colors.lacquer-chrome}"
  toolbar-toggle-on-hover:
    backgroundColor: "{colors.ivory-primary}"
  input:
    backgroundColor: "{colors.lacquer-raised}"
    textColor: "{colors.ivory-secondary}"
    rounded: "{rounded.control}"
    padding: "3px 6px"
  chord-tile:
    backgroundColor: "{colors.lacquer-raised}"
    textColor: "{colors.ivory-tertiary}"
    rounded: "{rounded.tile}"
    padding: "0 8px"
    height: "22px"
  chord-tile-hover:
    backgroundColor: "{colors.lacquer-hover}"
    textColor: "{colors.ivory-secondary}"
  chord-tile-played:
    backgroundColor: "{colors.lacquer-raised}"
    textColor: "{colors.ivory-primary}"
  status-chip:
    textColor: "{colors.ivory-tertiary}"
    rounded: "{rounded.pill}"
    padding: "0 10px 0 8px"
    height: "22px"
  status-chip-attention:
    backgroundColor: "{colors.amber-fill}"
    textColor: "{colors.amber}"
  setup-step-next:
    backgroundColor: "{colors.amber-wash}"
    textColor: "{colors.ivory-primary}"
    rounded: "{rounded.tile}"
    padding: "0 12px 0 6px"
    height: "26px"
  scroll-thumb:
    backgroundColor: "{colors.ivory-faint}"
    rounded: "{rounded.control}"
    width: "6px"
---

<!-- Provenance: recorded after the build from the shipped artifact (commits 5338eec, efe044d, 6f28a91 on feature/beautiful-torvalds-9762d6). Direction contract: .impeccable/surfaces/src-pianomidivisualizationapp-mainwindow-xaml.md, seed key a1a0326c. Form: "Piano character", pinned by the user; the dealt player-piano roll (position 5) folded into the song lane and take timeline. Finish review: ship, scoped to the eight scored fixes. Product truth lives in PRODUCT.md and is not repeated here. -->

# Design System: Piano MIDI Visualization

Platform note: this is a native WPF (.NET 10) app. The normative source is `src/PianoMidiVisualizationApp/Themes/Dark.xaml`: `Color.*` primitives, `Brush.*` role brushes built only from them, `Size.Readout.*`, and the control templates. The code-behind renderers (`PianoKeyboardControl`, `FallingNotesControl`, `TakeTimeline`, `CircleOfFifthsControl`, `GrandStaffControl`) resolve those brushes by key and must keep doing so. Token slugs above map one-to-one to `Color.*` keys (`lacquer-stage` is `Color.Lacquer.Stage`, `held-blue` is `Color.Blue`, `target-magenta` is `Color.Target`, and so on). The CSS in the sidecar is a translation for preview only.

## Overview

**Creative North Star: "The Piano's Case"**

The window is the instrument's body. A fixed ladder of warm lacquer greys recedes; the ivory keys and the ivory chord name lead; one amber marks the key you're in and whatever is still waiting on you. The chrome (title bar, sidebar, panels, status bar) sits a step darker than the stage, so the house lights are down and the only lift in the window is where the readout and the keys are. The player glances up and finds the chord name and the lit keys at once.

Density is instrument-panel tight: 12px body text, 22px tiles, 3px corners, a 900x420 minimum window where every row is budgeted. Controls are lacquer keys with no border at rest; only inputs are outlined. A switched-on control becomes an ivory keytop with lacquer ink rather than borrowing a hue. Every other hue has exactly one job, published as a colour table and held identical across the keyboard, circle of fifths, chord strip, song lane and recorder.

The song lane and the take timeline are piano rolls: notes are flat slots cut into a deep lacquer well, bar numbers sit in the margin, and the keyboard's top edge is the tracker bar every note is read against. It rejects the generic dark-IDE arrangement: grey panels, bordered grey buttons, and a muted blue accent borrowed for every "on".

**Key Characteristics:**
- Six lacquer steps, darkest first, with nothing between them; chrome darker than stage.
- One ivory ramp carries text and keytops; Muted and brighter clear 4.5:1 up to Raised.
- Amber is the only brand hue: key centre and "waiting on you", never decoration.
- A published one-job-per-hue colour table (blue = you playing, magenta = the app asking).
- Ink, not boxes: borderless controls, outlined inputs, one edged tile at a time.
- Roles never rest on colour alone: edge marks, lips, outlines and word tags back every hue.

## Colors

A warm-black lacquer ladder, an ivory text-and-keytop ramp, one amber, and a small table of single-job signal hues.

### Primary
- **Key-Centre Amber** (`amber`): text and glyphs that mean the key centre or attention: the status chip's text and dot while something isn't ready, the attention dot. The amber family carries the same meaning at other weights: **Strong Amber** (`amber-strong`) for marks (the tonic's edge, MIDI-learn listening, warnings); **Pale Amber** (`amber-pale`) for the in-key edge mark on ebony or held keys; **Deep Amber** (`amber-deep`) for the circle's selected wedge under ivory text, the tonic's front lip on ivory and the metronome downbeat flash; **Amber Border** (`amber-border`) for attention outlines and the in-key front lip on ivory; **Amber Wash / Wash Hover / Fill** (`amber-wash`, `amber-wash-hover`, `amber-fill`) for amber washes on the stage and the chrome respectively (the first-run next step, the circle's related keys); **Amber Ink** (`amber-ink`) for text set on an amber fill.

### Secondary
- **Held-Key Blue** (`held-blue`): you are playing. Held keys, the circle's chord-root marker, the played chord tile's edge (6.3:1 on the stage), your recorded notes on the take timeline, a learned piano control firing.
- **Asking Magenta** (`target-magenta`): the app asking for a key. Hint outlines and dots on the keyboard (with a deep keyline so they separate from light fills), the hovered chord tile's outline, the chord wait mode is holding for.

### Tertiary
- **Right-Hand Teal** (`right-hand-teal`) and **Left-Hand Violet** (`left-hand-violet`): your two hands in a song, hues used nowhere else. Accompaniment parts are quiet neutrals (stone, pewter, clay, graphite) so colour always means your hands.
- **Hit Green** (`hit-green`): you hit it, and nothing else.
- **Miss Red** (`miss-red`): you missed it. **Record Red** (`record-red`) bright while armed or recording, **Record Idle** (`record-idle`) dull at rest.
- **Close Red** (`danger-red`): Windows' own close-button red, the title-bar close hover only. **Danger Text** (`danger-text`): the hover ink of a remove glyph.

### Neutral
- **Lacquer Deep** (`lacquer-deep`): wells the eye reads into — the song lane, the take timeline, the MIDI log, the circle's minor ring.
- **Lacquer Chrome** (`lacquer-chrome`): the house — title bar, sidebar, panels, toolbar strip, status bar; also the ink on ivory.
- **Lacquer Stage** (`lacquer-stage`): the lit stage the readout and keys sit on; overlays laid over it (settings drawer, song cards).
- **Lacquer Raised** (`lacquer-raised`): controls, tiles and inputs at rest; popups.
- **Lacquer Hover** (`lacquer-hover`): a control under the pointer; separators.
- **Lacquer Line** (`lacquer-line`): input and popup borders, and the strongest fill — a selected or sounding row.
- **Keytop Ivory** (`ivory-key`): white keytops, the playhead, activity flashes.
- **Ivory Primary / Secondary / Tertiary** (`ivory-primary`, `ivory-secondary`, `ivory-tertiary`): text from the chord name down to headers and quiet labels; Secondary is also the focus ring, the sounding-chord edge and the loop range.
- **Ivory Pressed** (`ivory-pressed`): "on" — a switched-on toggle or the primary action.
- **Ivory Muted** (`ivory-muted`): the lowest ink for words (hints, bar numbers, gesture text, shortcut hints). `Brush.Text.Faint` and `Brush.Text.Hint` deliberately share this ink; the names keep the intent.
- **Ivory Faint** (`ivory-faint`): marks only — staff lines, the metronome's dot, the scrollbar thumb (4.2:1 on deep). Never words.
- **Ivory Disabled** (`ivory-disabled`): disabled text and the idle MIDI dot.

### Named Rules
**The Lacquer Ladder Rule.** Surfaces use only the six lacquer steps, and nothing sits between them. Depth is a step on the ladder, never a new grey.

**The House Lights Down Rule.** Chrome is one step darker than the stage. Anything that is not the readout, the staff, the circle or the keys belongs on chrome.

**The One Job Per Hue Rule.** Blue is you playing; amber is the key centre and whatever is waiting on you; magenta is the app asking for a key; teal and violet are your hands; green is hit; red is miss and recording. A hue is never borrowed for a second meaning in any widget.

**The On Means Ivory Rule.** A switched-on toggle (metronome, loop, 7ths) and the one primary action are ivory keytops with lacquer ink. Amber never means "this is on"; blue never means "this is on".

**The Marks Not Words Rule.** Ivory Faint draws marks. Any text uses Ivory Muted or brighter, which clears 4.5:1 on every lacquer step up to Raised.

## Typography

**Body Font:** Segoe UI (the Windows system UI face; WPF's default, never overridden)
**Data Font:** Consolas
**Icon Font:** Segoe MDL2 Assets (`Font.Icon`); music glyphs on the staff come from Segoe UI Symbol, drawn by `GrandStaffControl`

**Character:** One native sans at every level, with weight and size doing the work; Consolas appears only where the content is data and digits must hold still.

### Hierarchy
- **Display** (Bold 700, 64px, `Size.Readout.Name`): the chord name at the centre of the stage. The only display-size text in the app.
- **Headline** (SemiBold 600 at 18px, `Size.Readout.Function`): the Roman numeral on the readout's voicing line, sized to share that line's 19px box. Song-card titles and empty states run 16–20px SemiBold/Bold.
- **Title** (SemiBold 600, 12px): panel headers such as "Chord Progression" and "Next chord", in Ivory Tertiary, sentence case.
- **Body** (Regular 400, 12px; 14px for the readout's voicing, `Size.Readout.Voicing`): controls, menus, tiles, lists.
- **Label** (Regular 400, 10–11px): secondary details, gesture text, song details, the function tag.
- **Data** (Consolas, 18px for the readout's notes `Size.Readout.Notes`, 14px intervals `Size.Readout.Intervals`, 11–12px for the MIDI log and clocks): pitches, intervals, the MIDI log, time readouts.
- **Key labels** (set in `PianoKeyboardControl`): Medium 500 at rest, SemiBold 600 when held; white keys 9px (10px for the octave C, 11px held), black keys 8px (9px held). The held label clears 6.2:1 on held blue, the black held label 7.2:1, the C label on the tonic amber 4.9:1.

### Named Rules
**The Data Wears Consolas Rule.** Monospace is for pitches, intervals, the MIDI log and clocks only, so digits don't shift as they change. It is never a stylistic voice for labels or headers.

**The Weight Not Case Rule.** Hierarchy comes from size and weight in sentence case. There are no uppercase labels, tracked-out headers or kickers.

## Layout

A single fixed-structure window, minimum 900x420, default 1400x620. Rows, top to bottom: a 32px chrome title bar; the stage (a star row); then the recorder, MIDI log, AI chat and status bar as collapsible auto rows. The stage splits into the practice toolbar (song mode), the readout row (MinHeight 140), the diatonic chord strip, the practice toolbar strip and the keyboard. The readout row is three columns: grand staff left, chord readout centred, inlaid circle of fifths right. The chord-progression sidebar is a fixed 220px column on chrome that collapses to zero.

The vertical budget is tight: about 24px of slack at 900x420. The chord strip is one line of 22px tiles because it has only 24px to spend; every readout line carries a MinHeight so releasing the keys never collapses it. The keyboard scales down in a Viewbox below roughly 1070px wide with the sidebar open, and clips rather than shrinks vertically.

Spacing is small and even, in 2 / 4 / 6 / 8 / 12 / 16px steps. Inline gaps between controls are 6–8px; panel insets are 8–16px; tiles sit 2px apart. Zen mode hides the chrome and leaves the stage.

## Elevation & Depth

Depth is tonal, not shadowed. A surface is lifted or sunk by moving one step on the lacquer ladder: the deep wells (song lane, take timeline, MIDI log, the circle's segments) sit below the stage; controls sit a step above it. Overlays on the stage are set apart by a scrim (`#B3000000`) or the deep lane beneath, not by a shadow.

Shadows exist only where the object is physical or floats above everything:

### Shadow Vocabulary
- **Keytop** (`DropShadowEffect`: direction 270, depth 1, blur 2, opacity 0.15, black): white keys, a hairline of lift at the front.
- **Ebony sharp** (`DropShadowEffect`: direction 315, depth 3, blur 5, opacity 0.5, black): black keys standing proud of the ivory. The ebony fill is a four-stop bevel: a lit lacquer edge, a fast falloff, a slow darkening to Lacquer Deep.
- **Piano echo** (`DropShadowEffect`: direction 270, depth 3, blur 14, opacity 0.45, black): the transient echo pill over the stage, the only floating chrome.

### Named Rules
**The Step Not Shadow Rule.** Chrome, panels, tiles and inputs are flat. To separate a surface, move it a step on the ladder; shadows are reserved for the keys and the echo.

## Shapes

Small, firm corners. Controls, inputs, menus and scroll thumbs are 3px; chord tiles, setup steps and the focus ring are 4px; cards over the song lane are 6px (chat bubbles 8px). Pills are fully rounded to half their height: the 22px status chip (11px), the song-lane badges (9px), the piano echo (14px). White keys round only their front corners (4px); black keys 2px.

Borders are rare. Buttons and toggles keep a transparent 1px border so any state that draws one keeps the same size. Inputs, combo boxes and popups have a 1px Lacquer Line outline. Among chord tiles only the played one is edged. Separators are 1px Lacquer Hover. Keys carry a 0.5px keyline.

**The Ink Not Boxes Rule.** Buttons carry no border at rest; only inputs are outlined. An edge is a state (played, hovered hint, focused), not decoration.

## Components

### Buttons
Lacquer keys: quiet at rest, ivory when they matter.
- **Shape:** gently squared (3px), 1px transparent border, 3px/10px padding.
- **Default:** Lacquer Raised with Ivory Secondary text. Hover lifts to Lacquer Hover with Ivory Primary text; pressed sinks to Lacquer Stage; disabled drops to Lacquer Chrome with Ivory Disabled text.
- **Primary** (`Style.PrimaryButton`, one per view: "Start audio", "Send", "Play again"): an ivory keytop (Ivory Pressed) with Lacquer Chrome ink, SemiBold; hover brightens to Ivory Primary, pressed settles to Ivory Secondary.
- **Toolbar toggle** (`Style.ToolbarToggle`): the default button while off (padding 0/7px); on, it becomes an ivory keytop with lacquer ink, and brightens to Ivory Primary on hover.
- **Glyph buttons** (`Style.GlyphButton`): Segoe MDL2 glyphs at 11px in a 24px minimum hit area, Ivory Muted, brightening to Ivory Primary on hover. Only removing something (`Style.RemoveGlyphButton`: a saved chord, a take) turns Danger Text red on hover. Title-bar buttons are 46x32, flat, and only close turns Windows' red.
- **Focus:** a 1.5px Ivory Secondary ring, 4px radius, 2px outside the control, replacing WPF's dotted black one.

### Chips
- **Status chip** (`Style.StatusChip`): a 22px pill in the title bar with a 1px Lacquer Hover outline, Ivory Tertiary text and a 7px dot. While the keyboard or audio isn't ready it fills Amber Fill with an Amber Border and amber text; its dot is grey when fine, amber when not, and flashes Keytop Ivory with each note played. Never focusable, so it can't claim Space.

### Cards / Containers
- **Corner Style:** 6px for cards laid over the song lane; panels themselves are square.
- **Background:** panels on Lacquer Chrome; overlays on Lacquer Stage; popups and menus on Lacquer Raised with a 1px Lacquer Line border and 4px vertical padding.
- **Shadow Strategy:** none; see Elevation & Depth.
- **Internal Padding:** 8–16px.

### Inputs / Fields
- **Style:** Lacquer Raised fill, 1px Lacquer Line outline, 3px radius, 3px/6px padding (combo boxes 3px/8px), Ivory Secondary text, Ivory Primary caret. Combo boxes draw a 1.4px Ivory Muted chevron and can show a Tag placeholder in Ivory Tertiary.
- **Hover:** the combo box outline brightens to Ivory Muted.
- **Check and radio:** 14px box (3px radius) or ring on Lacquer Raised with a Lacquer Line stroke; checked shows an Ivory Primary tick or 6px dot and the stroke moves to Ivory Muted.
- **Disabled:** Ivory Disabled text and strokes.

### Navigation
- **Menus:** the title-bar menu is flat 12px Ivory Secondary text; highlighted items take Lacquer Hover. Dropdowns are Lacquer Raised popups, rows 6px tall-padded with a 26px check column, gesture text in 11px Muted, 1px Lacquer Hover separators inset to the text.
- **Scrollbars:** 8px, a clear track that shows Lacquer Raised under the pointer so the scroll area can be found, and an Ivory Faint thumb (3px radius) with a 24px length floor; the thumb brightens to Muted on hover and Tertiary while dragging.

### Keyboard (signature)
Ivory keytops (`ivory-key` fading to `#EAE5DA`) and beveled ebony sharps, with meanings from the colour table. Held keys are held blue (a paler "ringing" blue while the sustain pedal holds them); the tonic is a full amber keytop; other in-key white keys take a warm wash well short of the tonic. Every role also has a non-colour cue: an in-key white key carries a dark amber front lip (Amber Border, 3.7:1 against the wash; the tonic's lip is Deep Amber and thicker, 3.6:1 against the tonic); in-key black and held keys carry a light amber top edge; hints are magenta outlines and dots with a deep keyline. Idle keys label only the Cs unless "All note names" is on.

### Diatonic Chord Strip (signature)
Borderless 22px lacquer tiles (Lacquer Raised, 4px radius, 52px minimum width, 2px apart) with Ivory Tertiary text; the numeral is SemiBold. The tile matching what's held keeps its lacquer fill, gains a held-blue edge and Ivory Primary text; it is the only edged tile, so it reads without the hue. A hovered tile lifts to Lacquer Hover with a magenta outline, matching the hint markers it draws on the keys. Never focusable.

### Piano Roll: Song Lane and Take Timeline (signature)
Both are rolls in a Lacquer Deep well. Song notes are flat slots in teal (right hand) and violet (left hand), accompaniment in warm neutrals, auto-play notes translucent; bar lines Lacquer Hover, octave lines Lacquer Raised, bar numbers Ivory Muted in the margin, the hit line Ivory Tertiary at the keyboard's top edge. Recorded notes are held blue; the playhead is Keytop Ivory at 1.5px. A-B loops are ivory ranges (Ivory Secondary edges in the song lane; a 10% Ivory Primary region with Ivory Tertiary edges on the timeline), with the outside dimmed.

### Circle of Fifths (signature)
Inlaid in the stage: major segments Lacquer Chrome, minor Lacquer Deep, like ebony set into the case. The selected key is a Deep Amber wedge under ivory text (its count clears 4.7:1); related keys take Amber Fill; hover is Ivory Muted; the chord-root marker is a held-blue outline.

### Setup Checklist
Row buttons 26px tall, 4px radius, flat until hovered (Lacquer Hover). The next step to do sits on an Amber Wash (Amber Wash Hover on hover) with Ivory Primary text; done steps drop to Ivory Tertiary. Never focusable.

### Metronome
A toolbar toggle with a drawn glyph and a beat light inside it. Off, the light is a hollow Ivory Muted ring on lacquer; on (the toggle is ivory), it is an Ivory Faint dot that flashes Lacquer Chrome per beat and Deep Amber on the downbeat.

## Do's and Don'ts

### Do:
- **Do** build every brush from a `Color.*` token in `Dark.xaml` and resolve it by key in code-behind renderers; nothing else in the app picks a colour.
- **Do** keep chrome one lacquer step darker than the stage (`lacquer-chrome` under `lacquer-stage`).
- **Do** mark "on" and the single primary action with an ivory keytop and lacquer ink.
- **Do** give every colour-coded role a second cue: an edge mark, a lip, an outline, or a word tag (the Roman-numeral tags "secondary", "borrowed", "chromatic").
- **Do** keep words at Ivory Muted or brighter; use Ivory Faint only for marks.
- **Do** put Consolas on data that changes (pitches, intervals, clocks, the log) and Segoe UI on everything else.
- **Do** make stage controls that sit beside the save shortcut non-focusable (chord tiles, the status chip, setup steps, metronome buttons) so Space stays "save chord".
- **Do** put colours for stateful controls in Style setters, never local attributes, so hover and state triggers can paint.

### Don't:
- **Don't** add a grey between the lacquer steps, or a new surface colour outside the ladder.
- **Don't** use amber as decoration or to mean "this control is on"; it is the key centre and what is waiting on you.
- **Don't** borrow held-key blue for selection, links or "on"; blue means you are playing.
- **Don't** draw a border on a button or toggle at rest; only inputs are outlined.
- **Don't** use Ivory Faint for any text.
- **Don't** add shadows to chrome, panels, tiles or inputs; change the lacquer step instead.
- **Don't** use green for anything but a hit, or red for anything but a miss, recording, removal, or the window's close button.
- **Don't** use Unicode look-alike glyphs (✕, ↻, □) for icons; use Segoe MDL2 Assets codes or a drawn path.

### Open ideas (not shipped; not rules)
Considered at finish review and not taken: a fallboard lip above the keys, an inlay rim on the circle, a lacquer hairline between chrome and stage, and tracking on the chord name.
