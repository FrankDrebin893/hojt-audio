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

use crate::chain::{AmpChain, GATE_RELEASE_MS, GATE_THRESHOLD_DB};
use crate::comp::{Comp, CompSettings};
use crate::delay::{Delay, DelaySettings};
use crate::drive::{BackDrive, Drive};
use crate::eq::{Eq, EqSettings};
use crate::fuzz::{Fuzz, FuzzSettings};
use crate::gate::Gate;
use crate::mods::chorus::{Chorus, ChorusSettings};
use crate::mods::flanger::{Flanger, FlangerSettings};
use crate::mods::phaser::{Phaser, PhaserSettings};
use crate::mods::tremolo::{Tremolo, TremoloSettings};
use crate::order::Zone;
use crate::pitch::{Pitch, PitchSettings};
use crate::reverb::{Reverb, ReverbSettings};
use crate::slots::{EffectKind, MAX_PER_KIND};
use crate::wah::{Wah, WahSettings};

/// What a block of the chain is told, read once per block: the settings of its kind
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BlockSettings {
    /// No block on the slot: nothing is played
    #[default]
    None,
    Gate {
        on: bool,
        threshold_db: f32,
        release_ms: f32,
    },
    Drive {
        on: bool,
        drive: f32,
        tone: f32,
        level: f32,
    },
    Comp(CompSettings),
    Fuzz(FuzzSettings),
    Pitch(PitchSettings),
    Wah(WahSettings),
    Eq(EqSettings),
    Chorus(ChorusSettings),
    Flanger(FlangerSettings),
    Phaser(PhaserSettings),
    Tremolo(TremoloSettings),
    Delay(DelaySettings),
    Reverb(ReverbSettings),
}

impl BlockSettings {
    /// Whether the block is switched on
    pub fn is_on(&self) -> bool {
        match self {
            BlockSettings::None => false,
            BlockSettings::Gate { on, .. } | BlockSettings::Drive { on, .. } => *on,
            BlockSettings::Comp(s) => s.on,
            BlockSettings::Fuzz(s) => s.on,
            BlockSettings::Pitch(s) => s.on,
            BlockSettings::Wah(s) => s.on,
            BlockSettings::Eq(s) => s.on,
            BlockSettings::Chorus(s) => s.on,
            BlockSettings::Flanger(s) => s.on,
            BlockSettings::Phaser(s) => s.on,
            BlockSettings::Tremolo(s) => s.on,
            BlockSettings::Delay(s) => s.on,
            BlockSettings::Reverb(s) => s.on,
        }
    }

    /// The settings of a block that is not there or not of `kind`: every effect off
    fn of_kind(kind: EffectKind, settings: &BlockSettings) -> BlockSettings {
        let fits = matches!(
            (kind, settings),
            (EffectKind::Gate, BlockSettings::Gate { .. })
                | (EffectKind::Drive, BlockSettings::Drive { .. })
                | (EffectKind::Comp, BlockSettings::Comp(_))
                | (EffectKind::Fuzz, BlockSettings::Fuzz(_))
                | (EffectKind::Pitch, BlockSettings::Pitch(_))
                | (EffectKind::Wah, BlockSettings::Wah(_))
                | (EffectKind::Eq, BlockSettings::Eq(_))
                | (EffectKind::Chorus, BlockSettings::Chorus(_))
                | (EffectKind::Flanger, BlockSettings::Flanger(_))
                | (EffectKind::Phaser, BlockSettings::Phaser(_))
                | (EffectKind::Tremolo, BlockSettings::Tremolo(_))
                | (EffectKind::Delay, BlockSettings::Delay(_))
                | (EffectKind::Reverb, BlockSettings::Reverb(_))
        );
        if fits {
            *settings
        } else {
            BlockSettings::None
        }
    }
}

// The effect of a unit: one instance for either side where the two sides need different ones
#[allow(clippy::large_enum_variant)]
enum Body {
    Gate(Gate),
    Drive { front: Drive, back: BackDrive },
    Comp(Comp),
    Fuzz(Fuzz),
    Pitch { front: Pitch, back: [Pitch; 2] },
    Wah { front: Wah, back: [Wah; 2] },
    Eq(Eq),
    Chorus(Chorus),
    Flanger(Flanger),
    Phaser(Phaser),
    Tremolo(Tremolo),
    Delay { front: Delay, back: Delay },
    Reverb { front: Reverb, back: Reverb },
}

/// One effect of the pool, and where it stands now
pub struct Unit {
    kind: EffectKind,
    body: Body,
    // The zone and slot it plays on, or `None` while no block of the chain uses it
    assigned: Option<(Zone, u8)>,
    host_rate: f32,
    oversampled_rate: f32,
}

impl Unit {
    fn new(kind: EffectKind) -> Unit {
        let body = match kind {
            EffectKind::Gate => Body::Gate(Gate::new()),
            EffectKind::Drive => Body::Drive { front: Drive::new(), back: BackDrive::new() },
            EffectKind::Comp => Body::Comp(Comp::new()),
            EffectKind::Fuzz => Body::Fuzz(Fuzz::new()),
            EffectKind::Pitch => {
                Body::Pitch { front: Pitch::new(), back: [Pitch::new(), Pitch::new()] }
            }
            EffectKind::Wah => Body::Wah { front: Wah::new(), back: [Wah::new(), Wah::new()] },
            EffectKind::Eq => Body::Eq(Eq::new()),
            EffectKind::Chorus => Body::Chorus(Chorus::new()),
            EffectKind::Flanger => Body::Flanger(Flanger::new()),
            EffectKind::Phaser => Body::Phaser(Phaser::new()),
            EffectKind::Tremolo => Body::Tremolo(Tremolo::new()),
            EffectKind::Delay => Body::Delay { front: Delay::new(), back: Delay::new() },
            EffectKind::Reverb => Body::Reverb { front: Reverb::new(), back: Reverb::new() },
        };
        Unit { kind, body, assigned: None, host_rate: 44100.0, oversampled_rate: 176_400.0 }
    }

    fn rate_in(&self, zone: Zone) -> f32 {
        match zone {
            Zone::Front => self.oversampled_rate,
            Zone::Back => self.host_rate,
        }
    }

    /// Designs everything that depends on the sample rate, for the side the unit stands on (or
    /// the front where it stands nowhere). Allocates: not for the audio thread
    fn set_sample_rate(&mut self, host_rate: f32, oversampled_rate: f32) {
        self.host_rate = host_rate;
        self.oversampled_rate = oversampled_rate;
        match &mut self.body {
            Body::Gate(gate) => gate.set_sample_rate(oversampled_rate),
            Body::Drive { front, back } => {
                front.configure(oversampled_rate);
                back.configure(host_rate);
            }
            Body::Pitch { front, back } => {
                front.set_sample_rate(oversampled_rate);
                back.iter_mut().for_each(|side| side.set_sample_rate(host_rate));
            }
            Body::Wah { front, back } => {
                front.set_sample_rate(oversampled_rate);
                back.iter_mut().for_each(|side| side.set_sample_rate(host_rate));
            }
            Body::Delay { front, back } => {
                front.set_sample_rate(oversampled_rate);
                back.set_sample_rate(host_rate);
            }
            Body::Reverb { front, back } => {
                front.set_sample_rate(oversampled_rate);
                back.set_sample_rate(host_rate);
            }
            // One instance for both sides: made for the front, set up again where it stands
            _ => self.enter(Zone::Front),
        }
        self.assigned = None;
    }

    /// Sets the unit up for `zone` from rest: what it held on the side it left is not heard
    /// if it comes back. Does not allocate
    fn enter(&mut self, zone: Zone) {
        let rate = self.rate_in(zone);
        match &mut self.body {
            Body::Gate(gate) => {
                gate.reset();
                gate.snap();
            }
            Body::Drive { front, back } => {
                front.reset();
                back.reset();
                front.set_on(false);
                back.set_on(false);
                front.snap();
                back.snap();
            }
            Body::Comp(comp) => comp.set_sample_rate(rate),
            Body::Fuzz(fuzz) => fuzz.set_sample_rate(rate, zone == Zone::Back),
            Body::Pitch { front, back } => {
                front.reset();
                back.iter_mut().for_each(Pitch::reset);
            }
            Body::Wah { front, back } => {
                front.reset();
                back.iter_mut().for_each(Wah::reset);
            }
            Body::Eq(eq) => eq.set_sample_rate(rate),
            Body::Chorus(chorus) => chorus.set_sample_rate(rate),
            Body::Flanger(flanger) => flanger.set_sample_rate(rate),
            Body::Phaser(phaser) => phaser.set_sample_rate(rate),
            Body::Tremolo(tremolo) => tremolo.set_sample_rate(rate),
            Body::Delay { front, back } => {
                // Nothing that is idle is cleared: that would only cost time
                if !front.is_idle() {
                    front.reset();
                }
                if !back.is_idle() {
                    back.reset();
                }
            }
            Body::Reverb { front, back } => {
                if !front.is_idle() {
                    front.reset();
                }
                if !back.is_idle() {
                    back.reset();
                }
            }
        }
    }

    /// As after a long time switched off. Does not allocate
    fn reset(&mut self) {
        let zone = self.assigned.map_or(Zone::Front, |(zone, _)| zone);
        self.enter(zone);
    }

    pub fn assigned(&self) -> Option<(Zone, u8)> {
        self.assigned
    }

    /// Whether nothing of earlier input is left to hear
    pub fn is_idle(&self) -> bool {
        match &self.body {
            // As the pedals count in `AmpChain::is_idle`: those with no tail are always idle
            Body::Gate(_) | Body::Drive { .. } | Body::Comp(_) | Body::Fuzz(_) => true,
            Body::Pitch { front, back } => front.is_idle() && back.iter().all(Pitch::is_idle),
            Body::Wah { .. } | Body::Eq(_) => true,
            Body::Chorus(chorus) => chorus.is_idle(),
            Body::Flanger(flanger) => flanger.is_idle(),
            Body::Phaser(phaser) => phaser.is_idle(),
            Body::Tremolo(tremolo) => tremolo.is_idle(),
            Body::Delay { front, back } => front.is_idle() && back.is_idle(),
            Body::Reverb { front, back } => front.is_idle() && back.is_idle(),
        }
    }

    /// Passes on what is not read at every call: the drive's dials and switch, over the next
    /// `steps` samples of the oversampled rate, and the gate's. `snap` where nothing is heard
    /// of the unit yet
    pub fn tell(&mut self, settings: &BlockSettings, zone: Zone, steps: u32, snap: bool) {
        let settings = BlockSettings::of_kind(self.kind, settings);
        match (&mut self.body, settings) {
            (Body::Drive { front, back }, BlockSettings::Drive { on, drive, tone, level }) => {
                front.set_on(on && zone == Zone::Front);
                back.set_on(on && zone == Zone::Back);
                front.set(drive, tone, level, steps);
                back.set(drive, tone, level, steps);
                if snap {
                    front.snap();
                    back.snap();
                }
            }
            (Body::Drive { front, back }, _) => {
                front.set_on(false);
                back.set_on(false);
            }
            (Body::Gate(gate), BlockSettings::Gate { on, threshold_db, release_ms }) => {
                gate.set(
                    on,
                    threshold_db.clamp(GATE_THRESHOLD_DB[0], GATE_THRESHOLD_DB[1]),
                    release_ms.clamp(GATE_RELEASE_MS[0], GATE_RELEASE_MS[1]),
                );
                if snap {
                    gate.snap();
                }
            }
            (Body::Gate(gate), _) => gate.set(false, GATE_THRESHOLD_DB[0], GATE_RELEASE_MS[0]),
            _ => {}
        }
    }

    /// In front of the amp: one channel, at the oversampled rate. Does not allocate
    pub fn process_front(&mut self, settings: &BlockSettings, high: &mut [f32]) {
        let settings = BlockSettings::of_kind(self.kind, settings);
        match (&mut self.body, settings) {
            (Body::Gate(gate), _) => {
                let mut detector = [0.0f32; crate::chain::OVERSAMPLED_CHUNK];
                detector[..high.len()].copy_from_slice(high);
                gate.process(&detector[..high.len()], high);
            }
            (Body::Drive { front, .. }, _) => front.process(high),
            (Body::Comp(comp), s) => comp.process_mono(&comp_of(s), high),
            (Body::Fuzz(fuzz), s) => fuzz.process_mono(&fuzz_of(s), high),
            (Body::Pitch { front, .. }, s) => front.process_mono(&pitch_of(s), high),
            (Body::Wah { front, .. }, s) => front.process_mono(&wah_of(s), high),
            (Body::Eq(eq), s) => eq.process_mono(&eq_of(s), high),
            (Body::Chorus(chorus), s) => chorus.process_mono(&chorus_of(s), high),
            (Body::Flanger(flanger), s) => flanger.process_mono(&flanger_of(s), high),
            (Body::Phaser(phaser), s) => phaser.process_mono(&phaser_of(s), high),
            (Body::Tremolo(tremolo), s) => tremolo.process_mono(&tremolo_of(s), high),
            (Body::Delay { front, .. }, s) => {
                let s = delay_of(s);
                if s.on || !front.is_idle() {
                    AmpChain::mono_through_stereo(high, |l, r| front.process(&s, l, r));
                }
            }
            (Body::Reverb { front, .. }, s) => {
                let s = reverb_of(s);
                if s.on || !front.is_idle() {
                    AmpChain::mono_through_stereo(high, |l, r| front.process(&s, l, r));
                }
            }
        }
    }

    /// Behind the cabinet: two channels, at the host's rate. Does not allocate
    pub fn process_back(&mut self, settings: &BlockSettings, left: &mut [f32], right: &mut [f32]) {
        let settings = BlockSettings::of_kind(self.kind, settings);
        match (&mut self.body, settings) {
            // The gate stands in front of the amp only
            (Body::Gate(_), _) => {}
            (Body::Drive { back, .. }, _) => back.process_stereo(left, right),
            (Body::Comp(comp), s) => comp.process_stereo(&comp_of(s), left, right),
            (Body::Fuzz(fuzz), s) => fuzz.process_stereo(&fuzz_of(s), left, right),
            (Body::Pitch { back, .. }, s) => {
                let s = pitch_of(s);
                back[0].process_mono(&s, left);
                back[1].process_mono(&s, right);
            }
            (Body::Wah { back, .. }, s) => {
                let s = wah_of(s);
                back[0].process_mono(&s, left);
                back[1].process_mono(&s, right);
            }
            (Body::Eq(eq), s) => eq.process_stereo(&eq_of(s), left, right),
            (Body::Chorus(chorus), s) => chorus.process_stereo(&chorus_of(s), left, right),
            (Body::Flanger(flanger), s) => flanger.process_stereo(&flanger_of(s), left, right),
            (Body::Phaser(phaser), s) => phaser.process_stereo(&phaser_of(s), left, right),
            (Body::Tremolo(tremolo), s) => tremolo.process_stereo(&tremolo_of(s), left, right),
            (Body::Delay { back, .. }, s) => back.process(&delay_of(s), left, right),
            (Body::Reverb { back, .. }, s) => back.process(&reverb_of(s), left, right),
        }
    }
}

fn comp_of(s: BlockSettings) -> CompSettings {
    if let BlockSettings::Comp(s) = s {
        s
    } else {
        CompSettings::default()
    }
}
fn fuzz_of(s: BlockSettings) -> FuzzSettings {
    if let BlockSettings::Fuzz(s) = s {
        s
    } else {
        FuzzSettings::default()
    }
}
fn pitch_of(s: BlockSettings) -> PitchSettings {
    if let BlockSettings::Pitch(s) = s {
        s
    } else {
        PitchSettings::default()
    }
}
fn wah_of(s: BlockSettings) -> WahSettings {
    if let BlockSettings::Wah(s) = s {
        s
    } else {
        WahSettings::default()
    }
}
fn eq_of(s: BlockSettings) -> EqSettings {
    if let BlockSettings::Eq(s) = s {
        s
    } else {
        EqSettings::default()
    }
}
fn chorus_of(s: BlockSettings) -> ChorusSettings {
    if let BlockSettings::Chorus(s) = s {
        s
    } else {
        ChorusSettings::default()
    }
}
fn flanger_of(s: BlockSettings) -> FlangerSettings {
    if let BlockSettings::Flanger(s) = s {
        s
    } else {
        FlangerSettings::default()
    }
}
fn phaser_of(s: BlockSettings) -> PhaserSettings {
    if let BlockSettings::Phaser(s) = s {
        s
    } else {
        PhaserSettings::default()
    }
}
fn tremolo_of(s: BlockSettings) -> TremoloSettings {
    if let BlockSettings::Tremolo(s) = s {
        s
    } else {
        TremoloSettings::default()
    }
}
fn delay_of(s: BlockSettings) -> DelaySettings {
    if let BlockSettings::Delay(s) = s {
        s
    } else {
        DelaySettings::default()
    }
}
fn reverb_of(s: BlockSettings) -> ReverbSettings {
    if let BlockSettings::Reverb(s) = s {
        s
    } else {
        ReverbSettings::default()
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
