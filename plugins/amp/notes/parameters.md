# Parameters

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

- `lib.rs` takes every parameter default and range from `AmpSettings::default` and the limits in
  the DSP modules; a test pins the values

| Parameter (id) | Range | Default | Description |
|----------------|-------|---------|-------------|
| Bypass (`bypass`) | on / off | off | The On switch on the strip |
| Input (`in_gain`) | -24 to +24 dB | 0 dB | Gain in front of everything except the gate's detector |
| Gate (`gate_on`) | on / off | on | Noise gate on the input |
| Gate Threshold (`gate_thresh`) | -80 to -20 dB | -60 dB | Input level that opens the gate (closes 3 dB lower) |
| Gate Release (`gate_release`) | 20-500 ms | 100 ms | Time to close after a 40 ms hold |
| Comp (`comp_on`) | on / off | off | Compressor pedal |
| Comp Sustain (`comp_sustain`) | 0.0-10.0 | 5.0 | How much it evens out: threshold and ratio together |
| Comp Level (`comp_level`) | 0.0-10.0 | 5.0 | The pedal's output, -12 to +12 dB around as loud as off |
| Drive (`drive_on`) | on / off | off | Overdrive pedal, in front of the amp or behind the cabinet |
| Drive Gain (`drive_gain`) | 0.0-10.0 | 3.0 | Amount of clipping in the pedal |
| Drive Tone (`drive_tone`) | 0.0-10.0 | 5.0 | The pedal's top end, 1.5 to 7 kHz |
| Drive Level (`drive_level`) | 0.0-10.0 | 5.0 | The pedal's output, -20 to +20 dB into the amp |
| Fuzz (`fuzz_on`) | on / off | off | Fuzz pedal, either side of the amp |
| Fuzz Amount (`fuzz_fuzz`) | 0.0-10.0 | 5.0 | Gain into the clipper, 26 to 56 dB |
| Pitch (`pitch_on`) | on / off | off | Pitch shifter pedal, in front of the amp or behind the cabinet |
| Pitch Semitones (`pitch_semitones`) | -12 to +12 | 0 | Whole semitones: the guitar down an octave to up one. Shown `-5`, `0`, `+7`; double-click returns to 0 |
| Pitch Blend (`pitch_blend`) | 0-100 % | 100 % | The guitar as it comes in (0) to the shifted guitar alone (100). The dry part is not delayed |
| Pitch Tracking (`pitch_latency`) | 2-50 ms | 15 ms | The most the shifted guitar may lag behind the input. The lag is fixed, whatever the note: the largest of 5.3, 10.7, 21 and 43 ms that fits (5.3 ms under that). Longer is cleaner |
| Fuzz Tone (`fuzz_tone`) | 0.0-10.0 | 5.0 | Dark (a low-pass at 650 Hz) to bright (a high-pass at 1.1 kHz), the mids scooped in between |
| Amp (`amp`) | Klar / Brøl / Torden / Glød / Sang / Drøn / Knald / Gnist / Skarp / Hvas / Træ / Smæld | Brøl | Amp and its cabinet. Variant ids `klar`, `brol`, `torden`, `glod`, `sang`, `dron`, `knald`, `gnist`, `skarp`, `hvas`, `trae`, `smaeld`. New amps go at the end: a project remembers its amp by its number |
| Gain, Bass, Mid, Treble, Presence, Master (`gain`, `bass`, `mid`, `treble`, `presence`, `master`) | 0.0-10.0 | 5.0 | The amp's dials |
| Amp 2 (`amp2_on`) | on / off | off | The second amp, beside the first behind the same front pedals. Off, it is not run at all |
| Amp 2 Model (`amp2`) | the same twelve, same variant ids as `amp` | Klar | The second amp's own choice |
| Amp 2 Gain, Bass, Mid, Treble, Presence, Master (`amp2_gain`, `amp2_bass`, `amp2_mid`, `amp2_treble`, `amp2_presence`, `amp2_master`) | 0.0-10.0 | 5.0 | The second amp's dials |
| Amp Blend (`amp_blend`) | 0.0-10.0 | 5.0 | 0 is the first amp alone, 10 the second alone. Not heard while Amp 2 is off |
| EQ (`eq_on`) | on / off | off | EQ pedal, either side of the amp |
| EQ Low, Mid, High (`eq_low`, `eq_mid`, `eq_high`) | 0.0-10.0 | 5.0 | -12 to +12 dB, 5 is flat: Low and High shelves at 100 Hz and 4 kHz, Mid a bell |
| EQ Freq (`eq_freq`) | 0.0-10.0 | 5.0 | The centre of Mid, 200 Hz to 5 kHz on a log scale (5 is 1 kHz) |
| Wah (`wah_on`) | on / off | off | Wah pedal, in front of the amp or behind the cabinet |
| Wah Pedal (`wah_pedal`) | 0.0-10.0 | 5.0 | The rocker: 0 is the heel (350 Hz), 10 the toe (2.2 kHz), on a log scale. Map a controller or an expression pedal to it in the host. In Auto mode, where the rocker rests |
| Wah Bite (`wah_q`) | 0.0-10.0 | 5.0 | The resonance, Q 3 to 9: a wider, woofier peak to a narrow, vocal one |
| Wah Mode (`wah_mode`) | Pedal / Auto | Pedal | Pedal: the rocker is where the dial puts it. Auto: the playing opens it up from the resting position |
| Wah Sens (`wah_sens`) | 0.0-10.0 | 5.0 | Auto only: how far a loud note opens the pedal, as a share of the sweep |
| Chorus, Phaser, Flanger, Tremolo (`chorus_on`, `phaser_on`, `flanger_on`, `trem_on`) | on / off | off | Modulation pedals |
| Chorus Rate (`chorus_rate`) | 0.1-6 Hz | 0.8 Hz | Speed of the sweep, 7 to 12 ms |
| Phaser Rate (`phaser_rate`) | 0.1-8 Hz | 0.5 Hz | Speed of the sweep, 200 Hz to 1.6 kHz |
| Flanger Rate (`flanger_rate`) | 0.05-5 Hz | 0.3 Hz | Speed of the sweep, 0.5 to 6 ms |
| Tremolo Rate (`trem_rate`) | 0.5-12 Hz | 5 Hz | Speed of the volume wobble |
| Depth (`chorus_depth`, `phaser_depth`, `flanger_depth`, `trem_depth`) | 0-100 % | 50 % | Width of the sweep; for chorus and flanger also the level of the voice |
| Cabinet (`cab_on`) | on / off | on | Off passes the amp at about the same level, for a cabinet elsewhere |
| Cab Mic (`cab_mic`) | 0.0-10.0 | 5.0 | Darker to brighter microphone position |
| Cab Mic Type (`cab_mic_type`) | Dynamic, Ribbon, Condenser, Close, Air | Dynamic | Which microphone hears the cabinet (`MicType` in `cab.rs`): Dynamic leaves it as designed; Close is dark with the lows up, Air bright with the lows let go. New ones go at the end, a project stores the position |
| Cab Resonance (`cab_res`) | 0.0-10.0 | 5.0 | Less or more low thump |
| Delay (`delay_on`) | on / off | off | Stereo delay behind the cabinet, or mono in front of the amp |
| Delay Time (`delay_time`) | 20-1000 ms | 350 ms | Time between repeats |
| Delay Feedback (`delay_feedback`) | 0-90 % | 35 % | Level of each repeat against the one before |
| Delay Mix (`delay_mix`) | 0-100 % | 25 % | Level of the repeats |
| Delay Sync (`delay_sync`) | on / off | off | The repeats follow the song's tempo: the Time knob picks Delay Note, and the time is `60000 / bpm` times the note's beats, kept to 20-1000 ms. With no tempo from the host, the Time knob's ms. Factory presets set it off |
| Delay Note (`delay_note`) | 1/4, 1/8 dotted, 1/8, 1/8 triplet, 1/16 | 1/8 dotted | The note value while Sync is on (`suite_common::dsp::NoteValue`). Variant ids `quarter`, `dotted_eighth`, `eighth`, `eighth_triplet`, `sixteenth`; a new one goes at the end |
| Reverb (`reverb_on`) | on / off | off | Stereo reverb behind the delay, or mono in front of the amp |
| Reverb Decay (`reverb_decay`) | 0.3-6.0 s | 1.5 s | Time for the tail to fall by 60 dB |
| Reverb Mix (`reverb_mix`) | 0-100 % | 20 % | Level of the tail |
| Output (`out_level`) | -30 to +6 dB | 0 dB | Output level, in front of the safety clip |
| Tuner (`tuner`) | on / off | off | Silent tuning. Not automatable, not in presets, not changed when a project is loaded |
| Looper Level (`looper_level`) | -60 to +6 dB | 0 dB | The loop against the live sound; -60 is silence. Not in presets. The looper's buttons are commands, not parameters |
| Metronome (`metronome_on`) | on / off | off | The click, on top of the finished sound. Not in presets |
| Metronome Tempo (`metronome_bpm`) | 40-300 bpm | 120 | Beats a minute: quarter notes, in 6/8 eighths |
| Metronome Sync (`metronome_sync`) | on / off | off | Follow the host's tempo and position; silent while the host is stopped. With no tempo from the host, the Tempo knob's |
| Metronome Time Signature (`metronome_time_sig`) | 2/4, 3/4, 4/4, 6/8 | 4/4 | Clicks to the bar. Variant ids `two_four`, `three_four`, `four_four`, `six_eight` |
| Metronome Volume (`metronome_volume`) | -60 to 0 dB | -12 dB | The click's level; -60 is silence |
| Metronome Sound (`metronome_sound`) | Woodblock, Beep, Click | Woodblock | Variant ids `woodblock`, `beep`, `click` |
| Metronome Output (`metronome_out`) | Mixed, Separate | Mixed | Where the click is heard: Mixed on the main output; Separate alone on the auxiliary stereo output `Metronome`, the main output the amp alone (mixed in where the host gives no usable such output, always in the standalone program). Variant ids `mixed`, `separate`. Not in presets |

Dials are stored 0.0-1.0 and shown as 0.0-10.0.
