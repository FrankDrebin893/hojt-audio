//! A ring of lamps standing round a control: the shape of a loop (the amp's looper, #280).
//!
//! The ring is drawn by [`lamp_ring`] from a [`Ring`] that says which lamps burn: none, the
//! first part of the ring filled (a recording growing), or a head going round with one lamp
//! behind it (a loop playing). It is a signal display, not a control: it has no click of its own.
//! The lamps start at the top and go clockwise, and an unlit lamp is a dark socket so the ring
//! is always there.

use nih_plug_egui::egui::{vec2, Color32, Painter, Pos2};

/// How many lamps the ring has
pub const RING_LAMPS: usize = 32;
/// The radius of one lamp
pub const RING_LAMP_RADIUS: f32 = 3.6;
/// Recording: red
pub const RING_RED: Color32 = Color32::from_rgb(230, 60, 40);
/// Overdubbing: amber, as the metronome's lamp
pub const RING_AMBER: Color32 = Color32::from_rgb(255, 168, 28);
/// Playing: green
pub const RING_GREEN: Color32 = Color32::from_rgb(120, 220, 120);
/// An unlit lamp: dark socket
const RING_OFF: Color32 = Color32::from_rgb(40, 34, 30);

/// What the ring shows
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ring {
    /// Nothing: every lamp dark
    Dark,
    /// The first `fraction` (0 to 1) of the ring lit, from the top clockwise, in `color`; at least
    /// one lamp once it is above 0
    Filled { fraction: f32, color: Color32 },
    /// One lamp at `fraction` of the way round, the one behind it at half brightness
    Head { fraction: f32, color: Color32 },
}

/// The lamp the head of a ring at `fraction` of the way round is on
pub fn head_lamp(fraction: f32) -> usize {
    ((fraction.clamp(0.0, 1.0) * RING_LAMPS as f32) as usize).min(RING_LAMPS - 1)
}

/// How many lamps a filled ring at `fraction` has lit
pub fn filled_lamps(fraction: f32) -> usize {
    if fraction <= 0.0 {
        0
    } else {
        ((fraction.min(1.0) * RING_LAMPS as f32).ceil() as usize).clamp(1, RING_LAMPS)
    }
}

/// The colour of each lamp of `ring`, `None` for a dark one, and the share of its colour it
/// burns with
pub fn lamps(ring: Ring) -> [Option<(Color32, f32)>; RING_LAMPS] {
    let mut out = [None; RING_LAMPS];
    match ring {
        Ring::Dark => {}
        Ring::Filled { fraction, color } => {
            for lamp in out.iter_mut().take(filled_lamps(fraction)) {
                *lamp = Some((color, 1.0));
            }
        }
        Ring::Head { fraction, color } => {
            let head = head_lamp(fraction);
            out[head] = Some((color, 1.0));
            out[(head + RING_LAMPS - 1) % RING_LAMPS] = Some((color, 0.5));
        }
    }
    out
}

/// Where lamp `index` stands on a ring of `radius` round `center`: the first at the top
pub fn lamp_position(center: Pos2, radius: f32, index: usize) -> Pos2 {
    let angle =
        -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::TAU / RING_LAMPS as f32;
    center + vec2(angle.cos(), angle.sin()) * radius
}

/// Draws `ring`: `RING_LAMPS` lamps on a circle of `radius` round `center`
pub fn lamp_ring(painter: &Painter, center: Pos2, radius: f32, ring: Ring) {
    for (index, lit) in lamps(ring).into_iter().enumerate() {
        let at = lamp_position(center, radius, index);
        let color = match lit {
            None => RING_OFF,
            Some((color, share)) => RING_OFF.lerp_to_gamma(color, share),
        };
        painter.circle_filled(at, RING_LAMP_RADIUS, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a_dark_ring_burns_nothing_and_a_filled_one_burns_its_share_from_the_top() {
        assert!(lamps(Ring::Dark).iter().all(Option::is_none));
        let lit = |fraction| {
            lamps(Ring::Filled { fraction, color: RING_RED }).iter().filter(|l| l.is_some()).count()
        };
        assert_eq!(lit(0.0), 0);
        assert_eq!(lit(0.001), 1);
        assert_eq!(lit(0.5), RING_LAMPS / 2);
        assert_eq!(lit(1.0), RING_LAMPS);
        assert_eq!(lit(7.0), RING_LAMPS);
        let filled = lamps(Ring::Filled { fraction: 0.25, color: RING_RED });
        assert!(filled[0].is_some() && filled[7].is_some() && filled[8].is_none());
    }

    #[test]
    fn test_the_head_is_one_lamp_with_one_dimmer_behind_it_and_wraps_at_the_top() {
        for step in 0..=1000 {
            let fraction = step as f32 / 1000.0;
            let burning: Vec<_> = lamps(Ring::Head { fraction, color: RING_GREEN })
                .iter()
                .enumerate()
                .filter_map(|(i, l)| l.map(|(_, share)| (i, share)))
                .collect();
            assert_eq!(burning.len(), 2, "at {fraction}");
            assert!(burning.iter().any(|&(_, share)| share == 1.0));
        }
        // Round the top: lamp 0 is the head and the one behind it is the last
        let at_top = lamps(Ring::Head { fraction: 0.0, color: RING_GREEN });
        assert_eq!(at_top[0].map(|l| l.1), Some(1.0));
        assert_eq!(at_top[RING_LAMPS - 1].map(|l| l.1), Some(0.5));
        assert_eq!(head_lamp(1.0), RING_LAMPS - 1);
    }
}
