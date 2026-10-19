//! Surfaces that are lit pixel by pixel instead of filled with a flat colour.
//!
//! Vector shapes cannot show what makes a real pedal look real: a bevel that catches the light,
//! the flake in powder coat, knurled and brushed metal, a soft shadow. Each function here renders
//! such a surface once, with a small software renderer (a signed distance field gives the shape
//! and its edge normals, one fixed light gives the shading), and `cached` keeps the result as an
//! egui texture. The renders are plain functions of their inputs, so tests can look at the pixels.
//!
//! All sizes in the public functions are in points; `ppp` (pixels per point) makes the texture
//! as sharp as the screen.

use std::hash::Hash;

use nih_plug_egui::egui::{
    pos2, Color32, ColorImage, Context, Id, Painter, Pos2, Rect, TextureHandle, TextureOptions,
    Vec2,
};

use super::theme::{CHROME, INK, KNOB_CAP_EDGE, KNOB_CAP_TOP, KNOB_SKIRT};

/// Room around a box for its shadow, in points
pub const BOX_SHADOW_MARGIN: f32 = 6.0;
/// Room around a knob for its shadow, in points
pub const KNOB_MARGIN: f32 = 11.0;

/// Direction to the light, from the top left and in front. Unit length
const LIGHT: [f32; 3] = [-0.447, -0.646, 0.617];
// Halfway between the light and the viewer
const HALF: [f32; 3] = [-0.250, -0.359, 0.899];

pub(super) type Rgb = [f32; 3];

pub(super) fn rgb(color: Color32) -> Rgb {
    [color.r() as f32, color.g() as f32, color.b() as f32]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let length = dot(v, v).sqrt().max(1e-6);
    [v[0] / length, v[1] / length, v[2] / length]
}

/// A surface normal tilted by `(x, y)`, which is the slope of the surface
pub(super) fn tilted(x: f32, y: f32) -> [f32; 3] {
    normalize([x, y, 1.0])
}

/// How much light the surface gets: 1 on a surface that faces the viewer, more or less elsewhere
pub(super) fn diffuse(normal: [f32; 3]) -> f32 {
    (dot(normal, LIGHT) / LIGHT[2]).max(0.0)
}

pub(super) fn specular(normal: [f32; 3], sharpness: f32) -> f32 {
    dot(normal, HALF).max(0.0).powf(sharpness)
}

pub(super) fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x9E37_79B1)
        ^ (y as u32).wrapping_mul(0x85EB_CA6B)
        ^ seed.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Smooth noise between 0 and 1 that changes over about one unit
pub(super) fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (smoothstep(0.0, 1.0, x - ix), smoothstep(0.0, 1.0, y - iy));
    let (ix, iy) = (ix as i32, iy as i32);
    let top = hash(ix, iy, seed) + (hash(ix + 1, iy, seed) - hash(ix, iy, seed)) * fx;
    let bottom =
        hash(ix, iy + 1, seed) + (hash(ix + 1, iy + 1, seed) - hash(ix, iy + 1, seed)) * fx;
    top + (bottom - top) * fy
}

/// Distance from `p` to the edge of a rounded rectangle centred on `c`: negative inside
pub(super) fn rounded_box(p: [f32; 2], c: [f32; 2], half: [f32; 2], radius: f32) -> f32 {
    let q = [(p[0] - c[0]).abs() - (half[0] - radius), (p[1] - c[1]).abs() - (half[1] - radius)];
    let outside = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt();
    outside + q[0].max(q[1]).min(0.0) - radius
}

/// A premultiplied pixel being built from layers
#[derive(Clone, Copy, Default)]
pub(super) struct Pixel {
    rgb: Rgb,
    alpha: f32,
}

impl Pixel {
    /// `color` laid over what is there, `coverage` of the way
    pub(super) fn over(&mut self, color: Rgb, coverage: f32) {
        let coverage = coverage.clamp(0.0, 1.0);
        for channel in 0..3 {
            self.rgb[channel] = color[channel] * coverage + self.rgb[channel] * (1.0 - coverage);
        }
        self.alpha = coverage + self.alpha * (1.0 - coverage);
    }

    pub(super) fn color(self) -> Color32 {
        let channel = |value: f32| value.clamp(0.0, 255.0).round() as u8;
        // `rgb` holds the premultiplied value already: `over` weighs it by coverage
        let alpha = (self.alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgba_premultiplied(
            channel(self.rgb[0]).min(alpha),
            channel(self.rgb[1]).min(alpha),
            channel(self.rgb[2]).min(alpha),
            alpha,
        )
    }
}

pub(super) fn image(width: usize, height: usize, pixels: Vec<Pixel>) -> ColorImage {
    ColorImage { size: [width, height], pixels: pixels.into_iter().map(Pixel::color).collect() }
}

/// Number of device pixels for a length in points
pub(super) fn device(points: f32, ppp: f32) -> f32 {
    points * ppp
}

pub(super) fn shaded(base: Rgb, light: f32, glint: f32) -> Rgb {
    // Painted metal gets less contrast than the light would give it
    let level = 0.55 + 0.45 * light.clamp(0.0, 2.0);
    [
        base[0] * level + 255.0 * glint,
        base[1] * level + 255.0 * glint,
        base[2] * level + 255.0 * glint,
    ]
}

/// A pedal enclosure in powder coat: a bevelled top face, the darker lower edge of the box and
/// a soft shadow on the bench. Returns the image and the margin around the box in pixels.
pub fn box_image(
    size: Vec2,
    radius: f32,
    edge_height: f32,
    paint: Color32,
    ppp: f32,
) -> (ColorImage, usize) {
    let margin = device(BOX_SHADOW_MARGIN, ppp).round() as usize;
    let box_w = device(size.x, ppp).round().max(1.0);
    let box_h = device(size.y, ppp).round().max(1.0);
    let (width, height) = (box_w as usize + 2 * margin, box_h as usize + 2 * margin);
    let m = margin as f32;
    let radius = device(radius, ppp);
    let edge = device(edge_height, ppp);
    let bevel = device(4.5, ppp);
    let blur = device(2.6, ppp);
    let base = rgb(paint);
    let side_base = [base[0] * 0.70, base[1] * 0.70, base[2] * 0.70];

    let centre = [m + box_w / 2.0, m + box_h / 2.0];
    let half = [box_w / 2.0, box_h / 2.0];
    let top_centre = [centre[0], centre[1] - edge / 2.0];
    let top_half = [half[0], half[1] - edge / 2.0];
    let top_distance = |p: [f32; 2]| rounded_box(p, top_centre, top_half, radius);

    let mut pixels = vec![Pixel::default(); width * height];
    for y in 0..height {
        for x in 0..width {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let mut pixel = Pixel::default();

            // Shadow, a little lower than the box
            let shadow = rounded_box([p[0], p[1] - device(1.6, ppp)], centre, half, radius);
            pixel.over([0.0, 0.0, 0.0], 0.5 * (-(shadow.max(0.0) / blur).powi(2)).exp());

            // The box seen from the front: a lower edge that gets darker towards the bench
            let outline = rounded_box(p, centre, half, radius);
            let down = ((p[1] - (centre[1] + half[1] - edge)) / edge.max(1.0)).clamp(0.0, 1.0);
            let side = [
                side_base[0] * (1.0 - 0.18 * down),
                side_base[1] * (1.0 - 0.18 * down),
                side_base[2] * (1.0 - 0.18 * down),
            ];
            pixel.over(side, (0.5 - outline).clamp(0.0, 1.0));

            // The top face
            let d = top_distance(p);
            let coverage = (0.5 - d).clamp(0.0, 1.0);
            if coverage > 0.0 {
                let inside = (-d).max(0.0);
                // The way outwards, from the slope of the distance
                let gx = top_distance([p[0] + 0.5, p[1]]) - top_distance([p[0] - 0.5, p[1]]);
                let gy = top_distance([p[0], p[1] + 0.5]) - top_distance([p[0], p[1] - 0.5]);
                // A rounded edge: steep at the rim, flat from `bevel` inwards
                let round = 1.0 - smoothstep(0.0, bevel, inside);
                let tilt = round * round * 1.8;
                let (mut nx, mut ny) = (gx * tilt, gy * tilt);
                // Powder coat: fine flake and a slow orange peel, both in the normal
                let flake = hash(x as i32, y as i32, 7) - 0.5;
                let peel_x =
                    value_noise(p[0] / device(3.0, ppp), p[1] / device(3.0, ppp), 11) - 0.5;
                let peel_y =
                    value_noise(p[0] / device(3.0, ppp), p[1] / device(3.0, ppp), 23) - 0.5;
                nx += peel_x * 0.07 + flake * 0.10;
                ny += peel_y * 0.07 + (hash(x as i32, y as i32, 13) - 0.5) * 0.10;
                let normal = tilted(nx, ny);
                // A soft sheen over the whole face, lighter at the top
                let sheen = 1.0 + 0.07 * (0.5 - (p[1] - m) / box_h) * 2.0;
                let glint = specular(normal, 38.0) * 0.45 * (0.35 + round);
                let mut color = shaded(base, diffuse(normal), glint);
                color = [color[0] * sheen, color[1] * sheen, color[2] * sheen];
                // A hairline of shadow where the top meets the lower edge
                let seam = smoothstep(0.0, device(1.5, ppp), inside);
                let dark = 0.80 + 0.20 * seam;
                color = [color[0] * dark, color[1] * dark, color[2] * dark];
                pixel.over(color, coverage);
            }
            pixels[y * width + x] = pixel;
        }
    }
    (image(width, height, pixels), margin)
}

/// The cap of a knob: what it is made of and how it is shaped. The image of a knob is round
/// with every cap; what turns (the pointer, or a chicken head's wing) is drawn over it
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KnobCap {
    /// Glossy black plastic with flutes down its skirt and a brushed dish on top: the knob
    /// of every pedal
    Fluted,
    /// Black bakelite: a low round base under a wing that points
    ChickenHead,
    /// The chicken-head in cream bakelite, with a dark line along its wing
    CreamChickenHead,
    /// A black knurled skirt under a domed cap of brushed gold
    GoldTop,
    /// A black domed cap in a flared skirt of turned chrome
    ChromeSkirt,
}

/// A knob with the cap `cap`, and its shadow. The pointer is not in it: the caller draws that,
/// since it turns.
pub fn knob_image(radius: f32, cap: KnobCap, ppp: f32) -> ColorImage {
    match cap {
        KnobCap::Fluted => fluted_knob(radius, ppp),
        KnobCap::ChickenHead => round_knob(radius, ppp, |at| chicken_head(at, BLACK_BAKELITE)),
        KnobCap::CreamChickenHead => round_knob(radius, ppp, |at| chicken_head(at, CREAM_BAKELITE)),
        KnobCap::GoldTop => round_knob(radius, ppp, gold_top),
        KnobCap::ChromeSkirt => round_knob(radius, ppp, chrome_skirt),
    }
}

/// A point of a knob's face: `t` from the middle (0) to the rim (1), (`ux`, `uy`) the way
/// outwards, `phi` its angle and `dist` its distance from the middle in pixels
#[derive(Clone, Copy)]
struct KnobPoint {
    t: f32,
    ux: f32,
    uy: f32,
    phi: f32,
    dist: f32,
    ppp: f32,
}

/// A round knob and its shadow, the colour of its face given by `face`
fn round_knob(radius: f32, ppp: f32, face: impl Fn(KnobPoint) -> Rgb) -> ColorImage {
    let side = ((radius + KNOB_MARGIN) * 2.0 * ppp).ceil() as usize;
    let c = side as f32 / 2.0;
    let r = device(radius, ppp);
    let shadow_offset = [device(0.4, ppp), device(1.2, ppp)];
    let tight = device(1.0, ppp);
    let soft = device(3.5, ppp);

    let mut pixels = vec![Pixel::default(); side * side];
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let dist = (dx * dx + dy * dy).sqrt();
            let mut pixel = Pixel::default();

            let (sx, sy) = (dx - shadow_offset[0], dy - shadow_offset[1]);
            let gap = ((sx * sx + sy * sy).sqrt() - r).max(0.0);
            // Where the knob meets the paint it is dark; a little further out only faintly so
            let contact = 0.55 * (-(gap / tight).powi(2)).exp();
            let ambient = 0.20 * (-(gap / soft).powi(2)).exp();
            pixel.over([0.0; 3], 1.0 - (1.0 - contact) * (1.0 - ambient));

            let coverage = (r - dist + 0.5).clamp(0.0, 1.0);
            if coverage > 0.0 {
                let t = dist / r;
                let phi = dy.atan2(dx);
                let color = face(KnobPoint { t, ux: phi.cos(), uy: phi.sin(), phi, dist, ppp });
                // The rim catches a little light all round
                let lip = smoothstep(0.93, 1.0, t) * 0.10 * 255.0;
                pixel.over([color[0] + lip, color[1] + lip, color[2] + lip], coverage);
            }
            pixels[y * side + x] = pixel;
        }
    }
    image(side, side, pixels)
}

/// The knob of every pedal: fluted black plastic with a brushed dish on top
fn fluted_knob(radius: f32, ppp: f32) -> ColorImage {
    let flutes = (radius * 0.8).round().max(8.0);
    let skirt = rgb(KNOB_SKIRT);
    let cap = rgb(KNOB_CAP_TOP);
    let rim = rgb(KNOB_CAP_EDGE);
    round_knob(radius, ppp, |KnobPoint { t, ux, uy, phi, dist, ppp }| {
        if t > 0.7 {
            // Skirt: flutes run up and down it, and it rounds off at the rim
            let out = (t - 0.7) / 0.3;
            let round = out.powi(3) * 1.1;
            let flute = (flutes * phi).sin() * 0.55;
            let normal = tilted(ux * round - uy * flute, uy * round + ux * flute);
            let base = [skirt[0] * 1.6, skirt[1] * 1.6, skirt[2] * 1.6];
            shaded(base, diffuse(normal), specular(normal, 26.0) * 0.5)
        } else {
            // Cap: a shallow dome with a bevel at its rim and a brushed dish in the middle
            let edge = smoothstep(0.62, 0.70, t);
            let slope = t / 0.7 * 0.18 + edge * 1.2;
            let normal = tilted(ux * slope, uy * slope);
            let brush = value_noise(dist * 1.6 / ppp.max(1.0), 0.5, 5) - 0.5;
            // Two soft bright bands across the turned face, as on turned metal
            let band = (0.5 + 0.5 * (2.0 * (phi - 2.2)).cos()).powi(6);
            let dish = 1.0 - smoothstep(0.50, 0.54, t);
            let base = [
                cap[0] + (rim[0] - cap[0]) * edge,
                cap[1] + (rim[1] - cap[1]) * edge,
                cap[2] + (rim[2] - cap[2]) * edge,
            ];
            let mut color = shaded(base, diffuse(normal), specular(normal, 30.0) * 0.4);
            let shine = (band * 0.30 + brush * 0.10) * dish * 255.0 * (1.0 - t);
            color = [color[0] + shine, color[1] + shine, color[2] + shine];
            // Dark groove between the cap and the skirt
            let groove = 1.0 - 0.55 * (1.0 - smoothstep(0.0, 0.025, (t - 0.70).abs()));
            [color[0] * groove, color[1] * groove, color[2] * groove]
        }
    })
}

/// `color` with a dark groove round the face at `at`, `width` wide (both in `t`)
fn groove(color: Rgb, t: f32, at: f32, width: f32, depth: f32) -> Rgb {
    let dark = 1.0 - depth * (1.0 - smoothstep(0.0, width, (t - at).abs()));
    [color[0] * dark, color[1] * dark, color[2] * dark]
}

/// Black bakelite: a low glossy base that rounds off at its rim. The wing is drawn over it
/// The bakelite of a chicken-head knob: black, and cream
const BLACK_BAKELITE: Rgb = [30.0, 27.0, 25.0];
const CREAM_BAKELITE: Rgb = [
    CREAM_BAKELITE_COLOR.r() as f32,
    CREAM_BAKELITE_COLOR.g() as f32,
    CREAM_BAKELITE_COLOR.b() as f32,
];
/// The cream bakelite of a `CreamChickenHead`, its base and its wing
pub(super) const CREAM_BAKELITE_COLOR: Color32 = Color32::from_rgb(216, 204, 178);

fn chicken_head(KnobPoint { t, ux, uy, .. }: KnobPoint, bakelite: Rgb) -> Rgb {
    let slope = if t > 0.78 { ((t - 0.78) / 0.22).powi(2) * 1.4 } else { t * 0.22 };
    let normal = tilted(ux * slope, uy * slope);
    let color = shaded(bakelite, diffuse(normal), specular(normal, 44.0) * 0.55);
    groove(color, t, 0.78, 0.03, 0.45)
}

/// A domed cap of brushed gold in a black skirt with a fine knurl
fn gold_top(KnobPoint { t, ux, uy, phi, dist, ppp }: KnobPoint) -> Rgb {
    if t > 0.72 {
        let out = (t - 0.72) / 0.28;
        let round = out.powi(2) * 1.2;
        let knurl = (64.0 * phi).sin() * 0.25;
        let normal = tilted(ux * round - uy * knurl, uy * round + ux * knurl);
        let skirt = rgb(KNOB_SKIRT);
        let base = [skirt[0] * 1.6, skirt[1] * 1.6, skirt[2] * 1.6];
        shaded(base, diffuse(normal), specular(normal, 26.0) * 0.45)
    } else {
        let gold = [196.0, 156.0, 72.0];
        let edge = smoothstep(0.62, 0.72, t);
        let slope = t / 0.72 * 0.35 + edge;
        let normal = tilted(ux * slope, uy * slope);
        // Turned rings, and the light caught across them
        let rings = value_noise(dist * 2.2 / ppp.max(1.0), 0.5, 17) - 0.5;
        let band = (0.5 + 0.5 * (2.0 * (phi - 2.2)).cos()).powi(4);
        let mut color = shaded(gold, diffuse(normal) + rings * 0.25, specular(normal, 22.0) * 0.5);
        let shine = band * 0.22 * 255.0 * (1.0 - t);
        color = [color[0] + shine, color[1] + shine * 0.9, color[2] + shine * 0.6];
        groove(color, t, 0.72, 0.025, 0.6)
    }
}

/// A glossy black dome in a flared skirt of turned chrome
fn chrome_skirt(KnobPoint { t, ux, uy, phi, dist, ppp }: KnobPoint) -> Rgb {
    if t > 0.6 {
        // The skirt falls away outwards and rounds off at its rim
        let out = (t - 0.6) / 0.4;
        let slope = 0.55 + out.powi(4) * 1.2;
        let normal = tilted(ux * slope, uy * slope);
        let rings = value_noise(dist * 2.5 / ppp.max(1.0), 0.5, 19) - 0.5;
        let band = (0.5 + 0.5 * (2.0 * (phi - 2.2)).cos()).powi(5);
        let glint = specular(normal, 18.0) * 0.6 + band * 0.25;
        let color = shaded(rgb(CHROME), diffuse(normal) + rings * 0.2, glint);
        // A reflected horizon: the top of the skirt is lighter than its bottom
        let sky = 1.0 - 0.12 * uy;
        groove([color[0] * sky, color[1] * sky, color[2] * sky], t, 0.6, 0.03, 0.6)
    } else {
        let black = [34.0, 31.0, 28.0];
        let slope = (t / 0.6).powi(2) * 0.8;
        let normal = tilted(ux * slope, uy * slope);
        shaded(black, diffuse(normal), specular(normal, 40.0) * 0.55)
    }
}

/// A slotted screw head in chrome: domed, with a slot at `slot_angle`, and its shadow
pub fn screw_image(scale: f32, slot_angle: f32, ppp: f32) -> ColorImage {
    let radius = 6.0 * scale;
    let side = ((radius + 3.0) * 2.0 * ppp).ceil() as usize;
    let c = side as f32 / 2.0;
    let r = device(radius, ppp);
    let chrome = rgb(CHROME);
    let ink = rgb(INK);
    let slot_dir = [slot_angle.cos(), slot_angle.sin()];
    let slot_len = device(4.0 * scale, ppp);
    let slot_half = device(0.8 * scale, ppp).max(0.6);

    let mut pixels = vec![Pixel::default(); side * side];
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let dist = (dx * dx + dy * dy).sqrt();
            let mut pixel = Pixel::default();

            let shadow = (dx * dx + (dy - device(1.0, ppp)).powi(2)).sqrt() - r;
            pixel.over([0.0; 3], 0.45 * (-(shadow.max(0.0) / device(1.4, ppp)).powi(2)).exp());

            let coverage = (r - dist + 0.5).clamp(0.0, 1.0);
            if coverage > 0.0 {
                let t = dist / r;
                let slope = t * t * 0.9;
                let (ux, uy) = if dist > 0.0 { (dx / dist, dy / dist) } else { (0.0, 0.0) };
                let normal = tilted(ux * slope, uy * slope);
                let brush = (hash(x as i32, y as i32, 3) - 0.5) * 0.08;
                let mut color =
                    shaded(chrome, diffuse(normal) + brush, specular(normal, 20.0) * 0.55);
                // The slot: its distance from the line through the middle
                let along = dx * slot_dir[0] + dy * slot_dir[1];
                let across = (-dx * slot_dir[1] + dy * slot_dir[0]).abs();
                let slot = if along.abs() <= slot_len {
                    (slot_half + 0.5 - across).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                color = [
                    color[0] + (ink[0] - color[0]) * slot,
                    color[1] + (ink[1] - color[1]) * slot,
                    color[2] + (ink[2] - color[2]) * slot,
                ];
                // A thin dark ring where the head meets the paint
                let ring = smoothstep(0.88, 1.0, t) * 0.55;
                let color =
                    [color[0] * (1.0 - ring), color[1] * (1.0 - ring), color[2] * (1.0 - ring)];
                pixel.over(color, coverage);
            }
            pixels[y * side + x] = pixel;
        }
    }
    image(side, side, pixels)
}

/// Room around a covered box for its shadow, in points
pub const TOLEX_MARGIN: f32 = 6.0;
/// Room around a plate for its shadow, in points
pub const PLATE_MARGIN: f32 = 3.0;

/// The vinyl a box is covered in
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tolex {
    /// A fine pebbled grain
    Levant,
    /// A coarse, irregular grain
    Bronco,
    /// Smooth vinyl with a soft gloss
    Smooth,
    /// Lacquered twill: light threads crossed by darker ones on the diagonal
    Tweed,
}

/// The cloth or metal in front of the speakers
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Grille {
    /// Bundles of three threads woven over and under each other in blocks
    Basketweave,
    /// A fine weave of light and brown threads
    Wheat,
    /// A fine weave of black and white threads at random
    SaltPepper,
    /// A fine weave with a metallic thread that catches the light
    Silver,
    /// Matte felt with no weave
    Felt,
    /// Perforated steel, the speakers seen through its holes
    Mesh,
}

/// A plate the controls are mounted on
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Plate {
    BrushedBrass,
    BrushedChrome,
    BlackAnodised,
    /// Lacquered dark wood
    DarkWood,
}

impl Plate {
    /// The colour of the plate before it is lit: for the wood, its lighter rings. What is
    /// printed on the plate must read on this
    pub fn color(self) -> Color32 {
        match self {
            Plate::BrushedBrass => Color32::from_rgb(192, 158, 86),
            Plate::BrushedChrome => Color32::from_rgb(196, 194, 188),
            Plate::BlackAnodised => Color32::from_rgb(40, 40, 42),
            Plate::DarkWood => Color32::from_rgb(92, 54, 30),
        }
    }
}

/// A speaker behind a grille: its centre, in points from the grille's top left corner, and
/// its radius
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cone {
    pub center: Vec2,
    pub radius: f32,
}

/// What a face is like at one point: its colour, how its grain tilts it, and how it shines
#[derive(Clone, Copy)]
struct Surface {
    color: Rgb,
    slope: [f32; 2],
    shine: f32,
    sharpness: f32,
}

fn scaled(color: Rgb, by: f32) -> Rgb {
    [color[0] * by, color[1] * by, color[2] * by]
}

fn tinted(color: Rgb, by: Rgb) -> Rgb {
    [color[0] * by[0], color[1] * by[1], color[2] * by[2]]
}

/// A rounded rectangle `size` points large, lit point by point: `surface` says what it is like
/// at a point (given in points from its top left corner, and as the pixel it falls in). Its rim
/// rounds off over `bevel` points, and in a `margin` round it falls a shadow `shadow` dark.
/// Returns the image and the margin round the face in pixels
#[allow(clippy::too_many_arguments)]
fn face_image(
    size: Vec2,
    radius: f32,
    bevel: f32,
    margin: f32,
    shadow: f32,
    ppp: f32,
    surface: impl Fn([f32; 2], [i32; 2]) -> Surface + Sync,
) -> (ColorImage, usize) {
    let margin_px = device(margin, ppp).round() as usize;
    let face_w = device(size.x, ppp).round().max(1.0);
    let face_h = device(size.y, ppp).round().max(1.0);
    let (width, height) = (face_w as usize + 2 * margin_px, face_h as usize + 2 * margin_px);
    let m = margin_px as f32;
    let radius = device(radius, ppp);
    let bevel = device(bevel, ppp).max(1.0);
    let blur = device(margin * 0.43, ppp).max(0.5);
    let drop = device(margin * 0.27, ppp);
    let centre = [m + face_w / 2.0, m + face_h / 2.0];
    let half = [face_w / 2.0, face_h / 2.0];
    let distance = |p: [f32; 2]| rounded_box(p, centre, half, radius);

    let render_row = |y: usize, row: &mut [Pixel]| {
        for x in 0..width {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let mut pixel = Pixel::default();
            let d = distance(p);
            let coverage = (0.5 - d).clamp(0.0, 1.0);
            // The shadow only shows where the face does not cover it
            if shadow > 0.0 && coverage < 1.0 {
                let d = rounded_box([p[0], p[1] - drop], centre, half, radius);
                pixel.over([0.0; 3], shadow * (-(d.max(0.0) / blur).powi(2)).exp());
            }
            if coverage > 0.0 {
                let inside = (-d).max(0.0);
                // The rim rounds off: steep at the edge, flat from `bevel` inwards
                let round = 1.0 - smoothstep(0.0, bevel, inside);
                let tilt = round * round * 1.6;
                let (gx, gy) = if tilt > 0.0 {
                    (
                        distance([p[0] + 0.5, p[1]]) - distance([p[0] - 0.5, p[1]]),
                        distance([p[0], p[1] + 0.5]) - distance([p[0], p[1] - 0.5]),
                    )
                } else {
                    (0.0, 0.0)
                };
                let at = [(p[0] - m) / ppp, (p[1] - m) / ppp];
                let s = surface(at, [x as i32, y as i32]);
                let normal = tilted(gx * tilt + s.slope[0], gy * tilt + s.slope[1]);
                let glint = specular(normal, s.sharpness) * s.shine * (0.5 + round);
                let color = shaded(s.color, diffuse(normal), glint);
                // A hairline of shadow right at the edge
                let seam = 0.8 + 0.2 * smoothstep(0.0, device(1.0, ppp), inside);
                pixel.over(scaled(color, seam), coverage);
            }
            row[x] = pixel;
        }
    };

    // A large face is many pixels: its rows are shared out over the cores
    let mut pixels = vec![Pixel::default(); width * height];
    // A small face is quicker done here than handed out
    let threads = if width * height < 65_536 {
        1
    } else {
        std::thread::available_parallelism().map_or(1, |n| n.get()).min(8)
    };
    let rows_each = height.div_ceil(threads).max(1);
    if threads == 1 {
        for (y, row) in pixels.chunks_mut(width).enumerate() {
            render_row(y, row);
        }
        return (image(width, height, pixels), margin_px);
    }
    std::thread::scope(|scope| {
        for (chunk, rows) in pixels.chunks_mut(rows_each * width).enumerate() {
            let render_row = &render_row;
            scope.spawn(move || {
                for (i, row) in rows.chunks_mut(width).enumerate() {
                    render_row(chunk * rows_each + i, row);
                }
            });
        }
    });
    (image(width, height, pixels), margin_px)
}

/// Points scattered one to a unit cell: from `(x, y)`, the offset to the nearest and the
/// distances to the nearest and the second nearest
fn cells(x: f32, y: f32, seed: u32) -> ([f32; 2], f32, f32) {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (mut offset, mut nearest, mut second) = ([0.0; 2], f32::MAX, f32::MAX);
    for cy in iy - 1..=iy + 1 {
        for cx in ix - 1..=ix + 1 {
            // One hash gives the point both its coordinates
            let h = hash(cx, cy, seed);
            let px = cx as f32 + 0.15 + 0.7 * h;
            let py = cy as f32 + 0.15 + 0.7 * (h * 4096.0).fract();
            let d = [x - px, y - py];
            let square = d[0] * d[0] + d[1] * d[1];
            if square < nearest {
                second = nearest;
                nearest = square;
                offset = d;
            } else if square < second {
                second = square;
            }
        }
    }
    (offset, nearest.sqrt(), second.sqrt())
}

/// Pebbled vinyl: a dome round every point of `cells` `size` points apart, a crevice between
fn pebbles(base: Rgb, q: [f32; 2], size: f32, height: f32, crevice: f32, seed: u32) -> Surface {
    let (offset, nearest, second) = cells(q[0] / size, q[1] / size, seed);
    let deep = 1.0 - smoothstep(0.0, crevice, second - nearest);
    // A slow mottle, as no roll of vinyl is even
    let mottle = (value_noise(q[0] / 9.0, q[1] / 9.0, seed + 2) - 0.5) * 0.08;
    Surface {
        color: scaled(base, 1.0 - 0.32 * deep + mottle),
        slope: [offset[0] * height * (1.0 - deep), offset[1] * height * (1.0 - deep)],
        shine: 0.35,
        sharpness: 16.0,
    }
}

/// One thread of a weave: its colour, `across` it from 0 to 1, and which way it runs
fn thread(color: Rgb, across: f32, vertical: bool, round: f32) -> (Rgb, [f32; 2]) {
    let off = across - 0.5;
    // Darker in the gap to the next thread
    let gap = 1.0 - 0.4 * smoothstep(0.34, 0.5, off.abs());
    let slope = off * round;
    (scaled(color, gap), if vertical { [slope, 0.0] } else { [0.0, slope] })
}

/// A weave of threads `pitch` points apart, each thread over one and under one: whether the
/// vertical thread is on top at `q`, the index of the thread on top, and how far across it
fn plain_weave(q: [f32; 2], pitch: f32) -> (bool, i32, f32) {
    let (u, v) = (q[0] / pitch, q[1] / pitch);
    let (i, j) = (u.floor() as i32, v.floor() as i32);
    if (i + j).rem_euclid(2) == 0 {
        (true, i, u - i as f32)
    } else {
        (false, j, v - j as f32)
    }
}

fn tolex_at(tolex: Tolex, base: Rgb, q: [f32; 2], px: [i32; 2]) -> Surface {
    match tolex {
        Tolex::Levant => pebbles(base, q, 1.0, 0.9, 0.2, 61),
        Tolex::Bronco => {
            let mut surface = pebbles(base, q, 2.2, 0.8, 0.25, 67);
            let fine = value_noise(q[0] * 1.5, q[1] * 1.5, 71) - 0.5;
            surface.slope[0] += fine * 0.3;
            surface.slope[1] += (value_noise(q[0] * 1.5, q[1] * 1.5, 73) - 0.5) * 0.3;
            surface.shine = 0.28;
            surface
        }
        Tolex::Smooth => {
            let peel = |seed| (value_noise(q[0] / 4.0, q[1] / 4.0, seed) - 0.5) * 0.06;
            let speck = (hash(px[0], px[1], 79) - 0.5) * 0.02;
            Surface {
                color: scaled(base, 1.0 + speck),
                slope: [peel(75), peel(77)],
                shine: 0.6,
                sharpness: 40.0,
            }
        }
        Tolex::Tweed => {
            // A twill: each thread floats over two and under two, a step further on every
            // row, so the darker threads run on the diagonal
            let pitch = 1.2;
            let (u, v) = (q[0] / pitch, q[1] / pitch);
            let (i, j) = (u.floor() as i32, v.floor() as i32);
            let light = scaled(base, 0.92 + 0.16 * hash(i, 0, 83));
            let dark = scaled(tinted(base, [0.55, 0.47, 0.36]), 0.9 + 0.2 * hash(0, j, 85));
            let (color, slope) = if (i - j).rem_euclid(4) < 2 {
                thread(light, u - i as f32, true, 1.6)
            } else {
                thread(dark, v - j as f32, false, 1.6)
            };
            // Under the lacquer, a little yellower
            Surface { color: tinted(color, [1.0, 0.96, 0.85]), slope, shine: 0.55, sharpness: 30.0 }
        }
    }
}

/// How much darker a cloth is at `q` for the speakers behind it: faint discs, a little darker
/// still where the surround is
fn behind_cloth(q: [f32; 2], cones: &[Cone]) -> f32 {
    let mut tone = 1.0_f32;
    for cone in cones {
        let d = ((q[0] - cone.center.x).powi(2) + (q[1] - cone.center.y).powi(2)).sqrt();
        let inside = 1.0 - smoothstep(cone.radius - 2.0, cone.radius + 2.0, d);
        let ring = 1.0 - smoothstep(0.0, 2.5, (d - cone.radius * 0.92).abs());
        tone = tone.min(1.0 - 0.13 * inside - 0.05 * ring);
    }
    tone
}

/// What is seen through the holes of a mesh at `q`: a speaker, or the black baffle
fn behind_mesh(q: [f32; 2], cones: &[Cone]) -> Rgb {
    for cone in cones {
        let (dx, dy) = (q[0] - cone.center.x, q[1] - cone.center.y);
        let d = (dx * dx + dy * dy).sqrt();
        let t = d / cone.radius;
        if t >= 1.06 {
            continue;
        }
        if t >= 1.0 {
            // The rim of the basket
            return [44.0, 42.0, 40.0];
        }
        let (ux, uy) = if d > 0.0 { (dx / d, dy / d) } else { (0.0, 0.0) };
        let (slope, base) = if t < 0.24 {
            // The dust cap, domed
            (t / 0.24 * 0.7, [54.0, 50.0, 46.0])
        } else if t < 0.84 {
            // The paper cone falls away inwards
            (-0.45, [42.0, 38.0, 34.0])
        } else {
            // The roll of the surround
            ((t - 0.92) / 0.08 * 1.1, [30.0, 28.0, 26.0])
        };
        let normal = tilted(ux * slope, uy * slope);
        return shaded(base, diffuse(normal), specular(normal, 16.0) * 0.15);
    }
    [14.0, 13.0, 12.0]
}

fn grille_at(grille: Grille, base: Rgb, q: [f32; 2], px: [i32; 2], cones: &[Cone]) -> Surface {
    let cloth = |color: Rgb, slope: [f32; 2], shine: f32, sharpness: f32| Surface {
        color: scaled(color, behind_cloth(q, cones)),
        slope,
        shine,
        sharpness,
    };
    match grille {
        Grille::Basketweave => {
            // Blocks of three threads, the blocks turning a quarter from one to the next
            let pitch = 0.9;
            let block = 3.0 * pitch;
            let (bi, bj) = ((q[0] / block).floor() as i32, (q[1] / block).floor() as i32);
            let vertical = (bi + bj).rem_euclid(2) == 0;
            let (across, along) = if vertical { (q[0], q[1]) } else { (q[1], q[0]) };
            let index = (across / pitch).floor() as i32;
            let tone = 0.9 + 0.2 * hash(index, vertical as i32, 91);
            let (color, slope) =
                thread(scaled(base, tone), across / pitch - index as f32, vertical, 1.3);
            // Each block dips where it goes under the next
            let end = (along / block).fract() - 0.5;
            let dip = 1.0 - 0.3 * smoothstep(0.38, 0.5, end.abs());
            cloth(scaled(color, dip), slope, 0.15, 10.0)
        }
        Grille::Wheat => {
            let (vertical, index, across) = plain_weave(q, 0.8);
            let tone = if vertical { 1.12 } else { 0.72 } + 0.1 * hash(index, vertical as i32, 93);
            let (color, slope) = thread(scaled(base, tone), across, vertical, 1.2);
            cloth(color, slope, 0.12, 10.0)
        }
        Grille::SaltPepper => {
            let pitch = 0.75;
            let (vertical, index, across) = plain_weave(q, pitch);
            // Black and white yarn spun together: every crossing is one or the other, so it
            // speckles instead of running in stripes
            let along = ((if vertical { q[1] } else { q[0] }) / pitch).floor() as i32;
            let white = hash(index * 2 + vertical as i32, along, 95) > 0.5;
            let color = if white { [214.0, 208.0, 196.0] } else { base };
            let (color, slope) = thread(color, across, vertical, 1.2);
            cloth(color, slope, 0.12, 10.0)
        }
        Grille::Silver => {
            let (vertical, index, across) = plain_weave(q, 0.85);
            let glint = hash(index, vertical as i32, 97);
            let (color, slope) = thread(scaled(base, 0.85 + 0.3 * glint), across, vertical, 1.3);
            let shine = if glint > 0.8 { 1.0 } else { 0.55 };
            cloth(color, slope, shine, 30.0)
        }
        Grille::Felt => {
            let fibre = value_noise(q[0] * 2.5, q[1] * 2.5, 99) - 0.5;
            let speck = hash(px[0], px[1], 101) - 0.5;
            let slope =
                [(hash(px[0], px[1], 103) - 0.5) * 0.15, (hash(px[0], px[1], 105) - 0.5) * 0.15];
            cloth(scaled(base, 1.0 + fibre * 0.14 + speck * 0.12), slope, 0.04, 8.0)
        }
        Grille::Mesh => {
            // Round holes on a hexagonal grid
            let (pitch, hole) = (3.0, 1.05);
            let row_height = pitch * 0.866;
            let first = (q[1] / row_height).floor() as i32;
            let (mut nearest, mut to) = (f32::MAX, [0.0_f32; 2]);
            for row in first..=first + 1 {
                let shift = if row.rem_euclid(2) == 1 { pitch / 2.0 } else { 0.0 };
                let col = ((q[0] - shift) / pitch).round();
                let c = [col * pitch + shift, row as f32 * row_height];
                let d = [q[0] - c[0], q[1] - c[1]];
                let dist = (d[0] * d[0] + d[1] * d[1]).sqrt();
                if dist < nearest {
                    nearest = dist;
                    to = d;
                }
            }
            if nearest < hole {
                // Through the hole; its upper left part lies in the steel's shadow
                let shaded_by = [to[0] - 0.3, to[1] - 0.45];
                let lit = (shaded_by[0].powi(2) + shaded_by[1].powi(2)).sqrt() < hole;
                let color = scaled(behind_mesh(q, cones), if lit { 1.0 } else { 0.55 });
                return Surface { color, slope: [0.0; 2], shine: 0.0, sharpness: 1.0 };
            }
            // The steel rounds down into every hole
            let rim = (1.0 - smoothstep(hole, hole + 0.45, nearest)) * 1.2;
            let (ux, uy) = (to[0] / nearest, to[1] / nearest);
            let brush = (hash(px[0], px[1], 107) - 0.5) * 0.06;
            Surface {
                color: scaled(base, 1.0 + brush),
                slope: [-ux * rim, -uy * rim],
                shine: 0.45,
                sharpness: 24.0,
            }
        }
    }
}

fn plate_at(plate: Plate, q: [f32; 2], px: [i32; 2]) -> Surface {
    let brushed = |base: Rgb| {
        // Fine lines along the plate, each a little lighter or darker than the next
        let streak = (value_noise(q[0] / 24.0, q[1] / 0.22, 81) - 0.5) * 0.5
            + (hash(0, px[1], 87) - 0.5) * 0.3;
        Surface {
            color: scaled(base, 1.0 + streak * 0.18),
            slope: [0.0, streak * 0.12],
            shine: 0.5,
            sharpness: 14.0,
        }
    };
    match plate {
        Plate::BrushedBrass | Plate::BrushedChrome => brushed(rgb(plate.color())),
        Plate::BlackAnodised => {
            let satin = (hash(px[0], px[1], 109) - 0.5) * 0.12;
            Surface {
                color: scaled(rgb(plate.color()), 1.0 + satin),
                slope: [0.0; 2],
                shine: 0.2,
                sharpness: 10.0,
            }
        }
        Plate::DarkWood => {
            // Growth rings, bent by a slow wander, and pores along the grain
            let grain = q[1] * 0.55
                + (value_noise(q[0] / 40.0, q[1] / 4.0, 83) - 0.5) * 5.0
                + (value_noise(q[0] / 7.0, q[1] / 1.2, 89) - 0.5) * 0.6;
            let ring = smoothstep(0.5, 0.95, grain.rem_euclid(1.0));
            let (early, late) = (rgb(plate.color()), [52.0, 28.0, 14.0]);
            let pores = 1.0 + (value_noise(q[0] / 6.0, q[1] / 0.3, 111) - 0.5) * 0.12;
            let color = [
                (early[0] + (late[0] - early[0]) * ring) * pores,
                (early[1] + (late[1] - early[1]) * ring) * pores,
                (early[2] + (late[2] - early[2]) * ring) * pores,
            ];
            Surface { color, slope: [0.0; 2], shine: 0.4, sharpness: 30.0 }
        }
    }
}

/// A box covered in `tolex` of `color`: the vinyl wraps its rounded edges, and it throws a
/// shadow on the bench. Returns the image and the margin round the box in pixels
pub fn tolex_image(
    size: Vec2,
    radius: f32,
    tolex: Tolex,
    color: Color32,
    ppp: f32,
) -> (ColorImage, usize) {
    let base = rgb(color);
    face_image(size, radius, 7.0, TOLEX_MARGIN, 0.5, ppp, |q, px| tolex_at(tolex, base, q, px))
}

/// A grille of `grille` in `color` over a baffle, `cones` behind it: faint darker discs behind
/// a cloth, the speakers themselves through a mesh. It lies in what is round it, so it has no
/// shadow of its own
pub fn grille_image(
    size: Vec2,
    radius: f32,
    grille: Grille,
    color: Color32,
    cones: &[Cone],
    ppp: f32,
) -> ColorImage {
    let base = rgb(color);
    face_image(size, radius, 2.5, 0.0, 0.0, ppp, |q, px| grille_at(grille, base, q, px, cones)).0
}

/// A plate of `plate` with a narrow bevel and a small shadow. Returns the image and the
/// margin round the plate in pixels
pub fn plate_image(size: Vec2, radius: f32, plate: Plate, ppp: f32) -> (ColorImage, usize) {
    face_image(size, radius, 1.2, PLATE_MARGIN, 0.4, ppp, |q, px| plate_at(plate, q, px))
}

/// The texture for `key`: rendered by `render` the first time and kept by the context after that
pub(super) fn cached(
    ctx: &Context,
    key: impl Hash,
    render: impl FnOnce() -> ColorImage,
) -> TextureHandle {
    let id = Id::new(("raster", key));
    if let Some(texture) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return texture;
    }
    let texture = ctx.load_texture("raster", render(), TextureOptions::LINEAR);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

pub(super) fn blit(painter: &Painter, texture: &TextureHandle, rect: Rect) {
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

/// Paint `rect` as a bevelled box in powder coat, with its lower edge `edge_height` tall and a
/// shadow around it
pub fn paint_box(painter: &Painter, rect: Rect, radius: f32, edge_height: f32, paint: Color32) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let key = (
        "box",
        rect.width().to_bits(),
        rect.height().to_bits(),
        radius.to_bits(),
        edge_height.to_bits(),
        paint,
        ppp.to_bits(),
    );
    let mut margin = 0;
    let texture = cached(ctx, key, || {
        let (image, m) = box_image(rect.size(), radius, edge_height, paint, ppp);
        margin = m;
        image
    });
    // On a cache hit the margin was not rendered: it follows from the sizes
    let margin = if margin == 0 { device(BOX_SHADOW_MARGIN, ppp).round() as usize } else { margin };
    blit(painter, &texture, rect.expand(margin as f32 / ppp));
}

/// A knob's body with the cap `cap`, and its shadow, centred on `center`
pub fn paint_knob(painter: &Painter, center: Pos2, radius: f32, cap: KnobCap) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let key = ("knob", cap, radius.to_bits(), ppp.to_bits());
    let texture = cached(ctx, key, || knob_image(radius, cap, ppp));
    blit(
        painter,
        &texture,
        Rect::from_center_size(center, Vec2::splat((radius + KNOB_MARGIN) * 2.0)),
    );
}

/// A screw head and its shadow centred on `center`
pub fn paint_screw(painter: &Painter, center: Pos2, slot_angle: f32, scale: f32) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let key = ("screw", scale.to_bits(), (slot_angle * 100.0).round() as i32, ppp.to_bits());
    let texture = cached(ctx, key, || screw_image(scale, slot_angle, ppp));
    blit(painter, &texture, Rect::from_center_size(center, Vec2::splat((6.0 * scale + 3.0) * 2.0)));
}

/// A face rendered by `render` once for its `key`, laid on `rect` with `margin` points round it
fn paint_face(
    painter: &Painter,
    rect: Rect,
    margin: f32,
    key: impl Hash,
    render: impl FnOnce(f32) -> ColorImage,
) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let texture = cached(ctx, (key, ppp.to_bits()), || render(ppp));
    blit(painter, &texture, rect.expand(device(margin, ppp).round() / ppp));
}

/// Paint `rect` as a box covered in `tolex` of `color`, with a shadow round it
pub fn paint_tolex(painter: &Painter, rect: Rect, radius: f32, tolex: Tolex, color: Color32) {
    let key = ("tolex", rect.width().to_bits(), rect.height().to_bits(), radius.to_bits());
    paint_face(painter, rect, TOLEX_MARGIN, (key, tolex, color), |ppp| {
        tolex_image(rect.size(), radius, tolex, color, ppp).0
    });
}

/// Paint `rect` as a grille of `grille` in `color` with `cones` behind it
pub fn paint_grille(
    painter: &Painter,
    rect: Rect,
    radius: f32,
    grille: Grille,
    color: Color32,
    cones: &[Cone],
) {
    let key = ("grille", rect.width().to_bits(), rect.height().to_bits(), radius.to_bits());
    let behind: Vec<_> = cones
        .iter()
        .map(|cone| (cone.center.x.to_bits(), cone.center.y.to_bits(), cone.radius.to_bits()))
        .collect();
    paint_face(painter, rect, 0.0, (key, grille, color, behind), |ppp| {
        grille_image(rect.size(), radius, grille, color, cones, ppp)
    });
}

/// Paint `rect` as a plate of `plate`, with a small shadow round it
pub fn paint_plate(painter: &Painter, rect: Rect, radius: f32, plate: Plate) {
    let key = ("plate", rect.width().to_bits(), rect.height().to_bits(), radius.to_bits());
    paint_face(painter, rect, PLATE_MARGIN, (key, plate), |ppp| {
        plate_image(rect.size(), radius, plate, ppp).0
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::PAINT_TEAL;
    use nih_plug_egui::egui::vec2;

    fn pixel(image: &ColorImage, x: usize, y: usize) -> Color32 {
        image.pixels[y * image.size[0] + x]
    }

    fn luminance(color: Color32) -> f32 {
        0.299 * color.r() as f32 + 0.587 * color.g() as f32 + 0.114 * color.b() as f32
    }

    #[test]
    fn test_box_is_paint_in_the_middle_and_clear_outside_its_shadow() {
        let (image, margin) = box_image(vec2(200.0, 300.0), 24.0, 5.0, PAINT_TEAL, 1.0);
        assert_eq!(image.size, [200 + 2 * margin, 300 + 2 * margin]);
        let middle = pixel(&image, image.size[0] / 2, image.size[1] / 2);
        // Within the flake and sheen of the paint
        assert!((middle.g() as i32 - PAINT_TEAL.g() as i32).abs() < 40, "{middle:?}");
        assert!(middle.a() == 255);
        assert_eq!(pixel(&image, 0, 0), Color32::TRANSPARENT);
    }

    #[test]
    fn test_box_edge_facing_the_light_is_lighter_than_the_one_facing_away() {
        let (image, margin) = box_image(vec2(200.0, 300.0), 24.0, 5.0, PAINT_TEAL, 1.0);
        let row = image.size[1] / 2;
        let mean =
            |x: usize| (0..9).map(|dy| luminance(pixel(&image, x, row + dy))).sum::<f32>() / 9.0;
        let left = mean(margin + 1);
        let right = mean(image.size[0] - margin - 2);
        assert!(left > right + 5.0, "left {left}, right {right}");
    }

    #[test]
    fn test_knob_is_round_with_its_shadow_below_and_to_the_right() {
        let image = knob_image(30.0, KnobCap::Fluted, 1.0);
        let side = image.size[0];
        assert_eq!(image.size[0], image.size[1]);
        assert_eq!(pixel(&image, 0, 0), Color32::TRANSPARENT);
        assert_eq!(pixel(&image, side / 2, side / 2).a(), 255);
        let below = pixel(&image, side / 2 + 2, side / 2 + 34).a();
        let above = pixel(&image, side / 2 + 2, side / 2 - 34).a();
        assert!(below > above, "below {below}, above {above}");
    }

    #[test]
    fn test_screw_has_a_dark_slot() {
        let image = screw_image(1.0, 0.0, 2.0);
        let side = image.size[0];
        let on_slot = luminance(pixel(&image, side / 2 + 4, side / 2));
        let off_slot = luminance(pixel(&image, side / 2 + 4, side / 2 + 6));
        assert!(on_slot < off_slot - 20.0, "{on_slot} vs {off_slot}");
    }

    #[test]
    fn test_pixels_are_valid_premultiplied() {
        let (image, _) = box_image(vec2(80.0, 80.0), 14.0, 5.0, PAINT_TEAL, 2.0);
        for color in image.pixels {
            assert!(color.r() <= color.a() && color.g() <= color.a() && color.b() <= color.a());
        }
    }

    fn assert_premultiplied(image: &ColorImage) {
        for color in &image.pixels {
            assert!(
                color.r() <= color.a() && color.g() <= color.a() && color.b() <= color.a(),
                "{color:?}"
            );
        }
    }

    /// The mean luminance of a few rows of `image` in column `x`, round its middle
    fn column(image: &ColorImage, x: usize) -> f32 {
        let row = image.size[1] / 2;
        (0..9).map(|dy| luminance(pixel(image, x, row + dy))).sum::<f32>() / 9.0
    }

    const TOLEXES: [Tolex; 4] = [Tolex::Levant, Tolex::Bronco, Tolex::Smooth, Tolex::Tweed];
    const GRILLES: [Grille; 6] = [
        Grille::Basketweave,
        Grille::Wheat,
        Grille::SaltPepper,
        Grille::Silver,
        Grille::Felt,
        Grille::Mesh,
    ];
    const PLATES: [Plate; 4] =
        [Plate::BrushedBrass, Plate::BrushedChrome, Plate::BlackAnodised, Plate::DarkWood];
    const CAPS: [KnobCap; 5] = [
        KnobCap::Fluted,
        KnobCap::ChickenHead,
        KnobCap::CreamChickenHead,
        KnobCap::GoldTop,
        KnobCap::ChromeSkirt,
    ];
    const TAN: Color32 = Color32::from_rgb(150, 120, 80);

    #[test]
    fn test_tolex_is_valid_clear_outside_its_shadow_and_lit_from_the_left() {
        for tolex in TOLEXES {
            let (image, margin) = tolex_image(vec2(160.0, 120.0), 10.0, tolex, TAN, 2.0);
            assert_premultiplied(&image);
            assert_eq!(pixel(&image, 0, 0), Color32::TRANSPARENT, "{tolex:?}");
            assert_eq!(pixel(&image, image.size[0] / 2, image.size[1] / 2).a(), 255);
            let (left, right) =
                (column(&image, margin + 2), column(&image, image.size[0] - margin - 3));
            assert!(left > right + 5.0, "{tolex:?}: left {left}, right {right}");
        }
    }

    #[test]
    fn test_plates_are_valid_clear_outside_their_shadow_and_lit_from_the_left() {
        for plate in PLATES {
            let (image, margin) = plate_image(vec2(160.0, 60.0), 3.0, plate, 2.0);
            assert_premultiplied(&image);
            assert_eq!(pixel(&image, 0, 0), Color32::TRANSPARENT, "{plate:?}");
            let (left, right) =
                (column(&image, margin + 1), column(&image, image.size[0] - margin - 2));
            assert!(left > right + 5.0, "{plate:?}: left {left}, right {right}");
        }
    }

    #[test]
    fn test_grilles_are_valid_clear_at_their_corners_and_lit_from_the_left() {
        let cones = [Cone { center: vec2(80.0, 50.0), radius: 40.0 }];
        for grille in GRILLES {
            let image = grille_image(vec2(160.0, 100.0), 4.0, grille, TAN, &cones, 2.0);
            assert_premultiplied(&image);
            assert_eq!(pixel(&image, 0, 0), Color32::TRANSPARENT, "{grille:?}");
            let (left, right) = (column(&image, 1), column(&image, image.size[0] - 2));
            assert!(left > right + 5.0, "{grille:?}: left {left}, right {right}");
        }
    }

    #[test]
    fn test_cloth_is_darker_over_a_speaker() {
        let cones = [Cone { center: vec2(50.0, 50.0), radius: 40.0 }];
        for grille in [Grille::Basketweave, Grille::Wheat, Grille::Silver, Grille::Felt] {
            let (with, without) = (
                grille_image(vec2(100.0, 100.0), 4.0, grille, TAN, &cones, 1.0),
                grille_image(vec2(100.0, 100.0), 4.0, grille, TAN, &[], 1.0),
            );
            let middle = |image: &ColorImage| {
                (30..70)
                    .flat_map(|y| (30..70).map(move |x| (x, y)))
                    .map(|(x, y)| luminance(pixel(image, x, y)))
                    .sum::<f32>()
            };
            assert!(middle(&with) < middle(&without) * 0.95, "{grille:?}");
        }
    }

    #[test]
    fn test_mesh_holes_are_darker_than_the_steel() {
        let steel = Color32::from_rgb(170, 168, 164);
        let image = grille_image(vec2(60.0, 60.0), 4.0, Grille::Mesh, steel, &[], 4.0);
        // A hole is centred on every grid point: (30, 0) is one, a point half a pitch
        // further along a row lies in the steel between two
        let at = |x: f32, y: f32| luminance(pixel(&image, (x * 4.0) as usize, (y * 4.0) as usize));
        let (hole, between) =
            (at(30.0 + 0.3, 2.0 * 3.0 * 0.866 + 0.3), at(31.5, 2.0 * 3.0 * 0.866 - 1.3));
        assert!(hole < between - 40.0, "hole {hole}, steel {between}");
    }

    #[test]
    fn test_every_knob_cap_is_round_with_its_shadow_below_and_to_the_right() {
        for cap in CAPS {
            let image = knob_image(30.0, cap, 1.0);
            let side = image.size[0];
            assert_eq!(image.size[0], image.size[1]);
            assert_premultiplied(&image);
            assert_eq!(pixel(&image, 0, 0), Color32::TRANSPARENT, "{cap:?}");
            assert_eq!(pixel(&image, side / 2, side / 2).a(), 255, "{cap:?}");
            // Round: as far out in every direction
            for (dx, dy) in [(29, 0), (0, 29), (-29, 0), (0, -29), (20, 20), (-20, -20)] {
                let (x, y) = ((side / 2) as i32 + dx, (side / 2) as i32 + dy);
                assert_eq!(pixel(&image, x as usize, y as usize).a(), 255, "{cap:?} {dx} {dy}");
            }
            let below = pixel(&image, side / 2 + 2, side / 2 + 34).a();
            let above = pixel(&image, side / 2 + 2, side / 2 - 34).a();
            assert!(below > above, "{cap:?}: below {below}, above {above}");
        }
    }

    #[test]
    fn test_fluted_knob_is_the_knob_it_was_to_the_pixel() {
        for (radius, ppp) in [(48.0, 1.0), (30.0, 2.0), (19.0, 1.5), (17.0, 2.0)] {
            let (now, before) =
                (knob_image(radius, KnobCap::Fluted, ppp), fluted_before(radius, ppp));
            assert_eq!(now.size, before.size);
            assert!(now.pixels == before.pixels, "radius {radius} at {ppp}");
        }
    }

    /// How long the largest surfaces take to render the first time they are shown, at 2x:
    /// `cargo test -p suite_common --release render_times -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn render_times() {
        let size = vec2(440.0, 480.0);
        let time = |name: String, render: &dyn Fn()| {
            let start = std::time::Instant::now();
            render();
            println!("{name:<24} {:>6.1} ms", start.elapsed().as_secs_f64() * 1000.0);
        };
        for tolex in TOLEXES {
            time(format!("tolex {tolex:?}"), &|| drop(tolex_image(size, 10.0, tolex, TAN, 2.0)));
        }
        let cones = crate::ui::amp::Speakers::Four12.cones(size);
        for grille in GRILLES {
            time(format!("grille {grille:?}"), &|| {
                drop(grille_image(size, 4.0, grille, TAN, &cones, 2.0))
            });
        }
        for plate in PLATES {
            time(format!("plate {plate:?}"), &|| drop(plate_image(size, 3.0, plate, 2.0)));
        }
    }

    /// `knob_image` as it was before the caps came, kept to hold `KnobCap::Fluted` to it
    fn fluted_before(radius: f32, ppp: f32) -> ColorImage {
        let side = ((radius + KNOB_MARGIN) * 2.0 * ppp).ceil() as usize;
        let c = side as f32 / 2.0;
        let r = device(radius, ppp);
        let flutes = (radius * 0.8).round().max(8.0);
        let shadow_offset = [device(0.4, ppp), device(1.2, ppp)];
        let tight = device(1.0, ppp);
        let soft = device(3.5, ppp);
        let skirt = rgb(KNOB_SKIRT);
        let cap = rgb(KNOB_CAP_TOP);
        let rim = rgb(KNOB_CAP_EDGE);

        let mut pixels = vec![Pixel::default(); side * side];
        for y in 0..side {
            for x in 0..side {
                let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
                let dist = (dx * dx + dy * dy).sqrt();
                let mut pixel = Pixel::default();

                let (sx, sy) = (dx - shadow_offset[0], dy - shadow_offset[1]);
                let gap = ((sx * sx + sy * sy).sqrt() - r).max(0.0);
                // Where the knob meets the paint it is dark; a little further out only faintly so
                let contact = 0.55 * (-(gap / tight).powi(2)).exp();
                let ambient = 0.20 * (-(gap / soft).powi(2)).exp();
                pixel.over([0.0; 3], 1.0 - (1.0 - contact) * (1.0 - ambient));

                let coverage = (r - dist + 0.5).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    let t = dist / r;
                    let phi = dy.atan2(dx);
                    let (ux, uy) = (phi.cos(), phi.sin());
                    let color = if t > 0.7 {
                        // Skirt: flutes run up and down it, and it rounds off at the rim
                        let out = (t - 0.7) / 0.3;
                        let round = out.powi(3) * 1.1;
                        let flute = (flutes * phi).sin() * 0.55;
                        let normal = tilted(ux * round - uy * flute, uy * round + ux * flute);
                        let base = [skirt[0] * 1.6, skirt[1] * 1.6, skirt[2] * 1.6];
                        shaded(base, diffuse(normal), specular(normal, 26.0) * 0.5)
                    } else {
                        // Cap: a shallow dome with a bevel at its rim and a brushed dish in the middle
                        let edge = smoothstep(0.62, 0.70, t);
                        let slope = t / 0.7 * 0.18 + edge * 1.2;
                        let normal = tilted(ux * slope, uy * slope);
                        let brush = value_noise(dist * 1.6 / ppp.max(1.0), 0.5, 5) - 0.5;
                        // Two soft bright bands across the turned face, as on turned metal
                        let band = (0.5 + 0.5 * (2.0 * (phi - 2.2)).cos()).powi(6);
                        let dish = 1.0 - smoothstep(0.50, 0.54, t);
                        let base = [
                            cap[0] + (rim[0] - cap[0]) * edge,
                            cap[1] + (rim[1] - cap[1]) * edge,
                            cap[2] + (rim[2] - cap[2]) * edge,
                        ];
                        let mut color = shaded(base, diffuse(normal), specular(normal, 30.0) * 0.4);
                        let shine = (band * 0.30 + brush * 0.10) * dish * 255.0 * (1.0 - t);
                        color = [color[0] + shine, color[1] + shine, color[2] + shine];
                        // Dark groove between the cap and the skirt
                        let groove = 1.0 - 0.55 * (1.0 - smoothstep(0.0, 0.025, (t - 0.70).abs()));
                        [color[0] * groove, color[1] * groove, color[2] * groove]
                    };
                    // The rim catches a little light all round
                    let lip = smoothstep(0.93, 1.0, t) * 0.10 * 255.0;
                    pixel.over([color[0] + lip, color[1] + lip, color[2] + lip], coverage);
                }
                pixels[y * side + x] = pixel;
            }
        }
        image(side, side, pixels)
    }

    /// Writes a sheet of the surfaces as raw RGB (width, height, then pixels) to the file named
    /// by `RASTER_PREVIEW`, for looking at: `cargo test -p suite_common preview -- --ignored`
    #[test]
    #[ignore]
    fn preview() {
        let Ok(path) = std::env::var("RASTER_PREVIEW") else { return };
        let ppp = 2.0;
        let (w, h) = (1800usize, 2440usize);
        let mut sheet = vec![[18.0_f32, 17.0, 16.0]; w * h];
        let mut put = |image: &ColorImage, ox: usize, oy: usize| {
            for y in 0..image.size[1] {
                for x in 0..image.size[0] {
                    let c = image.pixels[y * image.size[0] + x];
                    let (px, py) = (ox + x, oy + y);
                    if px < w && py < h {
                        let a = c.a() as f32 / 255.0;
                        let dst = &mut sheet[py * w + px];
                        dst[0] = c.r() as f32 + dst[0] * (1.0 - a);
                        dst[1] = c.g() as f32 + dst[1] * (1.0 - a);
                        dst[2] = c.b() as f32 + dst[2] * (1.0 - a);
                    }
                }
            }
        };
        let (b, _) = box_image(vec2(200.0, 270.0), 24.0, 5.0, PAINT_TEAL, ppp);
        put(&b, 10, 10);
        let (b, _) = box_image(vec2(200.0, 270.0), 24.0, 5.0, crate::ui::theme::PAINT_OXBLOOD, ppp);
        put(&b, 450, 10);
        put(&knob_image(48.0, KnobCap::Fluted, ppp), 20, 300);
        put(&knob_image(30.0, KnobCap::Fluted, ppp), 260, 330);
        put(&knob_image(19.0, KnobCap::Fluted, ppp), 400, 350);
        put(&screw_image(1.0, 0.6, ppp), 500, 350);
        put(&screw_image(1.0, -0.9, ppp), 560, 350);
        use crate::ui::hardware::*;
        put(&stomp_image(1.0, false, ppp), 600, 300);
        put(&stomp_image(1.0, true, ppp), 740, 300);
        let off = Color32::from_rgb(90, 26, 18);
        put(&lamp_image(true, off, false, 1.0, ppp), 640, 160);
        put(&lamp_image(false, off, false, 1.0, ppp), 700, 160);
        put(&lamp_image(false, Color32::from_rgb(58, 12, 10), true, 1.0, ppp), 760, 160);
        put(&tape_image(vec2(90.0, 22.0), ppp), 620, 230);
        put(&paper_image(vec2(120.0, 46.0), ppp), 600, 100);

        // The covered half stack: tolex, grilles with a speaker behind each, plates, the caps
        let rgb = Color32::from_rgb;
        let tolexes = [
            (Tolex::Levant, rgb(26, 24, 23)),
            (Tolex::Bronco, rgb(70, 44, 30)),
            (Tolex::Smooth, rgb(212, 198, 168)),
            (Tolex::Tweed, rgb(198, 172, 122)),
        ];
        for (i, (tolex, color)) in tolexes.into_iter().enumerate() {
            put(&tolex_image(vec2(200.0, 130.0), 10.0, tolex, color, ppp).0, 10 + i * 440, 580);
        }
        let grilles = [
            (Grille::Basketweave, rgb(124, 116, 100)),
            (Grille::Wheat, rgb(192, 162, 112)),
            (Grille::SaltPepper, rgb(34, 32, 30)),
            (Grille::Silver, rgb(150, 150, 148)),
            (Grille::Felt, rgb(96, 30, 28)),
            (Grille::Mesh, rgb(150, 150, 148)),
        ];
        let cone = [Cone { center: vec2(65.0, 55.0), radius: 45.0 }];
        for (i, (grille, color)) in grilles.into_iter().enumerate() {
            let image = grille_image(vec2(130.0, 110.0), 4.0, grille, color, &cone, ppp);
            put(&image, 20 + i * 295, 880);
        }
        for (i, plate) in PLATES.into_iter().enumerate() {
            put(&plate_image(vec2(200.0, 60.0), 3.0, plate, ppp).0, 10 + i * 440, 1130);
        }
        for (i, cap) in CAPS.into_iter().enumerate() {
            put(&knob_image(30.0, cap, ppp), 40 + i * 350, 1280);
            put(&knob_image(20.0, cap, ppp), 210 + i * 350, 1300);
        }
        use crate::ui::amp::*;
        use crate::ui::theme::{INK, SILK};
        let cream = rgb(222, 214, 196);
        let first = (
            HeadStyle {
                tolex: Tolex::Levant,
                covering: rgb(26, 24, 23),
                piping: Some(cream),
                grille: Grille::Basketweave,
                cloth: rgb(124, 116, 100),
                cloth_stripe: None,
                grille_height: 50.0,
                panel: Plate::BrushedBrass,
                print: INK,
                panel_stripe: None,
                jewel: rgb(214, 36, 28),
            },
            CabStyle {
                tolex: Tolex::Levant,
                covering: rgb(26, 24, 23),
                piping: Some(cream),
                grille: Grille::Basketweave,
                cloth: rgb(124, 116, 100),
                speakers: Speakers::Four12,
                set_high: false,
                corners: Some(Corners::Chrome),
                plate: Plate::BlackAnodised,
                plate_height: 120.0,
                print: SILK,
            },
            KnobCap::ChickenHead,
        );
        let second = (
            HeadStyle {
                tolex: Tolex::Tweed,
                covering: rgb(198, 172, 122),
                piping: None,
                grille: Grille::Wheat,
                cloth: rgb(110, 44, 32),
                cloth_stripe: Some(rgb(200, 160, 70)),
                grille_height: 50.0,
                panel: Plate::BrushedChrome,
                print: INK,
                panel_stripe: Some(rgb(200, 40, 32)),
                jewel: rgb(240, 160, 40),
            },
            CabStyle {
                tolex: Tolex::Tweed,
                covering: rgb(198, 172, 122),
                piping: None,
                grille: Grille::Mesh,
                cloth: rgb(60, 60, 62),
                speakers: Speakers::One10,
                set_high: true,
                corners: Some(Corners::Steel),
                plate: Plate::DarkWood,
                plate_height: 120.0,
                print: SILK,
            },
            KnobCap::GoldTop,
        );
        for (i, (head, cab, cap)) in [first, second].into_iter().enumerate() {
            let top_left = nih_plug_egui::egui::pos2(10.0 + i as f32 * 450.0, 730.0);
            preview_stack(&mut put, top_left, vec2(440.0, 480.0), 180.0, &head, &cab, cap, ppp);
        }
        let mut out = Vec::new();
        out.extend((w as u32).to_le_bytes());
        out.extend((h as u32).to_le_bytes());
        for p in sheet {
            out.extend(p.map(|v| v.clamp(0.0, 255.0) as u8));
        }
        std::fs::write(path, out).unwrap();
    }
}
