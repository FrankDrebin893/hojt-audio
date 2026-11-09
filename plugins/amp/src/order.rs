//! The order the pedals stand in: which of them are in front of the amp and which behind
//! the cabinet, and in what order within each of the two.
//!
//! Not every pedal can stand everywhere. The gate is always first, since it listens to the
//! guitar as it arrives, and the amp and its cabinet are one block in the middle: neither is
//! part of the order. The drive pedal, the pitch shifter and the wah stay in front of the amp,
//! where they share the amp's oversampling. The fuzz, the compressor, the modulation pedals, the EQ,
//! the delay and the reverb go either side (the delay and the reverb run on one channel in front
//! of the amp, #268).
//!
//! The order is not a parameter: a host cannot automate an arrangement. It is the twelve-pedal
//! view of the chain model (`slots::Chain`, which the project saves): the editor changes it
//! through `StoredChain`, and the chain reads it with the dials.

/// The pedals that can be moved
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pedal {
    Comp,
    Drive,
    Chorus,
    Phaser,
    Flanger,
    Tremolo,
    Delay,
    Reverb,
    Fuzz,
    Pitch,
    Eq,
    Wah,
}

/// The two stretches of the chain a pedal can stand in
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    /// Between the gate and the amp: one channel, at the amp's oversampled rate
    Front,
    /// Behind the cabinet: two channels, at the host's rate
    Back,
}

impl Pedal {
    pub const ALL: [Pedal; 12] = [
        Pedal::Comp,
        Pedal::Drive,
        Pedal::Chorus,
        Pedal::Phaser,
        Pedal::Flanger,
        Pedal::Tremolo,
        Pedal::Delay,
        Pedal::Reverb,
        Pedal::Fuzz,
        Pedal::Pitch,
        Pedal::Eq,
        Pedal::Wah,
    ];

    /// Frozen: what a saved project calls the pedal
    pub fn id(self) -> &'static str {
        match self {
            Pedal::Comp => "comp",
            Pedal::Drive => "drive",
            Pedal::Chorus => "chorus",
            Pedal::Phaser => "phaser",
            Pedal::Flanger => "flanger",
            Pedal::Tremolo => "tremolo",
            Pedal::Delay => "delay",
            Pedal::Reverb => "reverb",
            Pedal::Fuzz => "fuzz",
            Pedal::Pitch => "pitch",
            Pedal::Eq => "eq",
            Pedal::Wah => "wah",
        }
    }

    fn from_id(id: &str) -> Option<Pedal> {
        Pedal::ALL.into_iter().find(|pedal| pedal.id() == id)
    }

    /// Position in `ALL`
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn can_be_in(self, zone: Zone) -> bool {
        match self {
            Pedal::Drive | Pedal::Pitch | Pedal::Wah => zone == Zone::Front,
            Pedal::Delay
            | Pedal::Reverb
            | Pedal::Fuzz
            | Pedal::Comp
            | Pedal::Chorus
            | Pedal::Phaser
            | Pedal::Flanger
            | Pedal::Tremolo
            | Pedal::Eq => true,
        }
    }
}

/// What a saved project calls the amp and its cabinet, between the two zones
pub const AMP_ID: &str = "amp";

/// Every pedal once: the first `front` of them in front of the amp in that order, the rest
/// behind the cabinet in theirs. Always a legal arrangement
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChainOrder {
    pedals: [Pedal; Pedal::ALL.len()],
    front: usize,
}

/// The chain as it was before it could be rearranged, with the pedals that came later
/// where they are used most: the pitch shifter first, since it follows a note best as the
/// guitar plays it (the gate stays in front of everything), the wah after it, ahead of the
/// fuzz (a wah in front of a drive or a fuzz is the sound), then the fuzz, where it hears
/// the guitar as it is, then the compressor, and the phaser in front of the amp; the other
/// modulation behind the cabinet in front of the delay, and the EQ after them, where a room
/// or a speaker is put right
impl Default for ChainOrder {
    fn default() -> Self {
        Self {
            pedals: [
                Pedal::Pitch,
                Pedal::Wah,
                Pedal::Fuzz,
                Pedal::Comp,
                Pedal::Drive,
                Pedal::Phaser,
                Pedal::Chorus,
                Pedal::Flanger,
                Pedal::Tremolo,
                Pedal::Eq,
                Pedal::Delay,
                Pedal::Reverb,
            ],
            front: 6,
        }
    }
}

impl ChainOrder {
    /// In front of the amp, first to last
    pub fn front(&self) -> &[Pedal] {
        &self.pedals[..self.front]
    }

    /// Behind the cabinet, first to last
    pub fn back(&self) -> &[Pedal] {
        &self.pedals[self.front..]
    }

    pub fn zone_of(&self, pedal: Pedal) -> Zone {
        if self.front().contains(&pedal) {
            Zone::Front
        } else {
            Zone::Back
        }
    }

    /// The order with `pedal` taken out and put back as number `place` of `zone` (0 is
    /// first; past the end is last). A pedal that cannot stand in that zone stays where it is
    // For the tests of the chain: the board works out a whole row and hands it over as ids
    #[cfg(test)]
    pub fn moved(&self, pedal: Pedal, zone: Zone, place: usize) -> ChainOrder {
        if !pedal.can_be_in(zone) {
            return *self;
        }
        let mut rows = [Row::new(), Row::new()];
        for (row, pedals) in rows.iter_mut().zip([self.front(), self.back()]) {
            for &other in pedals.iter().filter(|&&other| other != pedal) {
                row.push(other);
            }
        }
        let row = &mut rows[zone as usize];
        row.insert(place.min(row.len), pedal);
        Self::from_rows(&rows)
    }

    /// From two rows that hold every pedal once between them
    fn from_rows(rows: &[Row; 2]) -> ChainOrder {
        let [front, back] = rows;
        debug_assert_eq!(front.len + back.len, Pedal::ALL.len());
        let mut pedals = [Pedal::Comp; Pedal::ALL.len()];
        for (slot, &pedal) in pedals.iter_mut().zip(front.pedals().iter().chain(back.pedals())) {
            *slot = pedal;
        }
        ChainOrder { pedals, front: front.len }
    }

    /// The order a saved project names: the ids of the pedals with `amp` between the two
    /// zones. Whatever is wrong with the list is put right and not refused, since a project
    /// has to open: a name this build does not know is skipped, a pedal named twice counts
    /// once, a pedal on the side it cannot stand on and one that is not named at all go to
    /// their zone of the default order, behind the pedals that are in front of them there
    pub fn from_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> ChainOrder {
        let default = ChainOrder::default();
        let mut rows = [Row::new(), Row::new()];
        let mut zone = Zone::Front;
        let mut named = [false; Pedal::ALL.len()];
        for id in ids {
            if id == AMP_ID {
                zone = Zone::Back;
            } else if let Some(pedal) = Pedal::from_id(id) {
                if std::mem::replace(&mut named[pedal.index()], true) {
                    continue;
                }
                if pedal.can_be_in(zone) {
                    rows[zone as usize].push(pedal);
                }
            }
        }

        // The pedals without a place, in the order of the default: each goes behind the
        // last of those that the default has in front of it in its zone
        for pedal in default.pedals {
            if rows.iter().any(|row| row.pedals().contains(&pedal)) {
                continue;
            }
            let zone = default.zone_of(pedal);
            let ahead = if zone == Zone::Front { default.front() } else { default.back() };
            let ahead = &ahead[..ahead.iter().position(|&other| other == pedal).unwrap_or(0)];
            let row = &mut rows[zone as usize];
            let place = row
                .pedals()
                .iter()
                .rposition(|other| ahead.contains(other))
                .map_or(0, |last| last + 1);
            row.insert(place, pedal);
        }
        Self::from_rows(&rows)
    }

    /// What a saved project names the order as: see `from_ids`
    pub fn ids(&self) -> Vec<&'static str> {
        let front = self.front().iter().map(|pedal| pedal.id());
        let back = self.back().iter().map(|pedal| pedal.id());
        front.chain(std::iter::once(AMP_ID)).chain(back).collect()
    }
}

/// One of the two rows while an order is put together. Holds no memory of its own: an order
/// may be restored from a saved project on the audio thread
#[derive(Clone, Copy)]
struct Row {
    slots: [Pedal; Pedal::ALL.len()],
    len: usize,
}

impl Row {
    fn new() -> Self {
        Self { slots: [Pedal::Comp; Pedal::ALL.len()], len: 0 }
    }

    fn pedals(&self) -> &[Pedal] {
        &self.slots[..self.len]
    }

    fn push(&mut self, pedal: Pedal) {
        self.insert(self.len, pedal);
    }

    /// Does nothing once the row holds every pedal there is
    fn insert(&mut self, place: usize, pedal: Pedal) {
        if self.len < self.slots.len() {
            self.slots.copy_within(place..self.len, place + 1);
            self.slots[place] = pedal;
            self.len += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_legal(order: &ChainOrder) -> bool {
        let every_pedal_once = Pedal::ALL
            .iter()
            .all(|pedal| order.pedals.iter().filter(|other| *other == pedal).count() == 1);
        every_pedal_once
            && order.front().iter().all(|pedal| pedal.can_be_in(Zone::Front))
            && order.back().iter().all(|pedal| pedal.can_be_in(Zone::Back))
    }

    #[test]
    fn test_default_is_the_chain_as_it_was_with_the_new_pedals_around_it() {
        let order = ChainOrder::default();
        assert!(is_legal(&order));
        assert_eq!(
            order.front(),
            [Pedal::Pitch, Pedal::Wah, Pedal::Fuzz, Pedal::Comp, Pedal::Drive, Pedal::Phaser]
        );
        assert_eq!(
            order.back(),
            [Pedal::Chorus, Pedal::Flanger, Pedal::Tremolo, Pedal::Eq, Pedal::Delay, Pedal::Reverb]
        );
        // The pedals the chain had before keep their order: drive, amp, delay, reverb
        let before = |first: Pedal, second: Pedal| {
            let place = |pedal| order.pedals.iter().position(|&other| other == pedal).unwrap();
            place(first) < place(second)
        };
        assert!(order.zone_of(Pedal::Drive) == Zone::Front && before(Pedal::Delay, Pedal::Reverb));
        assert_eq!(
            order.ids(),
            [
                "pitch", "wah", "fuzz", "comp", "drive", "phaser", "amp", "chorus", "flanger",
                "tremolo", "eq", "delay", "reverb"
            ]
        );
    }

    #[test]
    fn test_ids_are_unique_and_known() {
        for pedal in Pedal::ALL {
            assert_eq!(Pedal::from_id(pedal.id()), Some(pedal));
            assert_eq!(Pedal::ALL[pedal.index()], pedal);
            assert_ne!(pedal.id(), AMP_ID);
        }
        assert_eq!(Pedal::from_id("wah"), Some(Pedal::Wah));
        assert_eq!(Pedal::from_id("octaver"), None);
        // At the end, so that the number a saved order is kept as stays the same for the rest
        assert_eq!(Pedal::ALL.last(), Some(&Pedal::Wah));
    }

    #[test]
    fn test_a_pedal_moves_within_its_zone_and_across_the_amp() {
        let order = ChainOrder::default();

        let chorus_first = order.moved(Pedal::Chorus, Zone::Front, 0);
        assert_eq!(
            chorus_first.front(),
            [
                Pedal::Chorus,
                Pedal::Pitch,
                Pedal::Wah,
                Pedal::Fuzz,
                Pedal::Comp,
                Pedal::Drive,
                Pedal::Phaser
            ]
        );
        assert_eq!(
            chorus_first.back(),
            [Pedal::Flanger, Pedal::Tremolo, Pedal::Eq, Pedal::Delay, Pedal::Reverb]
        );

        let reverb_into_delay = order.moved(Pedal::Reverb, Zone::Back, 4);
        assert_eq!(
            reverb_into_delay.back(),
            [Pedal::Chorus, Pedal::Flanger, Pedal::Tremolo, Pedal::Eq, Pedal::Reverb, Pedal::Delay]
        );

        // The EQ stands on either side: in front of the amp, last before it, and first behind
        let eq_in_front = order.moved(Pedal::Eq, Zone::Front, 99);
        assert_eq!(
            eq_in_front.front(),
            [
                Pedal::Pitch,
                Pedal::Wah,
                Pedal::Fuzz,
                Pedal::Comp,
                Pedal::Drive,
                Pedal::Phaser,
                Pedal::Eq
            ]
        );
        assert_eq!(
            eq_in_front.back(),
            [Pedal::Chorus, Pedal::Flanger, Pedal::Tremolo, Pedal::Delay, Pedal::Reverb]
        );
        let eq_first_behind = eq_in_front.moved(Pedal::Eq, Zone::Back, 0);
        assert_eq!(eq_first_behind.back()[0], Pedal::Eq);
        assert_eq!(eq_first_behind.front().len(), 6);
        assert!(is_legal(&eq_in_front) && is_legal(&eq_first_behind));

        let comp_behind_drive = order.moved(Pedal::Comp, Zone::Front, 4);
        assert_eq!(
            comp_behind_drive.front(),
            [Pedal::Pitch, Pedal::Wah, Pedal::Fuzz, Pedal::Drive, Pedal::Comp, Pedal::Phaser]
        );

        let fuzz_last = order.moved(Pedal::Fuzz, Zone::Back, 99);
        assert_eq!(
            fuzz_last.front(),
            [Pedal::Pitch, Pedal::Wah, Pedal::Comp, Pedal::Drive, Pedal::Phaser]
        );
        assert_eq!(fuzz_last.back().last(), Some(&Pedal::Fuzz));

        // Past the end is last, and where a pedal already stands is no move
        assert_eq!(order.moved(Pedal::Comp, Zone::Back, 99).back().last(), Some(&Pedal::Comp));
        assert_eq!(order.moved(Pedal::Drive, Zone::Front, 4), order);
        for moved in [chorus_first, reverb_into_delay, comp_behind_drive, fuzz_last] {
            assert!(is_legal(&moved));
        }
    }

    #[test]
    fn test_no_move_takes_a_pedal_where_it_cannot_stand() {
        let order = ChainOrder::default();
        assert_eq!(order.moved(Pedal::Drive, Zone::Back, 0), order);
        assert_eq!(order.moved(Pedal::Pitch, Zone::Back, 0), order);
        assert_eq!(order.moved(Pedal::Wah, Zone::Back, 0), order);
        assert_eq!(order.moved(Pedal::Wah, Zone::Back, 99), order);

        // Every move from every order that a few moves lead to
        let mut orders = vec![order];
        for _ in 0..2 {
            for from in orders.clone() {
                for pedal in Pedal::ALL {
                    for zone in [Zone::Front, Zone::Back] {
                        for place in 0..=Pedal::ALL.len() {
                            let moved = from.moved(pedal, zone, place);
                            assert!(is_legal(&moved), "{from:?}: {pedal:?} to {zone:?} {place}");
                            if !orders.contains(&moved) {
                                orders.push(moved);
                            }
                        }
                    }
                }
            }
        }
        assert!(orders.len() > 100);

        // And each of them comes back from its names as it was
        for order in orders {
            assert_eq!(ChainOrder::from_ids(order.ids()), order);
        }
    }

    #[test]
    fn test_a_damaged_list_still_gives_a_legal_order() {
        let default = ChainOrder::default();
        // No list at all, and a list from a build with fewer pedals: the pedals of before
        // where the project had them, the others where the default has them
        assert_eq!(ChainOrder::from_ids([]), default);
        let older = ChainOrder::from_ids(["drive", "amp", "reverb", "delay"]);
        assert_eq!(
            older.front(),
            [Pedal::Pitch, Pedal::Wah, Pedal::Fuzz, Pedal::Comp, Pedal::Drive, Pedal::Phaser]
        );
        // A project saved before there was a fuzz: the pedals where it had them, and the fuzz
        // first, where the default has it
        let before_fuzz =
            ["comp", "drive", "phaser", "amp", "chorus", "flanger", "tremolo", "delay", "reverb"];
        assert_eq!(ChainOrder::from_ids(before_fuzz), default);
        // And one saved before the pitch shifter: it is first in front of the amp
        let before_pitch = [
            "fuzz", "comp", "drive", "phaser", "amp", "chorus", "flanger", "tremolo", "delay",
            "reverb",
        ];
        assert_eq!(ChainOrder::from_ids(before_pitch), default);
        let moved = ChainOrder::from_ids(["drive", "comp", "amp", "reverb", "phaser"]);
        assert_eq!(
            moved.front(),
            [Pedal::Pitch, Pedal::Wah, Pedal::Fuzz, Pedal::Drive, Pedal::Comp]
        );
        assert_eq!(
            older.back(),
            [Pedal::Chorus, Pedal::Flanger, Pedal::Tremolo, Pedal::Eq, Pedal::Reverb, Pedal::Delay]
        );
        // And one saved before the EQ: it is behind the tremolo and in front of the delay,
        // where the default has it, and a project that moved the pedals around keeps them
        let before_eq = [
            "pitch", "fuzz", "comp", "drive", "phaser", "amp", "chorus", "flanger", "tremolo",
            "delay", "reverb",
        ];
        assert_eq!(ChainOrder::from_ids(before_eq), default);
        let moved_about = ChainOrder::from_ids([
            "drive", "pitch", "amp", "delay", "tremolo", "chorus", "reverb", "flanger", "comp",
            "fuzz", "phaser",
        ]);
        assert_eq!(
            moved_about.back(),
            [
                Pedal::Delay,
                Pedal::Tremolo,
                Pedal::Chorus,
                Pedal::Reverb,
                Pedal::Flanger,
                Pedal::Eq,
                Pedal::Comp,
                Pedal::Fuzz,
                Pedal::Phaser
            ]
        );
        assert_eq!(moved_about.front(), [Pedal::Drive, Pedal::Pitch, Pedal::Wah]);
        // And one saved before the wah: it is behind the pitch shifter, where the default has it,
        // wherever the project put that, and the pedals that were moved stay where they were
        let before_wah = [
            "pitch", "fuzz", "comp", "drive", "phaser", "amp", "chorus", "flanger", "tremolo",
            "eq", "delay", "reverb",
        ];
        assert_eq!(ChainOrder::from_ids(before_wah), default);
        let pitch_later = ChainOrder::from_ids(["fuzz", "pitch", "drive", "amp", "delay"]);
        assert_eq!(&pitch_later.front()[..3], [Pedal::Fuzz, Pedal::Pitch, Pedal::Wah]);
        // A wah named behind the cabinet cannot stand there: it goes in front of the amp
        let wah_behind = ChainOrder::from_ids(["amp", "wah", "delay"]);
        assert_eq!(wah_behind.zone_of(Pedal::Wah), Zone::Front);
        assert!(is_legal(&wah_behind));
        // The EQ named in front of the amp stays there
        let eq_first = ChainOrder::from_ids(["eq", "amp"]);
        assert!(eq_first.front().contains(&Pedal::Eq));
        assert!(is_legal(&eq_first));

        // Names of a later build, a pedal twice, pedals on the wrong side, no amp
        let odd = ChainOrder::from_ids([
            "wah", "chorus", "chorus", "delay", "amp", "drive", "comp", "octaver",
        ]);
        assert!(is_legal(&odd));
        assert_eq!(odd.zone_of(Pedal::Chorus), Zone::Front);
        assert_eq!(odd.zone_of(Pedal::Comp), Zone::Back);
        assert_eq!(odd.zone_of(Pedal::Drive), Zone::Front);
        // The delay and the reverb stand in front of the amp too (#268)
        assert_eq!(odd.zone_of(Pedal::Delay), Zone::Front);
        let all_in_front = ChainOrder::from_ids(Pedal::ALL.map(Pedal::id));
        assert!(is_legal(&all_in_front));
        assert!(all_in_front.back().is_empty());
        let delay_first = ChainOrder::default().moved(Pedal::Delay, Zone::Front, 0);
        assert_eq!(delay_first.front()[0], Pedal::Delay);
        let reverb_second = ChainOrder::default().moved(Pedal::Reverb, Zone::Front, 1);
        assert_eq!(reverb_second.front()[1], Pedal::Reverb);
        assert!(is_legal(&delay_first) && is_legal(&reverb_second));
    }
}
