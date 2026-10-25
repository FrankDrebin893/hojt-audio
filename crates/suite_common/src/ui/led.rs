//! A built-in LED display: a dark recessed window with a dot-matrix readout in amber, lit pixel
//! by pixel like the rest of `raster` (no image files). The dots that are not lit stay faintly
//! visible, so the grid reads as a real display.
//!
//! The window and the two kinds of dot are rendered once by `cached`; a readout is only a
//! row of blits, so it costs nothing until the text changes and the window is painted again.

use nih_plug_egui::egui::{vec2, Color32, ColorImage, Painter, Pos2, Rect, Vec2};

use super::raster::{blit, cached, device, hash, image, rounded_box, smoothstep, Pixel, Rgb};

/// Space from one dot to the next, in points, across and down
pub const LED_PITCH: f32 = 5.0;
/// A character is this many dots wide and high
const GLYPH_COLS: usize = 5;
const GLYPH_ROWS: usize = 7;
/// The columns a character takes, with the blank one after it
const ADVANCE_COLS: usize = GLYPH_COLS + 1;
/// Space between the edge of the window and the dots
const PAD: Vec2 = vec2(12.0, 8.0);
/// Height of every LED display: seven rows of dots and the padding
pub const LED_HEIGHT: f32 = GLYPH_ROWS as f32 * LED_PITCH + 2.0 * PAD.y;

// Room around the window for its shadow, and the width of its bezel, in points
const WINDOW_MARGIN: f32 = 3.0;
const BEZEL: f32 = 3.0;
const WINDOW_RADIUS: f32 = 5.0;

// The dot: its lit core and its glow, in points from its middle
const DOT_CORE: f32 = 1.95;
const DOT_REACH: f32 = 5.6;

const AMBER: Rgb = [255.0, 168.0, 28.0];
const AMBER_HOT: Rgb = [255.0, 236.0, 176.0];
const GHOST: Rgb = [58.0, 33.0, 12.0];
const GLASS: Rgb = [12.0, 7.0, 5.0];
/// The colour of anything drawn on the display that is not a dot
const AMBER_DIM: Color32 = Color32::from_rgb(92, 54, 16);

/// How wide a display is that holds `chars` characters
pub const fn led_width(chars: usize) -> f32 {
    (chars * ADVANCE_COLS - 1) as f32 * LED_PITCH + 2.0 * PAD.x
}

/// The five columns of each of the seven rows of `letter`, the leftmost dot the highest bit
fn glyph(letter: char) -> [u8; GLYPH_ROWS] {
    const BOX: [u8; 7] = [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111];
    match letter.to_ascii_uppercase() {
        ' ' => [0; 7],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        'Å' => [0b00100, 0b01010, 0b01110, 0b10001, 0b11111, 0b10001, 0b10001],
        'Æ' => [0b01111, 0b10100, 0b10100, 0b11110, 0b10100, 0b10100, 0b10111],
        'Ø' => [0b01110, 0b10011, 0b10101, 0b10101, 0b10101, 0b11001, 0b01110],
        'Ä' => [0b01010, 0b00000, 0b01110, 0b10001, 0b11111, 0b10001, 0b10001],
        'Ö' => [0b01010, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        'Ü' => [0b01010, 0b00000, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'É' => [0b00010, 0b00100, 0b11111, 0b10000, 0b11110, 0b10000, 0b11111],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 0b11111],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        '#' => [0b01010, 0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0b01010],
        '&' => [0b01100, 0b10010, 0b10100, 0b01000, 0b10101, 0b10010, 0b01101],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '~' => [0, 0, 0b01000, 0b10101, 0b00010, 0, 0],
        '*' => [0, 0b00100, 0b10101, 0b01110, 0b10101, 0b00100, 0],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        '/' => [0b00001, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b10000],
        // The other accented letters are shown without their accents
        'À' | 'Á' | 'Â' | 'Ã' => glyph('A'),
        'È' | 'Ê' | 'Ë' => glyph('E'),
        'Ì' | 'Í' | 'Î' | 'Ï' => glyph('I'),
        'Ò' | 'Ó' | 'Ô' | 'Õ' => glyph('O'),
        'Ù' | 'Ú' | 'Û' => glyph('U'),
        'Ç' => glyph('C'),
        'Ñ' => glyph('N'),
        'Ý' => glyph('Y'),
        _ => BOX,
    }
}

/// The dots of `text` that are lit, as (column, row) from the top left dot, on a display of
/// `chars` characters: what does not fit is cut off at its right end
pub fn text_dots(text: &str, chars: usize) -> Vec<(usize, usize)> {
    let mut lit = Vec::new();
    for (place, letter) in text.chars().take(chars).enumerate() {
        for (row, bits) in glyph(letter).into_iter().enumerate() {
            for col in 0..GLYPH_COLS {
                if bits & (1 << (GLYPH_COLS - 1 - col)) != 0 {
                    lit.push((place * ADVANCE_COLS + col, row));
                }
            }
        }
    }
    lit
}

/// How many columns of dots a display of `chars` characters has
fn columns(chars: usize) -> usize {
    chars * ADVANCE_COLS - 1
}

/// The middle of the dot at `col`, `row` of the display in `window`
fn dot_center(window: Rect, col: usize, row: usize) -> Pos2 {
    window.min + PAD + vec2((col as f32 + 0.5) * LED_PITCH, (row as f32 + 0.5) * LED_PITCH)
}

/// The glass of a display in a dark bezel, with its shadow, `size` in points
pub fn window_image(size: Vec2, ppp: f32) -> ColorImage {
    let (w, h) = (device(size.x, ppp).round().max(1.0), device(size.y, ppp).round().max(1.0));
    let m = device(WINDOW_MARGIN, ppp).round();
    let (width, height) = (w as usize + 2 * m as usize, h as usize + 2 * m as usize);
    let (c, half) = ([m + w / 2.0, m + h / 2.0], [w / 2.0, h / 2.0]);
    let (radius, bezel) = (device(WINDOW_RADIUS, ppp), device(BEZEL, ppp));
    let mut pixels = vec![Pixel::default(); width * height];
    for y in 0..height {
        for x in 0..width {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let mut pixel = Pixel::default();
            let shadow = rounded_box([p[0], p[1] - device(2.0, ppp)], c, half, radius);
            pixel.over([0.0; 3], 0.4 * (-(shadow.max(0.0) / device(1.5, ppp)).powi(2)).exp());

            // The bezel: a dark moulding that catches a little light on its lower edge, the
            // way a recess does
            let d = rounded_box(p, c, half, radius);
            let down = ((p[1] - m) / h).clamp(0.0, 1.0);
            let lip = 26.0 + 30.0 * smoothstep(0.55, 1.0, down);
            pixel.over([lip, lip - 2.0, lip - 5.0], (0.5 - d).clamp(0.0, 1.0));

            // The glass, with the shadow the bezel throws on its top edge and a faint sheen
            let glass = rounded_box(p, c, [half[0] - bezel, half[1] - bezel], radius - bezel);
            let depth = (-glass).max(0.0);
            let shade = 1.0 - 0.7 * (-(depth / device(3.0, ppp)).powi(2)).exp() * (1.0 - down);
            let sheen =
                1.0 + 0.9 * smoothstep(0.6, 0.0, down) * smoothstep(0.0, 0.4, (p[0] - m) / w);
            let grain = 1.0 + (hash(x as i32, y as i32, 53) - 0.5) * 0.12;
            let glass_rgb = GLASS.map(|channel| channel * shade * sheen * grain);
            pixel.over(glass_rgb, (0.5 - glass).clamp(0.0, 1.0));
            pixels[y * width + x] = pixel;
        }
    }
    image(width, height, pixels)
}

/// One dot of the display: its unlit self, or lit with the glow round it
pub fn dot_image(lit: bool, ppp: f32) -> ColorImage {
    let side = (2.0 * DOT_REACH * ppp).ceil() as usize;
    let c = side as f32 / 2.0;
    let mut pixels = vec![Pixel::default(); side * side];
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let dist = (dx * dx + dy * dy).sqrt() / ppp;
            let mut pixel = Pixel::default();
            if lit {
                pixel.over(AMBER, (-(dist / 3.0).powi(2)).exp() * 0.42);
            }
            let core = (DOT_CORE - dist + 0.5 / ppp).clamp(0.0, 1.0);
            if lit {
                // White hot in the middle, amber at the rim
                let heat = (1.0 - dist / DOT_CORE).clamp(0.0, 1.0).powi(2) * 0.6;
                let colour = [0, 1, 2].map(|i| AMBER[i] + (AMBER_HOT[i] - AMBER[i]) * heat);
                pixel.over(colour, core);
            } else {
                pixel.over(GHOST, core);
            }
            pixels[y * side + x] = pixel;
        }
    }
    image(side, side, pixels)
}

/// The window of an LED display filling `rect`
pub fn paint_window(painter: &Painter, rect: Rect) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let key = ("led-window", rect.width().to_bits(), rect.height().to_bits(), ppp.to_bits());
    let texture = cached(ctx, key, || window_image(rect.size(), ppp));
    blit(painter, &texture, rect.expand(device(WINDOW_MARGIN, ppp).round() / ppp));
}

/// A dot centred on `center`, lit `level` of the way (0 to 1): the unlit dot is always there
fn paint_dot(painter: &Painter, center: Pos2, level: f32) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    for lit in [false, true] {
        if lit && level <= 0.0 {
            continue;
        }
        let texture = cached(ctx, ("led-dot", lit, ppp.to_bits()), || dot_image(lit, ppp));
        let rect = Rect::from_center_size(center, Vec2::splat(2.0 * DOT_REACH));
        let tint = if lit { Color32::WHITE.gamma_multiply(level.min(1.0)) } else { Color32::WHITE };
        painter.image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            tint,
        );
    }
}

/// Every dot of the display in `window`, unlit
fn paint_grid(painter: &Painter, window: Rect, chars: usize) {
    for col in 0..columns(chars) {
        for row in 0..GLYPH_ROWS {
            paint_dot(painter, dot_center(window, col, row), 0.0);
        }
    }
}

/// An LED display of `chars` characters centred on `center` showing `text`, which is cut off
/// at its right end if it is longer. The text starts at the left, so a name being typed grows
/// to the right. Returns the window, for the caller's buttons and clicks
pub fn led_display(painter: &Painter, center: Pos2, chars: usize, text: &str) -> Rect {
    let window = Rect::from_center_size(center, vec2(led_width(chars), LED_HEIGHT));
    paint_window(painter, window);
    paint_grid(painter, window, chars);
    for (col, row) in text_dots(&text.to_uppercase(), chars) {
        paint_dot(painter, dot_center(window, col, row), 1.0);
    }
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
/// Columns of dots from one lamp of the needle to the next, and across one lamp
const NEEDLE_STEP: usize = 3;
const NEEDLE_WIDTH: usize = 2;

/// The tuner on an LED display of `chars` characters centred on `center`: the note's name at
/// its left end, and in the rest of it a needle of 17 lamps, flat to the left and sharp to the
/// right, the middle one tall and lit only when the note is in tune. Without a note the name is
/// two dashes and the lamps are dark. As high as every LED display
pub fn tuner_display(
    painter: &Painter,
    center: Pos2,
    chars: usize,
    note: Option<&str>,
    cents: Option<f32>,
) {
    let window = Rect::from_center_size(center, vec2(led_width(chars), LED_HEIGHT));
    paint_window(painter, window);
    paint_grid(painter, window, chars);

    for (col, row) in text_dots(note.unwrap_or("--"), TUNER_NOTE_CHARS) {
        paint_dot(painter, dot_center(window, col, row), 1.0);
    }
    // A divider between the name and the needle, in the gap between two columns
    let divider_col = TUNER_NOTE_CHARS * ADVANCE_COLS;
    let divider = dot_center(window, divider_col, 0).x - LED_PITCH / 2.0 - LED_PITCH;
    painter.line_segment(
        [
            Pos2::new(divider, window.top() + BEZEL + 3.0),
            Pos2::new(divider, window.bottom() - BEZEL - 3.0),
        ],
        nih_plug_egui::egui::Stroke::new(1.0, AMBER_DIM),
    );

    let lamps = 17;
    let needle = (lamps - 1) * NEEDLE_STEP + NEEDLE_WIDTH;
    let free = columns(chars) - divider_col;
    let first = divider_col + free.saturating_sub(needle) / 2;
    for lamp in -8..=8_i32 {
        let level = needle_brightness(lamp, cents);
        let col = first + (lamp + 8) as usize * NEEDLE_STEP;
        // The middle lamp is the whole height of the letters, the others the middle three rows
        let rows = if lamp == 0 { 0..GLYPH_ROWS } else { 2..5 };
        for row in rows {
            for across in 0..NEEDLE_WIDTH {
                paint_dot(painter, dot_center(window, col + across, row), level);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color32) -> f32 {
        0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32
    }

    #[test]
    fn test_every_letter_a_name_can_hold_has_a_glyph_of_its_own() {
        let blank = glyph(' ');
        let unknown = glyph('\u{2603}');
        assert_eq!(blank, [0; 7]);
        let shown =
            ('A'..='Z').chain('0'..='9').chain("ÅÆØÄÖÜÉ-_.+#&()~*?".chars()).collect::<Vec<char>>();
        for letter in shown {
            assert_ne!(glyph(letter), unknown, "{letter} is drawn as the unknown sign");
        }
        // Lower case is shown in capitals, as the tape did
        assert_eq!(glyph('a'), glyph('A'));
        // Every glyph stays within its five columns
        for letter in ('A'..='Z').chain('0'..='9').chain("ÅÆØ-_".chars()) {
            assert!(glyph(letter).iter().all(|&row| row < 32), "{letter}");
        }
    }

    #[test]
    fn test_text_lights_its_dots_where_its_letters_are_and_is_cut_at_the_end() {
        let dots = text_dots("I", 4);
        assert!(dots.iter().all(|&(col, row)| col < GLYPH_COLS && row < GLYPH_ROWS));
        // The bar of the I, from top to bottom
        assert!(dots.contains(&(2, 0)) && dots.contains(&(2, 6)));
        // The second letter starts after the first one's columns and the gap
        let two = text_dots("II", 4);
        assert!(two.iter().any(|&(col, _)| col >= ADVANCE_COLS));
        assert!(two.iter().all(|&(col, _)| col < 2 * ADVANCE_COLS - 1));
        // What does not fit is cut off, the caret of a name being typed is a line under it
        assert_eq!(text_dots("IIII", 2), text_dots("II", 2));
        let caret = text_dots("_", 1);
        assert_eq!(caret.len(), GLYPH_COLS);
        assert!(caret.iter().all(|&(_, row)| row == GLYPH_ROWS - 1));
    }

    #[test]
    fn test_a_display_is_as_wide_as_its_characters_and_as_high_as_every_other() {
        let step = ADVANCE_COLS as f32 * LED_PITCH;
        assert_eq!(led_width(2) - led_width(1), step);
        assert_eq!(led_width(16), 499.0);
        assert_eq!(led_width(10), 319.0);
        assert_eq!(LED_HEIGHT, 51.0);
    }

    #[test]
    fn test_a_lit_dot_is_amber_and_glows_and_an_unlit_one_is_dark() {
        let (lit, ghost) = (dot_image(true, 1.0), dot_image(false, 1.0));
        let middle = |image: &ColorImage| {
            image.pixels[image.size[1] / 2 * image.size[0] + image.size[0] / 2]
        };
        let color = middle(&lit);
        assert!(color.r() > 240 && color.r() > color.g() && color.g() > color.b());
        assert!(luminance(middle(&ghost)) < luminance(color) / 3.0);
        // Only a lit dot glows past its core
        let edge = |image: &ColorImage| image.pixels[image.size[1] / 2 * image.size[0] + 1].a();
        assert!(edge(&lit) > 0 && edge(&ghost) == 0);
    }

    #[test]
    fn test_the_window_is_a_dark_recess_with_a_shadow_round_it() {
        let window = window_image(vec2(120.0, 44.0), 1.0);
        let (w, h) = (window.size[0], window.size[1]);
        assert!(luminance(window.pixels[h / 2 * w + w / 2]) < 20.0);
        assert_eq!(window.pixels[0].a(), 0);
        // The bezel is lighter than the glass
        let bezel = window.pixels[h / 2 * w + WINDOW_MARGIN as usize + 1];
        assert!(luminance(bezel) > luminance(window.pixels[h / 2 * w + w / 2]));
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
        let chars = 16;
        let needle = 16 * NEEDLE_STEP + NEEDLE_WIDTH;
        assert!(TUNER_NOTE_CHARS * ADVANCE_COLS + needle <= columns(chars));
    }
}
