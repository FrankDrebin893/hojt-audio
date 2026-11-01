//! Mock-ups for the explore step of #177 (the second amp's look). Test-only, never wired to
//! the DSP and never shipped: `stack` stands in for `Pedals::stack` when `EXPLORE` is `a`,
//! `b1`, `b2` or `c`, and the `explore_render` test draws the window with it.
//! Each direction was rendered with the constants it needs set before the run (they are what
//! the direction costs the window), `EXPLORE=<v> UPDATE_SNAPSHOTS=1 cargo test -p amp --lib
//! explore_render -- --ignored`, the picture taken from `tests/snapshots/explore_<v>.png`:
//! a: `HEAD_HEIGHT` 372, `WINDOW_HEIGHT` 842. b1, b2: 248 and 718. c: 180 and 650, with
//! `STACK_WIDTH` 700 and `WINDOW_WIDTH` 2997.
//! Amp 1 is Brøl and amp 2 is Klar, amp 2 on, Blend at the middle, in every direction.

use super::*;
use nih_plug_egui::egui::{epaint::CubicBezierShape, Stroke};
use suite_common::ui::theme::{CABLE, CHROME, CHROME_DARK, CHROME_LIGHT, INK, SILK};
use suite_common::ui::widgets::label_tape;
use suite_common::ui::{HeadStyle, KnobCap, Plate};

/// A head drawn in `rect` as `amp` looks, its six dials in a row from `first` with `spacing`
/// between, its nameplate at `plate` (from the head's top left) unless it is None. Returns the
/// faces and the middle of its input jack
#[allow(clippy::too_many_arguments)]
fn head(
    ui: &mut Ui,
    p: &Pedals,
    rect: Rect,
    amp: Amp,
    grille_height: f32,
    lit: bool,
    dials: [&FloatParam; 6],
    first: f32,
    spacing: f32,
    radius: f32,
    name: Option<(&str, f32)>,
) -> (HeadFaces, Pos2, [Pos2; 6]) {
    let look = looks::look(amp);
    let style = HeadStyle { grille_height, ..look.head };
    let (panel, print) = (style.panel, style.print);
    let faces = ui::amp_head(ui, rect, &style, lit);
    let corner = faces.panel.left_top();
    let at: [Pos2; 6] =
        std::array::from_fn(|i| corner + vec2(first + spacing * i as f32, DIAL_Y));
    for ((param, label), at) in dials.into_iter().zip(DIALS).zip(at) {
        panel_knob(ui, p.setter, param, at, radius, label, look.knobs, print);
    }
    let jack = corner + vec2(HEAD_JACK_X, DIAL_Y);
    print_label(
        ui.painter(),
        Pos2::new(jack.x, ui::panel_caption(at[0], radius).y),
        "In",
        LAMP_CAPTION_SIZE,
        print,
    );
    if let Some((id, width)) = name {
        nameplate_stepper(ui, faces.grille.center(), width, panel, print, amp.model().name, id);
    }
    (faces, jack, at)
}

fn cable(ui: &Ui, points: [Pos2; 4]) {
    let shadow = points.map(|point| point + vec2(2.0, 3.0));
    for (points, color) in [(shadow, Color32::from_black_alpha(90)), (points, CABLE)] {
        ui.painter().add(CubicBezierShape::from_points_stroke(
            points,
            false,
            Color32::TRANSPARENT,
            Stroke::new(5.0, color),
        ));
    }
}

fn jack(ui: &Ui, at: Pos2) {
    ui.painter().circle_filled(at, 8.0, CHROME_DARK);
    ui.painter().circle_filled(at, 6.0, CHROME);
    ui.painter().circle_filled(at, 4.0, CHROME_DARK);
}

/// The cabinet of the amp that plays, with the plate's old controls in a row of its own
/// (`knobs` are the Mic and Res places, `cab_switch` the Cab switch), and `extra` for what a
/// direction adds. Returns the cabinet's top left
fn cabinet(
    ui: &mut Ui,
    p: &Pedals,
    amp: Amp,
    rect: Rect,
    knobs: [f32; 2],
    cab_switch: f32,
    knob: f32,
    stepper_x: f32,
) -> Pos2 {
    let chosen = p.params.cabinet.lock().unwrap_or_else(PoisonError::into_inner).clone();
    let look = looks::cab_look(amp, &chosen, p.cab_loader.status());
    ui::cabinet(ui, rect, look);
    let cab = rect.min;
    ui::amp::mounted_plate(ui.painter(), CAB_JACK_PLATE.translate(cab.to_vec2()), look.plate);
    let shown = p.cab_loader.display_name(&chosen, CAB_NAME_CHARS);
    silk_label(ui.painter(), cab + vec2(stepper_x, SPEAKER_CAPTION.y), "Speaker", LAMP_CAPTION_SIZE);
    stepper(ui, cab + vec2(stepper_x, SPEAKER_STEPPER.y), SPEAKER_WIDTH - 8.0, &shown, "cabinet");
    silk_label(ui.painter(), cab + vec2(stepper_x, MIC_CAPTION.y), "Mic Type", LAMP_CAPTION_SIZE);
    stepper(ui, cab + vec2(stepper_x, MIC_STEPPER.y), MIC_STEPPER_WIDTH, p.params.cab_mic_type.value().short_name(), "mic");
    for ((param, caption), x) in [(&p.params.cab_mic, "Mic"), (&p.params.cab_res, "Res")]
        .into_iter()
        .zip(knobs)
    {
        param_knob(ui, p.setter, param, cab + vec2(x, CAB_KNOBS[0].y), knob, caption, None);
    }
    silk_label(ui.painter(), cab + vec2(cab_switch, CAB_SWITCH_CAPTION.y), "Cab", LAMP_CAPTION_SIZE);
    param_switch(
        ui,
        p.setter,
        &p.params.cab_on,
        cab + vec2(cab_switch, CAB_SWITCH.y),
        cab + vec2(cab_switch, CAB_LED.y),
    );
    silk_label(ui.painter(), cab + CAB_JACK_CAPTION, "Out", LAMP_CAPTION_SIZE);
    cab
}

pub(super) fn stack(p: &Pedals, ui: &mut Ui, top_left: Pos2, variant: &str) -> Jacks {
    match variant {
        "a" => a(p, ui, top_left),
        "b1" => b(p, ui, top_left, false),
        "b2" => b(p, ui, top_left, true),
        _ => c(p, ui, top_left),
    }
}

/// A, two heads on one cabinet: amp 1 on top, amp 2 under it, the cabinet's plate gets Blend
fn a(p: &Pedals, ui: &mut Ui, tl: Pos2) -> Jacks {
    let params = p.params;
    let (a1, a2) = (params.amp.value(), params.amp2.value());
    let (h1, h2) = (180.0, 204.0);
    let low = Rect::from_min_size(tl + vec2(0.0, h1 - 12.0), vec2(STACK_WIDTH, h2));
    let top = Rect::from_min_size(tl, vec2(STACK_WIDTH, h1));
    // The lower head first: the upper one stands on it and covers its handle. Its grille is
    // taller, for amp 2's switch
    let (faces2, jack2, _) = head(
        ui, p, low, a2, 64.0, true, params.amp2_dials(), DIAL_FIRST_X, DIAL_SPACING, DIAL_RADIUS,
        Some(("amp2", NAMEPLATE_WIDTH)),
    );
    let (_, jack1, _) = head(
        ui, p, top, a1, 40.0, !params.bypass.value(), params.amp_dials(), DIAL_FIRST_X,
        DIAL_SPACING, DIAL_RADIUS, Some(("amp", NAMEPLATE_WIDTH)),
    );
    let look2 = looks::look(a2);
    let g = faces2.grille;
    let sw = Rect::from_center_size(Pos2::new(g.right() - 52.0, g.center().y), vec2(84.0, g.height() - 6.0));
    ui::amp::mounted_plate(ui.painter(), sw, look2.head.panel);
    rig_led(ui.painter(), sw.center() - vec2(28.0, 0.0), true);
    small_footswitch(ui, sw.center() + vec2(12.0, 0.0), "amp2_on");
    // The input cable is shared: a link from the lower head's jack to the upper one's, round
    // the left edge of the panels
    cable(ui, [jack2, jack2 + vec2(-52.0, -40.0), jack1 + vec2(-52.0, 40.0), jack1]);
    jack(ui, jack1);
    jack(ui, jack2);

    let cab_rect = Rect::from_min_size(tl + vec2(0.0, h1 + h2 - 12.0), vec2(STACK_WIDTH, PEDAL_HEIGHT));
    let cab = cabinet(ui, p, a1, cab_rect, [206.0, 264.0], 400.0, 15.0, 96.0);
    // Blend, the one new control on the plate: a bigger knob
    param_knob(ui, p.setter, &params.amp_blend, cab + vec2(333.0, 66.0), 20.0, "Blend", None);
    Jacks { input: jack2, output: cab + CAB_JACK }
}

/// B, one head with a flip: both nameplates on the grille, Blend between them, the dials of
/// the amp that is picked on the panel (`second`: amp 2 is picked)
fn b(p: &Pedals, ui: &mut Ui, tl: Pos2, second: bool) -> Jacks {
    let params = p.params;
    let (a1, a2) = (params.amp.value(), params.amp2.value());
    let shown = if second { a2 } else { a1 };
    let h = HEAD_HEIGHT;
    let rect = Rect::from_min_size(tl, vec2(STACK_WIDTH, h));
    let dials = if second { params.amp2_dials() } else { params.amp_dials() };
    let (faces, jack_at, _) = head(
        ui, p, rect, shown, 108.0, true, dials, DIAL_FIRST_X, DIAL_SPACING, DIAL_RADIUS, None,
    );
    let look = looks::look(shown);
    let (panel, print) = (look.head.panel, look.head.print);
    let g = faces.grille;
    let y = g.center().y;
    // Across the grille: amp 1, Blend on its plate, amp 2, amp 2's switch on its plate
    let (width, reach) = (64.0, 52.0);
    let left = Pos2::new(g.left() + 68.0, y + 8.0);
    let right = Pos2::new(g.left() + 286.0, y + 8.0);
    nameplate_stepper(ui, left, width, panel, print, a1.model().name, "amp");
    nameplate_stepper(ui, right, width, panel, print, a2.model().name, "amp2");
    let _ = reach;
    // Which one the dials below belong to: its lamp is lit
    rig_led(ui.painter(), left - vec2(0.0, 30.0), !second);
    rig_led(ui.painter(), right - vec2(0.0, 30.0), second);
    let plate_h = g.height() - 10.0;
    let middle = Rect::from_center_size(Pos2::new(g.left() + 175.0, y), vec2(76.0, plate_h));
    ui::amp::mounted_plate(ui.painter(), middle, Plate::BlackAnodised);
    panel_knob(
        ui, p.setter, &params.amp_blend, Pos2::new(middle.center().x, middle.top() + 48.0), 13.0,
        "Blend", KnobCap::Fluted, SILK,
    );
    let sw = Rect::from_center_size(Pos2::new(g.right() - 36.0, y), vec2(64.0, plate_h));
    ui::amp::mounted_plate(ui.painter(), sw, Plate::BlackAnodised);
    rig_led(ui.painter(), Pos2::new(sw.center().x, sw.top() + 16.0), true);
    small_footswitch(ui, Pos2::new(sw.center().x, sw.top() + 52.0), "amp2_on");
    silk_label(ui.painter(), Pos2::new(sw.center().x, sw.bottom() - 12.0), "Amp 2", LAMP_CAPTION_SIZE);

    let cab_rect = Rect::from_min_size(tl + vec2(0.0, h), vec2(STACK_WIDTH, PEDAL_HEIGHT));
    let cab = cabinet(ui, p, a1, cab_rect, CAB_KNOBS.map(|k| k.x), CAB_SWITCH.x, CAB_KNOB_RADIUS, SPEAKER_STEPPER.x);
    Jacks { input: jack_at, output: cab + CAB_JACK }
}

/// C, two lanes: a lane per amp (its head and its own cabinet side by side), that meet at a
/// fader plate on the right with Blend as a crossfader and the cabinet controls both share
fn c(p: &Pedals, ui: &mut Ui, tl: Pos2) -> Jacks {
    let params = p.params;
    let (a1, a2) = (params.amp.value(), params.amp2.value());
    let lane_h = 236.0;
    let head_w = 420.0;
    let cab_w = 150.0;
    let mut jacks = [Pos2::ZERO; 2];
    let mut ends = [Pos2::ZERO; 2];
    for (i, (amp, dials)) in
        [(a1, params.amp_dials()), (a2, params.amp2_dials())].into_iter().enumerate()
    {
        let origin = tl + vec2(0.0, (lane_h + 18.0) * i as f32);
        let rect = Rect::from_min_size(origin, vec2(head_w, lane_h));
        let look = looks::look(amp);
        let id = if i == 0 { "amp" } else { "amp2" };
        let (faces, jack_at, at) =
            head(ui, p, rect, amp, 40.0, true, dials, 50.0, 56.0, 17.0, Some((id, NAMEPLATE_WIDTH)));
        // The lane's lower panel: what it is, and its switch
        let line = at[0].y + 78.0;
        let left = faces.panel.left();
        let right = faces.panel.right();
        print_label(ui.painter(), Pos2::new(left + 60.0, line), if i == 0 { "Amp 1" } else { "Amp 2" }, 18.0, look.head.print);
        if i == 0 {
            print_label(ui.painter(), Pos2::new(right - 90.0, line), "Follows On", 13.0, look.head.print);
        } else {
            print_label(ui.painter(), Pos2::new(right - 120.0, line), "On", 15.0, look.head.print);
            rig_led(ui.painter(), Pos2::new(right - 95.0, line), true);
            small_footswitch(ui, Pos2::new(right - 50.0, line), "amp2_on");
        }
        // Its own cabinet, beside it
        let chosen = params.cabinet.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let cab_look = looks::cab_look(amp, &chosen, p.cab_loader.status());
        let cab_rect = Rect::from_min_size(origin + vec2(head_w + 6.0, 0.0), vec2(cab_w, lane_h));
        let mut style = *cab_look;
        style.plate_height = 0.0;
        ui::cabinet(ui, cab_rect, &style);
        jacks[i] = jack_at;
        ends[i] = cab_rect.right_center();
    }
    // The input is shared, a link between the two lanes' jacks
    cable(ui, [jacks[1], jacks[1] + vec2(-70.0, -50.0), jacks[0] + vec2(-70.0, 50.0), jacks[0]]);
    jack(ui, jacks[0]);
    jack(ui, jacks[1]);

    // The plate where the lanes meet: Blend as a fader between them, then what both share
    let left = tl.x + head_w + cab_w + 12.0;
    let plate = Rect::from_min_size(Pos2::new(left, tl.y), vec2(STACK_WIDTH - left + tl.x, 2.0 * lane_h + 18.0));
    ui::amp::mounted_plate(ui.painter(), plate, Plate::BlackAnodised);
    for end in ends {
        ui::coupler(ui.painter(), end.x, plate.left(), end.y);
    }
    let x = plate.center().x;
    let at = |dy: f32| Pos2::new(x, plate.top() + dy);
    silk_label(ui.painter(), at(22.0), "Amp 1", LAMP_CAPTION_SIZE);
    let track = Rect::from_min_max(at(38.0) - vec2(4.0, 0.0), at(150.0) + vec2(4.0, 0.0));
    ui.painter().rect_filled(track, 4.0, Color32::from_rgb(8, 8, 8));
    ui.painter().rect_stroke(track, 4.0, Stroke::new(1.0, CHROME_DARK), nih_plug_egui::egui::StrokeKind::Outside);
    for k in 0..=10 {
        let ty = track.top() + track.height() * k as f32 / 10.0;
        let len = if k % 5 == 0 { 12.0 } else { 6.0 };
        for s in [-1.0, 1.0] {
            ui.painter().line_segment(
                [Pos2::new(x + s * 12.0, ty), Pos2::new(x + s * (12.0 + len), ty)],
                Stroke::new(1.5, SILK),
            );
        }
    }
    let blend = params.amp_blend.unmodulated_normalized_value();
    let cap_y = track.top() + track.height() * blend;
    let cap = Rect::from_center_size(Pos2::new(x, cap_y), vec2(34.0, 22.0));
    ui.painter().rect_filled(cap.translate(vec2(1.0, 3.0)), 3.0, Color32::from_black_alpha(120));
    ui.painter().rect_filled(cap, 3.0, CHROME_LIGHT);
    ui.painter().rect_stroke(cap, 3.0, Stroke::new(1.0, CHROME_DARK), nih_plug_egui::egui::StrokeKind::Inside);
    ui.painter().line_segment([Pos2::new(cap.left() + 5.0, cap_y), Pos2::new(cap.right() - 5.0, cap_y)], Stroke::new(2.0, INK));
    silk_label(ui.painter(), at(168.0), "Amp 2", LAMP_CAPTION_SIZE);
    silk_label(ui.painter(), at(192.0), "Blend", LAMP_CAPTION_SIZE);
    label_tape(ui.painter(), at(202.0), &params.amp_blend.normalized_value_to_string(blend, true), 12.0);
    // What both share: Mic and Res, the Cab switch, and the one output
    for ((param, caption), dx) in [(&params.cab_mic, "Mic"), (&params.cab_res, "Res")].into_iter().zip([-30.0, 30.0]) {
        param_knob(ui, p.setter, param, at(272.0) + vec2(dx, 0.0), CAB_KNOB_RADIUS, caption, None);
    }
    let out = at(352.0);
    jack(ui, out);
    silk_label(ui.painter(), out + vec2(0.0, 22.0), "Out", LAMP_CAPTION_SIZE);
    silk_label(ui.painter(), at(398.0), "Cab", LAMP_CAPTION_SIZE);
    param_switch(ui, p.setter, &params.cab_on, at(452.0), at(416.0));
    Jacks { input: jacks[1], output: out }
}
