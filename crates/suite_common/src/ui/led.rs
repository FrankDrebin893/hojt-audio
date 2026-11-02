//! A built-in LED display: a black glass window with a readout in flat amber light, lit cell by
//! cell like the rest of `raster` (no image files). The cells that are not lit stay very faintly
//! visible, so the grid reads as a real display.
//!
//! The window and the cells are rendered once by `cached`; a readout is only a row of blits, so
//! it costs nothing until the text changes and the window is painted again.

use nih_plug_egui::egui::{vec2, Color32, ColorImage, Painter, Pos2, Rect, Vec2};

use super::raster::{blit, cached, device, image, rounded_box, smoothstep, Pixel, Rgb};

/// Space from one cell to the next, in points, across and down
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
const BEZEL: f32 = 2.0;
const WINDOW_RADIUS: f32 = 4.0;

// A cell of the readout is a square with sharp corners; `CELL_GAP` of the pitch is left dark
// between two of them, and a lit cell joins the lit one beside it and the one under it across the
// gap, so the strokes of a letter run on like the bars of a segment display. A lit cell has a
// thin even bloom of `BLOOM_REACH` points
const CELL_GAP: f32 = 2.0;
const BLOOM_REACH: f32 = 3.0;
const BLOOM: f32 = 0.30;
// The reflection on the glass: how far down it goes, as a part of the window, and how strong
const REFLECTION_END: f32 = 0.42;
const REFLECTION: f32 = 0.05;
/// Half the side of the picture of one cell, in points: the cell, the join and the bloom
const CELL_HALF: f32 = LED_PITCH / 2.0 + BLOOM_REACH;

const LIT: Rgb = [255.0, 186.0, 52.0];
const GHOST: Rgb = [30.0, 20.0, 12.0];
const GLASS: Rgb = [5.0, 4.0, 4.0];
/// The colour of anything drawn on the display that is not a cell
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

            // The glass: flat black, with a faint reflection over its upper part that ends in
            // a clean edge, the way a pane of glass over a display catches a window
            let glass = rounded_box(p, c, [half[0] - bezel, half[1] - bezel], radius - bezel);
            pixel.over(GLASS, (0.5 - glass).clamp(0.0, 1.0));
            let reflection = (0.5 - glass).clamp(0.0, 1.0)
                * smoothstep(0.0, 0.1, down)
                * smoothstep(0.0, -0.08, down - REFLECTION_END);
            pixel.over([255.0; 3], REFLECTION * reflection);
            pixels[y * width + x] = pixel;
        }
    }
    image(width, height, pixels)
}

/// How far `p` is from the box of half sides `half` centred on `c`: negative inside
fn box_distance(p: [f32; 2], c: [f32; 2], half: [f32; 2]) -> f32 {
    let q = [(p[0] - c[0]).abs() - half[0], (p[1] - c[1]).abs() - half[1]];
    (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt() + q[0].max(q[1]).min(0.0)
}

/// One cell of the display, as a square picture `2 * CELL_HALF` points across with the cell in
/// its middle: lit with its bloom, or unlit. A lit cell reaches across the gap to the lit cell
/// `right` of it and the one `down` under it
pub fn cell_image(lit: bool, right: bool, down: bool, ppp: f32) -> ColorImage {
    let side = (2.0 * CELL_HALF * ppp).round() as usize;
    let c = side as f32 / 2.0;
    let core = (LED_PITCH - CELL_GAP) / 2.0;
    let mut boxes = vec![([0.0, 0.0], [core, core])];
    // A join is as wide as the cell and runs on to the middle of the next one
    let reach = LED_PITCH / 2.0;
    if lit && right {
        boxes.push(([reach, 0.0], [reach, core]));
    }
    if lit && down {
        boxes.push(([0.0, reach], [core, reach]));
    }
    let mut pixels = vec![Pixel::default(); side * side];
    for y in 0..side {
        for x in 0..side {
            let p = [(x as f32 + 0.5 - c) / ppp, (y as f32 + 0.5 - c) / ppp];
            let dist = boxes
                .iter()
                .map(|&(center, half)| box_distance(p, center, half))
                .fold(f32::MAX, f32::min);
            let cover = (0.5 - dist * ppp).clamp(0.0, 1.0);
            let mut pixel = Pixel::default();
            if lit {
                pixel.over(LIT, BLOOM * (-(dist.max(0.0) / (BLOOM_REACH / 2.2)).powi(2)).exp());
                pixel.over(LIT, cover);
            } else {
                pixel.over(GHOST, cover);
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

/// The middle of a cell on the middle of a device pixel, so that its edges fall between two pixels
fn snapped(center: Pos2, ppp: f32) -> Pos2 {
    let snap = |at: f32| ((at * ppp - 0.5).round() + 0.5) / ppp;
    Pos2::new(snap(center.x), snap(center.y))
}

/// The picture of the cell `key` (see `cell_image`) on `center`, tinted `tint`
fn blit_cell(painter: &Painter, center: Pos2, lit: bool, right: bool, down: bool, tint: Color32) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let rect = Rect::from_center_size(snapped(center, ppp), Vec2::splat(2.0 * CELL_HALF));
    let key = ("led-cell", lit, right, down, ppp.to_bits());
    let texture = cached(ctx, key, || cell_image(lit, right, down, ppp));
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    painter.image(texture.id(), rect, uv, tint);
}

/// A lit cell centred on `center`, `level` of the way (0 to 1), joined to the lit cell `right` of
/// it and the one `down` under it. Over the unlit cell that `paint_grid` drew
fn paint_cell(painter: &Painter, center: Pos2, level: f32, right: bool, down: bool) {
    if level > 0.0 {
        blit_cell(
            painter,
            center,
            true,
            right,
            down,
            Color32::WHITE.gamma_multiply(level.min(1.0)),
        );
    }
}

/// Every cell of the display in `window`, unlit
fn paint_grid(painter: &Painter, window: Rect, chars: usize) {
    for col in 0..columns(chars) {
        for row in 0..GLYPH_ROWS {
            blit_cell(painter, dot_center(window, col, row), false, false, false, Color32::WHITE);
        }
    }
}

/// The lit cells `lit` of the display in `window`, each joined to its lit neighbours
fn paint_lit(painter: &Painter, window: Rect, lit: &[(usize, usize)]) {
    for &(col, row) in lit {
        let (right, down) = (lit.contains(&(col + 1, row)), lit.contains(&(col, row + 1)));
        paint_cell(painter, dot_center(window, col, row), 1.0, right, down);
    }
}

/// An LED display of `chars` characters centred on `center` showing `text`, which is cut off
/// at its right end if it is longer. The text starts at the left, so a name being typed grows
/// to the right. Returns the window, for the caller's buttons and clicks
pub fn led_display(painter: &Painter, center: Pos2, chars: usize, text: &str) -> Rect {
    let window = Rect::from_center_size(center, vec2(led_width(chars), LED_HEIGHT));
    paint_window(painter, window);
    paint_grid(painter, window, chars);
    paint_lit(painter, window, &text_dots(&text.to_uppercase(), chars));
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

    paint_lit(painter, window, &text_dots(note.unwrap_or("--"), TUNER_NOTE_CHARS));
    // A divider between the name and the needle, in the gap between two columns
    let divider_col = TUNER_NOTE_CHARS * ADVANCE_COLS;
    let divider = dot_center(window, divider_col, 0).x - LED_PITCH / 2.0 - LED_PITCH;
    painter.line_segment(
        [
            Pos2::new(divider, window.top() + BEZEL + 3.0),
            Pos2::new(divider, window.bottom() - BEZEL - 3.0),
        ],
        nih_plug_egui::egui::Stroke::new(1.0_f32, AMBER_DIM),
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
        let last = rows.end;
        for row in rows {
            for across in 0..NEEDLE_WIDTH {
                let (right, down) = (across + 1 < NEEDLE_WIDTH, row + 1 < last);
                paint_cell(painter, dot_center(window, col + across, row), level, right, down);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_middle(image: &ColorImage) -> Color32 {
        image.pixels[image.size[1] / 2 * image.size[0] + image.size[0] / 2]
    }

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
    fn test_a_lit_cell_is_flat_amber_with_sharp_corners_and_a_thin_bloom() {
        let (lit, ghost) =
            (cell_image(true, false, false, 1.0), cell_image(false, false, false, 1.0));
        let at = |image: &ColorImage, x: usize, y: usize| image.pixels[y * image.size[0] + x];
        let middle = image_middle(&lit);
        assert!(middle.r() > 240 && middle.r() > middle.g() && middle.g() > middle.b());
        // Flat: the corner of the cell is as lit as its middle, which a round dot is not
        let corner = at(&lit, 4, 4);
        assert_eq!((corner.r(), corner.g(), corner.b()), (middle.r(), middle.g(), middle.b()));
        // The unlit cell is only just there
        assert!(luminance(image_middle(&ghost)) < luminance(middle) / 8.0);
        // The bloom is thin: a little light in the gap, none at the edge of the picture
        let (gap, edge) = (at(&lit, 7, 5), at(&lit, 10, 5));
        assert!(gap.a() > 0 && gap.a() < 120, "the gap holds the bloom only: {}", gap.a());
        assert_eq!(edge.a(), 0);
        assert_eq!(at(&ghost, 7, 5).a(), 0);
    }

    #[test]
    fn test_a_lit_cell_runs_into_the_lit_cell_beside_it_and_under_it() {
        let alone = cell_image(true, false, false, 1.0);
        let joined = cell_image(true, true, true, 1.0);
        let at = |image: &ColorImage, x: usize, y: usize| image.pixels[y * image.size[0] + x];
        // In the gap to the right and the one below
        for (x, y) in [(7, 5), (5, 7)] {
            assert_eq!(at(&joined, x, y).a(), 255, "{x},{y}");
            assert!(at(&alone, x, y).a() < 255);
        }
        // Not to the left or above
        assert!(at(&joined, 3, 5).a() < 255 && at(&joined, 5, 3).a() < 255);
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
