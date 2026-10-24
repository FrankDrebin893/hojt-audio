---
version: 1
slug: "src-pianomidivisualizationapp-mainwindow-xaml"
primary_target: "src/PianoMidiVisualizationApp/MainWindow.xaml"
related_targets: ["src/PianoMidiVisualizationApp/Themes/Dark.xaml","src/PianoMidiVisualizationApp/Views/PianoKeyboardControl.xaml.cs"]
---

# Main window

**Scope:** the whole WPF main window and everything it hosts: title bar, stage (recessed OLED
display spanning the 61-key keyboard with grand staff, chord readout, circle of fifths, and
chord strip; practice toolbar; 61-key keyboard; and the right-hand hardware panel with rotary
halo knobs and twelve rubber panel keys), song practice (song bar, falling-notes lane),
recorder, MIDI log, AI chat, status bar, settings overlay and piano controls. **Mode:** Operate.

**Task:** the player glances up from the keys, between phrases, at a monitor about an arm's
length away. They read the chord and the keys, then act, mostly from the piano or with a
shortcut. Things they see many times a minute: held keys, the chord name and numeral, the
tonic and in-key keys (with out-of-key keys dimmed), falling notes landing. Occasionally: the
toolbar, the right-hand hardware panel, the recorder. Rarely: settings.

**Constraints:** this is a retune of an existing layout. Don't reinvent controls, don't use
display fonts in labels, and don't add decorative motion. Colour meanings were set by the
keyboard-legibility work: retune hues, never re-merge meanings. Contrast: body text ≥4.5:1,
large text and glyphs ≥3:1, held-key labels ≥4.5:1 against the held fill. A key state never
relies on colour alone. The window must still fit at the 900x420 minimum.

**Memorable moment:** held ivory keys lit blue under a crisp chord name in the recessed OLED
display spanning the full width of the keyboard, with out-of-key keys dimmed, the tonic marked
clearly, and nothing else competing.

## Direction contract

THESIS: The window is an instrument's front panel ("Scarlet anodised", building on the piano
character foundation). Brushed scarlet aluminium chrome and the black OLED window frame the
reading, white keys and the white chord name lead, out-of-key keys dim when a key signature is
active, and amber marks what is waiting on you. It refuses the generic dark-IDE arrangement:
grey panels, bordered grey buttons, and a muted blue accent borrowed for every "on".

OWN-WORLD: Brushed scarlet aluminium (`Color.Lacquer.Chrome`) across every chrome surface, a
recessed true-black OLED display (`Color.Lacquer.Stage`), black rubber controls
(`Color.Lacquer.Raised/Hover/Line`), and a crisp white legend ramp (`Color.Ivory.*`). The keys
are neutral white with ebony sharps; when a key is selected, out-of-key white and black keys
are dimmed (`KeyRole.OutOfKey`) while in-key and tonic keys carry edge markers. A switched-on
control turns white with black ink. Each signal hue has one job: blue is you playing, amber is
what waits on you, magenta is the app asking for a key, teal and violet are your hands, green
and red are hit and miss (with red also for recording, and green-to-red level meters on the
halo knobs).

STORY: The player glances up and finds the chord name in the full-width OLED display and the
lit keys at once. The toolbar and right-hand hardware panel stay calm until wanted. Dimmed
out-of-key keys and crisp edge markers show where the scale and tonic sit, and amber shows what
still needs setting up.

FIRST VIEWPORT: The title bar and stage share one continuous brushed scarlet panel. On the left
(`Grid.Column="0"`), the recessed black OLED display spans the full width of the 61-key
keyboard below it, with the chord name at 72px in the centre, the grand staff on the left, the
circle of fifths on the right, and the diatonic chord strip along its bottom edge with physical
soft keys below the bezel. Beneath the OLED sit the recessed practice toolbar and the 61-key
keyboard: white and ebony keys, out-of-key keys dimmed, the tonic marked, held keys lit blue.
On the right (`FrontPanelColumn`, `Grid.Column="1"`), the hardware front panel holds the master
Volume and metronome Click rotary halo knobs and twelve rubber shortcut keys with LEDs. The
primary action is playing the piano.

FORM: Scarlet anodised front panel (concept K), evolved from piano character. It leads a
grounded list of seven: grand piano case, digital-piano front panel, engraved urtext score,
piano interior (gold plate and felt), player-piano roll, mechanical metronome, recital
programme. The dealt player-piano roll (position 5) is folded in on the song lane and the
recorder timeline: notes are flat cut slots, bar numbers sit in the margin, and the hit line is
the tracker bar. Six declined challengers each donated one discipline: the lacquer ladder
(Game Boy), house lights down (cyclorama), on means ivory/white (HyperCard), amber only on the
cord you pull (drawcord cape), ink not boxes (timetable), and a published colour table
(teletext). Seed key a1a0326c.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
