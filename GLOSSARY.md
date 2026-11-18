# HØJT

A free, beginner-friendly set of guitar and band tools (plugins, a practice app and a few web tools) so that people can start playing without paying for a pile of plugins first. This file is the product's vocabulary only. How work is organised (issues, specs, routines) is in `CLAUDE.md`; why things are the way they are is in `docs/adr/`.

## Brand and products

**HØJT**:
The brand, Danish for "loud" and "high". Written with the Ø everywhere a person reads it.
_Avoid_: Hojt (except as the vendor string)

**HØJT DSP**:
All the plugins together, as a suite.
_Avoid_: the VST suite, the audio plugins

**Hojt Audio**:
The ASCII vendor string a DAW lists the plugins under. It never changes.

**Plugin**:
One VST3 / CLAP product of HØJT DSP, with one window: Pitch Shifter, Drum Synth, Bass Synth, Shimmer Pad, Glass Bells, Guitar Amp. Also runs as a standalone program.
_Avoid_: pedal (see below), effect (when meaning the whole product)

**Piano viz**:
The Windows app for practising piano with a MIDI keyboard. Not part of HØJT DSP.
_Avoid_: piano app, PMVA

**Resources**:
Free tools for learning to play on the website, one folder each (the Guitar and Piano Chord Atlas so far).

## Guitar Amp

**Guitar Amp**:
The plugin with a full signal chain from the guitar's input to the stereo output.

**Rig**:
The Guitar Amp's whole window and everything in it: strip, pedals and stack.

**Signal chain**:
The path the sound takes through the rig, in the **chain order**, which the player sets.

**Chain order**:
The order of the pedals and the stack in the signal chain, as the player arranged them.

**Pedal**:
A stompbox effect the player can add to the signal chain, with its own switch and knobs: gate, pitch shifter, compressor, overdrive, fuzz, modulation, EQ, wah, delay, reverb. Shown as a small enclosure with its own paint. Pedals sit in front of the stack or behind its cabinets.
_Avoid_: effect (as a noun for the thing the player adds), mini pedal, block

**Stack**:
The amp section of the signal chain: one or two lanes and the mix plate where they meet. It is one place in the chain order.
_Avoid_: amp section

**Lane**:
One amp's path in the stack (preamp, tone stack, power amp, cabinet). Amp 1 is always in the signal; Amp 2 has a switch. At most two.
_Avoid_: channel, second amp slot

**Amp model**:
One of the amp sounds a lane can be set to (Klar, Brøl, Torden...), defined by a set of constants.
_Avoid_: amp type, preamp

**Head**:
The amp half of a lane as drawn: its panel, knobs and nameplate, which change look with the amp model.

**Cabinet**:
The speaker a lane plays through: a built-in one or, for Amp 1, the player's own WAV file.
_Avoid_: speaker, IR (except when talking about the file)

**Blend**:
The mix between Amp 1 and Amp 2.

**Strip**:
The bar across the top of the rig: what comes in and what is picked on the left (input meter, bank, preset), what goes out on the right (output, meters, tuner).

**Preset**:
A saved setting of the whole rig. Factory presets ship with the plugin; the player saves their own.

**Bank**:
A named group of a player's presets.

**Tuner**, **Metronome**, **Looper**:
The practice tools on the strip. They act on the whole rig, not on one pedal.

## Other plugins

**Kit**:
A drum sound set of the Drum Synth (Rock, Jazz, Metal, Groove). A project remembers its kit by number, so new kits go at the end.

**Bass voice**:
How the Bass Synth plays: Metal (picked) or Groove (finger).
