//! The effects that play from the parameters of a slot (#268): a second delay, a pedal on a
//! slot that is not its own, the gate as a block. The pedals themselves keep their instances in
//! `AmpChain`; these are the others, made with the chain and kept in a pool of
//! `MAX_PER_KIND` of each kind, so that putting a block in the chain allocates nothing on the
//! audio thread.
//!
//! A `Unit` is one such effect and is made for either side of the amp as the pedals are: the
//! effects that run on both sides at once-or-the-other have an instance for each (the delay and
//! the reverb at the oversampled rate in front and at the host's behind, the drive, the wah and
//! the pitch shifter likewise), the others are set up again for the side they stand on. What a
//! unit is told is `BlockSettings`, which the plugin works out from the slot's values (0.0 to
//! 1.0) exactly as it does from the same values of the pedal's own parameters.
//!
//! What a unit holds and how each kind is set up and played is not here: it is the kind's own
//! `Effect` (`effect.rs`), listed once in the table of `effects.rs`, which makes
//! `BlockSettings` and the effect's dispatch. This file keeps the assignment: which unit plays
//! which block, and when it starts from rest.

use crate::effect::Rates;
pub use crate::effects::BlockSettings;
use crate::effects::Body;
use crate::order::Zone;
use crate::slots::{EffectKind, MAX_PER_KIND};

/// One effect of the pool, and where it stands now
pub struct Unit {
    kind: EffectKind,
    body: Body,
    // The zone and slot it plays on, or `None` while no block of the chain uses it
    assigned: Option<(Zone, u8)>,
    rates: Rates,
}

impl Unit {
    fn new(kind: EffectKind) -> Unit {
        Unit {
            kind,
            body: Body::new(kind),
            assigned: None,
            rates: Rates { host: 44100.0, oversampled: 176_400.0 },
        }
    }

    /// Designs everything that depends on the sample rate, for the side the unit stands on (or
    /// the front where it stands nowhere). Allocates: not for the audio thread
    fn set_sample_rate(&mut self, host_rate: f32, oversampled_rate: f32) {
        self.rates = Rates { host: host_rate, oversampled: oversampled_rate };
        self.body.set_sample_rate(self.rates);
        self.assigned = None;
    }

    /// Sets the unit up for `zone` from rest: what it held on the side it left is not heard
    /// if it comes back. Does not allocate
    fn enter(&mut self, zone: Zone) {
        self.body.set_up(zone, self.rates);
    }

    /// As after a long time switched off. Does not allocate
    fn reset(&mut self) {
        let zone = self.assigned.map_or(Zone::Front, |(zone, _)| zone);
        self.body.reset(zone, self.rates);
    }

    pub fn assigned(&self) -> Option<(Zone, u8)> {
        self.assigned
    }

    /// Whether nothing of earlier input is left to hear
    pub fn is_idle(&self) -> bool {
        self.body.is_idle()
    }

    /// Passes on what is not read at every call: the drive's dials and switch, over the next
    /// `steps` samples of the oversampled rate, and the gate's. `snap` where nothing is heard
    /// of the unit yet
    pub fn tell(&mut self, settings: &BlockSettings, zone: Zone, steps: u32, snap: bool) {
        self.body.tell(settings, zone, steps, snap);
    }

    /// The effect itself, which plays through the same call as a pedal on its own parameters
    /// (`Body::process_front` and `process_back`)
    pub fn body_mut(&mut self) -> &mut Body {
        &mut self.body
    }
}

/// Every effect there is `MAX_PER_KIND` of, made with the chain
pub struct Pool {
    units: Vec<Unit>,
}

impl Pool {
    pub fn new() -> Pool {
        let units = EffectKind::ALL
            .into_iter()
            .flat_map(|kind| (0..MAX_PER_KIND).map(move |_| kind))
            .map(Unit::new)
            .collect();
        Pool { units }
    }

    fn index(kind: EffectKind, rank: u8) -> usize {
        kind.index() * MAX_PER_KIND + (rank as usize).min(MAX_PER_KIND - 1)
    }

    /// Designs every unit for the sample rate. Allocates: not for the audio thread
    pub fn set_sample_rate(&mut self, host_rate: f32, oversampled_rate: f32) {
        for unit in &mut self.units {
            unit.set_sample_rate(host_rate, oversampled_rate);
        }
    }

    pub fn unit_mut(&mut self, kind: EffectKind, rank: u8) -> &mut Unit {
        &mut self.units[Pool::index(kind, rank)]
    }

    pub fn units_mut(&mut self) -> impl Iterator<Item = &mut Unit> {
        self.units.iter_mut()
    }

    /// Every unit as after a long time switched off
    pub fn reset(&mut self) {
        for unit in &mut self.units {
            unit.reset();
        }
    }

    /// True when no unit has anything left to hear
    pub fn is_idle(&self) -> bool {
        self.units.iter().all(Unit::is_idle)
    }

    /// Moves the units to where `assign` says they stand: one that has moved or been put in
    /// starts from rest on its side, one that has been taken out is put away. Does not allocate
    pub fn follow(&mut self, assign: impl Fn(EffectKind, u8) -> Option<(Zone, u8)>) {
        for (index, unit) in self.units.iter_mut().enumerate() {
            let rank = (index % MAX_PER_KIND) as u8;
            let now = assign(unit.kind, rank);
            if now == unit.assigned {
                continue;
            }
            unit.assigned = now;
            match now {
                Some((zone, _)) => unit.enter(zone),
                None => unit.reset(),
            }
        }
    }
}
