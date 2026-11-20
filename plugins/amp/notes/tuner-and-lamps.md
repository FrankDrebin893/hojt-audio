# The tuner, the lamps and the level meters

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

- The tuner reads 48 Hz to 1420 Hz against A = 440 Hz (`REFERENCE_A_HZ`), a new reading every
  30 ms, none for silence, noise, hum or chords. A note is shown from a pluck on (a rise of
  12 dB within 50 ms; 6 dB to change notes or pluck again), and let go after a second if it
  does not fade like a string. Steady hum (50 or 60 Hz and its 2nd and 3rd harmonics, -70 dBFS
  or more) heard at rest, with nothing struck for 1.5 s, switches on notches for it in front
  of the detector, which pull a string within 2 Hz of them by a few cents; without hum they
  are off and the readings are unchanged. A loaded
  project never opens with the tuner on

- The Gate pedal's `OPEN` lamp is lit while the gate is on and lets the guitar through; `CLIP`
  beside Output is lit for 150 ms after the output went over the safety clip's knee
- Two level meters on the strip (`level_meter`): `INPUT` in the left wing shows the input before
  the input gain, `OUTPUT` by the clip lamp the output in front of the safety clip. They take a
  peak at once and fall 60 dB a second; the window is redrawn for them only while they move,
  30 times a second at most
