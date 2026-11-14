use std::f32::consts::FRAC_PI_4;

use crate::dsp::{bus_clip, semitones_to_ratio};
use crate::engines::groove::GrooveKit;
use crate::engines::metal::MetalKit;
use crate::kit::{Kit, KitPreset};
use crate::reverb::Room;
use crate::voices::cymbal::Cymbal;
use crate::voices::kick::Kick;
use crate::voices::snare::Snare;
use crate::voices::tom::{Tom, TOM_COUNT};
use crate::voices::{Hit, VoicePair};

// Cymbal slots
const HAT: usize = 0;
const CRASH1: usize = 1;
const CRASH2: usize = 2;
const RIDE: usize = 3;
const BELL: usize = 4;
const CHINA: usize = 5;
const SPLASH: usize = 6;
const CYMBAL_COUNT: usize = 7;

// Stereo positions from the drummer's seat, -1.0 (left) to 1.0 (right)
const CYMBAL_PANS: [f32; CYMBAL_COUNT] = [-0.45, -0.5, 0.5, 0.4, 0.4, 0.65, -0.15];
// Lowest (floor) tom to highest
const TOM_PANS: [f32; TOM_COUNT] = [0.5, 0.35, 0.15, -0.05, -0.2, -0.35];

// Balance between the pieces with all level controls at 0 dB
const KICK_GAIN: f32 = 0.5;
const SNARE_GAIN: f32 = 0.52;
const TOM_GAIN: f32 = 0.45;
const HAT_GAIN: f32 = 0.26;
const CYMBAL_GAIN: f32 = 0.26;

// How much of each piece goes to the room
const KICK_SEND: f32 = 0.15;
const SNARE_SEND: f32 = 0.5;
const TOM_SEND: f32 = 0.45;
const HAT_SEND: f32 = 0.25;
const CYMBAL_SEND: f32 = 0.35;

// Share of the decay time that full Damping takes away
const DAMPING_RANGE: f32 = 0.75;

// A note-off this soon after its note-on is not a hand on the cymbal: drawn notes are short
const CHOKE_MIN_HOLD_S: f32 = 0.12;
// What a hand leaves of a modal cymbal's decay times, in ms
const CHOKE_MS: f32 = 25.0;

// The hi-hat pedal closing past this (openness, 0.0-1.0) chokes a hat that rings
const HAT_CHOKE_BELOW: f32 = 0.1;

/// Linear levels per group, and the room amount (0.0-1.0)
pub struct Mix {
    pub kick: f32,
    pub snare: f32,
    pub toms: f32,
    pub hihat: f32,
    pub cymbals: f32,
    pub room: f32,
}

impl Default for Mix {
    fn default() -> Self {
        Self { kick: 1.0, snare: 1.0, toms: 1.0, hihat: 1.0, cymbals: 1.0, room: 0.25 }
    }
}

// The stems, in the order of the aux outputs
pub const KICK_STEM: usize = 0;
pub const SNARE_STEM: usize = 1;
pub const HAT_STEM: usize = 2;
pub const TOMS_STEM: usize = 3;
pub const CYMBALS_STEM: usize = 4;
pub const ROOM_STEM: usize = 5;
pub const STEM_COUNT: usize = 6;

/// One stereo pair for each group of the kit, before the bus: what the aux outputs carry.
/// For Rock and Jazz they add up to the stereo mix. Metal and Groove drive their bus, and
/// their stems leave that out
#[derive(Clone, Copy, Default)]
pub struct Stems {
    pub pairs: [(f32, f32); STEM_COUNT],
}

impl Stems {
    pub fn add(&mut self, stem: usize, left: f32, right: f32) {
        self.pairs[stem].0 += left;
        self.pairs[stem].1 += right;
    }

    pub fn add_scaled(&mut self, other: &Stems, scale: f32) {
        for (pair, other) in self.pairs.iter_mut().zip(other.pairs) {
            pair.0 += other.0 * scale;
            pair.1 += other.1 * scale;
        }
    }
}

#[derive(Default)]
struct Bus {
    left: f32,
    right: f32,
    send_left: f32,
    send_right: f32,
}

impl Bus {
    fn add(&mut self, sample: f32, pan: (f32, f32), send: f32) {
        let left = sample * pan.0;
        let right = sample * pan.1;
        self.left += left;
        self.right += right;
        self.send_left += left * send;
        self.send_right += right * send;
    }
}

/// The whole instrument: every voice, the General MIDI note map, the stereo mix and the room.
/// Rock and Jazz are played by the modal voices here, Metal and Groove by their own engines.
/// Nothing here allocates after `set_sample_rate`.
pub struct DrumKit {
    // None for the kits that have an engine of their own
    presets: [Option<KitPreset>; 4],
    kit: Kit,
    tune_semitones: f32,
    tune_ratio: f32,
    decay_scale: f32,
    pub mix: Mix,
    kick: VoicePair<Kick>,
    snare: VoicePair<Snare>,
    toms: [VoicePair<Tom>; TOM_COUNT],
    cymbals: [Cymbal; CYMBAL_COUNT],
    center_pan: (f32, f32),
    tom_pans: [(f32, f32); TOM_COUNT],
    cymbal_pans: [(f32, f32); CYMBAL_COUNT],
    room: Room,
    // Of the modal kit that was played last: its voices may still ring under another kit
    room_level: f32,
    metal: MetalKit,
    groove: GrooveKit,
    // How open the hi-hat pedal holds the hats, 0.0-1.0, and the modal kit that hit the
    // hats last: its closed hat is what a choke damps them to
    hat_openness: f32,
    hat_kit: Kit,
    // For the choke: samples played so far, the sample rate and when each note was struck
    sample_rate: f32,
    now: u64,
    struck: [u64; 128],
}

impl DrumKit {
    pub fn new() -> Self {
        let presets = Kit::ALL.map(KitPreset::new);
        let rock = presets[Kit::Rock as usize].as_ref().expect("Rock is a modal kit");
        let room_level = rock.room_level;
        let mut room = Room::new();
        room.set_decay(rock.room_decay_s);

        // Every voice gets its own noise seed
        Self {
            presets,
            kit: Kit::Rock,
            tune_semitones: 0.0,
            tune_ratio: 1.0,
            decay_scale: 1.0,
            mix: Mix::default(),
            kick: VoicePair::new(Kick::new(1), Kick::new(2)),
            snare: VoicePair::new(Snare::new(3), Snare::new(4)),
            toms: std::array::from_fn(|i| {
                VoicePair::new(Tom::new(10 + 2 * i as u32), Tom::new(11 + 2 * i as u32))
            }),
            cymbals: std::array::from_fn(|i| Cymbal::new(30 + i as u32)),
            center_pan: pan_gains(0.0),
            tom_pans: TOM_PANS.map(pan_gains),
            cymbal_pans: CYMBAL_PANS.map(pan_gains),
            room,
            room_level,
            metal: MetalKit::new(50),
            groove: GrooveKit::new(51),
            hat_openness: 1.0,
            hat_kit: Kit::Rock,
            sample_rate: 44100.0,
            now: 0,
            struck: [0; 128],
        }
    }

    /// Allocates the rooms' delay lines. Not for the audio thread.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.kick.set_sample_rate(sample_rate);
        self.snare.set_sample_rate(sample_rate);
        for tom in &mut self.toms {
            tom.set_sample_rate(sample_rate);
        }
        for cymbal in &mut self.cymbals {
            cymbal.set_sample_rate(sample_rate);
        }
        self.room.set_sample_rate(sample_rate);
        self.metal.set_sample_rate(sample_rate);
        self.groove.set_sample_rate(sample_rate);
    }

    pub fn reset(&mut self) {
        self.kick.reset();
        self.snare.reset();
        for tom in &mut self.toms {
            tom.reset();
        }
        for cymbal in &mut self.cymbals {
            cymbal.reset();
        }
        self.room.reset();
        self.metal.reset();
        self.groove.reset();
        self.hat_openness = 1.0;
        self.now = 0;
        self.struck = [0; 128];
    }

    /// Applies to the next hits. Voices that are ringing keep the sound they were hit with.
    pub fn set_kit(&mut self, kit: Kit) {
        if kit != self.kit {
            self.kit = kit;
            if let Some(preset) = &self.presets[kit as usize] {
                self.room.set_decay(preset.room_decay_s);
                self.room_level = preset.room_level;
            }
        }
    }

    /// Tuning of the drums (not the cymbals) for the next hits
    pub fn set_tune(&mut self, semitones: f32) {
        if semitones != self.tune_semitones {
            self.tune_semitones = semitones;
            self.tune_ratio = semitones_to_ratio(semitones);
        }
    }

    /// 0.0 lets everything ring, 1.0 is heavily damped. Applies to the next hits.
    pub fn set_damping(&mut self, damping: f32) {
        self.decay_scale = 1.0 - DAMPING_RANGE * damping.clamp(0.0, 1.0);
    }

    /// The hi-hat pedal (MIDI CC4): 0.0 shut, 1.0 open. Sets what the next hits on 46 sound
    /// like, from closed to open, on every kit. Shutting it on a hat that rings chokes it.
    /// Until it is first moved the hats are open, so a part without it sounds as it did
    pub fn set_hat_openness(&mut self, openness: f32) {
        let openness = openness.clamp(0.0, 1.0);
        if self.hat_openness >= HAT_CHOKE_BELOW && openness < HAT_CHOKE_BELOW {
            if let Some(preset) = &self.presets[self.hat_kit as usize] {
                self.cymbals[HAT].choke(&preset.cymbals.hat_closed, self.decay_scale);
            }
            self.metal.choke_hat(self.decay_scale);
            self.groove.choke_hat(self.decay_scale);
        }
        self.hat_openness = openness;
        self.metal.set_hat_openness(openness);
        self.groove.set_hat_openness(openness);
    }

    /// Where in the stream the next frame is, in samples: for the choke to tell a held note
    /// from a short one. Not counted in `process`, where one more store per frame costs 8 %
    /// of a ringing kit
    pub fn set_time(&mut self, samples: u64) {
        self.now = samples;
    }

    /// A hand on a cymbal: the note-off of a cymbal note that was held for a moment damps it
    /// at once. Hi-hats are left to the pedal. Applies to what rings now
    pub fn note_off(&mut self, note: u8) {
        let slot = match note {
            49 => CRASH1,
            57 => CRASH2,
            51 | 59 => RIDE,
            53 => BELL,
            52 => CHINA,
            55 => SPLASH,
            _ => return,
        };
        let held = self.now.wrapping_sub(self.struck[note as usize]);
        if (held as f32) < CHOKE_MIN_HOLD_S * self.sample_rate {
            return;
        }
        if let Some(preset) = &self.presets[self.kit as usize] {
            let cymbal = &preset.cymbals;
            let sound = match slot {
                CRASH1 => &cymbal.crash1,
                CRASH2 => &cymbal.crash2,
                RIDE => &cymbal.ride,
                BELL => &cymbal.bell,
                CHINA => &cymbal.china,
                _ => &cymbal.splash,
            };
            self.cymbals[slot].choke(&sound.damped(CHOKE_MS), self.decay_scale);
        }
        self.metal.choke_cymbal(note, self.decay_scale);
        self.groove.choke_cymbal(note, self.decay_scale);
    }

    /// General MIDI drum map. Velocity is 0.0-1.0.
    pub fn note_on(&mut self, note: u8, velocity: f32) {
        if velocity <= 0.0 {
            return;
        }
        self.struck[(note & 127) as usize] = self.now;

        let hit =
            Hit { velocity: velocity.min(1.0), tune: self.tune_ratio, decay: self.decay_scale };
        let preset = match (self.kit, &self.presets[self.kit as usize]) {
            (_, Some(preset)) => preset,
            (Kit::Groove, None) => return self.groove.note_on(note, &hit),
            (_, None) => return self.metal.note_on(note, &hit),
        };
        let cymbals = &preset.cymbals;

        match note {
            35 | 36 => self.kick.next_voice().trigger(&preset.kick, &hit),
            37 => self.snare.next_voice().trigger_side_stick(&hit),
            38 | 40 => self.snare.next_voice().trigger(&preset.snare, &hit),
            41 | 43 | 45 | 47 | 48 | 50 => {
                let index = tom_index(note);
                self.toms[index].next_voice().trigger(&preset.toms, index, &hit);
            }
            // The closed and pedal sounds have a short decay, which chokes an open hi-hat
            42 | 44 | 46 => {
                let sound = match note {
                    42 => cymbals.hat_closed,
                    44 => cymbals.hat_pedal,
                    _ => cymbals.hat_closed.between(&cymbals.hat_open, self.hat_openness),
                };
                self.cymbals[HAT].trigger(&sound, &hit);
                self.hat_kit = self.kit;
            }
            49 => self.cymbals[CRASH1].trigger(&cymbals.crash1, &hit),
            57 => self.cymbals[CRASH2].trigger(&cymbals.crash2, &hit),
            51 | 59 => self.cymbals[RIDE].trigger(&cymbals.ride, &hit),
            53 => self.cymbals[BELL].trigger(&cymbals.bell, &hit),
            52 => self.cymbals[CHINA].trigger(&cymbals.china, &hit),
            55 => self.cymbals[SPLASH].trigger(&cymbals.splash, &hit),
            _ => {}
        }
    }

    /// One stereo frame
    #[cfg(test)]
    pub fn process(&mut self) -> (f32, f32) {
        self.process_stems(&mut Stems::default())
    }

    /// One stereo frame, and the same frame as the stems that make it up
    pub fn process_stems(&mut self, stems: &mut Stems) -> (f32, f32) {
        let mix = &self.mix;
        let mut bus = Bus::default();

        let kick = self.kick.process_sample() * mix.kick * KICK_GAIN;
        bus.add(kick, self.center_pan, KICK_SEND);
        stems.add(KICK_STEM, kick * self.center_pan.0, kick * self.center_pan.1);
        let snare = self.snare.process_sample() * mix.snare * SNARE_GAIN;
        bus.add(snare, self.center_pan, SNARE_SEND);
        stems.add(SNARE_STEM, snare * self.center_pan.0, snare * self.center_pan.1);
        for (tom, pan) in self.toms.iter_mut().zip(self.tom_pans) {
            let tom = tom.process_sample() * mix.toms * TOM_GAIN;
            bus.add(tom, pan, TOM_SEND);
            stems.add(TOMS_STEM, tom * pan.0, tom * pan.1);
        }
        for (slot, (cymbal, pan)) in self.cymbals.iter_mut().zip(self.cymbal_pans).enumerate() {
            let (gain, send) = if slot == HAT {
                (mix.hihat * HAT_GAIN, HAT_SEND)
            } else {
                (mix.cymbals * CYMBAL_GAIN, CYMBAL_SEND)
            };
            let sample = cymbal.process_sample() * gain;
            bus.add(sample, pan, send);
            stems.add(
                if slot == HAT { HAT_STEM } else { CYMBALS_STEM },
                sample * pan.0,
                sample * pan.1,
            );
        }

        let (wet_left, wet_right) = self.room.process(bus.send_left, bus.send_right);
        let room_gain = mix.room * self.room_level;

        // An engine that is not ringing costs nothing
        let (metal_left, metal_right) = self.metal.process(mix, stems);
        let (groove_left, groove_right) = self.groove.process(mix, stems);
        stems.add(ROOM_STEM, wet_left * room_gain, wet_right * room_gain);

        // Single hits pass untouched. Peaks of a busy pattern are rounded off instead of
        // clipping.
        (
            bus_clip(bus.left + wet_left * room_gain + metal_left + groove_left),
            bus_clip(bus.right + wet_right * room_gain + metal_right + groove_right),
        )
    }
}

/// 0 is the lowest tom. Only called with tom notes.
pub(crate) fn tom_index(note: u8) -> usize {
    match note {
        41 => 0,
        43 => 1,
        45 => 2,
        47 => 3,
        48 => 4,
        _ => 5,
    }
}

/// Constant-power pan: (left, right) gains for a position from -1.0 to 1.0
fn pan_gains(pan: f32) -> (f32, f32) {
    let angle = (pan + 1.0) * FRAC_PI_4;
    (angle.cos(), angle.sin())
}

#[cfg(test)]
mod tests;
