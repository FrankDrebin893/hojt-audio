//! `Effect`: what the chain asks of any pedal (#371), on one side or the other.
//!
//! A pedal implements it next to its own code, and `effects.rs` lists every pedal once: one line
//! a pedal, from which the dispatch that plays them (static, a `match` on an enum: no `dyn` and
//! no heap in `process`) is made. A pedal that needs an instance for each side (the drive, the
//! wah, the pitch shifter, the delay, the reverb) keeps both inside its own `Effect` type, so
//! nothing outside it knows that.

use crate::dsp::oversample::MAX_FACTOR;
use crate::order::Zone;

// The chain works in pieces of at most this many samples, and reads the dials once per piece
pub const CHUNK: usize = 32;
// Room for a piece at the highest oversampled rate
pub const OVERSAMPLED_CHUNK: usize = CHUNK * MAX_FACTOR;

/// Runs a stereo effect on one channel, for an effect that stands in front of the amp:
/// the signal goes in on both sides and what comes out is the mean of the two, so the
/// level of the dry signal is as it was and the repeats and the tail are what a listener
/// in the middle would hear of them. Does not allocate
pub fn mono_through_stereo(high: &mut [f32], effect: impl FnOnce(&mut [f32], &mut [f32])) {
    let mut other = [0.0f32; OVERSAMPLED_CHUNK];
    let other = &mut other[..high.len()];
    other.copy_from_slice(high);
    effect(high, other);
    for (sample, other) in high.iter_mut().zip(other.iter()) {
        *sample = 0.5 * (*sample + *other);
    }
}

/// The two sample rates the chain runs at: in front of the amp the oversampled one, behind the
/// cabinet the host's
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rates {
    pub host: f32,
    pub oversampled: f32,
}

impl Rates {
    /// The rate a pedal on `zone` runs at
    pub fn in_zone(self, zone: Zone) -> f32 {
        match zone {
            Zone::Front => self.oversampled,
            Zone::Back => self.host,
        }
    }
}

/// What a pedal is told every block: a plain value, read once per block
pub trait EffectSettings: Copy + Default {
    /// Whether the pedal is switched on
    fn is_on(&self) -> bool;
}

/// One pedal as the chain plays it
pub trait Effect {
    type Settings: EffectSettings;

    /// A pedal that is not yet designed for any rate. May allocate: not for the audio thread
    fn new() -> Self;

    /// Designs everything that depends on the sample rates, for both sides. Allocates: not for
    /// the audio thread. A pedal with one instance for both sides is set up again for the front
    fn set_sample_rate(&mut self, rates: Rates) {
        self.set_up(Zone::Front, rates);
    }

    /// Sets the pedal up for `zone` from rest: what it held on the side it left is not heard if
    /// it comes back. Does not allocate
    fn set_up(&mut self, zone: Zone, rates: Rates);

    /// `set_up` for the pedal on its own parameters in `AmpChain` (`Voice::Home`), where `zone`
    /// is the side it stands on, or the front where the chain does not hold it. It differs
    /// from a slot's where the chain always did it differently (the pedal that has an instance
    /// for each side puts the one it left to rest; what is idle is not left alone): kept as it
    /// is, never tidied. Does not allocate
    fn set_up_home(&mut self, zone: Zone, rates: Rates) {
        self.set_up(zone, rates);
    }

    /// Puts the pedal to rest, on whichever side it was, as the chain does for a pedal it has
    /// taken out. Does not allocate
    fn put_away(&mut self);

    /// What the chain does with a pedal at `reset_stages`: as `put_away`, but what is idle is
    /// left alone where the pedal has a tail (clearing it would only cost time, and this is
    /// also called on the audio thread, when Bypass has faded). Does not allocate
    fn rest(&mut self) {
        self.put_away();
    }

    /// What is put to rest with the amp's stages when another amp is set up (the drive in
    /// front of the amp); the pedals' tails ring on through it, so nothing else is. Does not
    /// allocate
    fn rest_with_amp(&mut self) {}

    /// Samples a switch of the pedal takes to fade, which the amp follows when the pedal pushes
    /// it (the drive); 0 where the amp does not
    fn amp_fade_steps(&self) -> u32 {
        0
    }

    /// `tell` for the pedal on its own parameters: `zone` is the side it stands on, `None`
    /// where the chain does not hold it (it is then told its dials all the same). Does not
    /// allocate
    fn tell_home(
        &mut self,
        _settings: &Self::Settings,
        _zone: Option<Zone>,
        _steps: u32,
        _snap: bool,
    ) {
    }

    /// As after a long time switched off, on `zone`. Does not allocate
    fn reset(&mut self, zone: Zone, rates: Rates) {
        self.set_up(zone, rates);
    }

    /// Whether nothing of earlier input is left to hear (a pedal with no tail is always idle)
    fn is_idle(&self) -> bool;

    /// Passes on what is not read at every call (the drive's dials and switch over the next
    /// `steps` samples of the oversampled rate, the gate's), `None` where the block is not
    /// there: everything off. `snap` where nothing is heard of the pedal yet
    fn tell(&mut self, _settings: Option<&Self::Settings>, _zone: Zone, _steps: u32, _snap: bool) {}

    /// In front of the amp: one channel, at the oversampled rate. Does not allocate
    fn process_front(&mut self, settings: &Self::Settings, block: &mut [f32]);

    /// Behind the cabinet: two channels, at the host's rate. Does not allocate
    fn process_back(&mut self, settings: &Self::Settings, left: &mut [f32], right: &mut [f32]);
}
