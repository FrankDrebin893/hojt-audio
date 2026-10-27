# Product

<!-- impeccable:product-schema 1 -->

## Platform

windows

## Users

Two audiences with equal weight (confirmed 2026-09-27):

- **The author**, practising at home on a digital piano: learning harmony, practising songs,
  recording takes.
- **Self-taught players** who download the public release: people with a MIDI keyboard who want
  to understand what they are playing.

Both sit at the keyboard with both hands on the keys. The screen is a monitor or laptop above
or beside the keys, about an arm's length away or more, and they look at it between phrases,
not as a document in front of them. Reaching for the mouse means taking a hand off the piano.

## Product Purpose

The app plays a MIDI keyboard through any SoundFont (`.sf2`) or SFZ piano with low-latency
audio. While you play, it shows what you are holding and what that means in the key. Success
means the player understands the harmony under their hands while they play, without stopping
to work it out. Song practice and recording build on that.

## Positioning

**Live harmony understanding.** It names what you are holding in the context of the key, as
you play. The chord name covers slash chords, sevenths and extensions. Alongside it: the
figured-bass Roman numeral (V65, viiø7), the inversion, the voicing (close, open, shell,
drop 2), the notes spelled for the key and their intervals. The grand staff carries the key
signature, the circle of fifths shows the key and the chord's root, and the diatonic chord
strip lights the chord you're playing. Falling-notes players (Synthesia, Piano Marvel) and DAW
piano rolls show which notes to play. This app tells you what they are.

## Operating Context

- Windows 10/11 desktop, at a digital piano or MIDI controller connected by USB or a MIDI
  interface.
- Audio goes out through ASIO (around 5 ms with a good driver) or WASAPI. No piano sound is
  bundled: the user downloads an `.sf2` or `.sfz` (e.g. Salamander Grand Piano) and picks it in
  Settings.
- Sessions are long and repetitive: a phrase is played, the screen is glanced at, it is played
  again. Every panel can be shown or hidden, and zen mode (F11) cuts the screen down to the
  readout and the keyboard.
- Devices, sound, key, metronome, panel layout, window position and the open song persist in
  `%LOCALAPPDATA%\PianoMidiVisualizationApp\settings.json`. MIDI and audio reconnect on launch.

## Capabilities and Constraints

- **Hear:** SoundFont playback through MeltySynth, and SFZ through a built-in sample player with
  velocity layers. The CC64 sustain pedal holds the sound in both. There is a master volume and
  a metronome click volume.
- **See:** a 61-key keyboard (C2–C7) that scales with the window, beneath a recessed OLED
  display that spans the full width of the keys, with a right-hand hardware panel housing the
  volume and click halo knobs and twelve shortcut panel keys. There are 12 scales; when a key
  is selected, out-of-key white and black keys become transparent, in-key keys carry a crisp
  edge marker with the tonic set apart, and note names switch to sharps or flats to suit the
  key. "Mute out-of-key" silences wrong notes but still shows them. A key the pedal holds stays
  lit and still counts toward the chord name.
- **Understand:** the chord readout (which latches for two seconds after releasing the keys,
  dimming), grand staff, circle of fifths (click to change key) and diatonic chord strip (click
  to play).
- **Play from the piano:** in Settings → Piano controls, any key, pad, button or pedal can be
  learned for six actions: record, play the latest take, metronome on/off, tap tempo,
  play/pause the song and restart the song. A mapped key stops sounding its note until cleared.
  A short note over the stage confirms each action.
- **Songs:** open a `.mid` and practise with falling notes. Choose tracks and hands, set the
  speed, loop a bar range, use "Wait for me" and get scored.
- **Recorder:** the last 10 takes are kept. A note held by the pedal is recorded for as long
  as it sounds. Play back at 50–100%, loop an A–B region, and export a take as `.mid`.
- **Metronome:** 30–240 BPM, Tap tempo, beats per bar with an accented downbeat, and a beat
  light driven by the audio clock.
- **Also:** a MIDI log, and an optional AI Music Assistant. The assistant uses Google Gemini
  with the user's own API key, and messages plus chord context are sent to Google.
- **Tech:** C# / .NET 10 / WPF, shipped as a self-contained single-file `win-x64` exe with no
  installer. It isn't code-signed yet. The minimum window size is 900x420.
- **Design tooling limits:** Impeccable's `detect`, `live` and `generate` are web-only. On XAML,
  `detect` returns `[]`, which means no signal, not clean. UI is verified with the off-screen
  WPF harness (`.claude/skills/update-readme/screenshots`). Never take desktop or window
  screenshots of this app.
- **Licensing:** SoundFonts and samples are never committed. Adding or removing a NuGet
  package means updating `THIRD-PARTY-NOTICES.txt`.

## Brand Commitments

Nothing is fixed (confirmed 2026-09-27). The name "Piano MIDI Visualization", the `app.png`
icon, the voice and dark-only theming may all change.

The user chose the visual direction on 2026-09-27: **"piano character"**. That means
lacquer-black surfaces, ivory keys, the key-amber as the one brand hue, and chrome that
recedes so the chord readout and the keys lead. DESIGN.md holds that system.

On 2026-10-19 the user asked for the app to look like a display on hardware, and picked
**"phosphor scope"** from five concepts (`docs/design-concepts/`; the other four are kept for
later). It was tried in the app first, then replaced on 2026-10-20 after two more rounds
of concepts with real keys: the user picked **"scarlet anodised"** (concept K,
`docs/design-concepts/Scarlet.dc.html`). The chrome is brushed scarlet aluminium with white
legends; the readout and chord strip sit in a black OLED window with a bezel spanning the full
width of the 61-key keyboard; the keys are real ivory and ebony, a held key pressed down with a
blue-lit back edge and out-of-key keys dimmed when a key is active; amber stays the attention
hue; two halo knobs and twelve panel keys sit on the right-hand hardware panel beside the OLED
and keyboard. DESIGN.md holds the system. The phosphor build is in git history (commits 30a6ba0
and 08255c7).

## Evidence on Hand

- `README.md`: the feature list, shortcuts, requirements and getting started.
- `docs/screenshots/`: off-screen renders of the real UI (main window, song practice, recorder,
  zen mode, settings, first-run).
- `docs/index.html`: GitHub Pages landing page (`https://hojte.net/hojt-audio-piano-viz/`).
- `.impeccable/critique/`: the Impeccable critique snapshot from 2026-09-27 (24/40).
- There are no user counts, testimonials, reviews, download figures or press. Don't invent
  any.

## Product Principles

1. **The hands stay on the keys.** Anything needed mid-phrase must read from arm's length and
   be reachable from the piano itself (a learned key, pad or pedal), not only the mouse.
2. **The readout and the keys lead.** Everything else supports them and gives way when screen
   space is short.
3. **Speak like a musician.** Use exact theory terms (V65, drop 2, 1st inversion), spell notes
   for the key (Bb in F, not A#), and use plain words everywhere else.
4. **Every signal means one thing.** A state keeps one meaning across every widget it appears
   in.
5. **Nothing is lost.** Layouts, devices and sessions are remembered, and every way of hiding
   something has a way back.

## Accessibility & Inclusion

Committed 2026-09-27:

- **Contrast floor for reading at a distance:** body text ≥4.5:1, large text and UI glyphs
  ≥3:1, held-key labels ≥4.5:1 against the held fill.
- **Not colour alone:** key states (held, in key, out of key, tonic, each hand in a song) also
  differ by something other than hue, for colour-blind players.
- **Keyboard-only use:** every action can be reached without a mouse.

Screen-reader support was offered and not made a commitment.
