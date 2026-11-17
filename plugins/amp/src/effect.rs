//! `Effect`: what the chain asks of any pedal (#371), on one side or the other.
//!
//! A pedal implements it next to its own code, and `effects.rs` lists every pedal once: one line
//! a pedal, from which the dispatch that plays them (static, a `match` on an enum: no `dyn` and
//! no heap in `process`) is made. A pedal that needs an instance for each side (the drive, the
//! wah, the pitch shifter, the delay, the reverb) keeps both inside its own `Effect` type, so
//! nothing outside it knows that.

use crate::order::Zone;

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
