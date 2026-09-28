---
version: 1
slug: "src-pianomidivisualizationapp-mainwindow-xaml"
primary_target: "src/PianoMidiVisualizationApp/MainWindow.xaml"
related_targets: ["src/PianoMidiVisualizationApp/Themes/Dark.xaml","src/PianoMidiVisualizationApp/Views/PianoKeyboardControl.xaml.cs"]
---

# Main window

**Scope:** the whole WPF main window and everything it hosts: title bar, stage (grand staff,
chord readout, circle of fifths, chord strip, practice toolbar, keyboard), progression
sidebar, song practice (song bar, falling-notes lane), recorder, MIDI log, AI chat, status
bar, settings overlay and piano controls. **Mode:** Operate.

**Task:** the player glances up from the keys, between phrases, at a monitor about an arm's
length away. They read the chord and the keys, then act, mostly from the piano or with a
shortcut. Things they see many times a minute: held keys, the chord name and numeral, the
tonic and in-key keys, falling notes landing. Occasionally: the toolbar, the sidebar, the
recorder. Rarely: settings.

**Constraints:** this is a retune of an existing layout. Don't reinvent controls, don't use
display fonts in labels, and don't add decorative motion. Colour meanings were set by the
keyboard-legibility work: retune hues, never re-merge meanings. Contrast: body text ≥4.5:1,
large text and glyphs ≥3:1, held-key labels ≥4.5:1 against the held fill. A key state never
relies on colour alone. The window must still fit at the 900x420 minimum.

**Memorable moment:** held ivory keys in blue under an ivory chord name, on lacquer, with the
tonic in amber and nothing else competing.

## Direction contract

THESIS: The window is the piano's case. Lacquer recedes, ivory keys and the ivory chord name
lead, and one amber marks the key you're in. It refuses the generic dark-IDE arrangement:
grey panels, bordered grey buttons, and a muted blue accent borrowed for every "on".

OWN-WORLD: A fixed six-step ladder of warm lacquer greys (deep, chrome, stage, raised, hover,
line) and one ivory text ramp. The keys are ivory with ebony sharps. Amber is the only brand
hue: the key centre, and whatever is waiting on you. A switched-on control turns ivory with
lacquer ink. Chrome buttons have no border at rest. Each other hue has one job: blue is you
playing, magenta is the app asking for a key, teal and violet are your hands, green and red
are hit and miss. Red also means recording.

STORY: The player glances up and finds the chord name and the lit keys at once. The toolbar
and sidebar stay dark until wanted. Amber shows where the key centre is and what still needs
setting up.

FIRST VIEWPORT: The title bar and sidebar are the darkest lacquer. The stage is one step
lighter, with the ivory chord name at 64px in the centre, the staff on the left and the
inlaid circle of fifths on the right. The chord strip sits on the stage as borderless
lacquer tiles. The practice toolbar sits in a recessed lacquer strip. The keyboard spans the
bottom: ivory and ebony keys, the tonic amber, held keys blue. The primary action is playing
the piano, and after that Space to save.

FORM: Piano character, pinned by the user. It leads a grounded list of seven: grand piano
case, digital-piano front panel, engraved urtext score, piano interior (gold plate and felt),
player-piano roll, mechanical metronome, recital programme. The dealt player-piano roll
(position 5) is folded in on the song lane and the recorder timeline: notes are flat cut
slots, bar numbers sit in the margin, and the hit line is the tracker bar. Six declined
challengers each donated one discipline: the lacquer ladder (Game Boy), house lights down
(cyclorama), on means ivory (HyperCard), amber only on the cord you pull (drawcord cape), ink
not boxes (timetable), and a published colour table (teletext). Seed key a1a0326c.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
