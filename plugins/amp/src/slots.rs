//! The chain model: the blocks of the signal line, the slots that hold their knobs, and what
//! the project saves of them (#252, #266).
//!
//! The line has two zones: effects before the amps (one channel, at the amps' oversampled
//! rate) and effects after the cabinets (two channels, at the host's rate). Up to
//! `MAX_SLOTS` effect blocks stand in them, and one amp lane or more (`MAX_AMPS`) in the
//! middle; the first amp lane is always there.
//!
//! **Slots.** A block stands on a slot, 1 to 16, and a slot holds one effect kind with its
//! switch and up to `KNOBS` knobs: the parameters `slot<n>_on` and `slot<n>_k<m>` (`SlotParams`;
//! host names "Slot 3 Knob 2"). The kind of each slot and the order of the blocks are not
//! parameters: they are the `chain` the project saves (`StoredChain`).
//!
//! **The twelve pedals.** The pedals the amp had before it had a chain keep their own
//! parameters (`comp_on`, `delay_time` and so on, which hosts and projects know): a block of
//! kind K standing on the slot of that pedal (`Chain::home`) is bound to them, and the audio
//! reads them as it always did, so a host's automation of them keeps working and nothing is
//! copied or renamed. Every other block (a second delay, a pedal on the wrong slot, anything
//! on slots the pedals do not own) keeps its values in the slot parameters, which the audio reads
//! (`layout`, `extras`, #268). A pedal the chain does not hold is not played, whatever its own
//! switch says. A project or preset from before
//! has no `chain` and opens as `Chain::from_order` of its `chain-order`: the same twelve
//! pedals, in the saved order, with their own on/off state, which stays in their parameters.
//!
//! **Instances.** A chain holds at most `MAX_PER_KIND` (2) blocks of a kind: the audio makes the
//! effects that are not a pedal's own with the chain, that many of each kind (`extras::Pool`),
//! so that putting a block in allocates nothing on the audio thread.
//!
//! **Memory.** Taking a block out keeps its values by effect kind (`Memory`), and putting one
//! of that kind in again brings them back, on whichever slot it lands.

use nih_plug::params::persist::PersistentField;
use nih_plug::prelude::*;
use std::sync::atomic::{fence, AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use crate::comp::CompSettings;
use crate::delay::DelaySettings;
use crate::eq::EqSettings;
use crate::extras::BlockSettings;
use crate::fuzz::FuzzSettings;
use crate::layout::{Layout, Voice};
use crate::mods::chorus::ChorusSettings;
use crate::mods::flanger::FlangerSettings;
use crate::mods::phaser::PhaserSettings;
use crate::mods::tremolo::TremoloSettings;
use crate::order::{ChainOrder, Pedal, Zone, AMP_ID};
use crate::pitch::PitchSettings;
use crate::presets::{set_together, Edit};
use crate::reverb::ReverbSettings;
use crate::wah::WahSettings;
use crate::GuitarAmpParams;

/// Effect slots in a chain
pub const MAX_SLOTS: usize = 16;

/// Knobs of a slot (`K`): those of the widest pedal, the delay (Time, Feedback, Mix, Sync, Note).
/// Frozen with the ids `slot<n>_k1` to `slot<n>_k5`
pub const KNOBS: usize = 5;

/// Blocks of one kind in a chain (#268): the pedal's own on its slot and one more, or two on
/// other slots. The audio makes the instances of the effects that are not a pedal's own with
/// the chain, off the audio thread, so there is a number of them to make
pub const MAX_PER_KIND: usize = 2;

/// Amp lanes. The first is always there; the second is as it is today
pub const MAX_AMPS: usize = 2;

/// What a slot can hold
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Gate,
    Drive,
    Comp,
    Fuzz,
    Pitch,
    Wah,
    Eq,
    Chorus,
    Flanger,
    Phaser,
    Tremolo,
    Delay,
    Reverb,
}

impl EffectKind {
    pub const ALL: [EffectKind; 13] = [
        EffectKind::Gate,
        EffectKind::Drive,
        EffectKind::Comp,
        EffectKind::Fuzz,
        EffectKind::Pitch,
        EffectKind::Wah,
        EffectKind::Eq,
        EffectKind::Chorus,
        EffectKind::Flanger,
        EffectKind::Phaser,
        EffectKind::Tremolo,
        EffectKind::Delay,
        EffectKind::Reverb,
    ];

    /// Frozen: what a saved project calls the kind
    pub fn id(self) -> &'static str {
        match self {
            EffectKind::Gate => "gate",
            EffectKind::Drive => "drive",
            EffectKind::Comp => "comp",
            EffectKind::Fuzz => "fuzz",
            EffectKind::Pitch => "pitch",
            EffectKind::Wah => "wah",
            EffectKind::Eq => "eq",
            EffectKind::Chorus => "chorus",
            EffectKind::Flanger => "flanger",
            EffectKind::Phaser => "phaser",
            EffectKind::Tremolo => "tremolo",
            EffectKind::Delay => "delay",
            EffectKind::Reverb => "reverb",
        }
    }

    fn from_id(id: &str) -> Option<EffectKind> {
        EffectKind::ALL.into_iter().find(|kind| kind.id() == id)
    }

    /// Position in `ALL`
    pub fn index(self) -> usize {
        self as usize
    }

    /// How many knobs the kind uses of the `KNOBS` a slot has (Sync and Note of the delay,
    /// and the wah's mode, count as knobs)
    pub fn knob_count(self) -> usize {
        match self {
            EffectKind::Gate => 2,
            EffectKind::Drive => 3,
            EffectKind::Comp => 2,
            EffectKind::Fuzz => 2,
            EffectKind::Pitch => 3,
            EffectKind::Wah => 4,
            EffectKind::Eq => 4,
            EffectKind::Chorus | EffectKind::Flanger | EffectKind::Phaser => 2,
            EffectKind::Tremolo => 2,
            EffectKind::Delay => 5,
            EffectKind::Reverb => 2,
        }
    }

    /// The pedal the amp had before it had a chain. `None` for the gate, which stood in front
    /// of the chain and was never part of its order
    pub fn pedal(self) -> Option<Pedal> {
        Some(match self {
            EffectKind::Gate => return None,
            EffectKind::Drive => Pedal::Drive,
            EffectKind::Comp => Pedal::Comp,
            EffectKind::Fuzz => Pedal::Fuzz,
            EffectKind::Pitch => Pedal::Pitch,
            EffectKind::Wah => Pedal::Wah,
            EffectKind::Eq => Pedal::Eq,
            EffectKind::Chorus => Pedal::Chorus,
            EffectKind::Flanger => Pedal::Flanger,
            EffectKind::Phaser => Pedal::Phaser,
            EffectKind::Tremolo => Pedal::Tremolo,
            EffectKind::Delay => Pedal::Delay,
            EffectKind::Reverb => Pedal::Reverb,
        })
    }

    pub fn of_pedal(pedal: Pedal) -> EffectKind {
        EffectKind::ALL
            .into_iter()
            .find(|kind| kind.pedal() == Some(pedal))
            .unwrap_or(EffectKind::Comp)
    }

    /// The gate listens to the guitar as it arrives, so it only stands in front of the amps;
    /// the rest stand where their pedal does
    pub fn can_be_in(self, zone: Zone) -> bool {
        self.pedal().map_or(zone == Zone::Front, |pedal| pedal.can_be_in(zone))
    }

    /// The slot (counted from 0) of a pedal's own parameters, if the kind is one of them
    pub fn home(self) -> Option<usize> {
        self.pedal().map(Pedal::index)
    }

    // Four bits in the saved word: 0 is no block
    fn code(self) -> u64 {
        self.index() as u64 + 1
    }

    fn from_code(code: u64) -> Option<EffectKind> {
        EffectKind::ALL.get((code as usize).checked_sub(1)?).copied()
    }
}

/// One effect of the chain: the slot it stands on (counted from 0) and what it is
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub slot: u8,
    pub kind: EffectKind,
}

/// Why a block could not be added
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddError {
    /// All `MAX_SLOTS` are taken
    Full,
    /// The kind cannot stand in that zone
    WrongZone,
    /// The chain already has `MAX_PER_KIND` of that kind
    TooMany,
}

/// The line: the blocks before the amps and after the cabinets, in order, and how many amp
/// lanes there are. Plain data of a fixed size, so the audio thread can read and copy it
#[derive(Clone, Copy, Debug)]
pub struct Chain {
    blocks: [Block; MAX_SLOTS],
    len: usize,
    front: usize,
    amps: usize,
}

impl Default for Chain {
    /// The twelve pedals in the order the chain had before it could be rearranged
    fn default() -> Self {
        Chain::from_order(&ChainOrder::default())
    }
}

// Equal when the line is: what stands past the last block is no part of it
impl PartialEq for Chain {
    fn eq(&self, other: &Chain) -> bool {
        self.blocks() == other.blocks() && self.front == other.front && self.amps == other.amps
    }
}

impl Eq for Chain {}

const FILLER: Block = Block { slot: 0, kind: EffectKind::Comp };

impl Chain {
    /// No effects and the one amp that is always there
    pub fn empty() -> Chain {
        Chain { blocks: [FILLER; MAX_SLOTS], len: 0, front: 0, amps: 1 }
    }

    /// The twelve pedals in `order`, each on the slot of its own parameters, and the amp
    /// lanes of today
    pub fn from_order(order: &ChainOrder) -> Chain {
        let mut chain = Chain::empty();
        chain.amps = MAX_AMPS;
        for (place, &pedal) in order.front().iter().chain(order.back()).enumerate() {
            chain.blocks[place] =
                Block { slot: pedal.index() as u8, kind: EffectKind::of_pedal(pedal) };
        }
        chain.len = order.front().len() + order.back().len();
        chain.front = order.front().len();
        chain
    }

    /// The twelve-pedal view the board draws and a project saves for an older build: the
    /// pedals of the chain in its order. A pedal the chain lacks, or has twice or where it
    /// cannot stand, goes where `ChainOrder::from_ids` puts it, so this is always a legal
    /// order. That puts a pedal back that the chain has taken out, so the audio does not read
    /// this view: it reads `Layout::of`
    pub fn order(&self) -> ChainOrder {
        let front = self.front().iter().filter_map(|block| block.kind.pedal()).map(Pedal::id);
        let back = self.back().iter().filter_map(|block| block.kind.pedal()).map(Pedal::id);
        ChainOrder::from_ids(front.chain(std::iter::once(AMP_ID)).chain(back))
    }

    /// Every block, first to last
    pub fn blocks(&self) -> &[Block] {
        &self.blocks[..self.len]
    }

    /// The blocks in front of the amps, first to last
    pub fn front(&self) -> &[Block] {
        &self.blocks[..self.front]
    }

    /// The blocks behind the cabinets, first to last
    pub fn back(&self) -> &[Block] {
        &self.blocks[self.front..self.len]
    }

    pub fn amps(&self) -> usize {
        self.amps
    }

    /// How many blocks of `kind` the chain has
    pub fn count_of(&self, kind: EffectKind) -> usize {
        self.blocks().iter().filter(|block| block.kind == kind).count()
    }

    pub fn block_on(&self, slot: u8) -> Option<Block> {
        self.blocks().iter().copied().find(|block| block.slot == slot)
    }

    /// Whether the block stands on the slot of its own pedal's parameters (so the audio reads
    /// those) and not on the slot's parameters
    pub fn is_home(block: Block) -> bool {
        block.kind.home() == Some(block.slot as usize)
    }

    /// Put a block of `kind` in `zone` as number `place` (0 is first; past the end is last).
    /// It takes the slot of its own pedal's parameters when that is free and the lowest
    /// free slot otherwise, and the slot it took is given back
    pub fn add(&mut self, kind: EffectKind, zone: Zone, place: usize) -> Result<u8, AddError> {
        if !kind.can_be_in(zone) {
            return Err(AddError::WrongZone);
        }
        if self.len == MAX_SLOTS {
            return Err(AddError::Full);
        }
        if self.count_of(kind) >= MAX_PER_KIND {
            return Err(AddError::TooMany);
        }
        let free = |slot: usize| self.block_on(slot as u8).is_none();
        let slot = kind
            .home()
            .filter(|&home| free(home))
            .or_else(|| (0..MAX_SLOTS).find(|&slot| free(slot)))
            .unwrap_or(0) as u8;
        let (start, len) = match zone {
            Zone::Front => (0, self.front),
            Zone::Back => (self.front, self.len - self.front),
        };
        let at = start + place.min(len);
        self.blocks.copy_within(at..self.len, at + 1);
        self.blocks[at] = Block { slot, kind };
        self.len += 1;
        if zone == Zone::Front {
            self.front += 1;
        }
        Ok(slot)
    }

    /// Take the block on `slot` out. Its slot is free again; nothing else moves
    pub fn remove(&mut self, slot: u8) -> Option<Block> {
        let at = self.blocks().iter().position(|block| block.slot == slot)?;
        let block = self.blocks[at];
        self.blocks.copy_within(at + 1..self.len, at);
        self.len -= 1;
        if at < self.front {
            self.front -= 1;
        }
        Some(block)
    }

    /// Move the block on `slot` to be number `place` (0 is first; past the end is last) of
    /// `zone`. It keeps its slot, and with it the values it holds. A block that is not on the
    /// chain, or that cannot stand in that zone, leaves the chain as it is
    pub fn move_block(&mut self, slot: u8, zone: Zone, place: usize) -> Result<(), AddError> {
        let Some(block) = self.block_on(slot) else {
            return Ok(());
        };
        if !block.kind.can_be_in(zone) {
            return Err(AddError::WrongZone);
        }
        self.remove(slot);
        let (start, len) = match zone {
            Zone::Front => (0, self.front),
            Zone::Back => (self.front, self.len - self.front),
        };
        let at = start + place.min(len);
        self.blocks.copy_within(at..self.len, at + 1);
        self.blocks[at] = block;
        self.len += 1;
        if zone == Zone::Front {
            self.front += 1;
        }
        Ok(())
    }

    /// Add an amp lane, if there is room. Gives its number (the first is 0)
    pub fn add_amp(&mut self) -> Option<usize> {
        (self.amps < MAX_AMPS).then(|| {
            self.amps += 1;
            self.amps - 1
        })
    }

    /// Take an amp lane out. The first one cannot go, and nor can a lane that is not there
    pub fn remove_amp(&mut self, lane: usize) -> bool {
        let removable = lane > 0 && lane < self.amps;
        if removable {
            self.amps -= 1;
        }
        removable
    }

    /// The names a project saves: `amps=<n>`, then `<kind>@<slot>` for each block in order,
    /// with `amp` between the two zones
    pub fn ids(&self) -> Vec<String> {
        let block = |block: &Block| format!("{}@{}", block.kind.id(), block.slot + 1);
        let mut ids = vec![format!("amps={}", self.amps)];
        ids.extend(self.front().iter().map(block));
        ids.push(AMP_ID.to_string());
        ids.extend(self.back().iter().map(block));
        ids
    }

    /// The chain a saved project names. Whatever is wrong with the list is put right and not
    /// refused, since a project has to open: a name this build does not know is skipped, so
    /// is a slot named twice, a block on the side its kind cannot stand on, a third block of a
    /// kind and any block past the sixteenth. A list with no `amp` in it is damaged and gives the default
    pub fn from_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Chain {
        let mut chain = Chain::empty();
        let mut zone = Zone::Front;
        let mut seen_amp = false;
        let mut taken = [false; MAX_SLOTS];
        for id in ids {
            if id == AMP_ID {
                seen_amp = true;
                zone = Zone::Back;
                chain.front = chain.len;
            } else if let Some(amps) = id.strip_prefix("amps=") {
                chain.amps = amps.parse().unwrap_or(1).clamp(1, MAX_AMPS);
            } else if let Some((kind, slot)) = id.split_once('@') {
                let kind = EffectKind::from_id(kind);
                let slot = slot.parse::<usize>().ok().filter(|n| (1..=MAX_SLOTS).contains(n));
                if let (Some(kind), Some(slot)) = (kind, slot) {
                    let free = !std::mem::replace(&mut taken[slot - 1], true);
                    let room = chain.len < MAX_SLOTS && chain.count_of(kind) < MAX_PER_KIND;
                    if free && kind.can_be_in(zone) && room {
                        chain.blocks[chain.len] = Block { slot: slot as u8 - 1, kind };
                        chain.len += 1;
                    } else if free {
                        taken[slot - 1] = false;
                    }
                }
            }
        }
        if seen_amp {
            chain
        } else {
            Chain::default()
        }
    }

    // The chain as three words: sixteen bytes of blocks (kind code and slot, 0 for none),
    // then the number of blocks in front and of amp lanes
    fn to_words(self) -> [u64; 3] {
        let mut words = [0u64; 3];
        for (place, block) in self.blocks().iter().enumerate() {
            let byte = (block.kind.code() << 4) | block.slot as u64;
            words[place / 8] |= byte << (8 * (place % 8));
        }
        words[2] = self.front as u64 | ((self.amps as u64) << 8);
        words
    }

    /// `None` for words no chain has
    fn from_words(words: [u64; 3]) -> Option<Chain> {
        let mut chain = Chain::empty();
        let mut taken = [false; MAX_SLOTS];
        let mut ended = false;
        for place in 0..MAX_SLOTS {
            let byte = (words[place / 8] >> (8 * (place % 8))) & 0xff;
            if byte == 0 {
                ended = true;
                continue;
            }
            let kind = EffectKind::from_code(byte >> 4)?;
            let slot = (byte & 0xf) as usize;
            if ended || std::mem::replace(&mut taken[slot], true) {
                return None;
            }
            chain.blocks[place] = Block { slot: slot as u8, kind };
            chain.len += 1;
        }
        chain.front = (words[2] & 0xff) as usize;
        chain.amps = ((words[2] >> 8) & 0xff) as usize;
        let legal = chain.front <= chain.len
            && EffectKind::ALL.iter().all(|&kind| chain.count_of(kind) <= MAX_PER_KIND)
            && (1..=MAX_AMPS).contains(&chain.amps)
            && chain.front().iter().all(|block| block.kind.can_be_in(Zone::Front))
            && chain.back().iter().all(|block| block.kind.can_be_in(Zone::Back));
        legal.then_some(chain)
    }
}

/// What a slot holds: its switch and its knobs, each as the parameter stores it (0.0 to 1.0)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotValues {
    pub on: bool,
    pub knobs: [f32; KNOBS],
}

impl SlotValues {
    /// What a slot holds before anything sets it: off, every knob at the middle
    pub const DEFAULT: SlotValues = SlotValues { on: false, knobs: [0.5; KNOBS] };

    /// As a preset file writes it: `<0 or 1>:<knob>,<knob>,...`
    pub(crate) fn to_text(self) -> String {
        let knobs: Vec<String> = self.knobs.iter().map(|knob| knob.to_string()).collect();
        format!("{}:{}", u8::from(self.on), knobs.join(","))
    }

    /// `None` for text that is not that
    pub(crate) fn from_text(text: &str) -> Option<SlotValues> {
        let (on, knobs) = text.split_once(':')?;
        let on = match on {
            "0" => false,
            "1" => true,
            _ => return None,
        };
        let mut values = SlotValues { on, knobs: [0.0; KNOBS] };
        let mut parts = knobs.split(',');
        for knob in values.knobs.iter_mut() {
            *knob = parts.next()?.parse::<f32>().ok().filter(|v| (0.0..=1.0).contains(v))?;
        }
        parts.next().is_none().then_some(values)
    }
}

/// What a slot's parameters are: a switch and `KNOBS` plain knobs from 0 to 10, whose meaning
/// is the effect kind's that stands on the slot
#[derive(Params)]
pub struct SlotParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "k1"]
    pub k1: FloatParam,
    #[id = "k2"]
    pub k2: FloatParam,
    #[id = "k3"]
    pub k3: FloatParam,
    #[id = "k4"]
    pub k4: FloatParam,
    #[id = "k5"]
    pub k5: FloatParam,
}

impl SlotParams {
    /// The parameters of slot `number`, counted from 1
    pub fn new(number: usize) -> Self {
        let knob = |index: usize| crate::dial_param(&format!("Slot {number} Knob {index}"), 0.5);
        Self {
            on: BoolParam::new(format!("Slot {number} On"), false),
            k1: knob(1),
            k2: knob(2),
            k3: knob(3),
            k4: knob(4),
            k5: knob(5),
        }
    }

    pub fn knobs(&self) -> [&FloatParam; KNOBS] {
        [&self.k1, &self.k2, &self.k3, &self.k4, &self.k5]
    }
}

// The values `Memory` keeps for a kind: the switch and the knobs as bits, and whether it
// holds any. Atomics, since a host may restore a saved state from the audio thread
struct Saved {
    filled: AtomicBool,
    on: AtomicBool,
    knobs: [AtomicU32; KNOBS],
}

/// The values of the effects that were taken out, by kind: putting one of that kind in again
/// brings them back. Only the editor reads it
pub struct Memory {
    saved: [Saved; EffectKind::ALL.len()],
}

impl Memory {
    pub fn new() -> Self {
        Self {
            saved: std::array::from_fn(|_| Saved {
                filled: AtomicBool::new(false),
                on: AtomicBool::new(false),
                knobs: std::array::from_fn(|_| AtomicU32::new(0)),
            }),
        }
    }

    pub fn remember(&self, kind: EffectKind, values: SlotValues) {
        let saved = &self.saved[kind.index()];
        saved.on.store(values.on, Ordering::Relaxed);
        for (stored, knob) in saved.knobs.iter().zip(values.knobs) {
            stored.store(knob.to_bits(), Ordering::Relaxed);
        }
        saved.filled.store(true, Ordering::Relaxed);
    }

    pub fn recall(&self, kind: EffectKind) -> Option<SlotValues> {
        let saved = &self.saved[kind.index()];
        saved.filled.load(Ordering::Relaxed).then(|| SlotValues {
            on: saved.on.load(Ordering::Relaxed),
            knobs: std::array::from_fn(|index| {
                f32::from_bits(saved.knobs[index].load(Ordering::Relaxed))
            }),
        })
    }

    fn texts(&self) -> Vec<String> {
        EffectKind::ALL
            .into_iter()
            .filter_map(|kind| Some(format!("mem:{}:{}", kind.id(), self.recall(kind)?.to_text())))
            .collect()
    }

    fn read(&self, text: &str) {
        let parts = text.strip_prefix("mem:").and_then(|rest| rest.split_once(':'));
        if let Some((kind, values)) = parts {
            if let (Some(kind), Some(values)) =
                (EffectKind::from_id(kind), SlotValues::from_text(values))
            {
                self.remember(kind, values);
            }
        }
    }
}

struct Inner {
    // Three words and a sequence number: a write makes the number odd while it changes the
    // words, so a read that saw an odd or a changed number tries again
    sequence: AtomicU32,
    words: [AtomicU64; 3],
    // A `chain` came with the state being loaded: the `chain-order` that follows it is the
    // older form of the same, and is left alone
    explicit: AtomicBool,
    memory: Memory,
}

impl Inner {
    fn get(&self) -> Chain {
        for _ in 0..16 {
            let before = self.sequence.load(Ordering::Acquire);
            if !before.is_multiple_of(2) {
                std::hint::spin_loop();
                continue;
            }
            let words = std::array::from_fn(|index| self.words[index].load(Ordering::Relaxed));
            fence(Ordering::Acquire);
            if self.sequence.load(Ordering::Relaxed) == before {
                return Chain::from_words(words).unwrap_or_default();
            }
        }
        Chain::default()
    }

    fn put(&self, chain: Chain) {
        let words = chain.to_words();
        let begun = loop {
            let before = self.sequence.load(Ordering::Relaxed);
            if before.is_multiple_of(2)
                && self
                    .sequence
                    .compare_exchange_weak(before, before + 1, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
            {
                break before + 1;
            }
            std::hint::spin_loop();
        };
        // The odd number must be seen before any of the words
        fence(Ordering::Release);
        for (stored, word) in self.words.iter().zip(words) {
            stored.store(word, Ordering::Relaxed);
        }
        fence(Ordering::Release);
        self.sequence.store(begun + 1, Ordering::Release);
    }
}

/// The chain as the editor sets it and the audio thread reads it, without either waiting for
/// the other (a host may restore a saved state from the audio thread), and the memory of the
/// effects taken out. Saved with the project as `chain`
pub struct StoredChain {
    inner: Arc<Inner>,
}

impl StoredChain {
    pub fn new() -> Self {
        let inner = Inner {
            sequence: AtomicU32::new(0),
            words: Chain::default().to_words().map(AtomicU64::new),
            explicit: AtomicBool::new(false),
            memory: Memory::new(),
        };
        Self { inner: Arc::new(inner) }
    }

    pub fn get(&self) -> Chain {
        self.inner.get()
    }

    /// Any change that is not a state being loaded: a `chain` that came without a
    /// `chain-order` no longer hides the next old-style state from `LegacyOrder`
    pub fn put(&self, chain: Chain) {
        self.inner.explicit.store(false, Ordering::Relaxed);
        self.inner.put(chain);
    }

    /// The twelve-pedal view of the chain: what the audio reads and the board draws
    pub fn order(&self) -> ChainOrder {
        self.get().order()
    }

    /// The twelve pedals in `order`, as a preset or a drag on the board puts them. The amp
    /// lanes stay as they are
    pub fn put_order(&self, order: ChainOrder) {
        let mut chain = Chain::from_order(&order);
        chain.amps = self.get().amps;
        self.put(chain);
    }

    pub fn memory(&self) -> &Memory {
        &self.inner.memory
    }

    /// What the project saved before there was a chain calls it `chain-order`
    pub fn legacy(&self) -> LegacyOrder {
        LegacyOrder { inner: self.inner.clone() }
    }
}

/// Saved with the project as the list of `Chain::ids` and the memory
impl PersistentField<'_, Vec<String>> for StoredChain {
    fn set(&self, new_value: Vec<String>) {
        let texts = new_value.iter().map(String::as_str);
        self.inner.put(Chain::from_ids(texts.clone().filter(|text| !text.starts_with("mem:"))));
        for text in texts {
            self.inner.memory.read(text);
        }
        self.inner.explicit.store(true, Ordering::Relaxed);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&Vec<String>) -> R,
    {
        let mut saved = self.get().ids();
        saved.extend(self.inner.memory.texts());
        f(&saved)
    }
}

/// `chain-order`: the order of the twelve pedals as a project saved it before the chain, a
/// list of `ChainOrder::ids`. It is read when no `chain` came with it (the state is a
/// project's from before) and written as the twelve-pedal view of the chain, so that an older
/// build reading the project still finds its pedals in their order. A state is read key by key
/// in alphabetical order, `chain` before `chain-order`
pub struct LegacyOrder {
    inner: Arc<Inner>,
}

impl PersistentField<'_, Vec<String>> for LegacyOrder {
    fn set(&self, new_value: Vec<String>) {
        if self.inner.explicit.swap(false, Ordering::Relaxed) {
            return;
        }
        let order = ChainOrder::from_ids(new_value.iter().map(String::as_str));
        let amps = self.inner.get().amps();
        let mut chain = Chain::from_order(&order);
        chain.amps = amps;
        self.inner.put(chain);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&Vec<String>) -> R,
    {
        f(&self.inner.get().order().ids().into_iter().map(String::from).collect())
    }
}

// The parameters a pedal has had since before there was a chain, as its switch and its knobs
// Not called until the window edits the chain (#272), which does it through `Live`
#[cfg_attr(not(test), allow(dead_code))]
fn pedal_params(params: &GuitarAmpParams, kind: EffectKind) -> (ParamPtr, Vec<ParamPtr>) {
    let p = params;
    match kind {
        EffectKind::Gate => {
            (p.gate_on.as_ptr(), vec![p.gate_thresh.as_ptr(), p.gate_release.as_ptr()])
        }
        EffectKind::Drive => (
            p.drive_on.as_ptr(),
            vec![p.drive_gain.as_ptr(), p.drive_tone.as_ptr(), p.drive_level.as_ptr()],
        ),
        EffectKind::Comp => {
            (p.comp_on.as_ptr(), vec![p.comp_sustain.as_ptr(), p.comp_level.as_ptr()])
        }
        EffectKind::Fuzz => (p.fuzz_on.as_ptr(), vec![p.fuzz_fuzz.as_ptr(), p.fuzz_tone.as_ptr()]),
        EffectKind::Pitch => (
            p.pitch_on.as_ptr(),
            vec![p.pitch_semitones.as_ptr(), p.pitch_blend.as_ptr(), p.pitch_latency.as_ptr()],
        ),
        EffectKind::Wah => (
            p.wah_on.as_ptr(),
            vec![p.wah_pedal.as_ptr(), p.wah_q.as_ptr(), p.wah_mode.as_ptr(), p.wah_sens.as_ptr()],
        ),
        EffectKind::Eq => (
            p.eq_on.as_ptr(),
            vec![p.eq_low.as_ptr(), p.eq_mid.as_ptr(), p.eq_freq.as_ptr(), p.eq_high.as_ptr()],
        ),
        EffectKind::Chorus => {
            (p.chorus_on.as_ptr(), vec![p.chorus_rate.as_ptr(), p.chorus_depth.as_ptr()])
        }
        EffectKind::Flanger => {
            (p.flanger_on.as_ptr(), vec![p.flanger_rate.as_ptr(), p.flanger_depth.as_ptr()])
        }
        EffectKind::Phaser => {
            (p.phaser_on.as_ptr(), vec![p.phaser_rate.as_ptr(), p.phaser_depth.as_ptr()])
        }
        EffectKind::Tremolo => {
            (p.trem_on.as_ptr(), vec![p.trem_rate.as_ptr(), p.trem_depth.as_ptr()])
        }
        EffectKind::Delay => (
            p.delay_on.as_ptr(),
            vec![
                p.delay_time.as_ptr(),
                p.delay_feedback.as_ptr(),
                p.delay_mix.as_ptr(),
                p.delay_sync.as_ptr(),
                p.delay_note.as_ptr(),
            ],
        ),
        EffectKind::Reverb => {
            (p.reverb_on.as_ptr(), vec![p.reverb_decay.as_ptr(), p.reverb_mix.as_ptr()])
        }
    }
}

impl GuitarAmpParams {
    /// The parameters of slot `index`, counted from 0
    pub fn slot(&self, index: usize) -> &SlotParams {
        self.slots()[index]
    }

    /// The parameters of every slot, in order
    pub fn slots(&self) -> [&SlotParams; MAX_SLOTS] {
        [
            &self.slot1,
            &self.slot2,
            &self.slot3,
            &self.slot4,
            &self.slot5,
            &self.slot6,
            &self.slot7,
            &self.slot8,
            &self.slot9,
            &self.slot10,
            &self.slot11,
            &self.slot12,
            &self.slot13,
            &self.slot14,
            &self.slot15,
            &self.slot16,
        ]
    }

    // The switch and the knobs a block's values are in: its pedal's own parameters when it
    // stands on their slot (`Chain::is_home`), else its slot's. Only as many knobs as its kind uses
    fn params_of(&self, block: Block) -> (ParamPtr, Vec<ParamPtr>) {
        let (on, mut knobs) = if Chain::is_home(block) {
            pedal_params(self, block.kind)
        } else {
            let slot = self.slot(block.slot as usize);
            (slot.on.as_ptr(), slot.knobs().iter().map(|knob| knob.as_ptr()).collect())
        };
        knobs.truncate(block.kind.knob_count());
        (on, knobs)
    }
}

impl GuitarAmpParams {
    /// What the effects that play from a slot's parameters (`Voice::Slot`) are told, by slot
    /// (counted from 0), with the song at `tempo_bpm`. Every slot knob is 0.0 to 1.0 and
    /// means what the same knob of the pedal's own parameters means at that position (the
    /// parameter's own range, so a value that moves from one to the other, as `Memory` moves
    /// it, is heard as it was set)
    pub(crate) fn slot_settings(
        &self,
        layout: &Layout,
        tempo_bpm: Option<f64>,
    ) -> [BlockSettings; MAX_SLOTS] {
        let mut settings = [BlockSettings::None; MAX_SLOTS];
        for voice in layout.voices() {
            if let Voice::Slot { kind, slot, .. } = *voice {
                settings[slot as usize] = self.block_settings(kind, slot as usize, tempo_bpm);
            }
        }
        settings
    }

    /// What a block of `kind` is told from slot `slot` (counted from 0)
    pub(crate) fn block_settings(
        &self,
        kind: EffectKind,
        slot: usize,
        tempo_bpm: Option<f64>,
    ) -> BlockSettings {
        let params = self.slot(slot);
        let knobs = params.knobs().map(|knob| knob.value());
        self.block_settings_from(kind, params.on.value(), knobs, tempo_bpm)
    }

    /// What a block of `kind` is told from a switch and knobs at these positions (0.0 to 1.0)
    pub(crate) fn block_settings_from(
        &self,
        kind: EffectKind,
        on: bool,
        knobs: [f32; KNOBS],
        tempo_bpm: Option<f64>,
    ) -> BlockSettings {
        let [k1, k2, k3, k4, k5] = knobs;
        let p = self;
        match kind {
            EffectKind::Gate => BlockSettings::Gate {
                on,
                threshold_db: p.gate_thresh.preview_plain(k1),
                release_ms: p.gate_release.preview_plain(k2),
            },
            EffectKind::Drive => BlockSettings::Drive {
                on,
                drive: p.drive_gain.preview_plain(k1),
                tone: p.drive_tone.preview_plain(k2),
                level: p.drive_level.preview_plain(k3),
            },
            EffectKind::Comp => BlockSettings::Comp(CompSettings {
                on,
                sustain: p.comp_sustain.preview_plain(k1),
                level: p.comp_level.preview_plain(k2),
            }),
            EffectKind::Fuzz => BlockSettings::Fuzz(FuzzSettings {
                on,
                fuzz: p.fuzz_fuzz.preview_plain(k1),
                tone: p.fuzz_tone.preview_plain(k2),
            }),
            EffectKind::Pitch => BlockSettings::Pitch(PitchSettings {
                on,
                semitones: p.pitch_semitones.preview_plain(k1),
                blend: p.pitch_blend.preview_plain(k2),
                latency_ms: p.pitch_latency.preview_plain(k3),
            }),
            EffectKind::Wah => BlockSettings::Wah(WahSettings {
                on,
                mode: p.wah_mode.preview_plain(k3),
                pedal: p.wah_pedal.preview_plain(k1),
                bite: p.wah_q.preview_plain(k2),
                sens: p.wah_sens.preview_plain(k4),
            }),
            EffectKind::Eq => BlockSettings::Eq(EqSettings {
                on,
                low: p.eq_low.preview_plain(k1),
                mid: p.eq_mid.preview_plain(k2),
                freq: p.eq_freq.preview_plain(k3),
                high: p.eq_high.preview_plain(k4),
            }),
            EffectKind::Chorus => BlockSettings::Chorus(ChorusSettings {
                on,
                rate_hz: p.chorus_rate.preview_plain(k1),
                depth: p.chorus_depth.preview_plain(k2),
            }),
            EffectKind::Flanger => BlockSettings::Flanger(FlangerSettings {
                on,
                rate_hz: p.flanger_rate.preview_plain(k1),
                depth: p.flanger_depth.preview_plain(k2),
            }),
            EffectKind::Phaser => BlockSettings::Phaser(PhaserSettings {
                on,
                rate_hz: p.phaser_rate.preview_plain(k1),
                depth: p.phaser_depth.preview_plain(k2),
            }),
            EffectKind::Tremolo => BlockSettings::Tremolo(TremoloSettings {
                on,
                rate_hz: p.trem_rate.preview_plain(k1),
                depth: p.trem_depth.preview_plain(k2),
            }),
            EffectKind::Delay => BlockSettings::Delay(DelaySettings {
                on,
                time_ms: crate::delay_time_ms(
                    p.delay_time.preview_plain(k1),
                    p.delay_sync.preview_plain(k4),
                    p.delay_note.preview_plain(k5),
                    tempo_bpm,
                ),
                feedback: p.delay_feedback.preview_plain(k2),
                mix: p.delay_mix.preview_plain(k3),
            }),
            EffectKind::Reverb => BlockSettings::Reverb(ReverbSettings {
                on,
                decay_s: p.reverb_decay.preview_plain(k1),
                mix: p.reverb_mix.preview_plain(k2),
            }),
        }
    }
}

/// Where the values of the blocks are read and written: the parameters (`Live`), or whatever
/// stands in for them in a test, since a parameter can only be set through the host
pub trait Values {
    fn read(&self, block: Block) -> SlotValues;
    /// A kind's unused knobs are left alone
    fn write(&self, block: Block, values: SlotValues);
}

/// The parameters of a plugin, set through the host's setter as one edit
pub struct Live<'a> {
    pub params: &'a GuitarAmpParams,
    pub setter: &'a ParamSetter<'a>,
}

impl GuitarAmpParams {
    /// What the block holds now, as the parameters store it. A kind's unused knobs read 0
    pub(crate) fn values_of(&self, block: Block) -> SlotValues {
        let (on, knobs) = self.params_of(block);
        let mut values = SlotValues { on: false, knobs: [0.0; KNOBS] };
        // SAFETY: the pointers are of parameters of `self`
        unsafe {
            values.on = on.unmodulated_normalized_value() > 0.5;
            for (value, knob) in values.knobs.iter_mut().zip(knobs) {
                *value = knob.unmodulated_normalized_value();
            }
        }
        values
    }

    /// The edits that give the block `values`: its switch and the knobs its kind uses
    pub(crate) fn edits_of(&self, block: Block, values: SlotValues) -> Vec<Edit> {
        let (on, knobs) = self.params_of(block);
        let on_value = if values.on { 1.0 } else { 0.0 };
        std::iter::once(Edit::raw(on, on_value))
            .chain(knobs.into_iter().zip(values.knobs).map(|(knob, value)| Edit::raw(knob, value)))
            .collect()
    }
}

impl Values for Live<'_> {
    /// What the block holds now. A kind's unused knobs read 0
    fn read(&self, block: Block) -> SlotValues {
        self.params.values_of(block)
    }

    fn write(&self, block: Block, values: SlotValues) {
        set_together(self.setter, &self.params.edits_of(block, values));
    }
}

impl StoredChain {
    /// Make `chain` the whole chain, as loading a preset does (#271, decision #260). What the
    /// old chain holds of a kind that the new one lacks is kept by kind, as taking its block
    /// out keeps it (the first block of that kind in the line, read before any value is set),
    /// so putting the kind in again brings it back. A kind that stays has its values replaced
    /// by the preset's, which is not taking anything out. Nothing is written to the values
    pub fn replace(&self, values: &impl Values, chain: Chain) {
        let old = self.get();
        for kind in EffectKind::ALL {
            if chain.count_of(kind) > 0 {
                continue;
            }
            if let Some(block) = old.blocks().iter().copied().find(|block| block.kind == kind) {
                self.memory().remember(kind, values.read(block));
            }
        }
        self.put(chain);
    }

    /// Take the block on `slot` out of the chain, keeping its values by kind. A block on its
    /// pedal's own parameters (`Chain::is_home`) is switched off too: the audio reads the twelve
    /// pedals through `Chain::order`, which puts back a pedal the chain lacks, so only its switch
    /// keeps it silent. Putting the kind in again brings the switch back with the rest of the
    /// values. (#268's audio plays the blocks themselves, and then needs no switch)
    pub fn take_out(&self, values: &impl Values, slot: u8) -> Option<Block> {
        let mut chain = self.get();
        let block = chain.remove(slot)?;
        let kept = values.read(block);
        self.memory().remember(block.kind, kept);
        self.put(chain);
        if Chain::is_home(block) && kept.on {
            values.write(block, SlotValues { on: false, ..kept });
        }
        Some(block)
    }

    /// Put a block of `kind` into the chain, with the values the last of its kind had when it
    /// was taken out (and what its slot holds otherwise)
    pub fn put_in(
        &self,
        values: &impl Values,
        kind: EffectKind,
        zone: Zone,
        place: usize,
    ) -> Result<Block, AddError> {
        let mut chain = self.get();
        let slot = chain.add(kind, zone, place)?;
        let block = Block { slot, kind };
        if let Some(remembered) = self.memory().recall(kind) {
            values.write(block, remembered);
        }
        self.put(chain);
        Ok(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::{BTreeMap, HashMap};

    fn kinds(blocks: &[Block]) -> Vec<EffectKind> {
        blocks.iter().map(|block| block.kind).collect()
    }

    #[test]
    fn test_the_default_chain_is_the_twelve_pedals_in_the_order_they_always_had() {
        let chain = Chain::default();
        assert_eq!(chain.order(), ChainOrder::default());
        assert_eq!(chain.blocks().len(), Pedal::ALL.len());
        assert_eq!(chain.amps(), MAX_AMPS);
        use EffectKind::*;
        assert_eq!(kinds(chain.front()), [Pitch, Wah, Fuzz, Comp, Drive, Phaser]);
        assert_eq!(kinds(chain.back()), [Chorus, Flanger, Tremolo, Eq, Delay, Reverb]);
        // Each pedal stands on the slot of its own parameters
        assert!(chain.blocks().iter().all(|&block| Chain::is_home(block)));
    }

    #[test]
    fn test_every_order_of_the_pedals_comes_back_from_the_chain_and_from_its_names() {
        let default = ChainOrder::default();
        let mut orders = vec![default];
        for pedal in Pedal::ALL {
            for zone in [Zone::Front, Zone::Back] {
                for place in 0..8 {
                    orders.push(default.moved(pedal, zone, place));
                }
            }
        }
        for order in orders {
            let chain = Chain::from_order(&order);
            assert_eq!(chain.order(), order);
            let ids = chain.ids();
            assert_eq!(Chain::from_ids(ids.iter().map(String::as_str)), chain);
            assert_eq!(Chain::from_words(chain.to_words()), Some(chain));
        }
    }

    #[test]
    fn test_amp_one_cannot_be_removed_and_the_amps_stop_at_their_number() {
        let mut chain = Chain::empty();
        assert_eq!(chain.amps(), 1);
        assert!(!chain.remove_amp(0));
        assert_eq!(chain.add_amp(), Some(1));
        assert_eq!(chain.add_amp(), None);
        assert!(!chain.remove_amp(0));
        assert!(!chain.remove_amp(5));
        assert!(chain.remove_amp(1));
        assert_eq!(chain.amps(), 1);
        assert!(!chain.remove_amp(0));
        assert!(!chain.remove_amp(1));
    }

    #[test]
    fn test_a_block_that_is_moved_keeps_its_slot_and_stays_in_a_zone_it_may_have() {
        let mut chain = Chain::empty();
        let delay = chain.add(EffectKind::Delay, Zone::Back, 0).unwrap();
        let second = chain.add(EffectKind::Delay, Zone::Back, 1).unwrap();
        let comp = chain.add(EffectKind::Comp, Zone::Front, 0).unwrap();
        let gate = chain.add(EffectKind::Gate, Zone::Front, 1).unwrap();
        // A second delay is on a slot of its own; moving it across the amps keeps that slot
        assert!(chain.move_block(second, Zone::Front, 1).is_ok());
        assert_eq!(chain.front().iter().map(|b| b.slot).collect::<Vec<_>>(), [comp, second, gate]);
        assert_eq!(chain.back().iter().map(|b| b.slot).collect::<Vec<_>>(), [delay]);
        assert!(chain.move_block(second, Zone::Back, 9).is_ok());
        assert_eq!(chain.back().iter().map(|b| b.slot).collect::<Vec<_>>(), [delay, second]);
        assert!(!Chain::is_home(chain.block_on(second).unwrap()));
        // The gate stays in front; a block that is not there changes nothing
        let before = chain;
        assert_eq!(chain.move_block(gate, Zone::Back, 0), Err(AddError::WrongZone));
        assert!(chain.move_block(15, Zone::Back, 0).is_ok());
        assert_eq!(chain, before);
    }

    #[test]
    fn test_sixteen_slots_are_the_limit_and_a_kind_stands_only_where_it_can() {
        let mut chain = Chain::empty();
        for number in 0..MAX_SLOTS {
            let zone = if number % 2 == 0 { Zone::Front } else { Zone::Back };
            // Eight kinds, two of each (`MAX_PER_KIND`)
            let kind = EffectKind::ALL[1 + number / 2];
            let slot = chain.add(kind, zone, 99).unwrap();
            assert!(chain.block_on(slot).is_some());
        }
        assert_eq!(chain.blocks().len(), MAX_SLOTS);
        let mut slots: Vec<u8> = chain.blocks().iter().map(|block| block.slot).collect();
        slots.sort();
        assert_eq!(slots, (0..MAX_SLOTS as u8).collect::<Vec<_>>());
        assert_eq!(chain.add(EffectKind::Comp, Zone::Front, 0), Err(AddError::Full));

        // A third of a kind is refused
        let mut chain = Chain::empty();
        assert!(chain.add(EffectKind::Comp, Zone::Front, 0).is_ok());
        assert!(chain.add(EffectKind::Comp, Zone::Front, 0).is_ok());
        assert_eq!(chain.add(EffectKind::Comp, Zone::Back, 0), Err(AddError::TooMany));

        let mut chain = Chain::empty();
        // Every effect stands on either side (#268); the gate only in front
        assert!(chain.add(EffectKind::Drive, Zone::Back, 0).is_ok());
        chain = Chain::empty();
        assert_eq!(chain.add(EffectKind::Gate, Zone::Back, 0), Err(AddError::WrongZone));
        assert!(chain.blocks().is_empty());
    }

    #[test]
    fn test_blocks_go_in_at_their_place_and_take_the_slot_of_their_own_pedal_first() {
        let mut chain = Chain::empty();
        let delay = chain.add(EffectKind::Delay, Zone::Back, 0).unwrap();
        assert_eq!(delay as usize, Pedal::Delay.index());
        // A second one takes the lowest free slot
        let second = chain.add(EffectKind::Delay, Zone::Back, 0).unwrap();
        assert_eq!(second, 0);
        let drive = chain.add(EffectKind::Drive, Zone::Front, 5).unwrap();
        assert_eq!(chain.front(), [Block { slot: drive, kind: EffectKind::Drive }]);
        assert_eq!(chain.back()[0].slot, second);
        assert_eq!(chain.back()[1].slot, delay);
        // Out and in again: the slot is free for the next one
        assert_eq!(chain.remove(second).map(|block| block.kind), Some(EffectKind::Delay));
        assert_eq!(chain.remove(second), None);
        assert_eq!(chain.back().len(), 1);
        assert_eq!(chain.add(EffectKind::Comp, Zone::Front, 0), Ok(Pedal::Comp.index() as u8));
    }

    #[test]
    fn test_a_damaged_chain_still_opens_as_a_legal_one() {
        let chain = Chain::from_ids([
            "amps=9",
            "comp@1",
            "comp@1",
            "delay@2",
            "mystery@3",
            "fuzz@x",
            "fuzz@0",
            "fuzz@17",
            "amp",
            "drive@4",
            "delay@5",
            "eq@6",
        ]);
        assert_eq!(chain.amps(), MAX_AMPS);
        // The delay named in front of the amp stands there (#268)
        assert_eq!(kinds(chain.front()), [EffectKind::Comp, EffectKind::Delay]);
        // The drive named behind the cabinet stands there too (#268)
        assert_eq!(kinds(chain.back()), [EffectKind::Drive, EffectKind::Delay, EffectKind::Eq]);
        // No amp in it: damaged
        assert_eq!(Chain::from_ids(["comp@1"]), Chain::default());
        assert_eq!(Chain::from_ids([]), Chain::default());
        // Words no chain has
        for words in [[u64::MAX; 3], [0, 0, 5], [0x10, 0x10, 0x100]] {
            assert_eq!(Chain::from_words(words), None, "{words:x?}");
        }
        // More than sixteen are cut at the sixteenth
        let many: Vec<String> = (0..40)
            .map(|n| format!("{}@{}", EffectKind::ALL[1 + n % 8].id(), n % 16 + 1))
            .chain(["amp".to_string()])
            .collect();
        assert_eq!(Chain::from_ids(many.iter().map(String::as_str)).blocks().len(), MAX_SLOTS);
    }

    #[test]
    fn test_the_stored_chain_starts_at_the_default_and_survives_saving_with_its_memory() {
        let stored = StoredChain::new();
        assert_eq!(stored.get(), Chain::default());
        assert_eq!(stored.order(), ChainOrder::default());

        let mut chain = Chain::default();
        chain.remove(Pedal::Wah.index() as u8);
        chain.add(EffectKind::Delay, Zone::Back, 0).unwrap();
        assert_eq!(chain.add(EffectKind::Gate, Zone::Back, 2), Err(AddError::WrongZone));
        stored.put(chain);
        let values = SlotValues { on: true, knobs: [0.1, 0.25, 0.5, 0.75, 1.0 / 3.0] };
        stored.memory().remember(EffectKind::Wah, values);
        let saved = stored.map(|ids| ids.clone());

        let restored = StoredChain::new();
        restored.set(saved.clone());
        assert_eq!(restored.get(), chain);
        assert_eq!(restored.memory().recall(EffectKind::Wah), Some(values));
        assert_eq!(restored.memory().recall(EffectKind::Eq), None);
        assert_eq!(restored.map(|ids| ids.clone()), saved);

        // Words no chain has, from a damaged state, read as the default
        stored.inner.words[2].store(0xffff, Ordering::Relaxed);
        assert_eq!(stored.get(), Chain::default());
    }

    fn list(ids: &[&str]) -> String {
        format!("[{}]", ids.iter().map(|id| format!("\"{id}\"")).collect::<Vec<_>>().join(","))
    }

    #[test]
    fn test_a_project_from_before_opens_with_the_same_twelve_pedals_in_their_saved_order() {
        let params = GuitarAmpParams::default();
        let order = ChainOrder::default().moved(Pedal::Tremolo, Zone::Front, 0).moved(
            Pedal::Reverb,
            Zone::Back,
            0,
        );
        // What a build of before saved: `chain-order` and no `chain`
        let old = BTreeMap::from([("chain-order".to_string(), list(&order.ids()))]);
        params.deserialize_fields(&old);
        assert_eq!(params.chain.order(), order);
        assert_eq!(params.chain.get(), Chain::from_order(&order));

        // A project with neither keeps the default, and one of this build saves and opens the same
        let params = GuitarAmpParams::default();
        params.deserialize_fields(&BTreeMap::new());
        assert_eq!(params.chain.get(), Chain::default());
        let mut chain = Chain::default();
        chain.remove(Pedal::Eq.index() as u8);
        params.chain.put(chain);
        let saved = params.serialize_fields();
        assert!(saved.contains_key("chain") && saved.contains_key("chain-order"));
        let opened = GuitarAmpParams::default();
        opened.deserialize_fields(&saved);
        assert_eq!(opened.chain.get(), chain);
        // The older key of the same state did not put the twelve back, nor does a second load
        opened.deserialize_fields(&saved);
        assert_eq!(opened.chain.get(), chain);
        // And a project of before, after one of now, still opens as it was
        opened.deserialize_fields(&old);
        assert_eq!(opened.chain.get(), Chain::from_order(&order));

        // A state with a `chain` and no `chain-order`, then one of before
        let mut chain_only = saved.clone();
        chain_only.remove("chain-order");
        let params = GuitarAmpParams::default();
        params.deserialize_fields(&chain_only);
        assert_eq!(params.chain.get(), chain);
        params.chain.put(Chain::default());
        params.deserialize_fields(&old);
        assert_eq!(params.chain.get(), Chain::from_order(&order));
    }

    #[test]
    fn test_the_slots_have_generic_names_and_every_pedal_has_its_knobs() {
        let params = GuitarAmpParams::default();
        assert_eq!(params.slot(2).k2.name(), "Slot 3 Knob 2");
        assert_eq!(params.slot(15).on.name(), "Slot 16 On");
        assert_eq!(params.slots().len(), MAX_SLOTS);
        for kind in EffectKind::ALL {
            let (_, knobs) = pedal_params(&params, kind);
            assert_eq!(knobs.len(), kind.knob_count(), "{kind:?}");
        }
        // K is the widest pedal
        assert_eq!(EffectKind::ALL.into_iter().map(EffectKind::knob_count).max(), Some(KNOBS));
    }

    /// Stands in for the parameters: the values of a block that stands on its pedal's slot are
    /// kept apart from those of a block on a slot of its own, as the parameters keep them
    #[derive(Default)]
    struct Store(RefCell<HashMap<(bool, u8), SlotValues>>);

    impl Values for Store {
        fn read(&self, block: Block) -> SlotValues {
            let key = (Chain::is_home(block), block.slot);
            self.0
                .borrow()
                .get(&key)
                .copied()
                .unwrap_or(SlotValues { on: false, knobs: [0.5; KNOBS] })
        }
        fn write(&self, block: Block, values: SlotValues) {
            self.0.borrow_mut().insert((Chain::is_home(block), block.slot), values);
        }
    }

    fn block(kind: EffectKind, slot: u8) -> Block {
        Block { slot, kind }
    }

    #[test]
    fn test_a_duplicate_effect_keeps_its_own_settings() {
        let (chain, store) = (StoredChain::new(), Store::default());
        // The chorus is in the chain on its own slot, so another one is a duplicate, on the
        // lowest free slot: the thirteenth
        let home = block(EffectKind::Chorus, Pedal::Chorus.index() as u8);
        let second = chain.put_in(&store, EffectKind::Chorus, Zone::Back, 0).unwrap();
        assert_eq!(second.slot as usize, Pedal::ALL.len());
        assert_eq!(chain.get().blocks().len(), Pedal::ALL.len() + 1);
        let (a, b) = (
            SlotValues { on: true, knobs: [0.2, 0.9, 0.0, 0.0, 0.0] },
            SlotValues { on: false, knobs: [0.7, 0.1, 0.0, 0.0, 0.0] },
        );
        store.write(home, a);
        store.write(second, b);
        assert_eq!(store.read(home), a);
        assert_eq!(store.read(second), b);
        // Taking the duplicate out leaves the first as it is
        assert_eq!(chain.take_out(&store, second.slot), Some(second));
        assert_eq!(store.read(home), a);
        assert_eq!(chain.get().blocks().len(), Pedal::ALL.len());
    }

    #[test]
    fn test_taking_a_block_out_and_putting_it_in_again_brings_its_settings_back() {
        let (chain, store) = (StoredChain::new(), Store::default());
        let delay = block(EffectKind::Delay, Pedal::Delay.index() as u8);
        let values = SlotValues { on: true, knobs: [0.3, 0.6, 0.4, 1.0, 0.75] };
        store.write(delay, values);
        assert_eq!(chain.take_out(&store, delay.slot), Some(delay));
        assert_eq!(chain.take_out(&store, delay.slot), None);
        assert!(!chain.get().blocks().contains(&delay));
        // The slot is turned while the pedal is out
        store.write(delay, SlotValues { on: false, knobs: [0.0; KNOBS] });
        let again = chain.put_in(&store, EffectKind::Delay, Zone::Back, 0).unwrap();
        assert_eq!(again, delay);
        assert_eq!(store.read(again), values);

        // A duplicate on a slot of its own comes back with its values on whichever slot it lands
        let extra = chain.put_in(&store, EffectKind::Delay, Zone::Back, 0).unwrap();
        assert_eq!(extra.slot as usize, Pedal::ALL.len());
        let other = SlotValues { on: true, knobs: [0.9, 0.8, 0.7, 0.0, 0.5] };
        store.write(extra, other);
        assert_eq!(chain.take_out(&store, extra.slot), Some(extra));
        store.write(extra, SlotValues { on: false, knobs: [0.0; KNOBS] });
        let back = chain.put_in(&store, EffectKind::Delay, Zone::Back, 3).unwrap();
        assert_eq!(back, extra);
        assert_eq!(store.read(back), other);
        assert_eq!(chain.get().back()[3], back);
        // What a block of another kind left is not what a block of this one gets
        assert_eq!(chain.memory().recall(EffectKind::Wah), None);
    }

    #[test]
    fn test_replacing_the_chain_keeps_the_kinds_it_takes_out_and_only_those() {
        let (chain, store) = (StoredChain::new(), Store::default());
        let (comp, delay) = (
            block(EffectKind::Comp, Pedal::Comp.index() as u8),
            block(EffectKind::Delay, Pedal::Delay.index() as u8),
        );
        // A chain of the player's: a gate on a slot of its own, the comp, two delays
        let mine = Chain::from_ids(["amps=2", "gate@13", "comp@1", "amp", "delay@7", "delay@14"]);
        chain.put(mine);
        let gate = block(EffectKind::Gate, 12);
        let second = block(EffectKind::Delay, 13);
        let values = |at: f32| SlotValues { on: at > 0.5, knobs: [at, at / 2.0, 0.1, 0.2, 0.3] };
        for (block, at) in [(gate, 0.9), (comp, 0.3), (delay, 0.6), (second, 0.8)] {
            store.write(block, values(at));
        }
        // A preset that keeps the comp and has neither the gate nor a delay
        let theirs = Chain::from_ids(["amps=1", "comp@1", "amp", "reverb@8"]);
        chain.replace(&store, theirs);
        assert_eq!(chain.get(), theirs);
        let memory = chain.memory();
        assert_eq!(memory.recall(EffectKind::Gate), Some(values(0.9)));
        // Of two delays the first in the line is kept
        assert_eq!(memory.recall(EffectKind::Delay), Some(values(0.6)));
        // A kind that stays is not taken out, and a kind that was not there is not kept
        assert_eq!(memory.recall(EffectKind::Comp), None);
        assert_eq!(memory.recall(EffectKind::Reverb), None);
        // The values themselves are not touched by the replace
        assert_eq!(store.read(comp), values(0.3));
        // Back again: the gate and the delay come with what they had
        let put = chain.put_in(&store, EffectKind::Gate, Zone::Front, 0).unwrap();
        assert_eq!(store.read(put), values(0.9));
        let put = chain.put_in(&store, EffectKind::Delay, Zone::Back, 0).unwrap();
        assert_eq!(store.read(put), values(0.6));
        // Replacing with the chain that is there takes nothing out
        let again = StoredChain::new();
        again.replace(&store, Chain::default());
        assert!(EffectKind::ALL.iter().all(|&kind| again.memory().recall(kind).is_none()));
    }

    #[test]
    fn test_a_pedal_taken_out_is_switched_off_and_comes_back_switched_on() {
        // The audio plays the twelve pedals by their own switches (`Chain::order` puts a pedal the
        // chain lacks back in its place), so a pedal taken out must be off to be silent
        let (chain, store) = (StoredChain::new(), Store::default());
        let comp = block(EffectKind::Comp, Pedal::Comp.index() as u8);
        let values = SlotValues { on: true, knobs: [0.3, 0.6, 0.0, 0.0, 0.0] };
        store.write(comp, values);
        chain.take_out(&store, comp.slot).unwrap();
        assert!(!store.read(comp).on);
        assert_eq!(store.read(comp).knobs, values.knobs);
        assert_eq!(chain.memory().recall(EffectKind::Comp), Some(values));
        chain.put_in(&store, EffectKind::Comp, Zone::Front, 2).unwrap();
        assert_eq!(store.read(comp), values);
        // A duplicate on a slot of its own is left as it is: the audio does not play it (yet)
        let extra = chain.put_in(&store, EffectKind::Comp, Zone::Front, 0).unwrap();
        store.write(extra, values);
        chain.take_out(&store, extra.slot).unwrap();
        assert_eq!(store.read(extra), values);
    }

    /// The parameters are set as one edit, a pedal's own for a block on its slot and the
    /// slot's for any other, and the amp's other parameters are left alone
    #[test]
    fn test_the_values_go_to_the_pedals_own_parameters_or_to_the_slots_through_the_host() {
        use crate::user_presets::tests::{as_one_edit, Host};
        let params = GuitarAmpParams::default();
        let host = Host::default();
        let setter = ParamSetter::new(&host);
        let live = Live { params: &params, setter: &setter };
        let ptrs = |block| params.params_of(block);

        let delay = block(EffectKind::Delay, Pedal::Delay.index() as u8);
        let (on, knobs) = ptrs(delay);
        assert!(on == params.delay_on.as_ptr() && knobs[0] == params.delay_time.as_ptr());
        assert_eq!(knobs.len(), 5);
        let duplicate = block(EffectKind::Delay, 12);
        let (on, knobs) = ptrs(duplicate);
        assert!(on == params.slot13.on.as_ptr() && knobs[4] == params.slot13.k5.as_ptr());
        assert_eq!(ptrs(block(EffectKind::Reverb, 11)).1.len(), 2);

        let chain = StoredChain::new();
        chain.take_out(&live, delay.slot);
        chain.memory().remember(EffectKind::Delay, SlotValues { on: true, knobs: [0.5; KNOBS] });
        host.told.lock().unwrap().clear();
        chain.put_in(&live, EffectKind::Delay, Zone::Back, 0).unwrap();
        let told = host.told.lock().unwrap();
        assert!(as_one_edit(&told));
        assert_eq!(told.len(), 3 * 6);
        assert!(told.iter().any(|&(_, ptr)| ptr == params.delay_sync.as_ptr()));
        assert!(told.iter().all(|&(_, ptr)| ptr != params.reverb_on.as_ptr()));
    }

    /// What a block is told, as the numbers of its knobs in the order of the slot's
    fn numbers(told: BlockSettings) -> Vec<f32> {
        match told {
            BlockSettings::None => vec![],
            BlockSettings::Gate { threshold_db, release_ms, .. } => vec![threshold_db, release_ms],
            BlockSettings::Drive { drive, tone, level, .. } => vec![drive, tone, level],
            BlockSettings::Comp(s) => vec![s.sustain, s.level],
            BlockSettings::Fuzz(s) => vec![s.fuzz, s.tone],
            BlockSettings::Pitch(s) => vec![s.semitones as f32, s.blend, s.latency_ms],
            BlockSettings::Wah(s) => vec![s.pedal, s.bite, s.mode as usize as f32, s.sens],
            BlockSettings::Eq(s) => vec![s.low, s.mid, s.freq, s.high],
            BlockSettings::Chorus(s) => vec![s.rate_hz, s.depth],
            BlockSettings::Flanger(s) => vec![s.rate_hz, s.depth],
            BlockSettings::Phaser(s) => vec![s.rate_hz, s.depth],
            BlockSettings::Tremolo(s) => vec![s.rate_hz, s.depth],
            BlockSettings::Delay(s) => vec![s.time_ms, s.feedback, s.mix],
            BlockSettings::Reverb(s) => vec![s.decay_s, s.mix],
        }
    }

    /// A slot's knobs mean what the same knobs of the pedal's own parameters do at that
    /// position, in the same order (a value that moves between the two, as `Memory` moves it, is
    /// heard as it was set): every kind, each knob at a position of its own
    #[test]
    fn test_a_slot_knob_means_what_the_pedals_own_knob_does_at_that_position() {
        let params = GuitarAmpParams::default();
        for kind in EffectKind::ALL {
            let (on, own) = pedal_params(&params, kind);
            assert!(on != params.slot(0).on.as_ptr());
            for step in 0..=8usize {
                let mut knobs = [0.0; KNOBS];
                for (index, knob) in knobs.iter_mut().enumerate() {
                    *knob = ((step + 3 * index) % 9) as f32 / 8.0;
                }
                let told = params.block_settings_from(kind, true, knobs, None);
                assert!(told.is_on());
                let heard = numbers(told);
                let mut expected: Vec<f32> = own
                    .iter()
                    .zip(knobs)
                    // SAFETY: the pointers are of parameters of `params`
                    .map(|(ptr, position)| unsafe { ptr.preview_plain(position) })
                    .collect();
                // The delay's Sync and Note are no number of its settings
                expected.truncate(heard.len());
                assert_eq!(heard, expected, "{kind:?} step {step}");
            }
            assert!(!params.block_settings_from(kind, false, [0.5; KNOBS], None).is_on());
        }
    }

    /// With Sync on the time is the note's at the song's tempo, as for the pedal
    #[test]
    fn test_a_slot_delay_follows_the_tempo_with_sync_on() {
        let params = GuitarAmpParams::default();
        let knobs = [0.5, 0.4, 0.3, 1.0, 0.0];
        let told = |tempo| match params.block_settings_from(EffectKind::Delay, true, knobs, tempo) {
            BlockSettings::Delay(delay) => delay.time_ms,
            other => panic!("{other:?}"),
        };
        assert_eq!(told(None), told(None));
        assert!(told(Some(60.0)) != told(Some(120.0)));
        // Without Sync the tempo does not matter
        let free = |tempo| match params.block_settings_from(
            EffectKind::Delay,
            true,
            [0.5, 0.4, 0.3, 0.0, 0.0],
            tempo,
        ) {
            BlockSettings::Delay(delay) => delay.time_ms,
            other => panic!("{other:?}"),
        };
        assert_eq!(free(Some(60.0)), free(Some(120.0)));
    }

    const SPARE: usize = 12;

    /// A slot's values are read for the blocks that stand on them, and for no others
    #[test]
    fn test_only_the_slots_of_blocks_off_their_pedal_are_read() {
        let params = GuitarAmpParams::default();
        let mut chain = Chain::default();
        let layout = Layout::of(&chain);
        assert!(params.slot_settings(&layout, None).iter().all(|s| *s == BlockSettings::None));
        let slot = chain.add(EffectKind::Reverb, Zone::Back, 0).unwrap();
        assert_eq!(slot as usize, SPARE, "the first slot that is no pedal's own");
        let layout = Layout::of(&chain);
        let told = params.slot_settings(&layout, None);
        assert!(matches!(told[slot as usize], BlockSettings::Reverb(_)));
        assert_eq!(told.iter().filter(|s| **s != BlockSettings::None).count(), 1);
    }
}
