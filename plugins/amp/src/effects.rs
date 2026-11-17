//! The table of the pedals (#371): one line a pedal, and everything that has to name each of
//! them is made from it.
//!
//! A line gives the pedal's `EffectKind` (the variant of the same name), the `Pedal` that has
//! parameters of its own for it (`None` for the gate, which is no pedal of the order), its
//! `Effect` type and that type's settings. The macro makes from it:
//!
//! - `BlockSettings`: what a block of the chain is told, one variant a kind
//! - `Body`: the effect itself, one variant a kind
//! - the dispatch, `Body::new`, `set_sample_rate`, `set_up`, `reset`, `is_idle`, `tell`,
//!   `process_front` and `process_back`: a `match` on the enum, so it is static (no `dyn`, no
//!   heap) and a kind left out of the table does not compile
//!
//! A new pedal is its module with its `Effect` impl and one line here (and its parameters in
//! `lib.rs`, its paint in the editor).

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
use crate::slots::EffectKind;
use crate::wah::{WahEffect, WahSettings};

macro_rules! effects {
    ($($kind:ident { pedal: $pedal:expr, effect: $effect:ty, settings: $settings:ty }),* $(,)?) => {
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

        /// The pedal that has parameters of its own for `kind` (read by `slots.rs` from
        /// milestone 3 on)
        #[allow(dead_code)]
        pub fn pedal_of(kind: EffectKind) -> Option<Pedal> {
            match kind {
                $(EffectKind::$kind => $pedal,)*
            }
        }

        /// The settings of a block of `kind` that is switched off
        #[allow(dead_code)]
        pub fn off_of(kind: EffectKind) -> BlockSettings {
            match kind {
                $(EffectKind::$kind => BlockSettings::$kind(Default::default()),)*
            }
        }
    };
}

effects! {
    Gate { pedal: None, effect: Gate, settings: GateSettings },
    Drive { pedal: Some(Pedal::Drive), effect: DriveEffect, settings: DriveSettings },
    Comp { pedal: Some(Pedal::Comp), effect: Comp, settings: CompSettings },
    Fuzz { pedal: Some(Pedal::Fuzz), effect: Fuzz, settings: FuzzSettings },
    Pitch { pedal: Some(Pedal::Pitch), effect: PitchEffect, settings: PitchSettings },
    Wah { pedal: Some(Pedal::Wah), effect: WahEffect, settings: WahSettings },
    Eq { pedal: Some(Pedal::Eq), effect: Eq, settings: EqSettings },
    Chorus { pedal: Some(Pedal::Chorus), effect: Chorus, settings: ChorusSettings },
    Flanger { pedal: Some(Pedal::Flanger), effect: Flanger, settings: FlangerSettings },
    Phaser { pedal: Some(Pedal::Phaser), effect: Phaser, settings: PhaserSettings },
    Tremolo { pedal: Some(Pedal::Tremolo), effect: Tremolo, settings: TremoloSettings },
    Delay { pedal: Some(Pedal::Delay), effect: DelayEffect, settings: DelaySettings },
    Reverb { pedal: Some(Pedal::Reverb), effect: ReverbEffect, settings: ReverbSettings },
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
            let rows =
                EffectKind::ALL.iter().filter(|&&kind| pedal_of(kind) == Some(pedal)).count();
            assert_eq!(rows, 1, "{pedal:?} has {rows} rows");
        }
    }
}
