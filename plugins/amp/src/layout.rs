//! The line as the audio plays it (#268): what stands in front of the amps and behind the
//! cabinets, in order, each block either one of the twelve pedals playing from its own
//! parameters (`Voice::Home`) or an effect that plays from the parameters of its slot
//! (`Voice::Slot`: a second delay, a pedal on a slot that is not its own, the gate as a block).
//!
//! It is made from the chain model (`Layout::of`), which the editor sets and the project
//! saves, and it is plain data of a fixed size, so the audio thread can read and copy it. A
//! pedal the chain does not hold is not played at all, whatever its switch says.

use crate::order::{ChainOrder, Pedal, Zone};
use crate::slots::{Chain, EffectKind, MAX_PER_KIND, MAX_SLOTS};

/// One effect of the line
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Voice {
    /// A pedal playing from its own parameters, on the slot of those (`Chain::is_home`)
    Home(Pedal),
    /// An effect playing from the parameters of `slot` (counted from 0). `rank` says which of
    /// the instances of its kind it plays on: the blocks of a kind that stand on a slot of
    /// their own are counted by slot, so a block's instance does not change when another
    /// kind's is moved
    Slot { kind: EffectKind, slot: u8, rank: u8 },
}

const FILLER: Voice = Voice::Home(Pedal::Comp);

/// The line: the effects in front of the amps and behind the cabinets, in order
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    voices: [Voice; MAX_SLOTS],
    len: usize,
    front: usize,
}

// Equal when the line is: what stands past the last voice is no part of it
impl PartialEq for Layout {
    fn eq(&self, other: &Layout) -> bool {
        self.voices() == other.voices() && self.front == other.front
    }
}

impl Eq for Layout {}

/// The twelve pedals in the default order, each playing from its own parameters
impl Default for Layout {
    fn default() -> Self {
        Layout::from(ChainOrder::default())
    }
}

/// Every pedal once, from its own parameters: what the amp played before it had a chain
impl From<ChainOrder> for Layout {
    fn from(order: ChainOrder) -> Self {
        let mut layout = Layout { voices: [FILLER; MAX_SLOTS], len: 0, front: order.front().len() };
        for &pedal in order.front().iter().chain(order.back()) {
            layout.voices[layout.len] = Voice::Home(pedal);
            layout.len += 1;
        }
        layout
    }
}

impl Layout {
    /// What the chain plays. A block on the slot of its own pedal plays from the pedal's
    /// parameters, any other from its slot's. A block past the instances there are of its kind
    /// (`MAX_PER_KIND`, which `Chain` keeps to) is not played
    pub fn of(chain: &Chain) -> Layout {
        let mut layout = Layout { voices: [FILLER; MAX_SLOTS], len: 0, front: 0 };
        for (place, block) in chain.blocks().iter().enumerate() {
            let voice = match block.kind.pedal() {
                Some(pedal) if Chain::is_home(*block) => Some(Voice::Home(pedal)),
                _ => {
                    let rank = chain
                        .blocks()
                        .iter()
                        .filter(|other| {
                            other.kind == block.kind
                                && other.slot < block.slot
                                && !Chain::is_home(**other)
                        })
                        .count();
                    (rank < MAX_PER_KIND).then_some(Voice::Slot {
                        kind: block.kind,
                        slot: block.slot,
                        rank: rank as u8,
                    })
                }
            };
            if let Some(voice) = voice {
                layout.voices[layout.len] = voice;
                layout.len += 1;
                if place < chain.front().len() {
                    layout.front += 1;
                }
            }
        }
        layout
    }

    pub fn voices(&self) -> &[Voice] {
        &self.voices[..self.len]
    }

    /// In front of the amps, first to last
    pub fn front(&self) -> &[Voice] {
        &self.voices[..self.front]
    }

    /// Behind the cabinets, first to last
    pub fn back(&self) -> &[Voice] {
        &self.voices[self.front..self.len]
    }

    /// The zone a pedal plays in from its own parameters, or `None` where the chain does not
    /// hold it
    pub fn home_zone(&self, pedal: Pedal) -> Option<Zone> {
        let at = self.voices().iter().position(|&voice| voice == Voice::Home(pedal))?;
        Some(if at < self.front { Zone::Front } else { Zone::Back })
    }

    /// The zone and slot of the effect that plays on instance `rank` of `kind`, if one does
    pub fn zone_of_slot(&self, kind: EffectKind, rank: u8) -> Option<(Zone, u8)> {
        let at = self.voices().iter().position(|voice| {
            matches!(voice, Voice::Slot { kind: k, rank: r, .. } if *k == kind && *r == rank)
        })?;
        let zone = if at < self.front { Zone::Front } else { Zone::Back };
        match self.voices[at] {
            Voice::Slot { slot, .. } => Some((zone, slot)),
            Voice::Home(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_the_default_chain_is_the_twelve_pedals_from_their_own_parameters() {
        let layout = Layout::of(&Chain::default());
        assert_eq!(layout, Layout::default());
        assert_eq!(layout.voices().len(), 12);
        assert!(layout.voices().iter().all(|voice| matches!(voice, Voice::Home(_))));
        for pedal in Pedal::ALL {
            let zone = ChainOrder::default().zone_of(pedal);
            assert_eq!(layout.home_zone(pedal), Some(zone));
        }
    }

    #[test]
    fn test_a_pedal_the_chain_lacks_is_not_in_the_line() {
        let mut chain = Chain::default();
        let block = chain.remove(Pedal::Delay.index() as u8).unwrap();
        assert_eq!(block.kind, EffectKind::Delay);
        let layout = Layout::of(&chain);
        assert_eq!(layout.home_zone(Pedal::Delay), None);
        assert_eq!(layout.voices().len(), 11);
    }

    #[test]
    fn test_a_block_off_its_own_slot_and_a_second_one_play_from_their_slots() {
        let mut chain = Chain::default();
        // A second delay behind the cabinet, and a drive on a free slot in front
        let second = chain.add(EffectKind::Delay, Zone::Back, 0).unwrap();
        let drive = chain.add(EffectKind::Drive, Zone::Front, 0).unwrap();
        let layout = Layout::of(&chain);
        assert_eq!(layout.voices().len(), 14);
        assert_eq!(
            layout.back()[0],
            Voice::Slot { kind: EffectKind::Delay, slot: second, rank: 0 }
        );
        assert_eq!(
            layout.front()[0],
            Voice::Slot { kind: EffectKind::Drive, slot: drive, rank: 0 }
        );
        assert_eq!(layout.zone_of_slot(EffectKind::Delay, 0), Some((Zone::Back, second)));
        assert_eq!(layout.zone_of_slot(EffectKind::Delay, 1), None);
        // The pedals' own are where they were
        assert_eq!(layout.home_zone(Pedal::Delay), Some(Zone::Back));
        assert_eq!(layout.home_zone(Pedal::Drive), Some(Zone::Front));
    }

    #[test]
    fn test_instances_are_numbered_by_slot_not_by_place() {
        let mut chain = Chain::empty();
        let first = chain.add(EffectKind::Phaser, Zone::Back, 0).unwrap();
        assert_eq!(first, Pedal::Phaser.index() as u8, "the first one takes its own slot");
        // Two on other slots: the lower slot has instance 0 wherever it stands
        let mut chain = Chain::empty();
        chain.add(EffectKind::Reverb, Zone::Back, 0).unwrap();
        chain.remove(Pedal::Reverb.index() as u8);
        chain.add(EffectKind::Pitch, Zone::Back, 0).unwrap();
        let a = chain.add(EffectKind::Reverb, Zone::Back, 0).unwrap();
        let b = chain.add(EffectKind::Reverb, Zone::Back, 0).unwrap();
        assert_ne!(a, b);
        let layout = Layout::of(&chain);
        let ranks: Vec<(u8, u8)> = layout
            .voices()
            .iter()
            .filter_map(|voice| match *voice {
                Voice::Slot { kind: EffectKind::Reverb, slot, rank } => Some((slot, rank)),
                _ => None,
            })
            .collect();
        assert!(ranks.iter().all(|&(slot, rank)| {
            ranks.iter().filter(|&&(other, _)| other < slot).count() as u8 == rank
        }));
    }

    #[test]
    fn test_a_gate_block_plays_from_its_slot() {
        let mut chain = Chain::default();
        let slot = chain.add(EffectKind::Gate, Zone::Front, 0).unwrap();
        let layout = Layout::of(&chain);
        assert_eq!(layout.front()[0], Voice::Slot { kind: EffectKind::Gate, slot, rank: 0 });
    }

    #[test]
    fn test_a_chain_keeps_to_the_instances_there_are() {
        let mut chain = Chain::empty();
        for _ in 0..MAX_PER_KIND {
            chain.add(EffectKind::Delay, Zone::Back, 0).unwrap();
        }
        assert_eq!(
            chain.add(EffectKind::Delay, Zone::Back, 0),
            Err(crate::slots::AddError::TooMany)
        );
        assert_eq!(Layout::of(&chain).voices().len(), MAX_PER_KIND);
    }
}
