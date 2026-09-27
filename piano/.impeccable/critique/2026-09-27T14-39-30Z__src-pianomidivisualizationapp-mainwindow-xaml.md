---
target: main window UI
total_score: 24
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 3
target_identity: "file:D:\\Repos\\PianoMidiVisualizationApp\\src\\PianoMidiVisualizationApp\\MainWindow.xaml"
target_fingerprint: "sha256:70fbd14dc1052a20e040fa723d760b9aebcfd868b6d68e3560a4f67c8a663d5f"
target_path: "D:\\Repos\\PianoMidiVisualizationApp\\src\\PianoMidiVisualizationApp\\MainWindow.xaml"
timestamp: 2026-09-27T14-39-30Z
slug: src-pianomidivisualizationapp-mainwindow-xaml
---
⚠️ DEGRADED: single-context (sub-agents not spawned: the user did not ask for them)

Evidence: the five off-screen renders in docs/screenshots (rendered from the real UI at d7b3183) plus the XAML/C# source. No browser pass: WPF is not viewable in a browser.

## Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 2 | MIDI/audio state only visible inside the Settings overlay; first run gives no hint that nothing is connected |
| 2 | Match System / Real World | 3 | Excellent theory language (V65, drop 2); status bar calls MIDI 62 "D5" while the keyboard calls it D4 |
| 3 | User Control and Freedom | 3 | F11 round-trips layout, Esc closes settings; progression capped at 8 with the cap only reported after the fact |
| 4 | Consistency and Standards | 2 | Two transports on screen in song practice; blue and green each carry 2-3 meanings |
| 5 | Error Prevention | 2 | "Mute out-of-key" disabled with no key and no reason; Space with no held notes fails silently to the status bar |
| 6 | Recognition Rather Than Recall | 3 | Good tooltips (~70) and "Space to save"; metronome toggle glyph ambiguous; Shift+F1-F9 map has no mnemonic |
| 7 | Flexibility and Efficiency | 3 | Rich shortcuts and click-to-change-key; but every action needs a hand off the piano |
| 8 | Aesthetic and Minimalist Design | 2 | 61 always-on key labels; one toolbar mixes key, metronome and recorder (~15 controls) |
| 9 | Error Recovery | 2 | Errors surface as raw "Audio error: {ex.Message}" in a status bar the user can hide |
| 10 | Help and Documentation | 2 | Strong README; in-app, the first launch is a blank stage with only a "View" menu |
| **Total** | | **24/40** | **Acceptable** |

## Design Specificity Verdict

LLM assessment: the stage is authored for this product; the chrome is not. The chord readout (name, figured-bass numeral, inversion, voicing, spelled notes, intervals), the circle of fifths with its diatonic wedge and the key-spelled grand staff could only belong to this app. Everything around them is the generic dark-IDE default: #1E1E1E-#333 greys, a muted #2B5278 accent, bordered grey buttons. Nothing in the chrome says "piano". The keyboard, the most-looked-at object, is generic gradient keys carrying 61 grey labels.

Deterministic scan: `impeccable detect` returned 0 findings (exit 0), but its rule engine reads HTML/CSS/JSX only, so the XAML was not analysed. This is "no automated signal", not "clean". No visual overlay (not a browser surface).

## Overall Impression

The musical core is excellent, and the readout is the best thing on screen. The biggest opportunity is that the app is designed for someone at a mouse, but its user is at a piano with both hands busy. Song practice also breaks the line between the falling notes and the keys they fall onto.

## What's Working

1. Readout hierarchy: one unmistakable hero (the chord name), then four lines stepping down in size and contrast. The MinHeight lines keep the layout from jumping when you release the keys.
2. A consistent colour grammar on the stage: blue = what you're playing (held keys, circle root marker, lit chord tile), amber = the key (tonic keys, circle wedge). It holds across three separate widgets.
3. Zen mode: a confident distillation, and F11 restores your layout exactly.

## Priority Issues

**[P1] Song practice: the toolbar sits between the falling notes and the keys.** In song-practice.png, notes land on the hit line at y≈543, then a ~90px band of Key/Major/metronome/record controls, then the keyboard at y≈633. The eye must jump that gap at the exact moment timing matters. Two transports are also on screen: the song's (restart, pause, Speed 100%, Loop) and the recorder's (●, ▶, ⟲, 100%), with the same loop glyph and the same "100%".
Fix: in song mode, put the global toolbar above the lane (or fold key and metronome into the song bar) so notes land directly on the keys. Hide the recorder transport unless the recorder panel is open.
Command: /impeccable layout

**[P1] First run is a blank stage with no setup path.** The default layout is zen: an empty readout, a keyboard and "Ready". Playing produces no sound and nothing says why. Setup is behind View → Settings, and the MIDI/audio state (the connection dot) only lives inside that overlay.
Fix: the empty readout slot becomes a three-step checklist (Connect keyboard → Choose a piano sound → Start audio), each step a button, and turns into a faint "Play something" once done. Add a persistent status chip in the title bar ("Digital Piano · ASIO") that opens settings and turns amber when disconnected.
Command: /impeccable onboard

**[P1] Held-key labels vanish exactly when they matter.** White-key labels are a fixed #82827D (PianoKeyboardControl.xaml.cs:59) on idle and held keys alike, so on a held blue key they're about 1.6:1: "B" on B2 in main-window.png, "A/D/F" in zen-mode.png. Meanwhile all 61 keys carry a label, so the one label you want is the dimmest.
Fix: held keys get a high-contrast label spelled for the key (Bb, not A#). Idle keys label only the Cs (C2-C7), with "all labels" as a view option.
Command: /impeccable typeset

**[P2] Blue and green each mean too many things.** Blue is "you are playing" and also the left-hand part (#5B8DEF, Dark.xaml:90). Green is the in-key tint (pale on white keys, dark on black keys in zen-mode.png), the right-hand part (#6CC24A) and "correct" (#8BE36B). In song practice you can't tell a landing left-hand note from your own held key.
Fix: reserve blue for "you are playing". Move the parts to hues used nowhere else (the unused teal/violet part colours), and show in-key on black keys as a thin top stripe rather than a green body.
Command: /impeccable colorize

**[P2] Every control needs a hand off the keys.** Space saves the chord you are *holding* (SaveCurrentChord requires held notes, MainViewModel.cs:456), so a two-handed voicing can't be saved. The sustain pedal (CC64) is only logged (MidiInputService.cs:117): it neither sustains the sound nor drives anything.
Fix: latch the last chord in the readout for ~2s after release (dimming as it ages) so Space saves what you just played; honour CC64 for sound and for the readout; optional MIDI-learn so a spare pad or button saves the chord, arms recording or toggles the metronome.
Command: /impeccable shape

## Persona Red Flags

**Alex (power user):** saving a 4-chord progression means 4 hand lifts. Shift+F1-F9 panel toggles have no mnemonic (F6 circle, F8 staff). The song loop range reads "bars – 1 + – – 4 +": the range dash and the minus button run together.

**Jordan (first-timer):** launches to a blank zen stage, plays, hears nothing. "View" is the only menu. The Key picker is blank with no "No key" text. "Mute out-of-key" is disabled with no reason. "Click a key to select it" under the circle is the only in-app guidance.

**Sam (low vision / screen reader):** "Space to save" is #666 on #333 (≈2.2:1); held-key labels are ≈1.6:1. Key state (in-key vs tonic vs held) is carried by colour alone. The keyboard is a Canvas of Rectangles with no automation peers, so the main visual is invisible to a screen reader.

## Minor Observations

- Status bar octave mismatch: "NoteOn Ch1 Note=62 (D5)" vs D4 everywhere else. MidiInputService.cs:76 uses NAudio's NoteName (60 = C5); use MusicNaming.WithOctave. Raw MIDI text in primary chrome duplicates the MIDI log panel anyway.
- Settings strip: the "SF2:" label also takes .sfz; the path truncates mid-word ("Salamander Gra"); "AI:" is an empty field with no placeholder; "↻" is unlabelled; colon labels unlike the rest of the app. Group as Input / Sound / Assistant, show the file name not the path, show errors inline. (/impeccable clarify)
- Score row "✓ 24  ✗ 1  missed 0": two glyphs and a word. Pick one form.
- Take list "Take 2 10.15": the time of day reads as a version number. Use "10:15".
- Metronome toggle "● ♩" looks like record + note, and the real record button (red ●) is four controls to the right.
- Chord Progression sidebar: a large dead zone between the saved chords and "Next chord", with an empty state that is just "0/8 chords". Use it for "Hold a chord, press Space".

## Questions to Consider

- What if the piano were the remote control: pedal, spare pads, a key below C2?
- What if song practice were one surface: notes fall straight onto the keys, one transport?
- Does the keyboard need 61 labels, or only what you're playing and where C is?
- What if the readout and the keys were the only things in colour, and all chrome receded to one quiet grey?
- Should the app have a character beyond "dark IDE", e.g. lacquer black, ivory keys, and the key-amber as the brand hue?
