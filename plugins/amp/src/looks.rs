//! What each amp's head looks like. The sound of an amp is in `amp/model.rs`; its look is here,
//! one `Look` per amp in the order of `Amp::ALL`, so stepping to another amp changes the head
//! at once. All our own designs: no maker's logo, script or plate shape.

use nih_plug_egui::egui::Color32;
use suite_common::ui::theme::{INK, SILK};
use suite_common::ui::{Grille, HeadStyle, KnobCap, Plate, Tolex};

use crate::Amp;

/// How tall the grille across the top of every head is: room for the nameplate, and the rest
/// of the head for the panel with its one line of knobs
pub(crate) const GRILLE_HEIGHT: f32 = 40.0;

/// An amp's head, and the caps of the knobs on its panel
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Look {
    pub(crate) head: HeadStyle,
    pub(crate) knobs: KnobCap,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

const PILOT_RED: Color32 = rgb(214, 36, 28);
const PILOT_AMBER: Color32 = rgb(236, 150, 40);

/// The looks, in the order of `Amp::ALL`
const LOOKS: [Look; 4] = [
    // Klar: ivory, a wheat cloth with a fine gold thread, a chrome panel and chrome knobs
    Look {
        head: HeadStyle {
            tolex: Tolex::Smooth,
            covering: rgb(226, 216, 190),
            piping: None,
            grille: Grille::Silver,
            cloth: rgb(196, 168, 112),
            cloth_stripe: None,
            grille_height: GRILLE_HEIGHT,
            panel: Plate::BrushedChrome,
            print: INK,
            panel_stripe: None,
            jewel: PILOT_RED,
        },
        knobs: KnobCap::ChromeSkirt,
    },
    // Brøl: black, a salt-and-pepper basketweave piped in white, a brass panel and gold tops
    Look {
        head: HeadStyle {
            tolex: Tolex::Levant,
            covering: rgb(26, 24, 23),
            piping: Some(rgb(226, 222, 212)),
            grille: Grille::Basketweave,
            cloth: rgb(128, 124, 116),
            cloth_stripe: None,
            grille_height: GRILLE_HEIGHT,
            panel: Plate::BrushedBrass,
            print: INK,
            panel_stripe: None,
            jewel: PILOT_RED,
        },
        knobs: KnobCap::GoldTop,
    },
    // Torden: black, perforated steel piped in red, a black panel with one red line, and the
    // pedals' black fluted knobs
    Look {
        head: HeadStyle {
            tolex: Tolex::Bronco,
            covering: rgb(24, 22, 21),
            piping: Some(rgb(184, 30, 26)),
            grille: Grille::Mesh,
            cloth: rgb(150, 150, 148),
            cloth_stripe: None,
            grille_height: GRILLE_HEIGHT,
            panel: Plate::BlackAnodised,
            print: SILK,
            panel_stripe: Some(rgb(196, 34, 28)),
            jewel: PILOT_RED,
        },
        knobs: KnobCap::Fluted,
    },
    // Glød: lacquered tweed, an oxblood-brown cloth with a gold stripe, a dark wood panel and
    // cream chicken-heads
    Look {
        head: HeadStyle {
            tolex: Tolex::Tweed,
            covering: rgb(198, 172, 122),
            piping: None,
            grille: Grille::Wheat,
            cloth: rgb(104, 44, 34),
            cloth_stripe: Some(rgb(204, 164, 76)),
            grille_height: GRILLE_HEIGHT,
            panel: Plate::DarkWood,
            print: SILK,
            panel_stripe: None,
            jewel: PILOT_AMBER,
        },
        knobs: KnobCap::CreamChickenHead,
    },
];

/// The look of `amp`'s head
pub(crate) fn look(amp: Amp) -> &'static Look {
    &LOOKS[amp.index()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use suite_common::ui::theme::{contrast, least_contrast};

    // Each amp gets the look the spec gave it, whatever order the lists are in
    #[test]
    fn test_every_amp_has_its_own_look() {
        assert_eq!(LOOKS.len(), Amp::ALL.len());
        let expected = [
            (Amp::Klar, Tolex::Smooth, Plate::BrushedChrome, KnobCap::ChromeSkirt),
            (Amp::Glod, Tolex::Tweed, Plate::DarkWood, KnobCap::CreamChickenHead),
            (Amp::Brol, Tolex::Levant, Plate::BrushedBrass, KnobCap::GoldTop),
            (Amp::Torden, Tolex::Bronco, Plate::BlackAnodised, KnobCap::Fluted),
        ];
        for (amp, tolex, panel, knobs) in expected {
            let look = look(amp);
            assert_eq!((look.head.tolex, look.head.panel, look.knobs), (tolex, panel, knobs));
        }
    }

    // Each head is told from the others at a glance: by its covering, its grille, its panel
    // and its knobs, each on its own
    #[test]
    fn test_no_two_heads_are_alike() {
        for (i, a) in LOOKS.iter().enumerate() {
            for b in &LOOKS[i + 1..] {
                assert_ne!(a.head.tolex, b.head.tolex, "{a:?} and {b:?}");
                assert_ne!(a.head.grille, b.head.grille, "{a:?} and {b:?}");
                assert_ne!(a.head.panel, b.head.panel, "{a:?} and {b:?}");
                assert_ne!(a.knobs, b.knobs, "{a:?} and {b:?}");
            }
        }
    }

    // Ink on brass and chrome must read as well as cream on paint does
    #[test]
    fn test_the_print_on_every_panel_reads() {
        for amp in Amp::ALL {
            let head = look(amp).head;
            let reads = contrast(head.print, head.panel.color());
            assert!(reads >= least_contrast(), "{amp:?}: {reads} under {}", least_contrast());
        }
    }
}
