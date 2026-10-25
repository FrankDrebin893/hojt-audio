//! What each amp's head and each cabinet looks like. The sound of an amp is in `amp/model.rs`;
//! its look is here, one `Look` per amp in the order of `Amp::ALL`, so stepping to another amp
//! changes the head at once. The cabinet under it looks as the cabinet that plays (`cab_look`).
//! All our own designs: no maker's logo, script or plate shape.

use nih_plug_egui::egui::Color32;
use suite_common::ui::theme::{INK, SILK};
use suite_common::ui::{CabStyle, Corners, Grille, HeadStyle, KnobCap, Plate, Speakers, Tolex};

use crate::amp::model::SPEAKERS;
use crate::cab_stage::CabStatus;
use crate::user_cab;
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

// The cabinet under the head looks as the cabinet that plays: the amp's own, one of the
// built-in ones or a file. Its box keeps one size and its control plate one place whichever
// it is, so nothing on it moves; only the covering, the grille and the speakers change. The
// plate is the same black one on every cabinet, with the print in cream

/// How tall the control plate across the top of every cabinet is
pub(crate) const CAB_PLATE_HEIGHT: f32 = 124.0;

const fn cab(
    tolex: Tolex,
    covering: Color32,
    grille: Grille,
    cloth: Color32,
    speakers: Speakers,
) -> CabStyle {
    CabStyle {
        tolex,
        covering,
        piping: None,
        grille,
        cloth,
        speakers,
        set_high: false,
        corners: None,
        plate: Plate::BlackAnodised,
        plate_height: CAB_PLATE_HEIGHT,
        print: SILK,
    }
}

const BLACK: Color32 = rgb(24, 22, 21);
const WHITE_PIPING: Color32 = rgb(226, 222, 212);
const BASKETWEAVE: Color32 = rgb(128, 124, 116);
const STEEL_MESH: Color32 = rgb(150, 150, 148);
const SILVER_CLOTH: Color32 = rgb(172, 172, 168);

/// The amps' own cabinets, in the order of `Amp::ALL`
const OWN_CABS: [CabStyle; 4] = [
    // Klar: one 12" in an open back, ivory with a wheat cloth like its head's
    cab(Tolex::Smooth, rgb(226, 216, 190), Grille::Wheat, rgb(196, 168, 112), Speakers::One12),
    // Brøl: four 12" in a closed box, black with a basketweave piped in white, chrome corners
    CabStyle {
        piping: Some(WHITE_PIPING),
        corners: Some(Corners::Chrome),
        ..cab(Tolex::Levant, BLACK, Grille::Basketweave, BASKETWEAVE, Speakers::Four12)
    },
    // Torden: four 12" in a closed box, coarse black with perforated steel, steel corners
    CabStyle {
        corners: Some(Corners::Steel),
        ..cab(Tolex::Bronco, BLACK, Grille::Mesh, STEEL_MESH, Speakers::Four12)
    },
    // Glød: two 12" in an open back, tweed with an oxblood-brown cloth
    cab(Tolex::Tweed, rgb(198, 172, 122), Grille::Wheat, rgb(104, 44, 34), Speakers::Two12),
];

/// The built-in cabinets' looks, by the id of each in `SPEAKERS`
const SPEAKER_CABS: [(&str, CabStyle); 10] = [
    // Tan, a wheat cloth, the one small speaker high in the box
    (
        "1x10-small",
        CabStyle {
            set_high: true,
            ..cab(
                Tolex::Smooth,
                rgb(178, 140, 94),
                Grille::Wheat,
                rgb(204, 178, 124),
                Speakers::One10,
            )
        },
    ),
    (
        "1x12-open",
        cab(Tolex::Levant, rgb(232, 222, 196), Grille::Silver, SILVER_CLOTH, Speakers::One12),
    ),
    (
        "2x12-open",
        cab(Tolex::Levant, rgb(112, 112, 110), Grille::Silver, SILVER_CLOTH, Speakers::Two12),
    ),
    ("4x10-crisp", cab(Tolex::Smooth, BLACK, Grille::Silver, SILVER_CLOTH, Speakers::Four10)),
    (
        "4x12-mid",
        CabStyle {
            piping: Some(WHITE_PIPING),
            ..cab(Tolex::Levant, BLACK, Grille::Basketweave, BASKETWEAVE, Speakers::Four12)
        },
    ),
    ("4x12-tight", cab(Tolex::Bronco, BLACK, Grille::Mesh, STEEL_MESH, Speakers::Four12)),
    (
        "1x15-deep",
        cab(Tolex::Levant, rgb(30, 38, 64), Grille::Felt, rgb(26, 26, 28), Speakers::One15),
    ),
    // Walnut-brown, a faded brown cloth, the one small speaker high in the box
    (
        "1x8-tiny",
        CabStyle {
            set_high: true,
            ..cab(Tolex::Tweed, rgb(120, 84, 56), Grille::Wheat, rgb(150, 120, 86), Speakers::One8)
        },
    ),
    // Dark green bronco with a basketweave cloth
    (
        "2x12-punch",
        cab(Tolex::Bronco, rgb(30, 58, 44), Grille::Basketweave, BASKETWEAVE, Speakers::Two12),
    ),
    // Black and smooth with a dark felt, the biggest box
    ("4x12-heavy", cab(Tolex::Smooth, BLACK, Grille::Felt, rgb(34, 34, 36), Speakers::Four12)),
];

/// A file from the cabinets folder: a plain studio cabinet in grey felt, the one speaker
/// behind it only a shadow, for nobody knows what is in it
const FILE_CAB: CabStyle =
    cab(Tolex::Smooth, rgb(52, 52, 54), Grille::Felt, rgb(118, 118, 116), Speakers::One12);

/// The look of the cabinet that plays when `chosen` is the choice of cabinet and the loader
/// says `status` of it: what plays instead of a choice that is missing or cannot be used is
/// `amp`'s own, and so is its look
pub(crate) fn cab_look(amp: Amp, chosen: &str, status: CabStatus) -> &'static CabStyle {
    let own = &OWN_CABS[amp.index()];
    if chosen.is_empty() {
        own
    } else if let Some(speaker) = user_cab::speaker_of(chosen) {
        speaker_look(SPEAKERS[speaker].id).unwrap_or(own)
    } else if user_cab::is_speaker_choice(chosen) || status != CabStatus::Fine {
        own
    } else {
        &FILE_CAB
    }
}

/// The look of the built-in cabinet with this id
fn speaker_look(id: &str) -> Option<&'static CabStyle> {
    SPEAKER_CABS.iter().find(|(look_id, _)| *look_id == id).map(|(_, look)| look)
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

    // Every built-in cabinet has a look: one added to `SPEAKERS` without one fails here
    #[test]
    fn test_every_built_in_cabinet_has_a_look_of_its_own() {
        assert_eq!(SPEAKER_CABS.len(), SPEAKERS.len());
        for speaker in &SPEAKERS {
            assert!(speaker_look(speaker.id).is_some(), "{} has no look", speaker.id);
        }
        let expected = [
            ("1x10-small", Tolex::Smooth, Grille::Wheat, Speakers::One10),
            ("1x12-open", Tolex::Levant, Grille::Silver, Speakers::One12),
            ("2x12-open", Tolex::Levant, Grille::Silver, Speakers::Two12),
            ("4x10-crisp", Tolex::Smooth, Grille::Silver, Speakers::Four10),
            ("4x12-mid", Tolex::Levant, Grille::Basketweave, Speakers::Four12),
            ("4x12-tight", Tolex::Bronco, Grille::Mesh, Speakers::Four12),
            ("1x15-deep", Tolex::Levant, Grille::Felt, Speakers::One15),
            ("1x8-tiny", Tolex::Tweed, Grille::Wheat, Speakers::One8),
            ("2x12-punch", Tolex::Bronco, Grille::Basketweave, Speakers::Two12),
            ("4x12-heavy", Tolex::Smooth, Grille::Felt, Speakers::Four12),
        ];
        for (id, tolex, grille, speakers) in expected {
            let look = speaker_look(id).unwrap();
            assert_eq!((look.tolex, look.grille, look.speakers), (tolex, grille, speakers), "{id}");
        }
        assert!(speaker_look("1x10-small").unwrap().set_high);
        assert!(speaker_look("1x8-tiny").unwrap().set_high);
        assert_eq!(speaker_look("4x12-mid").unwrap().piping, Some(WHITE_PIPING));
    }

    // Each built-in cabinet is told from the others at a glance: by its covering, its grille
    // or its speakers
    #[test]
    fn test_no_two_built_in_cabinets_are_alike() {
        let seen = |look: &CabStyle| (look.tolex, look.grille, look.speakers);
        for (i, (a_id, a)) in SPEAKER_CABS.iter().enumerate() {
            for (b_id, b) in &SPEAKER_CABS[i + 1..] {
                assert_ne!(seen(a), seen(b), "{a_id} and {b_id}");
            }
        }
    }

    // The amp's own cabinet has a look for each amp, as the spec gave it
    #[test]
    fn test_every_amp_has_a_cabinet_of_its_own() {
        assert_eq!(OWN_CABS.len(), Amp::ALL.len());
        let expected = [
            (Amp::Klar, Tolex::Smooth, Grille::Wheat, Speakers::One12, None),
            (Amp::Glod, Tolex::Tweed, Grille::Wheat, Speakers::Two12, None),
            (
                Amp::Brol,
                Tolex::Levant,
                Grille::Basketweave,
                Speakers::Four12,
                Some(Corners::Chrome),
            ),
            (Amp::Torden, Tolex::Bronco, Grille::Mesh, Speakers::Four12, Some(Corners::Steel)),
        ];
        for (amp, tolex, grille, speakers, corners) in expected {
            let look = cab_look(amp, "", CabStatus::Fine);
            assert_eq!(
                (look.tolex, look.grille, look.speakers, look.corners),
                (tolex, grille, speakers, corners),
                "{amp:?}"
            );
        }
        assert_eq!(cab_look(Amp::Brol, "", CabStatus::Fine).piping, Some(WHITE_PIPING));
    }

    // What plays is what is shown: a built-in cabinet's own look, a file's look, and the amp's
    // own for a choice that is missing or cannot be used
    #[test]
    fn test_the_cab_looks_as_what_plays() {
        for amp in Amp::ALL {
            let own = cab_look(amp, "", CabStatus::Fine);
            for (index, (speaker, (id, look))) in SPEAKERS.iter().zip(&SPEAKER_CABS).enumerate() {
                assert_eq!(speaker.id, *id, "the looks are in the order of SPEAKERS");
                let chosen = user_cab::speaker_choice(index);
                assert_eq!(cab_look(amp, &chosen, CabStatus::Fine), look);
            }
            assert_eq!(cab_look(amp, "room.wav", CabStatus::Fine), &FILE_CAB);
            for status in [CabStatus::Missing, CabStatus::Bad] {
                assert_eq!(cab_look(amp, "room.wav", status), own, "{amp:?}");
            }
            // A built-in cabinet of another build plays the amp's own
            assert_eq!(cab_look(amp, "speaker:8x10-huge", CabStatus::Missing), own);
            assert_eq!(cab_look(amp, "speaker:8x10-huge", CabStatus::Fine), own);
        }
    }

    // Cream print on every cabinet's plate reads
    #[test]
    fn test_the_print_on_every_cab_plate_reads() {
        let all = OWN_CABS.iter().chain(SPEAKER_CABS.iter().map(|(_, look)| look));
        for look in all.chain([&FILE_CAB]) {
            assert!(contrast(look.print, look.plate.color()) >= least_contrast(), "{look:?}");
            assert_eq!(look.plate_height, CAB_PLATE_HEIGHT);
        }
    }
}
