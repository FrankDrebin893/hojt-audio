# Levels: loudness matching, the clean amp's headroom, tails and aliasing

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

- The delay's tail is cut at -72 dBFS, and repeats under -60 dBFS fade faster, so a 1000 ms
  delay at full feedback goes idle in about 37 s. Neither effect at its maximum pushes the
  output into the safety clip
- While the drive pedal is on, the second stage of Brøl and of Torden clips at twice the rate
  (`StageModel.finer_when_driven`) and Klar's power stage turns itself down under a heavy load
  (`PowerModel.driven_sag`), so no setting of the pedal aliases above -70 dB. With the pedal
  off the output is the same to the bit
- The amps are matched by loudness (BS.1770, `test_util::loudness_lufs`), not RMS, for a guitar
  recorded at -30 dBFS RMS (`REFERENCE_DI_DB`), its peaks around -12 dBFS: every factory
  preset is heard at -13 LUFS within 2 LU (Kobber -15.4), leads highest, as loud as the drums
  and the bass (#131). A clean
  amp follows the guitar dB for dB and one that clips hardly does, so they can only meet at
  one level: 6 dB quieter the clean settings are 2 to 4 LU under the others, 6 dB hotter as
  much over. The Input knob is what moves a guitar to that level
- Klar's power stage holds its peaks (`PowerModel.clean_to`): straight up to a knee, so a note
  held at -30 dBFS RMS has no distortion (under -60 dB), and rounding off the attacks that go
  over. Chords from a guitar 6 dB over the reference stay under the safety clip's knee on
  every preset; 12 dB over (-18 dBFS RMS) only on most of the presets that clip
  (`presets::HOLD_HOT_GUITAR`), and the others touch it: the Clip lamp says to turn Input
  down. Brøl below Gain 5 is made up in front of
  its power stage for the same reason
