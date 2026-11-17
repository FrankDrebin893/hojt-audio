//! The table of the pedals (#371): one line a pedal, and everything that has to name each of
//! them is made from it.
//!
//! A line gives the pedal's `EffectKind` (the variant of the same name), what a saved project
//! calls it (`id`, frozen), how many of a slot's knobs it uses (`knobs`) and how a slot's
//! switch and knobs make its settings (`slot`), the `Pedal` that has parameters of its own for
//! it (`None` for the gate, which is no pedal of the order), its `Effect` type, that type's
//! settings and how they are read from `AmpSettings` and the dials the chain is at (`home`).
//! The lines stand in the order of the effects bank's cards. The macro makes from it:
//!
//! - `BlockSettings`: what a block of the chain is told, one variant a kind
//! - `Body`: the effect itself, one variant a kind
//! - the dispatch, `Body::new`, `set_sample_rate`, `set_up`, `reset`, `is_idle`, `tell`,
//!   `process_front` and `process_back`, and the same for the pedal on its own parameters
//!   (`set_up_home`, `put_away`, `rest`, `rest_with_amp`, `tell_home`): a `match` on the enum,
//!   so it is static (no `dyn`, no heap) and a kind left out of the table does not compile
//! - `home_settings`, what a pedal on its own parameters is told, `pedal_of` and `kind_of`
//! - `id_of`, `knobs_of` and `slot_settings`, read by `slots.rs` (and `order.rs`: a pedal's id
//!   is its kind's), and `CARDS`, the effects bank's cards
//!
//! A new pedal is its module with its `Effect` impl and one line here (and its parameters in
//! `lib.rs`, its paint in the editor).

use crate::chain::{AmpSettings, Dials};
use crate::comp::{Comp, CompSettings};
use crate::delay::{DelayEffect, DelaySettings};
use crate::drive::{DriveEffect, DriveSettings};
use crate::effect::{Effect, EffectSettings, Rates};
use crate::eq::{Eq, EqSettings};
use crate::fuzz::{Fuzz, FuzzSettings};
use crate::gate::{Gate, GateSettings};
use crate::mods::chorus::{Chorus, ChorusSettings};
use crate::mods::flanger::{Flanger, FlangerSettings};
use crate::mods::phaser::{Phaser, PhaserSettings};
use crate::mods::tremolo::{Tremolo, TremoloSettings};
use crate::order::{Pedal, Zone};
use crate::pitch::{PitchEffect, PitchSettings};
use crate::reverb::{ReverbEffect, ReverbSettings};
use crate::slots::{EffectKind, KNOBS};
use crate::wah::{WahEffect, WahSettings};
use crate::GuitarAmpParams;
use nih_plug::prelude::Param;

macro_rules! effects {
    ($($kind:ident {
        id: $id:expr,
        knobs: $knobs:expr,
        slot: $slot:expr,
        pedal: $pedal:expr,
        effect: $effect:ty,
        settings: $settings:ty,
        home: $home:expr
    }),* $(,)?) => {
        /// What a block of the chain is told, read once per block: the settings of its kind
        #[derive(Clone, Copy, Debug, PartialEq, Default)]
        pub enum BlockSettings {
            /// No block on the slot: nothing is played
            #[default]
            None,
            $($kind($settings),)*
        }

        impl BlockSettings {
            /// Whether the block is switched on
            pub fn is_on(&self) -> bool {
                match self {
                    BlockSettings::None => false,
                    $(BlockSettings::$kind(settings) => EffectSettings::is_on(settings),)*
                }
            }
        }

        /// The effect of a block: one `Effect` of its kind
        #[allow(clippy::large_enum_variant)]
        pub enum Body {
            $($kind($effect),)*
        }

        impl Body {
            /// Allocates: not for the audio thread
            pub fn new(kind: EffectKind) -> Body {
                match kind {
                    $(EffectKind::$kind => Body::$kind(<$effect as Effect>::new()),)*
                }
            }

            /// Designs everything that depends on the sample rates. Allocates: not for the
            /// audio thread
            pub fn set_sample_rate(&mut self, rates: Rates) {
                match self {
                    $(Body::$kind(effect) => Effect::set_sample_rate(effect, rates),)*
                }
            }

            /// Sets the effect up for `zone` from rest. Does not allocate
            pub fn set_up(&mut self, zone: Zone, rates: Rates) {
                match self {
                    $(Body::$kind(effect) => Effect::set_up(effect, zone, rates),)*
                }
            }

            /// As after a long time switched off. Does not allocate
            pub fn reset(&mut self, zone: Zone, rates: Rates) {
                match self {
                    $(Body::$kind(effect) => Effect::reset(effect, zone, rates),)*
                }
            }

            /// Whether nothing of earlier input is left to hear
            pub fn is_idle(&self) -> bool {
                match self {
                    $(Body::$kind(effect) => Effect::is_idle(effect),)*
                }
            }

            /// `set_up` for a pedal on its own parameters. Does not allocate
            pub fn set_up_home(&mut self, zone: Zone, rates: Rates) {
                match self {
                    $(Body::$kind(effect) => Effect::set_up_home(effect, zone, rates),)*
                }
            }

            /// Puts the pedal to rest on whichever side it was. Does not allocate
            pub fn put_away(&mut self) {
                match self {
                    $(Body::$kind(effect) => Effect::put_away(effect),)*
                }
            }

            /// What the chain puts to rest when it resets its stages. Does not allocate
            pub fn rest(&mut self) {
                match self {
                    $(Body::$kind(effect) => Effect::rest(effect),)*
                }
            }

            /// What is put to rest with the amp's stages. Does not allocate
            pub fn rest_with_amp(&mut self) {
                match self {
                    $(Body::$kind(effect) => Effect::rest_with_amp(effect),)*
                }
            }

            /// Samples a switch of the pedal takes to fade, where the amp follows it
            pub fn amp_fade_steps(&self) -> u32 {
                match self {
                    $(Body::$kind(effect) => Effect::amp_fade_steps(effect),)*
                }
            }

            /// `tell` for a pedal on its own parameters, on `zone` or held by no zone
            pub fn tell_home(
                &mut self,
                settings: &BlockSettings,
                zone: Option<Zone>,
                steps: u32,
                snap: bool,
            ) {
                match (self, settings) {
                    $(
                        (Body::$kind(effect), BlockSettings::$kind(settings)) => {
                            Effect::tell_home(effect, settings, zone, steps, snap)
                        }
                        (Body::$kind(effect), _) => {
                            Effect::tell_home(effect, &Default::default(), zone, steps, snap)
                        }
                    )*
                }
            }

            /// What is not read at every call; settings of another kind are no block
            pub fn tell(&mut self, settings: &BlockSettings, zone: Zone, steps: u32, snap: bool) {
                match (self, settings) {
                    $(
                        (Body::$kind(effect), BlockSettings::$kind(settings)) => {
                            Effect::tell(effect, Some(settings), zone, steps, snap)
                        }
                        (Body::$kind(effect), _) => Effect::tell(effect, None, zone, steps, snap),
                    )*
                }
            }

            /// In front of the amp: one channel. Does not allocate
            pub fn process_front(&mut self, settings: &BlockSettings, block: &mut [f32]) {
                match (self, settings) {
                    $(
                        (Body::$kind(effect), BlockSettings::$kind(settings)) => {
                            Effect::process_front(effect, settings, block)
                        }
                        (Body::$kind(effect), _) => {
                            Effect::process_front(effect, &Default::default(), block)
                        }
                    )*
                }
            }

            /// Behind the cabinet: two channels. Does not allocate
            pub fn process_back(
                &mut self,
                settings: &BlockSettings,
                left: &mut [f32],
                right: &mut [f32],
            ) {
                match (self, settings) {
                    $(
                        (Body::$kind(effect), BlockSettings::$kind(settings)) => {
                            Effect::process_back(effect, settings, left, right)
                        }
                        (Body::$kind(effect), _) => {
                            Effect::process_back(effect, &Default::default(), left, right)
                        }
                    )*
                }
            }
        }

        /// The pedal that has parameters of its own for `kind`
        pub const fn pedal_of(kind: EffectKind) -> Option<Pedal> {
            match kind {
                $(EffectKind::$kind => $pedal,)*
            }
        }

        /// What a saved project calls `kind`. Frozen
        pub const fn id_of(kind: EffectKind) -> &'static str {
            match kind {
                $(EffectKind::$kind => $id,)*
            }
        }

        /// How many of a slot's `KNOBS` the kind uses
        pub const fn knobs_of(kind: EffectKind) -> usize {
            match kind {
                $(EffectKind::$kind => $knobs,)*
            }
        }

        /// What a block of `kind` is told from a switch and knobs at these positions (0.0 to
        /// 1.0): the parameters give each knob its range
        pub fn slot_settings(
            kind: EffectKind,
            params: &GuitarAmpParams,
            on: bool,
            knobs: [f32; KNOBS],
            tempo_bpm: Option<f64>,
        ) -> BlockSettings {
            match kind {
                $(EffectKind::$kind => {
                    let read: fn(&GuitarAmpParams, bool, [f32; KNOBS], Option<f64>) -> $settings =
                        $slot;
                    BlockSettings::$kind(read(params, on, knobs, tempo_bpm))
                })*
            }
        }

        /// The cards of the effects bank, in the order the table lists the kinds: every kind
        /// that is a pedal, the gate (the line's own first piece) having none
        pub const CARDS: [EffectKind; Pedal::ALL.len()] = {
            let kinds = [$(EffectKind::$kind,)*];
            let mut cards = [EffectKind::Gate; Pedal::ALL.len()];
            let (mut at, mut next) = (0, 0);
            while at < kinds.len() {
                if pedal_of(kinds[at]).is_some() {
                    cards[next] = kinds[at];
                    next += 1;
                }
                at += 1;
            }
            assert!(next == cards.len());
            cards
        };

        /// The kind of effect that plays for `pedal`
        pub fn kind_of(pedal: Pedal) -> EffectKind {
            for kind in EffectKind::ALL {
                if pedal_of(kind) == Some(pedal) {
                    return kind;
                }
            }
            unreachable!("every pedal has a row in the table")
        }

        /// What `pedal`, on its own parameters, is told: read from the settings, and from the
        /// dials the chain is at
        pub fn home_settings(pedal: Pedal, settings: &AmpSettings, dials: &Dials) -> BlockSettings {
            $(
                if Some(pedal) == $pedal {
                    let read: fn(&AmpSettings, &Dials) -> $settings = $home;
                    return BlockSettings::$kind(read(settings, dials));
                }
            )*
            BlockSettings::None
        }

        /// The settings of a block of `kind` that is switched off
        #[cfg(test)]
        pub fn off_of(kind: EffectKind) -> BlockSettings {
            match kind {
                $(EffectKind::$kind => BlockSettings::$kind(Default::default()),)*
            }
        }
    };
}

effects! {
    Gate {
        id: "gate", knobs: 2,
        slot: |p, on, [k1, k2, ..], _| GateSettings {
            on,
            threshold_db: p.gate_thresh.preview_plain(k1),
            release_ms: p.gate_release.preview_plain(k2),
        },
        pedal: None, effect: Gate, settings: GateSettings,
        home: |_, _| GateSettings::default()
    },
    Pitch {
        id: "pitch", knobs: 3,
        slot: |p, on, [k1, k2, k3, ..], _| PitchSettings {
            on,
            semitones: p.pitch_semitones.preview_plain(k1),
            blend: p.pitch_blend.preview_plain(k2),
            latency_ms: p.pitch_latency.preview_plain(k3),
        },
        pedal: Some(Pedal::Pitch), effect: PitchEffect, settings: PitchSettings,
        home: |s, _| s.pitch
    },
    Wah {
        id: "wah", knobs: 4,
        slot: |p, on, [k1, k2, k3, k4, _], _| WahSettings {
            on,
            mode: p.wah_mode.preview_plain(k3),
            pedal: p.wah_pedal.preview_plain(k1),
            bite: p.wah_q.preview_plain(k2),
            sens: p.wah_sens.preview_plain(k4),
        },
        pedal: Some(Pedal::Wah), effect: WahEffect, settings: WahSettings, home: |s, _| s.wah
    },
    Comp {
        id: "comp", knobs: 2,
        slot: |p, on, [k1, k2, ..], _| CompSettings {
            on,
            sustain: p.comp_sustain.preview_plain(k1),
            level: p.comp_level.preview_plain(k2),
        },
        pedal: Some(Pedal::Comp), effect: Comp, settings: CompSettings, home: |s, _| s.comp
    },
    Fuzz {
        id: "fuzz", knobs: 2,
        slot: |p, on, [k1, k2, ..], _| FuzzSettings {
            on,
            fuzz: p.fuzz_fuzz.preview_plain(k1),
            tone: p.fuzz_tone.preview_plain(k2),
        },
        pedal: Some(Pedal::Fuzz), effect: Fuzz, settings: FuzzSettings, home: |s, _| s.fuzz
    },
    Drive {
        id: "drive", knobs: 3,
        slot: |p, on, [k1, k2, k3, ..], _| DriveSettings {
            on,
            drive: p.drive_gain.preview_plain(k1),
            tone: p.drive_tone.preview_plain(k2),
            level: p.drive_level.preview_plain(k3),
        },
        pedal: Some(Pedal::Drive), effect: DriveEffect, settings: DriveSettings,
        home: |s, d| DriveSettings {
            on: s.drive_on,
            drive: d.drive_gain,
            tone: d.drive_tone,
            level: d.drive_level,
        }
    },
    Eq {
        id: "eq", knobs: 4,
        slot: |p, on, [k1, k2, k3, k4, _], _| EqSettings {
            on,
            low: p.eq_low.preview_plain(k1),
            mid: p.eq_mid.preview_plain(k2),
            freq: p.eq_freq.preview_plain(k3),
            high: p.eq_high.preview_plain(k4),
        },
        pedal: Some(Pedal::Eq), effect: Eq, settings: EqSettings, home: |s, _| s.eq
    },
    Phaser {
        id: "phaser", knobs: 2,
        slot: |p, on, [k1, k2, ..], _| PhaserSettings {
            on,
            rate_hz: p.phaser_rate.preview_plain(k1),
            depth: p.phaser_depth.preview_plain(k2),
        },
        pedal: Some(Pedal::Phaser), effect: Phaser, settings: PhaserSettings,
        home: |s, _| s.phaser
    },
    Chorus {
        id: "chorus", knobs: 2,
        slot: |p, on, [k1, k2, ..], _| ChorusSettings {
            on,
            rate_hz: p.chorus_rate.preview_plain(k1),
            depth: p.chorus_depth.preview_plain(k2),
        },
        pedal: Some(Pedal::Chorus), effect: Chorus, settings: ChorusSettings,
        home: |s, _| s.chorus
    },
    Flanger {
        id: "flanger", knobs: 2,
        slot: |p, on, [k1, k2, ..], _| FlangerSettings {
            on,
            rate_hz: p.flanger_rate.preview_plain(k1),
            depth: p.flanger_depth.preview_plain(k2),
        },
        pedal: Some(Pedal::Flanger), effect: Flanger, settings: FlangerSettings,
        home: |s, _| s.flanger
    },
    Tremolo {
        id: "tremolo", knobs: 2,
        slot: |p, on, [k1, k2, ..], _| TremoloSettings {
            on,
            rate_hz: p.trem_rate.preview_plain(k1),
            depth: p.trem_depth.preview_plain(k2),
        },
        pedal: Some(Pedal::Tremolo), effect: Tremolo, settings: TremoloSettings,
        home: |s, _| s.tremolo
    },
    Delay {
        id: "delay", knobs: 5,
        slot: |p, on, [k1, k2, k3, k4, k5], tempo_bpm| DelaySettings {
            on,
            time_ms: crate::delay_time_ms(
                p.delay_time.preview_plain(k1),
                p.delay_sync.preview_plain(k4),
                p.delay_note.preview_plain(k5),
                tempo_bpm,
            ),
            feedback: p.delay_feedback.preview_plain(k2),
            mix: p.delay_mix.preview_plain(k3),
        },
        pedal: Some(Pedal::Delay), effect: DelayEffect, settings: DelaySettings,
        home: |s, _| s.delay
    },
    Reverb {
        id: "reverb", knobs: 2,
        slot: |p, on, [k1, k2, ..], _| ReverbSettings {
            on,
            decay_s: p.reverb_decay.preview_plain(k1),
            mix: p.reverb_mix.preview_plain(k2),
        },
        pedal: Some(Pedal::Reverb), effect: ReverbEffect, settings: ReverbSettings,
        home: |s, _| s.reverb
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_every_kind_and_every_pedal_has_a_row_in_the_table() {
        for kind in EffectKind::ALL {
            assert_eq!(
                pedal_of(kind).map(Pedal::index),
                kind.home(),
                "{kind:?}: the table and the kind disagree"
            );
            // The row's body and settings are of the kind itself
            let body = Body::new(kind);
            let named = match (&body, off_of(kind)) {
                (Body::Gate(_), BlockSettings::Gate(_)) => EffectKind::Gate,
                (Body::Drive(_), BlockSettings::Drive(_)) => EffectKind::Drive,
                (Body::Comp(_), BlockSettings::Comp(_)) => EffectKind::Comp,
                (Body::Fuzz(_), BlockSettings::Fuzz(_)) => EffectKind::Fuzz,
                (Body::Pitch(_), BlockSettings::Pitch(_)) => EffectKind::Pitch,
                (Body::Wah(_), BlockSettings::Wah(_)) => EffectKind::Wah,
                (Body::Eq(_), BlockSettings::Eq(_)) => EffectKind::Eq,
                (Body::Chorus(_), BlockSettings::Chorus(_)) => EffectKind::Chorus,
                (Body::Flanger(_), BlockSettings::Flanger(_)) => EffectKind::Flanger,
                (Body::Phaser(_), BlockSettings::Phaser(_)) => EffectKind::Phaser,
                (Body::Tremolo(_), BlockSettings::Tremolo(_)) => EffectKind::Tremolo,
                (Body::Delay(_), BlockSettings::Delay(_)) => EffectKind::Delay,
                (Body::Reverb(_), BlockSettings::Reverb(_)) => EffectKind::Reverb,
                _ => panic!("{kind:?}: the row's effect and settings are of another kind"),
            };
            assert_eq!(named, kind);
            assert!(!off_of(kind).is_on(), "{kind:?}: off_of is on");
        }
        for pedal in Pedal::ALL {
            assert_eq!(pedal_of(kind_of(pedal)), Some(pedal));
        }
        for pedal in Pedal::ALL {
            let rows =
                EffectKind::ALL.iter().filter(|&&kind| pedal_of(kind) == Some(pedal)).count();
            assert_eq!(rows, 1, "{pedal:?} has {rows} rows");
        }
    }

    /// What the table now answers for `slots.rs`, `order.rs` and the bank is what those files
    /// held in matches of their own (frozen: saved projects and the look of the bank)
    #[test]
    fn test_the_tables_facts_are_the_ones_that_were_frozen() {
        use EffectKind::*;
        let ids = [
            (Gate, "gate", 2),
            (Drive, "drive", 3),
            (Comp, "comp", 2),
            (Fuzz, "fuzz", 2),
            (Pitch, "pitch", 3),
            (Wah, "wah", 4),
            (Eq, "eq", 4),
            (Chorus, "chorus", 2),
            (Flanger, "flanger", 2),
            (Phaser, "phaser", 2),
            (Tremolo, "tremolo", 2),
            (Delay, "delay", 5),
            (Reverb, "reverb", 2),
        ];
        assert_eq!(ids.map(|(kind, ..)| kind), EffectKind::ALL);
        for (kind, id, knobs) in ids {
            assert_eq!((id_of(kind), knobs_of(kind)), (id, knobs), "{kind:?}");
            assert!(knobs <= KNOBS);
        }
        for pedal in Pedal::ALL {
            assert_eq!(pedal.id(), id_of(kind_of(pedal)));
        }
        // The bank's cards: two rows of six, as drawn since the bank was made
        assert_eq!(
            CARDS,
            [Pitch, Wah, Comp, Fuzz, Drive, Eq, Phaser, Chorus, Flanger, Tremolo, Delay, Reverb]
        );
    }

    /// A body told the settings of another kind (which the chain never does) plays as if it
    /// were told its own, switched off, and is no block to `tell` either
    #[test]
    fn test_settings_of_another_kind_play_as_the_kind_switched_off() {
        let rates = Rates { host: 48_000.0, oversampled: 192_000.0 };
        let kinds = EffectKind::ALL;
        for (at, kind) in kinds.into_iter().enumerate() {
            let other = off_of(kinds[(at + 1) % kinds.len()]);
            let (mut wrong, mut right) = (Body::new(kind), Body::new(kind));
            wrong.set_sample_rate(rates);
            right.set_sample_rate(rates);
            let signal: Vec<f32> = (0..32).map(|i| 0.3 * (i as f32 * 0.2).sin()).collect();
            let (mut a, mut b) = (signal.clone(), signal.clone());
            wrong.process_front(&other, &mut a);
            right.process_front(&off_of(kind), &mut b);
            assert_eq!(a, b, "{kind:?} in front");
            let (mut a_left, mut a_right) = (signal.clone(), signal.clone());
            let (mut b_left, mut b_right) = (signal.clone(), signal.clone());
            wrong.process_back(&other, &mut a_left, &mut a_right);
            right.process_back(&off_of(kind), &mut b_left, &mut b_right);
            assert_eq!((a_left, a_right), (b_left, b_right), "{kind:?} behind");
            wrong.tell(&other, Zone::Front, 0, true);
            wrong.tell_home(&other, Some(Zone::Front), 0, true);
        }
    }
}
