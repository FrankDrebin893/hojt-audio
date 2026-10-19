//! The half stack of a rig: an amp head and the speaker cabinet it stands on. They are not
//! painted like a pedal but covered, as amps are: tolex over the wood, a grille of cloth or
//! metal in front of the speakers, a plate the controls are mounted on, piping between the
//! covering and the grille. Every surface is lit pixel by pixel (see `raster`); the head
//! carries a handle on top and a jewel pilot lamp, the cabinet metal corners.
//!
//! `amp_head` and `cabinet` draw only the furniture and return the faces the caller puts its
//! controls on.

use nih_plug_egui::egui::{pos2, vec2, Color32, ColorImage, Painter, Pos2, Rect, Ui, Vec2};

use super::raster::{
    blit, cached, device, diffuse, hash, image, paint_grille, paint_plate, paint_screw,
    paint_tolex, rgb, rounded_box, shaded, smoothstep, specular, tilted, Cone, Grille, Pixel,
    Plate, Rgb, Tolex,
};
use super::theme::CHROME;

/// Room over a head's box for its carry handle
pub const HANDLE_HEIGHT: f32 = 12.0;
const HANDLE_WIDTH: f32 = 120.0;
// The chrome caps the handle is fixed to the box with
const HANDLE_CAP: f32 = 16.0;
const HANDLE_MARGIN: f32 = 3.0;

const COVER_RADIUS: f32 = 10.0;
// Covering between the edge of the box and the faces
const HEAD_BORDER: f32 = 12.0;
const CAB_BORDER: f32 = 16.0;
// Covering between two faces
const FACE_GAP: f32 = 8.0;
const GRILLE_RADIUS: f32 = 4.0;
const PLATE_RADIUS: f32 = 3.0;
const PIPING_WIDTH: f32 = 2.5;
const PIPING_MARGIN: f32 = 3.0;
const PLATE_SCREW_INSET: f32 = 7.0;
const PLATE_SCREW_SCALE: f32 = 0.5;

/// The pilot lamp: its middle this far in from the right end of the head's panel
pub const JEWEL_INSET: f32 = 20.0;
const JEWEL_RADIUS: f32 = 6.0;
// The chrome bezel round the jewel, as a share of its radius
const JEWEL_BEZEL: f32 = 1.4;
const JEWEL_MARGIN: f32 = 4.0;

const CORNER_SIZE: f32 = 26.0;
const CORNER_MARGIN: f32 = 3.0;

/// How an amp head looks
#[derive(Clone, Copy, Debug)]
pub struct HeadStyle {
    pub tolex: Tolex,
    /// The colour of the tolex
    pub covering: Color32,
    /// The cord between the covering and the grille
    pub piping: Color32,
    pub grille: Grille,
    /// The colour of the grille's cloth or steel
    pub cloth: Color32,
    /// The grille runs across the top of the head, this tall; the panel fills the rest
    pub grille_height: f32,
    pub panel: Plate,
    /// What is printed on the panel: `SILK` on a dark one, `INK` on metal
    pub print: Color32,
    /// The colour of the pilot lamp's jewel
    pub jewel: Color32,
}

/// Where an amp head's controls go: the nameplate on the grille, the knobs on the panel
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadFaces {
    pub grille: Rect,
    pub panel: Rect,
}

/// The speakers in a cabinet: how many, and how large
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Speakers {
    One10,
    One12,
    Two12,
    Four10,
    Four12,
    One15,
}

/// Every layout of speakers
pub const SPEAKER_LAYOUTS: [Speakers; 6] = [
    Speakers::One10,
    Speakers::One12,
    Speakers::Two12,
    Speakers::Four10,
    Speakers::Four12,
    Speakers::One15,
];

impl Speakers {
    /// The speakers behind a grille of `size`, from its top left corner: a column and row
    /// each, a smaller speaker taking less of its share
    pub fn cones(self, size: Vec2) -> Vec<Cone> {
        let (columns, rows, share) = match self {
            Speakers::One10 => (1, 1, 0.62),
            Speakers::One12 => (1, 1, 0.78),
            Speakers::One15 => (1, 1, 0.94),
            Speakers::Two12 => (2, 1, 0.86),
            Speakers::Four10 => (2, 2, 0.74),
            Speakers::Four12 => (2, 2, 0.88),
        };
        let cell = vec2(size.x / columns as f32, size.y / rows as f32);
        let radius = cell.min_elem() / 2.0 * share;
        let mut cones = Vec::with_capacity(columns * rows);
        for row in 0..rows {
            for column in 0..columns {
                let center = vec2(cell.x * (column as f32 + 0.5), cell.y * (row as f32 + 0.5));
                cones.push(Cone { center, radius });
            }
        }
        cones
    }
}

/// How a speaker cabinet looks
#[derive(Clone, Copy, Debug)]
pub struct CabStyle {
    pub tolex: Tolex,
    /// The colour of the tolex
    pub covering: Color32,
    /// The cord between the covering and the grille
    pub piping: Color32,
    pub grille: Grille,
    /// The colour of the grille's cloth or steel
    pub cloth: Color32,
    pub speakers: Speakers,
    /// The control plate runs across the top of the cabinet, this tall; the grille fills
    /// the rest
    pub plate: Plate,
    pub plate_height: f32,
    /// What is printed on the plate: `SILK` on a dark one, `INK` on metal
    pub print: Color32,
}

/// Where a cabinet's controls go: on its plate. The grille is where the speakers are
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CabFaces {
    pub grille: Rect,
    pub plate: Rect,
}

/// The box of a head in `rect`: under the room for the handle
fn head_box(rect: Rect) -> Rect {
    Rect::from_min_max(rect.min + vec2(0.0, HANDLE_HEIGHT), rect.max)
}

fn head_faces(rect: Rect, style: &HeadStyle) -> HeadFaces {
    let inner = head_box(rect).shrink(HEAD_BORDER);
    let grille_height = style.grille_height.clamp(0.0, (inner.height() - FACE_GAP).max(0.0));
    let grille = Rect::from_min_size(inner.min, vec2(inner.width(), grille_height));
    let top = (grille.bottom() + FACE_GAP).min(inner.bottom());
    let panel = Rect::from_min_max(pos2(inner.left(), top), inner.max);
    HeadFaces { grille, panel }
}

fn cab_faces(rect: Rect, style: &CabStyle) -> CabFaces {
    let inner = rect.shrink(CAB_BORDER);
    let plate_height = style.plate_height.clamp(0.0, (inner.height() - FACE_GAP).max(0.0));
    let plate = Rect::from_min_size(inner.min, vec2(inner.width(), plate_height));
    let top = (plate.bottom() + FACE_GAP).min(inner.bottom());
    let grille = Rect::from_min_max(pos2(inner.left(), top), inner.max);
    CabFaces { grille, plate }
}

/// An amp head filling `rect`: a carry handle on top, the box covered in tolex, the grille
/// across its top with piping round it, the panel under it with a jewel pilot lamp at its
/// right end. Returns the faces, for the caller's nameplate and knobs.
pub fn amp_head(ui: &Ui, rect: Rect, style: &HeadStyle) -> HeadFaces {
    let painter = ui.painter();
    let faces = head_faces(rect, style);
    let body = head_box(rect);
    paint_tolex(painter, body, COVER_RADIUS, style.tolex, style.covering);
    paint_handle(
        painter,
        Rect::from_center_size(
            pos2(rect.center().x, rect.top() + HANDLE_HEIGHT / 2.0 + 1.0),
            vec2(HANDLE_WIDTH, HANDLE_HEIGHT),
        ),
    );
    paint_grille(painter, faces.grille, GRILLE_RADIUS, style.grille, style.cloth, &[]);
    paint_piping(painter, faces.grille, style.piping);
    mounted_plate(painter, faces.panel, style.panel);
    paint_jewel(
        painter,
        pos2(faces.panel.right() - JEWEL_INSET, faces.panel.center().y),
        style.jewel,
    );
    faces
}

/// A speaker cabinet filling `rect`: the box covered in tolex with a metal corner on each
/// corner, the control plate across its top, the grille under it with the speakers behind it
/// and piping round it. Returns the faces, for the caller's controls.
pub fn cabinet(ui: &Ui, rect: Rect, style: &CabStyle) -> CabFaces {
    let painter = ui.painter();
    let faces = cab_faces(rect, style);
    paint_tolex(painter, rect, COVER_RADIUS, style.tolex, style.covering);
    let cones = style.speakers.cones(faces.grille.size());
    paint_grille(painter, faces.grille, GRILLE_RADIUS, style.grille, style.cloth, &cones);
    paint_piping(painter, faces.grille, style.piping);
    mounted_plate(painter, faces.plate, style.plate);
    for (corner, flip) in [
        (rect.left_top(), [false, false]),
        (rect.right_top(), [true, false]),
        (rect.right_bottom(), [true, true]),
        (rect.left_bottom(), [false, true]),
    ] {
        paint_corner(painter, corner, flip);
    }
    faces
}

/// A plate on `rect` with a small screw in each corner
fn mounted_plate(painter: &Painter, rect: Rect, plate: Plate) {
    paint_plate(painter, rect, PLATE_RADIUS, plate);
    let inset = PLATE_SCREW_INSET;
    for (corner, slot) in [
        (rect.left_top() + vec2(inset, inset), 35.0_f32),
        (rect.right_top() + vec2(-inset, inset), -50.0),
        (rect.left_bottom() + vec2(inset, -inset), -50.0),
        (rect.right_bottom() + vec2(-inset, -inset), 35.0),
    ] {
        paint_screw(painter, corner, slot.to_radians(), PLATE_SCREW_SCALE);
    }
}

/// The cord laid round a face of `size` with corners of `radius`: a round tube `width` wide
/// over the face's edge, with a little shadow outside it. Its image runs `PIPING_MARGIN`
/// points past the face
pub fn piping_image(size: Vec2, radius: f32, width: f32, color: Color32, ppp: f32) -> ColorImage {
    let m = device(PIPING_MARGIN, ppp).round();
    let (face_w, face_h) = (device(size.x, ppp).round(), device(size.y, ppp).round());
    let (w, h) = (face_w as usize + 2 * m as usize, face_h as usize + 2 * m as usize);
    let (centre, half) = ([m + face_w / 2.0, m + face_h / 2.0], [face_w / 2.0, face_h / 2.0]);
    let radius = device(radius, ppp);
    let half_width = device(width, ppp) / 2.0;
    let blur = device(1.0, ppp);
    let base = rgb(color);
    let distance = |p: [f32; 2]| rounded_box(p, centre, half, radius);
    let mut pixels = vec![Pixel::default(); w * h];
    for y in 0..h {
        for x in 0..w {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let d = distance(p);
            let mut pixel = Pixel::default();
            let below = distance([p[0], p[1] - device(0.8, ppp)]) - half_width;
            if below > 0.0 {
                pixel.over([0.0; 3], 0.4 * (-(below / blur).powi(2)).exp());
            }
            let coverage = (half_width - d.abs() + 0.5).clamp(0.0, 1.0);
            if coverage > 0.0 {
                let gx = distance([p[0] + 0.5, p[1]]) - distance([p[0] - 0.5, p[1]]);
                let gy = distance([p[0], p[1] + 0.5]) - distance([p[0], p[1] - 0.5]);
                // Across the tube: falling away inwards and outwards from its crest
                let across = (d / half_width).clamp(-1.0, 1.0) * 1.5;
                let normal = tilted(gx * across, gy * across);
                let color = shaded(base, diffuse(normal), specular(normal, 30.0) * 0.5);
                pixel.over(color, coverage);
            }
            pixels[y * w + x] = pixel;
        }
    }
    image(w, h, pixels)
}

fn paint_piping(painter: &Painter, face: Rect, color: Color32) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let key = ("piping", face.width().to_bits(), face.height().to_bits(), color, ppp.to_bits());
    let texture =
        cached(ctx, key, || piping_image(face.size(), GRILLE_RADIUS, PIPING_WIDTH, color, ppp));
    blit(painter, &texture, face.expand(device(PIPING_MARGIN, ppp).round() / ppp));
}

/// A carry handle `size` large seen from the front: a black strap that bulges towards the
/// viewer between two chrome caps, and its shadow. Its image runs `HANDLE_MARGIN` points
/// past it
pub fn handle_image(size: Vec2, ppp: f32) -> ColorImage {
    let m = device(HANDLE_MARGIN, ppp).round();
    let (hw, hh) = (device(size.x, ppp).round(), device(size.y, ppp).round());
    let (w, h) = (hw as usize + 2 * m as usize, hh as usize + 2 * m as usize);
    let cap = device(HANDLE_CAP, ppp);
    let strap = [32.0, 29.0, 26.0];
    let chrome = rgb(CHROME);
    let centre = [m + hw / 2.0, m + hh / 2.0];
    let strap_half = [hw / 2.0 - cap * 0.6, hh * 0.38];
    let cap_half = [cap / 2.0, hh / 2.0];
    let caps = [[m + cap / 2.0, centre[1]], [m + hw - cap / 2.0, centre[1]]];
    let mut pixels = vec![Pixel::default(); w * h];
    for y in 0..h {
        for x in 0..w {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let mut pixel = Pixel::default();
            let shadow = rounded_box(
                [p[0], p[1] - device(1.5, ppp)],
                centre,
                [hw / 2.0, hh / 2.0],
                hh / 2.0,
            );
            pixel.over([0.0; 3], 0.45 * (-(shadow.max(0.0) / device(1.2, ppp)).powi(2)).exp());

            let d = rounded_box(p, centre, strap_half, strap_half[1]);
            let coverage = (0.5 - d).clamp(0.0, 1.0);
            if coverage > 0.0 {
                let v = (p[1] - centre[1]) / strap_half[1];
                let grain = (hash(x as i32, y as i32, 113) - 0.5) * 0.15;
                let normal = tilted(grain, v * 1.3 + grain);
                pixel.over(shaded(strap, diffuse(normal), specular(normal, 20.0) * 0.3), coverage);
            }
            for c in caps {
                let d = rounded_box(p, c, cap_half, device(2.0, ppp));
                let coverage = (0.5 - d).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    let (u, v) = ((p[0] - c[0]) / cap_half[0], (p[1] - c[1]) / cap_half[1]);
                    let normal = tilted(u.powi(3) * 0.8, v.powi(3) * 0.8 + v * 0.4);
                    let color = shaded(chrome, diffuse(normal), specular(normal, 20.0) * 0.6);
                    pixel.over(color, coverage);
                }
            }
            pixels[y * w + x] = pixel;
        }
    }
    image(w, h, pixels)
}

fn paint_handle(painter: &Painter, rect: Rect) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let key = ("handle", rect.width().to_bits(), rect.height().to_bits(), ppp.to_bits());
    let texture = cached(ctx, key, || handle_image(rect.size(), ppp));
    blit(painter, &texture, rect.expand(device(HANDLE_MARGIN, ppp).round() / ppp));
}

/// A pilot lamp: a faceted jewel of `color` `radius` large, lit from within, in a chrome
/// bezel, with its glow round it
pub fn jewel_image(radius: f32, color: Color32, ppp: f32) -> ColorImage {
    let side = ((radius * JEWEL_BEZEL + JEWEL_MARGIN) * 2.0 * ppp).ceil() as usize;
    let c = side as f32 / 2.0;
    let r = device(radius, ppp);
    let bezel = r * JEWEL_BEZEL;
    let base = rgb(color);
    let chrome = rgb(CHROME);
    let facet = std::f32::consts::TAU / 8.0;
    let mut pixels = vec![Pixel::default(); side * side];
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let dist = (dx * dx + dy * dy).sqrt();
            let mut pixel = Pixel::default();
            pixel.over(base, 0.35 * (-(dist / (bezel * 1.3)).powi(2)).exp());
            let outer = (bezel - dist + 0.5).clamp(0.0, 1.0);
            if outer > 0.0 {
                let t = ((dist - r) / (bezel - r)).clamp(0.0, 1.0);
                let slope = 0.3 + t * t * 1.0;
                let (ux, uy) = if dist > 0.0 { (dx / dist, dy / dist) } else { (0.0, 0.0) };
                let normal = tilted(ux * slope, uy * slope);
                let color = shaded(chrome, diffuse(normal), specular(normal, 18.0) * 0.6);
                pixel.over(color, outer);
            }
            let lens = (r - dist + 0.5).clamp(0.0, 1.0);
            if lens > 0.0 {
                let t = dist / r;
                // Eight facets round a flat table
                let phi = (dy.atan2(dx) / facet).round() * facet;
                let slope = if t < 0.45 { 0.0 } else { 0.6 };
                let normal = tilted(phi.cos() * slope, phi.sin() * slope);
                let glow = (1.0 - t).powi(2) * 90.0;
                let light = 0.75 + 0.35 * diffuse(normal);
                let gloss = specular(normal, 24.0) * 0.8;
                let color = [
                    base[0] * light + glow + (255.0 - base[0]) * gloss,
                    base[1] * light + glow * 0.6 + (240.0 - base[1]) * gloss,
                    base[2] * light + glow * 0.4 + (230.0 - base[2]) * gloss,
                ];
                pixel.over(color, lens);
            }
            pixels[y * side + x] = pixel;
        }
    }
    image(side, side, pixels)
}

fn paint_jewel(painter: &Painter, center: Pos2, color: Color32) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let key = ("jewel", color, ppp.to_bits());
    let texture = cached(ctx, key, || jewel_image(JEWEL_RADIUS, color, ppp));
    let side = (JEWEL_RADIUS * JEWEL_BEZEL + JEWEL_MARGIN) * 2.0;
    blit(painter, &texture, Rect::from_center_size(center, Vec2::splat(side)));
}

/// A chrome corner for the corner of a box: a quarter of a dome round the corner with a
/// rivet in it, wrapping the box's edges. Drawn for the top left corner, mirrored by `flip`
/// (left to right, top to bottom) for the others; the light stays where it is. Its image
/// runs `CORNER_MARGIN` points past it on every side
pub fn corner_image(flip: [bool; 2], ppp: f32) -> ColorImage {
    let m = CORNER_MARGIN;
    let side = ((CORNER_SIZE + 2.0 * m) * ppp).ceil() as usize;
    let chrome = rgb(CHROME);
    let size = CORNER_SIZE;
    let rivet = [size * 0.34, size * 0.34];
    // From a pixel to the corner's own points, the corner at the origin
    let local = |at: f32, flipped: bool| if flipped { size + m - at } else { at - m };
    let mut pixels = vec![Pixel::default(); side * side];
    for y in 0..side {
        for x in 0..side {
            let lx = local((x as f32 + 0.5) / ppp, flip[0]);
            let ly = local((y as f32 + 0.5) / ppp, flip[1]);
            let mut pixel = Pixel::default();
            // Where it is: inside the quarter circle round the corner, on the box
            let dist = (lx * lx + ly * ly).sqrt();
            let shadow_dist = ((lx - 0.4).powi(2) + (ly - 1.0).powi(2)).sqrt();
            if lx > -0.5 && ly > -0.5 {
                let gap = (shadow_dist - size).max(0.0);
                pixel.over([0.0; 3], 0.4 * (-(gap / 1.3).powi(2)).exp());
            }
            let coverage = ((size - dist) * ppp + 0.5).clamp(0.0, 1.0)
                * ((lx * ppp + 0.5).clamp(0.0, 1.0))
                * ((ly * ppp + 0.5).clamp(0.0, 1.0));
            if coverage > 0.0 {
                let (ux, uy) = if dist > 0.0 { (lx / dist, ly / dist) } else { (0.0, 0.0) };
                // Falling away to its curved edge, and round the box's edges
                let rim = (1.0 - smoothstep(0.0, 5.0, size - dist)) * 1.1;
                let wrap_x = (1.0 - smoothstep(0.0, 3.5, lx)) * 1.0;
                let wrap_y = (1.0 - smoothstep(0.0, 3.5, ly)) * 1.0;
                let mut slope = [ux * rim - wrap_x, uy * rim - wrap_y];
                // The rivet, a small dome
                let (rx, ry) = (lx - rivet[0], ly - rivet[1]);
                let rd = (rx * rx + ry * ry).sqrt();
                if rd < 3.0 {
                    slope = [rx / 3.0 * 1.2, ry / 3.0 * 1.2];
                }
                let seen = [
                    if flip[0] { -slope[0] } else { slope[0] },
                    if flip[1] { -slope[1] } else { slope[1] },
                ];
                let normal = tilted(seen[0], seen[1]);
                let mut color: Rgb = shaded(chrome, diffuse(normal), specular(normal, 20.0) * 0.6);
                // A reflected horizon: what faces up is lighter
                let sky = 1.0 - 0.10 * seen[1].clamp(-1.0, 1.0);
                color = [color[0] * sky, color[1] * sky, color[2] * sky];
                pixel.over(color, coverage);
            }
            pixels[y * side + x] = pixel;
        }
    }
    image(side, side, pixels)
}

fn paint_corner(painter: &Painter, corner: Pos2, flip: [bool; 2]) {
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let texture = cached(ctx, ("corner", flip, ppp.to_bits()), || corner_image(flip, ppp));
    let reach = |flipped: bool| {
        if flipped {
            -(CORNER_SIZE + CORNER_MARGIN)
        } else {
            -CORNER_MARGIN
        }
    };
    let min = corner + vec2(reach(flip[0]), reach(flip[1]));
    blit(
        painter,
        &texture,
        Rect::from_min_size(min, Vec2::splat(CORNER_SIZE + 2.0 * CORNER_MARGIN)),
    );
}

/// A head on a cabinet as `amp_head` and `cabinet` draw them, put together from the images
/// for the raster preview: `top_left` and the sizes in points, `put` lays an image down with
/// its top left corner at a pixel. A row of knobs of `cap` on the panel and a nameplate on
/// the grille stand in for the controls
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn preview_stack(
    put: &mut dyn FnMut(&ColorImage, usize, usize),
    top_left: Pos2,
    size: Vec2,
    head_height: f32,
    head: &HeadStyle,
    cab: &CabStyle,
    cap: super::raster::KnobCap,
    ppp: f32,
) {
    use super::raster::{
        grille_image, knob_image, plate_image, screw_image, tolex_image, KNOB_MARGIN,
    };
    let at = |pos: Pos2, margin: f32| {
        (((pos.x - margin) * ppp).round() as usize, ((pos.y - margin) * ppp).round() as usize)
    };
    let mut lay = |image: &ColorImage, pos: Pos2, margin: f32| {
        let (x, y) = at(pos, margin);
        put(image, x, y);
    };
    let plate = |lay: &mut dyn FnMut(&ColorImage, Pos2, f32), rect: Rect, kind: Plate| {
        let (image, m) = plate_image(rect.size(), PLATE_RADIUS, kind, ppp);
        lay(&image, rect.min, m as f32 / ppp);
        let inset = PLATE_SCREW_INSET;
        let screw = 6.0 * PLATE_SCREW_SCALE + 3.0;
        for (corner, slot) in [
            (rect.left_top() + vec2(inset, inset), 35.0_f32),
            (rect.right_top() + vec2(-inset, inset), -50.0),
            (rect.left_bottom() + vec2(inset, -inset), -50.0),
            (rect.right_bottom() + vec2(-inset, -inset), 35.0),
        ] {
            lay(&screw_image(PLATE_SCREW_SCALE, slot.to_radians(), ppp), corner, screw);
        }
    };

    let head_rect = Rect::from_min_size(top_left, vec2(size.x, head_height));
    let faces = head_faces(head_rect, head);
    let body = head_box(head_rect);
    let (image, m) = tolex_image(body.size(), COVER_RADIUS, head.tolex, head.covering, ppp);
    lay(&image, body.min, m as f32 / ppp);
    let handle = Rect::from_center_size(
        pos2(head_rect.center().x, head_rect.top() + HANDLE_HEIGHT / 2.0 + 1.0),
        vec2(HANDLE_WIDTH, HANDLE_HEIGHT),
    );
    lay(&handle_image(handle.size(), ppp), handle.min, HANDLE_MARGIN);
    let grille =
        grille_image(faces.grille.size(), GRILLE_RADIUS, head.grille, head.cloth, &[], ppp);
    lay(&grille, faces.grille.min, 0.0);
    let piping = piping_image(faces.grille.size(), GRILLE_RADIUS, PIPING_WIDTH, head.piping, ppp);
    lay(&piping, faces.grille.min, PIPING_MARGIN);
    plate(&mut lay, faces.panel, head.panel);
    let nameplate = Rect::from_center_size(faces.grille.center(), vec2(140.0, 30.0));
    plate(&mut lay, nameplate, head.panel);
    for index in 0..6 {
        let center =
            pos2(faces.panel.left() + 50.0 + index as f32 * 60.0, faces.panel.top() + 34.0);
        lay(&knob_image(20.0, cap, ppp), center, 20.0 + KNOB_MARGIN);
    }
    let jewel = pos2(faces.panel.right() - JEWEL_INSET, faces.panel.center().y);
    lay(
        &jewel_image(JEWEL_RADIUS, head.jewel, ppp),
        jewel,
        JEWEL_RADIUS * JEWEL_BEZEL + JEWEL_MARGIN,
    );

    let cab_rect = Rect::from_min_size(
        pos2(top_left.x, head_rect.bottom()),
        vec2(size.x, size.y - head_height),
    );
    let faces = cab_faces(cab_rect, cab);
    let (image, m) = tolex_image(cab_rect.size(), COVER_RADIUS, cab.tolex, cab.covering, ppp);
    lay(&image, cab_rect.min, m as f32 / ppp);
    let cones = cab.speakers.cones(faces.grille.size());
    let grille =
        grille_image(faces.grille.size(), GRILLE_RADIUS, cab.grille, cab.cloth, &cones, ppp);
    lay(&grille, faces.grille.min, 0.0);
    let piping = piping_image(faces.grille.size(), GRILLE_RADIUS, PIPING_WIDTH, cab.piping, ppp);
    lay(&piping, faces.grille.min, PIPING_MARGIN);
    plate(&mut lay, faces.plate, cab.plate);
    for (corner, flip) in [
        (cab_rect.left_top(), [false, false]),
        (cab_rect.right_top(), [true, false]),
        (cab_rect.right_bottom(), [true, true]),
        (cab_rect.left_bottom(), [false, true]),
    ] {
        let reach =
            |flipped: bool| if flipped { CORNER_SIZE + CORNER_MARGIN } else { CORNER_MARGIN };
        lay(&corner_image(flip, ppp), corner - vec2(reach(flip[0]), reach(flip[1])), 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{INK, SILK};

    fn head_style(grille_height: f32) -> HeadStyle {
        HeadStyle {
            tolex: Tolex::Levant,
            covering: Color32::from_rgb(24, 22, 21),
            piping: Color32::from_rgb(220, 214, 200),
            grille: Grille::Basketweave,
            cloth: Color32::from_rgb(120, 110, 90),
            grille_height,
            panel: Plate::BrushedBrass,
            print: INK,
            jewel: Color32::from_rgb(200, 30, 24),
        }
    }

    fn cab_style(speakers: Speakers, plate_height: f32) -> CabStyle {
        CabStyle {
            tolex: Tolex::Bronco,
            covering: Color32::from_rgb(24, 22, 21),
            piping: Color32::from_rgb(220, 214, 200),
            grille: Grille::SaltPepper,
            cloth: Color32::from_rgb(30, 28, 26),
            speakers,
            plate: Plate::BlackAnodised,
            plate_height,
            print: SILK,
        }
    }

    fn rects() -> [Rect; 4] {
        [
            Rect::from_min_size(pos2(10.0, 20.0), vec2(450.0, 180.0)),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(440.0, 480.0)),
            Rect::from_min_size(pos2(5.0, 5.0), vec2(200.0, 120.0)),
            Rect::from_min_size(pos2(0.0, 0.0), vec2(60.0, 40.0)),
        ]
    }

    #[test]
    fn test_head_faces_lie_inside_the_head_and_apart() {
        for rect in rects() {
            for grille_height in [0.0, 40.0, 60.0, 1000.0] {
                let faces = head_faces(rect, &head_style(grille_height));
                for face in [faces.grille, faces.panel] {
                    assert!(rect.contains_rect(face), "{face:?} not in {rect:?}");
                    assert!(face.width() >= 0.0 && face.height() >= 0.0, "{face:?}");
                }
                assert!(faces.grille.bottom() <= faces.panel.top(), "{faces:?}");
                // The handle has its room over the box
                assert!(faces.grille.top() >= rect.top() + HANDLE_HEIGHT);
                assert!(faces.panel.top() <= faces.panel.bottom(), "{faces:?}");
            }
        }
    }

    #[test]
    fn test_cab_faces_lie_inside_the_cabinet_and_apart() {
        for rect in rects() {
            for plate_height in [0.0, 120.0, 1000.0] {
                let faces = cab_faces(rect, &cab_style(Speakers::Four12, plate_height));
                for face in [faces.grille, faces.plate] {
                    assert!(rect.contains_rect(face), "{face:?} not in {rect:?}");
                    assert!(face.width() >= 0.0 && face.height() >= 0.0, "{face:?}");
                }
                assert!(faces.plate.bottom() <= faces.grille.top(), "{faces:?}");
                assert!(faces.grille.top() <= faces.grille.bottom(), "{faces:?}");
            }
        }
    }

    #[test]
    fn test_speakers_lie_inside_the_grille_and_apart_for_every_layout() {
        for size in [vec2(418.0, 150.0), vec2(408.0, 300.0), vec2(150.0, 300.0), vec2(80.0, 80.0)] {
            for speakers in SPEAKER_LAYOUTS {
                let cones = speakers.cones(size);
                assert!(!cones.is_empty());
                for cone in &cones {
                    let c = cone.center;
                    assert!(cone.radius > 0.0);
                    assert!(c.x - cone.radius >= 0.0 && c.x + cone.radius <= size.x, "{cone:?}");
                    assert!(c.y - cone.radius >= 0.0 && c.y + cone.radius <= size.y, "{cone:?}");
                }
                for (i, a) in cones.iter().enumerate() {
                    for b in &cones[i + 1..] {
                        assert!((a.center - b.center).length() >= a.radius + b.radius);
                    }
                }
            }
        }
    }

    fn valid(image: &ColorImage) {
        for c in &image.pixels {
            assert!(c.r() <= c.a() && c.g() <= c.a() && c.b() <= c.a(), "{c:?}");
        }
    }

    #[test]
    fn test_furniture_is_valid_premultiplied_and_clear_at_its_corners() {
        let images = [
            piping_image(vec2(100.0, 60.0), 4.0, 2.5, Color32::from_rgb(220, 214, 200), 2.0),
            handle_image(vec2(120.0, 12.0), 2.0),
            jewel_image(6.0, Color32::from_rgb(200, 30, 24), 2.0),
            corner_image([false, false], 2.0),
            corner_image([true, true], 2.0),
        ];
        for image in images {
            valid(&image);
            let [w, h] = image.size;
            // The piping's inside is the grille's: it is clear in the middle too
            for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
                assert!(image.pixels[y * w + x].a() < 8, "{:?}", image.pixels[y * w + x]);
            }
        }
    }
}
