# Changelog

Each release has a `## <version>` section here. The release workflow uses it as the text of the
GitHub release.

## Unreleased

### Resources

- The Guitar Chord Atlas has eight more chord types (m6/9, maj7♯5, 9sus4, 13sus4, m9♭5, 9♯5, 9♭5
  and 7♭13, 42 in all) and, on every chord, more voicings: shapes rooted on the 3rd string and
  inversions with the 3rd, 5th or 7th in the bass, and more open shapes. A Voicings switch
  shows All voicings or the Essential shapes only. `scripts/check-guitar-chords.js` plays every
  fingering in all 12 keys against its chord and the hand

### Every plugin

- The plugins have a name of their own, HØJT DSP, on the page and on the releases. The DAW
  still lists them under Hojt Audio, and projects open as before. The page also links the
  Piano MIDI Visualization, a free app for practising piano, which now lives beside them
- The plugins' page moved to https://hojte.net/hojt-audio/dsp/. The old address is now a page
  for both HØJT DSP and the piano viz, in one look, and its old links to a plugin still land on it
- A window that sits still costs next to nothing: it is drawn again only when something in
  it changes (a knob moved, automation, a lamp, a meter, the tuner), not with every frame of
  the screen. An idle Guitar Amp took up to two thirds of a CPU core and now takes a few percent of
  one; the other windows went from about a sixth of a core to under two percent. Nothing in the
  sound or the look changed

### Drum Synth

- The Jazz kit's cymbals are dry and defined: every ride stroke is a clear ping over a quieter,
  even wash, and the hats and crashes keep their sizzle without the swirl of beating tones. They
  are a little darker, and the ride bell is rounder. A project on the Jazz kit sounds different;
  Rock, Metal and Groove are as they were

### Guitar Amp

- The Delay can follow the song's tempo. A small Sync button right of the Mix knob, with its
  LED above it, turns it on: the Time knob then picks a note, 1/4, 1/8 dotted, 1/8, 1/8
  triplet or 1/16 (`1/8 .`, `1/8 T` on its tape), and the repeats land on the beat in the DAW
  and follow when the tempo changes (between 20 and 1000 ms, so a 1/4 under 60 bpm stays at
  1000 ms). Off, Time is milliseconds as before and keeps its setting. New parameters Delay
  Sync (on / off, off) and Delay Note (default 1/8 dotted). A project or a preset from before
  opens with Sync off and sounds as it did; the factory presets keep it off. The standalone
  follows its own tempo (120 bpm, or what `--tempo` says)
- Typing a preset name after Save now works in REAPER (and in any host that takes the keys of a
  plug-in window for its own shortcuts): the letters never reached the amp before, while the
  same window worked on its own. It now tells the host that it wants every key while a name is
  being typed
- The window's look follows its new width. Every pedal in the row is now the same size, so the
  row fills the window with an even gap between pedals and the cables between them, and the
  narrow pedals (Comp, Chorus, Phaser, Flanger, Tremolo, Fuzz) are as wide as the rest, their
  knobs and pictures spread to fit. The strip across the top is used evenly: the meters, the
  Bank and Preset displays, Save, Output, Clip, On and Tuner stand in one row with the same gap
  between them, and no big empty space is left in the middle
- The Bank and Preset displays are now built-in LED displays: a dark recessed window with
  amber dot-matrix letters, and the unlit dots faintly showing. They hold more (up to 15
  letters of a preset's name), and the name you type after Save appears in the same display with
  its caret. The tuner is on the same kind of display, with its needle of lamps in amber
- Two more amps join the four at the end of the Amp stepper: Sang, a smooth, compressed
  singing lead (model SG-60) with the mids pushed forward and the top rounded off, and Drøn (DR-80), a big, loose and
  dark amp that lets the low end through and sags under hard playing. Each has a cabinet and a
  head of its own (Sang deep blue with brass, Drøn dark green with dark wood) and two factory
  presets (Sang: Solo, Kor; Drøn: Tung, Ørken). A project keeps the amp it had
- A preset is named every time it is saved: Save makes the Preset display take a name, which is
  typed on its tape (enter or a second press of Save keeps it, escape drops it). The name starts
  as the name of the preset you loaded, so enter writes over it and a new name makes a new
  preset. The name is the file's name, with a number added if it is taken, and it is what the
  Preset display shows when the preset is loaded. Leave it empty and the preset is named after
  the one it started from. In a DAW the letters now reach the tape: before, the window never
  held the keyboard for it and nothing could be typed
- Two more microphones join Dynamic, Ribbon and Condenser at the end of the Mic Type stepper:
  Close (CLO), dark and close, with the low end pushed up, the top shut and a bite left in the
  upper mids, and Air (AIR), bright and thin, with the top opened wide and the lows let go. A
  project keeps the microphone it had
- The window is about 30% wider (2535 x 650, from 1950 x 650). The strip across the top has the
  room: its left controls stay where they were, the right ones stay at the right edge, and the
  space opens between them. The pedals and the amp keep their size, with wider gaps between
  them. Nothing in the sound changed
- The big cream GUITAR AMP banner across the middle of the amp head is gone; the dials and the
  Amp and Mic row stand where they did, on the bare paint
- A Fuzz pedal joins the board, in ember red with a geyser on it: thick, saturated fuzz that
  sustains and breaks up as a note dies. Fuzz (0 to 10) sets how hard it clips, Tone (0 to 10)
  goes from dark and woolly through scooped mids to thin and biting. It has no Level knob: it
  is about as loud as switched off at any setting. It stands first on the board, in front of
  the amp, and can be dragged anywhere, also behind the cabinet. Two new presets play it: Mose
  (Brøl, doom) and Sværm (Glød, a fuzz lead with delay). Projects and presets from before open
  with it off
- A new layout: every piece stands on one line in the order the sound goes through it, the
  Gate, the pedals in front of the amp, the amp's head on its cabinet, the pedals behind it,
  with patch cables and couplers from one to the next. Drag a pedal across the amp and the
  cables follow it. The presets, tuner, meters, Output, On and Tuner are on a slim strip across
  the top; the amp's dials and the Amp display are on the head. The Cab pedal is gone: Speaker,
  Mic Type, Mic, Res and the cabinet's switch are on a plate on the cabinet. The window is
  1950 x 650. Nothing in the sound changed
- Each of the four amps has a head of its own: Klar in ivory with a chrome panel, Glød in
  tweed with a wooden panel and cream knobs, Brøl in black with a brass panel and gold knobs,
  Torden in black with steel mesh, red piping and a black panel. Pick the amp on the nameplate
  across the grille, with its buttons or a click on the name; the head changes at once and the
  dials come back as each amp had them. The jewel lamp on the panel is lit while the amp is
  On. Nothing in the sound changed
- The cabinet under the head looks as the cabinet that plays: each amp's own (Klar's ivory
  1x12, Glød's tweed 2x12, Brøl's black 4x12 with white piping and chrome corners, Torden's
  4x12 with steel mesh and steel corners), each of the ten built-in cabinets with its own
  covering, grille and speakers, and a plain grey studio cabinet for a file of your own. A file
  that is gone or cannot be used shows the amp's own cabinet, which is what plays. The cabinet
  keeps its size and its controls stay where they are. Nothing in the sound changed

- Loading a preset or switching the amp is one step of the host's undo: in REAPER one Ctrl+Z
  puts the whole sound back, every dial, switch and the amp, instead of one knob at a time

- Ten built-in cabinets to play any amp through, picked on the cabinet's Speaker display
  between the amp's own (`OWN`) and your own files, each with a sound of its own:
  - `1X10 SMALL`: one 10" speaker in a small box, hardly any low end and a boxy honk
  - `1X12 OPEN`: one 12" in an open back, soft lows and a glassy top
  - `2X12 OPEN`: two 12"s in an open back, warm and full, with a sweet chime
  - `4X10 CRISP`: four 10"s, tight lows, deeply scooped mids and a bright, hard top
  - `4X12 MID`: four 12"s in a closed box, old style, thumping lows and a thick honk
  - `4X12 TIGHT`: four 12"s in a closed box, modern, a tight thump and a sharp edge
  - `1X15 DEEP`: one 15", deep and round with a soft top
  - `1X8 TINY`: one 8" in a small open box, no low end and a nasal, radio-like honk
  - `2X12 PUNCH`: two 12"s in a closed box, a firm low end, a thick low-mid box and a bright sizzle
  - `4X12 HEAVY`: four 12"s in a big closed box, a huge deep low end, a thick honk and a dark top

  A cabinet you pick stays when you change amps, and the Mic and Resonance dials work on it
- Your own preset bank: the Save button beside the Preset display keeps the sound as it
  stands, with the pedal order, the microphone and the cabinet. After a factory preset it
  makes a new preset named after it (`GRUS 2`); after one of your own it saves over it. They
  are files in `Documents/Hojt Audio/Presets`, which a double-click on the Preset caption
  opens: rename, copy, delete or share them there
- Preset banks, and a list to pick from: a click on the Preset display drops a list open with
  the banks on the left and the presets of the bank you click on the right, and a click on a
  preset loads it. `FACTORY` holds the built-in presets and `MINE` the ones you have saved so
  far. A new Bank display left of the Preset display says which bank you are in, and its arrows
  go round them
- `+ NEW BANK` at the end of the banks makes one of your own: type a name on its tape, `JAZZ`
  for one, and press enter — or click it again and take `BANK 1`, and rename the folder later.
  A bank is a folder in `Documents/Hojt Audio/Presets`, so you can also make one there, and
  move a preset from one bank to another by moving its file
- The Preset arrows now step through the bank you are in and come round in it, instead of
  running on from the factory presets into your own. Save keeps the sound in the bank you are
  in — in `FACTORY` it goes to `MINE`, as before — and the Preset caption opens that bank's
  folder, where you can move a preset from one bank to another

### Bass Synth

- Both voices sound like a bass through a pickup, not a synth. The top of a note fades soon
  after the attack while its low end keeps ringing, on low notes as on high ones, so the low E
  no longer rings with a clang. A fingered note on Groove has the growl of a pickup near the
  bridge, and a picked note on Metal has a firm attack
- No two notes in a row are the same, as when a person plays: each is plucked a little nearer
  to or further from the bridge, a little softer or sharper at the edge, and a hard pluck starts
  a few cents sharp and settles to pitch within a fraction of a second. A note played again
  while the last one still rings lands on the moving string instead of cutting it off. Fast
  repeated notes sound played, not looped, at the same level as before
- Metal's distortion is hot on the attack and cleans up as the note rings, so the top of a held
  note dies with the string instead of ringing on through the drive as a metallic clang, and
  the drive no longer holds a note up longer than the string rings
- The sine under the string (Sub) starts in step with the string's own low end and dies with
  it, so a held note stays a string to the end instead of turning into an organ tone
- On Metal the clean lows and the distorted mids now meet without a hollow between them
- The sine under the string is less than half as loud as it was at the same Sub, and sits under the
  string instead of being half of the note, so the note is a string you hear clearly rather than
  a sine with a pluck on it. Saved projects get this too, at the Sub they were saved with
- The string is plucked and heard at fixed places from the bridge, as on a real bass: each note
  is played on the highest open string at or below it, so each fret has a colour of its own,
  rounder up the neck, instead of every note having the same hole in its tone
- A bass cabinet behind both voices gives the note body in the low mids, definition from a
  presence rise, and stops where a bass speaker stops. The voices are about a decibel quieter
  than before, with more attack
- The voices' own settings changed to suit the new sound: Metal is Drive 40 %, Tone 2600 Hz,
  Sub 15 % and Sustain 3 s, Groove is Brightness 50 %, Sub 15 % and Tone 1400 Hz. The knobs and
  their ranges are the same, and a saved project keeps its knobs but plays the new sound
- The bass follows the pitch-bend wheel, and pitch bend drawn in the DAW: the ringing note bends
  smoothly up or down and comes back to pitch with the wheel, and a note played with the wheel
  off centre starts bent. The sine under the string bends with it, and a slide and a bend add
  up. A new knob in the String frame, Bend (1 to 12 st, 2 by default), sets how far the wheel
  goes at its end, up and down; the voice footswitches leave it where it is. With the wheel at
  centre the sound is exactly as before

### Glass Bells

- The delay can follow the song's tempo. A third footswitch, Sync, with its LED, stands beside
  the Delay switch: on, the Time knob picks a note, 1/4, 1/8 dotted, 1/8, 1/8 triplet or 1/16
  (`1/8 .`, `1/8 T` on its tape), and the echoes land on the beat in the DAW and follow when
  the tempo changes (between 50 and 1500 ms, so a 1/4 under 40 bpm stays at 1500 ms). Off,
  Time is milliseconds as before and keeps its setting. New parameters Delay Sync (on / off,
  off) and Delay Note (default 1/8 dotted). A project from before opens with Sync off and
  sounds as it did. The standalone follows its own tempo (120 bpm, or what `--tempo` says)

### Pitch Shifter

- A Blend knob, 0 to 100 %, mixes the guitar as you play it with the shifted one. At 100 %, the
  default, you hear only the shifted guitar, as before, so a saved project sounds the same.
  Turn it down for an octave under your note, or a fifth or a fourth beside it for a two-note
  harmony; in the middle both are at the same level. The dry guitar is not audibly delayed, and
  Bypass still gives the plain guitar at any Blend. At a shift of one or two semitones the
  shifted guitar can be heard a moment behind the dry one, as a short doubling
- The pedal's window is taller (400 x 600, as tall as the other pedals) to make room for the
  fourth knob: Max Latency, Blend and Smoothness in a row over Semitones

## 0.2.1

### Guitar Amp

- The Amp and Mic captions under the name band sit above their displays instead of under
  them, where they were half hidden, and the Tuner switch is to the right of the On switch,
  clear of the Mic display

### Bass Synth

- The model number in the lower left corner is no longer printed over by the art

## 0.2.0

### Every plugin

- A new look: the enclosures, knobs, screws, footswitches, lights and label tape are lit,
  with powder-coat flake, bevels and brushed metal, and each pedal has a screen-printed scene
  of its own on its face, around the controls. Every mini pedal of the Guitar Amp has one too

### Bass Synth, Shimmer Pad, Glass Bells

Three new instruments, each a pedal of its own:

- Bass Synth: a plucked string bass with two voices. Metal is picked, with clean lows under a
  distorted band of mids; Groove is played with a finger, warm, and slides between notes
  that overlap
- Shimmer Pad: a slow ambient pad of detuned saws and breath, with a chorus and a large hall
  whose tail comes back an octave up
- Glass Bells: soft glassy bells with a ping-pong delay and a hall

### Drum Synth

- Six extra stereo outputs for a host that has multi-out: Kick, Snare, Hi-Hat, Toms, Cymbals and
  Room, each on a track of its own to mix, EQ and compress apart. The main output is the whole
  kit as before. On Metal and Groove the stems are taken before the kit's bus drive, so they are
  cleaner than the main mix
- The Metal kit is a new instrument: a clicking kick, a cracking snare, cymbals of square
  waves and noise, its own room and a saturated bus. It replaces the Metal kit there was;
  projects set to Metal play the new one
- A fourth kit, Groove: warm and soft, with a round kick, ghost notes that are darker and
  shorter than the backbeat, dark cymbals and a shaker (notes 69, 70 and 82)
- The hi-hat follows an electronic kit's pedal (MIDI CC4): a hit on 46 with the pedal half
  down is a half-open hat, and shutting the pedal chokes a hat that rings. On every kit; a
  part without the pedal sounds as before
- A Choke switch: with it on, a note-off on a cymbal that was held for a moment damps it at
  once, as a hand does. Off by default. The kit switches have moved to make room

- Cymbals and hi-hats are clear: the top is noise in six bands that die away one after the
  other, and the resonances only carry the body, so the bell-like ring and the chorus of
  beating tones are gone, and the low hum under the tail too. Rock and Jazz kits
- The snare has its crack: louder and brighter wires, a fuller stick attack, and as loud in the
  kit as the kick and the toms

### Guitar Amp

- A fourth amp, Glød: warm and chiming at the edge of breakup, between Klar's clean headroom
  and Brøl's crunch. It has its own cabinet, remembers its own dials and has three presets
  (Pejs, Gnist, Bål)
- The noise gate closes behind a note on a noisy guitar: hiss only has to sit 3 dB under the
  threshold, not 6. A note's tail is cut 3 dB sooner, and the four presets that had the
  gate at -54 dB are at -57 dB, so they sound as before
- The tuner no longer shows mains hum as a note. A note is shown from a pluck on, and let go
  when what rang does not fade like a string; a string that already rings when the tuner is
  switched on is read from its next pluck
- Level meters on the head: a column of lamps for the level the guitar comes in at, left of the
  dials, and one for the output level in front of the safety clip, by the clip lamp. They jump
  to a peak and fall over about a third of a second; the lowest lamp is lit from -45 dB and the
  top one at full scale. The window is only redrawn while the lamps move
- The clippers of the high-gain amp and the overdrive use less CPU. The sound is the same to the
  ear; only the last bits of the output move
- Delay and reverb at their maximum no longer push the output into its safety clip: what each
  gives out is rounded off under full scale, and left alone below 70 % of it. The delay's
  last 28 dB of tail (everything under -72 dBFS) is cut, so a 1000 ms delay at full feedback
  falls idle after about 37 seconds instead of 62, and a 350 ms one after 14 instead of 22
- The cabinet can be heard through three microphones, picked with a Mic stepper beside the amp
  selector: Dynamic (the cabinet as it was, and the default), Ribbon (warm, dark, a relaxed
  top) and Condenser (air on top, the lower mids scooped). The Mic dial still moves the
  microphone on the cone
- Own cabinets: a file that cannot be used shows as `!NAME` instead of BAD FILE, stepping where
  the cabinets folder has no files says NO FILES for a moment, and hovering the Speaker
  caption shows where the folder is (a double-click on it opens the folder)
- The tuner keeps reading low strings through loud mains hum: once it has heard steady hum
  (50 or 60 Hz and its harmonics, -70 dBFS or louder) before a pluck, it takes the hum out in
  front of the detector, so a note no longer drops out at -45 dBFS hum. A string a note away
  from the hum (G1 against 50 Hz, B1 against 60 Hz) is read up to about 5 cents off while
  the hum is there. Without hum nothing changes
- A project with a large cabinet file on a slow disk no longer holds up the amp opening: the
  file is read in the background, and the amp plays through its own cabinet until it has arrived
- With a cabinet of your own, Mic and Resonance no longer follow the amp you picked: they work
  on the same frequencies for every amp (Mic a shelf up high, Resonance a bell at 110 Hz), and
  at their centre they leave your file as it is
- Switching amp, or moving a pedal, dips the sound for about 9 ms instead of 10: the fade is
  4.5 ms down and 4.5 ms up. It still does not click
- Your own cabinets can be 8-bit or 64-bit float WAV files, which showed BAD FILE before
- The amp is picked with a stepper (a name on label tape with a button at each end) under the
  dials, instead of three switches. Each amp still keeps its own dials
- Torden's palm mutes are tighter: about 6 dB less below 100 Hz against the mids than Brøl, up
  from about 4, and a low E keeps a little more of its fundamental than before
- Torden's Gain dial does more above 5: it is no longer squeezed flat by the middle of the dial,
  so 5 stays tighter and more dynamic and 5 to 10 grows into thicker, singing sustain. The
  dial is a little hotter at the top

### Guitar Amp, Shimmer Pad, Glass Bells

- VST3 hosts that stop processing a silent plugin no longer cut the delay and reverb tails:
  the plugins tell the host from the start that their tails ring on

## 0.1.0

First public release. VST3 and CLAP for Windows, macOS and Linux. The macOS and Linux builds
come from the build server and have not been tried in a DAW yet.

### Pitch Shifter

- Shifts a guitar by -12 to +12 semitones for live playing
- Delay of about one pitch period of the note being played: 12 ms on the low E string, under
  3 ms high up the neck
- Bit-exact passthrough at 0 semitones; the footswitch bypasses without a click

### Drum Synth

- A full drum kit with no samples: every sound is made in the plugin
- Three kits: Rock, Jazz, Metal
- General MIDI drum map; velocity changes the sound, not only the level
- Room reverb, Tune, Damping and a level per group

### Guitar Amp

- Three amps, each with its own cabinet: Klar (clean), Brøl (crunch), Torden (high gain)
- Ten pedals: gate, compressor, overdrive, phaser, chorus, flanger, tremolo, cabinet, delay,
  reverb. Drag a pedal by its title to move it in the chain
- Your own cabinet impulse responses (WAV) from `Documents/Hojt Audio/Cabinets`
- Factory presets and a built-in tuner
- 0.1 to 0.3 ms of latency
