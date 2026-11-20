# The chain: pedal order, slots, and effects on either side of the amp

Moved unchanged from `plugins/amp/CLAUDE.md` (issue 370); the router there says when to read it. Read it before editing the files it names.

- The pedal order (`order.rs`, held by the chain model `slots.rs`): Gate always first, amp and cabinet one block in the middle.
  Every pedal stands on either side of the amp (#268: Delay and Reverb in front, Drive, Pitch and
  Wah behind the cabinet, below); the gate is the one fixed place. Default: Pitch, Wah, Fuzz, Comp, Drive, Phaser, amp,
  Chorus, Flanger, Tremolo, EQ, Delay, Reverb, which with the new pedals off is the chain as it
  was, to the bit. A project saved before a pedal existed gets it where the default has it (the
  wah behind the pitch shifter, wherever the project put that). `Pedal::ALL` only ever grows at the
  end: the order is saved as a number of four bits a pedal too
- **Delay and reverb in front of the amp** (#268, the `front` instance inside `DelayEffect` and
  `ReverbEffect`, played from `AmpChain::homes`): one
  more `Delay` and `Reverb` made for the oversampled rate, so a pedal that changes sides
  allocates nothing (about 3 MB of lines for the delay at 192 kHz). Only the instance of the side
  the pedal stands on runs; the other is reset when the pedal crosses (`set_up_pedal`), so
  a tail does not come back. In front the effect is mono: the signal goes in on both channels of
  the stereo effect and the mean of the two comes out (`mono_through_stereo`), which leaves the
  dry signal as it was and gives the amp the repeats and the tail as a centred listener hears
  them, and the amp plays them. Behind the cabinet nothing changed, to the bit
  (`fingerprint_report` before and after). No latency in either place. `zone_report` prints the
  level, latency and cost of each effect on each side, `render_zones` writes them for listening
- **Slot parameters** (#268, `layout.rs`, `extras.rs`, `slots.rs`): a block on a slot that is
  not its pedal's own (`Chain::is_home`), a second Delay, a Gate block, plays from
  `slot<n>_on` and `slot<n>_k<m>`. `GuitarAmpParams::slot_settings` turns the slot's 0..1 values
  into the kind's settings with the own parameter's range (`preview_plain` of the pedal's own
  knob `m`: the same order as `pedal_params`), so a value that moves between the pedal's own
  parameters and a slot's (`Memory`) means the same in both; a test holds the order, another that
  a slot plays as the pedal does, to the bit, on each side. The pedals keep their instances in
  `AmpChain` and read their own parameters: their sound did not change (`fingerprint_report`
  before and after, identical). The others are `extras::Pool`: `MAX_PER_KIND` (2) units of each
  kind, each with an instance for either side where the sides need one (as the pedals do; about
  11 to 12 MB in all, mostly the delays and reverbs: `pool_memory`), made in `set_sample_rate`. A unit is
  assigned to a block by kind and `rank` (the block's place among the blocks of its kind that
  are on slots of their own, by slot number), moves when the layout changes (the output fades
  as for any change of order, `follow_order`) and starts from rest on the side it comes in
  on. Told at the same reads as the dials (`tell_pool`), so a switch falls on the same sample
  whatever the block size. A drive on a slot that is on in front of the amp pushes it as the
  pedal does (`AmpChain::driven`). A pedal absent from the chain is put to rest and not run.
  `slot_report` prints, for each kind and side, whether it is the pedal's sound, its level,
  latency and cost
- **Drive, wah and pitch shifter behind the cabinet** (#268, the `back` instances
  inside `DriveEffect`, `WahEffect` and `PitchEffect`, played from `AmpChain::homes`): made for the host's rate next to the ones in front, so a pedal that
  changes sides allocates nothing (the pitch shifter's buffers, about 1 MB, once a channel).
  Only the instance of the side the pedal stands on runs; the other is reset when it crosses,
  from rest. The wah and the pitch shifter are one instance a channel with the same settings,
  so a mono signal comes out the same on both sides; after a stereo effect (a chorus in front
  of them) each channel follows its own signal, which is what a stereo pedal pair would do. The
  drive raises the rate itself, like the fuzz (`drive::BackDrive`: two `Drive` and two
  oversamplers, 4x, 2x from 88.2 kHz) and crossfades to the untouched signal at the host's rate,
  so off it does not even pass the oversampler and leaves the signal alone, to the bit. A drive
  pedal behind the cabinet does not push the amp (`set_driven` is only for the pedal in front:
  `AmpChain::follow_drive_side`), and the second amp follows the same switch. In front of the amp
  nothing changed, to the bit (`fingerprint_report` before and after, with a pitch and a wah
  row added). The wah's and the pitch shifter's latency is what it is in front; the drive's is
  the oversampler's fraction of a millisecond, like the fuzz's. `zone_report` and `render_zones`
  cover these too

- The order is saved with the project (`chain-order`, a list of pedal ids with `amp` between
  the sides), not a parameter and not automatable. The chain reads it with the dials: the
  output fades to silence in 4.5 ms, the pedals change places, it fades back. A pedal that
  changes sides starts from rest. A list that is damaged or from another build is put right,
  never refused
