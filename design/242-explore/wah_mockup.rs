//! Mock-ups of the wah rocker for #242 (explore). Not built by the workspace: copy to
//! `plugins/amp/tests/wah_mockup.rs`, run `UPDATE_SNAPSHOTS=1 cargo test -p amp --test wah_mockup`,
//! and the pictures land in `plugins/amp/tests/snapshots/`. Not shipped.
//! The pedals are the editor's own (`mini_pedal`, knobs, tape, LED, switch); only the rocker is drawn here.
#![cfg(target_os = "linux")]

use nih_plug_egui::egui::epaint::{Mesh, Vertex, WHITE_UV};
use nih_plug_egui::egui::{
    pos2, vec2, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Ui,
};
use suite_common::ui::snapshot::pedal_harness;
use suite_common::ui::theme::{
    display, shade, spaced_text, CHROME, CHROME_DARK, CHROME_LIGHT, CHROME_SHINE, INK, PAINT_ORANGE,
    SILK,
};
use suite_common::ui::widgets::{knob_with, label_tape};
use suite_common::ui::{self, KnobCap, MiniStyle, Motif};

const W: f32 = 180.0;
const H: f32 = 310.0;
const GAP: f32 = 12.0;
const WAH_PAINT: Color32 = Color32::from_rgb(146, 118, 30);

const TRIO: [(f32, f32); 3] = [(-45.0, 71.0), (45.0, 71.0), (0.0, 169.0)];

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgb(
        lerp(a.r() as f32, b.r() as f32, t) as u8,
        lerp(a.g() as f32, b.g() as f32, t) as u8,
        lerp(a.b() as f32, b.b() as f32, t) as u8,
    )
}
fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// A quad with a colour at each corner (clockwise from the top left)
fn quad(p: &Painter, pts: [Pos2; 4], cols: [Color32; 4]) {
    let mut m = Mesh::default();
    for i in 0..4 {
        m.vertices.push(Vertex { pos: pts[i], uv: WHITE_UV, color: cols[i] });
    }
    m.indices.extend([0, 1, 2, 0, 2, 3]);
    p.add(Shape::mesh(m));
}

fn caption(p: &Painter, at: Pos2, text: &str, size: f32) {
    spaced_text(p, at, Align2::CENTER_CENTER, &text.to_uppercase(), display(size), SILK, 0.16);
}

fn tape_of(v: f32) -> String {
    format!("{:.1}", v * 10.0)
}

/// The Bite and Sens knobs as the editor's small knobs: name and value tape under them
fn knobs_below(ui: &mut Ui, c: Pos2, bite: Pos2, sens: Pos2, radius: f32) {
    for (id, at, name) in [("bite", bite, "Bite"), ("sens", sens, "Sens")] {
        let mut value = 0.5;
        ui::value_knob(ui, &format!("{id}{c:?}"), &mut value, 0.5, c + at.to_vec2(), radius, name, tape_of);
    }
}

/// The same two as a row on a panel: name above, value tape under
fn knobs_panel(ui: &mut Ui, c: Pos2, bite: Pos2, sens: Pos2, radius: f32) {
    for (id, at, name) in [("bite", bite, "Bite"), ("sens", sens, "Sens")] {
        let centre = c + at.to_vec2();
        ui.interact(Rect::from_center_size(centre, vec2(40.0, 40.0)), nih_plug_egui::egui::Id::new(("pk", id, c.x as i32)), nih_plug_egui::egui::Sense::drag());
        let p = ui.painter();
        knob_with(p, centre, radius, 0.5, None, KnobCap::Fluted, SILK);
        caption(p, ui::panel_caption(centre, radius), name, 13.0);
        label_tape(p, pos2(centre.x, centre.y + radius + 5.0), &tape_of(0.5), 10.0);
    }
}

/// The footswitch, and the lamp that says it is on
fn on_switch(ui: &mut Ui, c: Pos2, led: Pos2) {
    ui::rig_led(ui.painter(), c + led.to_vec2(), true);
    ui::small_footswitch(ui, c + vec2(0.0, 273.0), &format!("on{c:?}"));
}

/// The Auto push button (the Delay's Sync idiom): lamp above, caption under
fn auto_stack(ui: &mut Ui, c: Pos2, button: Pos2) {
    caption(ui.painter(), c + button.to_vec2() + vec2(0.0, 19.0), "Auto", 13.0);
    ui::rig_led(ui.painter(), c + button.to_vec2() + vec2(0.0, -24.0), false);
    ui::chrome_button(ui, c + button.to_vec2(), &format!("auto{c:?}"));
}

/// The neighbour for scale: the Pitch pedal as the editor draws it
fn reference(ui: &mut Ui, rect: Rect) {
    let face = ui::mini_pedal(
        ui,
        rect,
        &MiniStyle { title: "Pitch", paint: PAINT_ORANGE, motif: Motif::Horizon },
    );
    let c = face.center_top();
    for (i, (x, y)) in TRIO.iter().enumerate() {
        let mut v = 0.5;
        let name = ["Semitones", "Blend", "Tracking"][i];
        ui::value_knob(ui, &format!("ref{i}"), &mut v, 0.5, c + vec2(*x, *y), 17.0, name, tape_of);
    }
    ui::rig_led(ui.painter(), c + vec2(0.0, 71.0), true);
    ui::small_footswitch(ui, c + vec2(0.0, 273.0), "ref-on");
}

fn wah_face(ui: &mut Ui, rect: Rect) -> Pos2 {
    let face = ui::mini_pedal(
        ui,
        rect,
        &MiniStyle { title: "Wah", paint: WAH_PAINT, motif: Motif::Plain },
    );
    face.center_top()
}

// ---------------------------------------------------------------------------------------
// A: Treadle. The rocker drawn in perspective, dragged up and down; the toe end drops as the
// value rises.

fn direction_a(ui: &mut Ui, rect: Rect, v: f32) {
    let c = wah_face(ui, rect);
    let p = ui.painter().clone();
    let at = |x: f32, y: f32| c + vec2(x, y);

    // The cast base the treadle sits in
    let plate = Rect::from_min_max(at(-74.0, 46.0), at(74.0, 140.0));
    p.rect_filled(plate.translate(vec2(0.0, 3.0)), 9.0, Color32::from_black_alpha(110));
    p.rect_filled(plate, 9.0, rgb(24, 22, 20));
    p.rect_stroke(plate, 9.0, Stroke::new(2.0_f32, CHROME_DARK), StrokeKind::Inside);
    p.rect_stroke(plate.shrink(4.0), 6.0, Stroke::new(1.0_f32, Color32::from_black_alpha(160)), StrokeKind::Inside);

    // The treadle: far (toe) end narrower, near (heel) end wider; it rocks about its axle
    let yt = 62.0 + 8.0 * v;
    let yb = 106.0 + 8.0 * (1.0 - v) - 3.0 * v;
    let (wf, wn) = (86.0, 124.0);
    let edge = |t: f32| (lerp(yt, yb, t), lerp(wf, wn, t) / 2.0);
    let (tl, tr) = (at(-wf / 2.0, yt), at(wf / 2.0, yt));
    let (bl, br) = (at(-wn / 2.0, yb), at(wn / 2.0, yb));
    let lit = lerp(0.0, 1.0, v);
    let far = mix(rgb(60, 56, 51), rgb(92, 86, 78), lit);
    let near = mix(rgb(30, 28, 26), rgb(48, 45, 41), lit);
    quad(&p, [tl, tr, br, bl], [far, far, near, near]);
    // Grip ribs, closer together towards the toe
    for i in 1..=7 {
        let t = (i as f32 / 8.0).powf(1.15);
        let (y, hw) = edge(t);
        let inset = 8.0;
        p.line_segment([at(-hw + inset, y), at(hw - inset, y)], Stroke::new(2.4_f32, rgb(12, 11, 10)));
        p.line_segment(
            [at(-hw + inset, y + 2.0), at(hw - inset, y + 2.0)],
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(220, 210, 190, 46)),
        );
    }
    // Chrome rails down both sides, and the front lip of the heel
    for side in [-1.0f32, 1.0] {
        let rail = [at(side * wf / 2.0, yt), at(side * wn / 2.0, yb)];
        p.line_segment(rail, Stroke::new(4.0_f32, CHROME_DARK));
        p.line_segment(
            [rail[0] + vec2(-side * 1.0, 0.0), rail[1] + vec2(-side * 1.0, 0.0)],
            Stroke::new(1.6_f32, CHROME_SHINE),
        );
    }
    quad(&p, [bl, br, at(wn / 2.0 - 2.0, yb + 9.0), at(-wn / 2.0 + 2.0, yb + 9.0)], [CHROME_LIGHT, CHROME_LIGHT, CHROME_DARK, CHROME_DARK]);
    p.line_segment([bl, br], Stroke::new(1.5_f32, CHROME_SHINE));
    // The toe's edge: a thin dark strip where the treadle drops away
    p.line_segment([tl, tr], Stroke::new(2.0_f32, rgb(10, 9, 8)));
    // The axle bolts on the base, either side of the middle
    let (ym, hw) = edge(0.5);
    for side in [-1.0f32, 1.0] {
        let b = at(side * (hw + 10.0), ym);
        p.circle_filled(b + vec2(0.0, 1.5), 5.5, Color32::from_black_alpha(120));
        p.circle_filled(b, 5.0, CHROME);
        p.circle_filled(b + vec2(-0.8, -1.0), 3.2, CHROME_LIGHT);
        p.line_segment([b + vec2(-2.5, 0.0), b + vec2(2.5, 0.0)], Stroke::new(1.0_f32, INK));
    }
    caption(&p, at(-52.0, 55.0), "Toe", 12.0);
    caption(&p, at(-52.0, 131.0), "Heel", 12.0);
    label_tape(&p, at(40.0, 123.0), &tape_of(v), 11.0);

    knobs_below(ui, c, pos2(-48.0, 171.0), pos2(48.0, 171.0), 15.0);
    ui::rig_led(ui.painter(), c + vec2(0.0, 171.0), true);
    ui::small_footswitch(ui, c + vec2(0.0, 273.0), &format!("on{c:?}"));
    auto_stack(ui, c, pos2(60.0, 275.0));
}

// ---------------------------------------------------------------------------------------
// B: Slider. A tall slot with a chrome treadle cap, toe at the top.

fn direction_b(ui: &mut Ui, rect: Rect, v: f32) {
    let c = wah_face(ui, rect);
    let p = ui.painter().clone();
    let at = |x: f32, y: f32| c + vec2(x, y);

    let (top, len) = (80.0, 116.0);
    let y_of = |v: f32| top + len * (1.0 - v);
    let slot = Rect::from_min_max(at(-8.0, top), at(8.0, top + len));
    // The scale, both sides, long ticks at heel, middle, toe
    for tick in 0..=10 {
        let y = c.y + y_of(tick as f32 / 10.0);
        let long = tick % 5 == 0;
        let reach = if long { 10.0 } else { 5.0 };
        for side in [-1.0f32, 1.0] {
            p.line_segment(
                [pos2(c.x + side * 20.0, y), pos2(c.x + side * (20.0 + reach), y)],
                Stroke::new(if long { 2.0 } else { 1.3 }, SILK),
            );
        }
    }
    p.rect_filled(slot.expand(3.0), 6.0, Color32::from_black_alpha(90));
    p.rect_filled(slot, 4.0, rgb(8, 8, 8));
    p.rect_stroke(slot, 4.0, Stroke::new(1.5_f32, CHROME_DARK), StrokeKind::Outside);
    // The cap: a ribbed chrome treadle, wider than the slot, a little taller than a fader's
    let y = c.y + y_of(v);
    let cap = Rect::from_center_size(pos2(c.x, y), vec2(64.0, 34.0));
    p.rect_filled(cap.translate(vec2(1.0, 4.0)), 5.0, Color32::from_black_alpha(130));
    p.rect_filled(cap, 5.0, CHROME_DARK);
    quad(
        &p,
        [cap.left_top() + vec2(1.5, 1.5), cap.right_top() + vec2(-1.5, 1.5), cap.right_bottom() + vec2(-1.5, -1.5), cap.left_bottom() + vec2(1.5, -1.5)],
        [CHROME_SHINE, CHROME_SHINE, CHROME, CHROME],
    );
    p.rect_stroke(cap, 5.0, Stroke::new(1.0_f32, Color32::from_black_alpha(140)), StrokeKind::Inside);
    for i in 0..6 {
        let gy = cap.top() + 6.0 + i as f32 * 4.4;
        p.line_segment([pos2(cap.left() + 7.0, gy), pos2(cap.right() - 7.0, gy)], Stroke::new(1.4_f32, CHROME_DARK));
        p.line_segment([pos2(cap.left() + 7.0, gy + 1.4), pos2(cap.right() - 7.0, gy + 1.4)], Stroke::new(0.8_f32, CHROME_SHINE));
    }
    caption(&p, at(0.0, 50.0), "Toe", 12.0);
    caption(&p, at(0.0, 221.0), "Heel", 12.0);
    // Bite over Sens on the left; the rocker's own value on tape on the right
    knobs_below(ui, c, pos2(-62.0, 80.0), pos2(-62.0, 176.0), 14.0);
    caption(&p, at(62.0, 98.0), "Rocker", 12.0);
    label_tape(&p, at(62.0, 108.0), &tape_of(v), 11.0);
    on_switch(ui, c, pos2(62.0, 76.0));
    auto_stack(ui, c, pos2(62.0, 275.0));
}

// ---------------------------------------------------------------------------------------
// C: Face dial. One very large knob, the pedal's whole face, with heel and toe on its scale.

fn direction_c(ui: &mut Ui, rect: Rect, v: f32) {
    let c = wah_face(ui, rect);
    let p = ui.painter().clone();
    let at = |x: f32, y: f32| c + vec2(x, y);

    let centre = at(0.0, 108.0);
    let radius = 46.0;
    // A dished well around the knob, so it reads as the pedal's centre
    p.circle_filled(centre + vec2(0.0, 3.0), 62.0, Color32::from_black_alpha(70));
    p.circle_filled(centre, 60.0, shade(WAH_PAINT, 0.45));
    p.circle_stroke(centre, 60.0, Stroke::new(1.5_f32, Color32::from_black_alpha(130)));
    knob_with(&p, centre, radius, v, None, KnobCap::ChromeSkirt, SILK);
    caption(&p, at(-60.0, 160.0), "Heel", 12.0);
    caption(&p, at(60.0, 160.0), "Toe", 12.0);
    label_tape(&p, at(0.0, 169.0), &tape_of(v), 11.0);
    ui::stepper(ui, at(0.0, 204.0), 50.0, "Pedal", &format!("mode{c:?}"));

    knobs_panel(ui, c, pos2(-56.0, 256.0), pos2(56.0, 256.0), 13.0);
    on_switch(ui, c, pos2(0.0, 231.0));
}

fn board(ui: &mut Ui, o: Pos2, f: &dyn Fn(&mut Ui, Rect, f32), values: &[f32]) {
    let rect_at = |i: usize| Rect::from_min_size(o + vec2(10.0 + i as f32 * (W + GAP), 10.0), vec2(W, H));
    reference(ui, rect_at(0));
    for (i, v) in values.iter().enumerate() {
        f(ui, rect_at(i + 1), *v);
    }
}

fn shot(name: &str, f: impl Fn(&mut Ui, Rect, f32) + 'static, values: &'static [f32]) {
    let width = (10.0 + (values.len() as f32 + 1.0) * (W + GAP)) as u32;
    let mut harness = pedal_harness(width, 330, move |ctx| {
        ui::rig(ctx, |ui, o| board(ui, o, &f, values));
    });
    harness.snapshot(name);
}

#[test]
fn a_treadle() {
    shot("wah_a_treadle", direction_a, &[0.0, 0.5, 1.0]);
}
#[test]
fn b_slider() {
    shot("wah_b_slider", direction_b, &[0.0, 0.5, 1.0]);
}
#[test]
fn c_face_dial() {
    shot("wah_c_face_dial", direction_c, &[0.0, 0.5, 1.0]);
}

#[test]
fn overview() {
    let mut harness = pedal_harness(790, 330, move |ctx| {
        ui::rig(ctx, |ui, o| {
            let rect_at = |i: usize| {
                Rect::from_min_size(o + vec2(10.0 + i as f32 * (W + GAP), 10.0), vec2(W, H))
            };
            reference(ui, rect_at(0));
            direction_a(ui, rect_at(1), 0.5);
            direction_b(ui, rect_at(2), 0.5);
            direction_c(ui, rect_at(3), 0.5);
        });
    });
    harness.snapshot("wah_overview");
}
