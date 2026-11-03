//! The built-in display: a flat dark glass panel with a crisp readout in a clean monospaced face,
//! in cool white. No dot grid and no glow to fight the labels around it; the glass has a hairline
//! border and a faint lit top edge, and nothing else. It is drawn with plain shapes and the
//! embedded Plex Mono, so it costs nothing to paint and stays sharp at any scale.

use nih_plug_egui::egui::{vec2, Align2, Color32, Painter, Pos2, Rect, Stroke, StrokeKind};

use super::theme::mono;

/// How wide one character of the readout is, in points, and how large it is drawn
const CHAR_WIDTH: f32 = 13.5;
const TEXT_SIZE: f32 = 22.0;
/// Space between the edge of the glass and the readout, across
const PAD_X: f32 = 10.0;
/// Height of every display
pub const LED_HEIGHT: f32 = 34.0;
const RADIUS: f32 = 6.0;

const GLASS: Color32 = Color32::from_rgb(9, 12, 15);
const BORDER: Color32 = Color32::from_rgb(54, 61, 68);
/// The readout, and the lit lamp of the tuner's needle
const INK: Color32 = Color32::from_rgb(226, 240, 246);
/// The needle's lamps that are not lit, and the middle one when in tune
const UNLIT: Color32 = Color32::from_rgb(74, 84, 92);
const IN_TUNE: Color32 = Color32::from_rgb(112, 236, 162);

/// How wide a display is that holds `chars` characters
pub const fn led_width(chars: usize) -> f32 {
    chars as f32 * CHAR_WIDTH + 2.0 * PAD_X
}

/// The glass of a display filling `rect`, with its shadow
pub fn paint_window(painter: &Painter, rect: Rect) {
    painter.rect_filled(
        rect.translate(vec2(0.0, 2.0)).expand(1.0),
        RADIUS + 1.0,
        Color32::from_black_alpha(90),
    );
    painter.rect_filled(rect, RADIUS, GLASS);
    painter.rect_stroke(rect, RADIUS, Stroke::new(1.0_f32, BORDER), StrokeKind::Inside);
    // A faint lit edge along the top, inside the border
    let top = Rect::from_min_max(
        Pos2::new(rect.left() + RADIUS, rect.top() + 1.0),
        Pos2::new(rect.right() - RADIUS, rect.top() + 2.0),
    );
    painter.rect_filled(top, 0.0, Color32::from_white_alpha(16));
}

/// `text` in the readout's face, from `left`, one character to a place, cut at `chars` places
fn paint_text(painter: &Painter, window: Rect, text: &str, chars: usize, color: Color32) {
    for (place, letter) in text.chars().take(chars).enumerate() {
        let at = Pos2::new(
            window.left() + PAD_X + (place as f32 + 0.5) * CHAR_WIDTH,
            window.center().y + 1.0,
        );
        painter.text(at, Align2::CENTER_CENTER, letter, mono(TEXT_SIZE), color);
    }
}

/// A display of `chars` characters centred on `center` showing `text`, which is cut off at its
/// right end if it is longer. The text starts at the left, so a name being typed grows to the
/// right. Returns the window, for the caller's buttons and clicks
pub fn led_display(painter: &Painter, center: Pos2, chars: usize, text: &str) -> Rect {
    let window = Rect::from_center_size(center, vec2(led_width(chars), LED_HEIGHT));
    paint_window(painter, window);
    paint_text(painter, window, &text.to_uppercase(), chars, INK);
    window
}

/// How far a note is from in tune, as the needle of `tuner_display`: the brightness, 0 to 1, of
/// the lamp `lamp` places from the middle one (17 lamps, -8 to 8 for flat to sharp). The lit
/// spot moves like a needle, two lamps sharing it between their places. Within 2 cents only
/// the middle one is lit
pub fn needle_brightness(lamp: i32, cents: Option<f32>) -> f32 {
    const SIDE: f32 = 8.0;
    const SHARED: f32 = 0.4;
    const IN_TUNE_CENTS: f32 = 2.0;
    const RANGE_CENTS: f32 = 50.0;
    let in_tune = cents.is_some_and(|cents| cents.abs() <= IN_TUNE_CENTS);
    if lamp == 0 {
        return if in_tune { 1.0 } else { 0.0 };
    }
    // Out of tune the spot starts at the first lamp beside the middle one, so the middle one
    // says in tune and nothing else
    let spot = cents.filter(|_| !in_tune).map(|cents| {
        let off = (cents.abs().min(RANGE_CENTS) - IN_TUNE_CENTS) / (RANGE_CENTS - IN_TUNE_CENTS);
        (1.0 + off * (SIDE - 1.0)).copysign(cents)
    });
    // Fully lit until the spot is most of the way to the next lamp, then fading as that one
    // comes up
    spot.map_or(0.0, |spot| ((1.0 - (lamp as f32 - spot).abs()) / SHARED).clamp(0.0, 1.0))
}

/// Characters at the left of a `tuner_display` for the note's name
const TUNER_NOTE_CHARS: usize = 4;
/// Lamps of the needle: flat to the left, sharp to the right, and how far apart they stand
const LAMPS: i32 = 8;
const LAMP_STEP: f32 = 8.5;

/// The tuner on a display of `chars` characters centred on `center`: the note's name at its
/// left end, and in the rest of it a needle of 17 lamps, flat to the left and sharp to the
/// right, the middle one tall and lit green only when the note is in tune. Without a note the
/// name is two dashes and the lamps are dark. As high as every display
pub fn tuner_display(
    painter: &Painter,
    center: Pos2,
    chars: usize,
    note: Option<&str>,
    cents: Option<f32>,
) {
    let window = Rect::from_center_size(center, vec2(led_width(chars), LED_HEIGHT));
    paint_window(painter, window);
    paint_text(painter, window, note.unwrap_or("--"), TUNER_NOTE_CHARS, INK);

    // A divider between the name and the needle
    let divider = window.left() + PAD_X + TUNER_NOTE_CHARS as f32 * CHAR_WIDTH + 2.0;
    painter.line_segment(
        [Pos2::new(divider, window.top() + 7.0), Pos2::new(divider, window.bottom() - 7.0)],
        Stroke::new(1.0_f32, BORDER),
    );

    // The needle fills the rest, centred in it
    let middle = (divider + window.right() - PAD_X) / 2.0;
    for lamp in -LAMPS..=LAMPS {
        let level = needle_brightness(lamp, cents);
        let x = middle + lamp as f32 * LAMP_STEP;
        let half = if lamp == 0 { 9.0 } else { 5.0 };
        let bar = Rect::from_center_size(Pos2::new(x, window.center().y), vec2(3.0, 2.0 * half));
        let lit = if lamp == 0 { IN_TUNE } else { INK };
        let color = blend_color(UNLIT, lit, level);
        if level > 0.0 {
            painter.rect_filled(bar.expand(2.0), 3.0, lit.gamma_multiply(0.16 * level));
        }
        painter.rect_filled(bar, 1.5, color);
    }
}

fn blend_color(from: Color32, to: Color32, share: f32) -> Color32 {
    let mix =
        |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * share.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgb(mix(from.r(), to.r()), mix(from.g(), to.g()), mix(from.b(), to.b()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a_display_is_as_wide_as_its_characters_and_as_high_as_every_other() {
        assert_eq!(led_width(2) - led_width(1), CHAR_WIDTH);
        assert_eq!(LED_HEIGHT, 34.0);
        // A character fits its place: Plex Mono is 0.6 of its size wide
        const { assert!(TEXT_SIZE * 0.6 <= CHAR_WIDTH) };
    }

    #[test]
    fn test_the_needle_lights_the_middle_lamp_only_in_tune() {
        assert_eq!(needle_brightness(0, Some(1.0)), 1.0);
        assert_eq!(needle_brightness(0, Some(10.0)), 0.0);
        assert_eq!(needle_brightness(3, None), 0.0);
        // Flat lights the lamps to the left, sharp to the right
        assert!(needle_brightness(-1, Some(-3.0)) > 0.0);
        assert_eq!(needle_brightness(1, Some(-3.0)), 0.0);
        assert!(needle_brightness(8, Some(60.0)) > 0.0);
    }

    #[test]
    fn test_tuner_needle_fits_between_its_note_and_the_edge_of_the_display() {
        // The smallest display the tuner is drawn on is the preset display's
        let width = led_width(15);
        let note = PAD_X + TUNER_NOTE_CHARS as f32 * CHAR_WIDTH + 2.0;
        let needle = 2.0 * LAMPS as f32 * LAMP_STEP + 3.0;
        assert!(note + needle + PAD_X <= width, "{} > {width}", note + needle + PAD_X);
    }
}
