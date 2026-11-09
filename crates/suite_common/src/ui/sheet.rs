//! The sheet that rises from below, and the small pieces a rig builds its chain with: the plus
//! that opens it, the cross that takes a block out, the dashed place a block will stand in.
//!
//! A sheet is the second kind of surface of the rig (STYLE.md, "Plates and sheets", #259): where
//! a plate drops under an icon, a sheet rises from the bottom of the window over the line, with a
//! handle at its top edge. Like a plate it is drawn in a layer above the window and takes every
//! click in its own area, so nothing under it answers; it never closes by itself, and the caller
//! owns whether it is open. It closes on a click outside it, on escape, or on its own handle
//! (`Close::outside` for the click and the handle, as for a plate).

use nih_plug_egui::egui::{
    vec2, Align2, Area, Color32, CornerRadius, Id, Key, Order, Painter, Pos2, Rect, Response,
    Sense, Shape, Stroke, StrokeKind, Ui,
};

use super::plate::{Close, PLATE_RADIUS};
use super::theme::*;

/// The warm brass of the plus and of what is picked: the one colour on the line that is not
/// paint, hardware or print
pub const BRASS: Color32 = Color32::from_rgb(240, 214, 150);
/// The brass in shade, the plus's rim
const BRASS_DARK: Color32 = Color32::from_rgb(150, 118, 56);
/// The handle on a sheet's top edge: how wide it is drawn and how big a click on it is
const HANDLE: (f32, f32) = (46.0, 5.0);
const HANDLE_HIT: (f32, f32) = (150.0, 20.0);
/// The radius of the plus in a gap of the line
pub const PLUS_RADIUS: f32 = 12.0;

/// Draws a sheet filling `rect` in a layer above everything else and runs `add_contents` on it
/// with that rectangle. `rect` is in the window's own points and may run past the window's bottom
/// edge (a sheet that is still rising); it is not moved back inside. `fresh` is a click on the
/// frame that opened the sheet, which is not a click outside it. `id_source` tells this sheet from
/// another.
pub fn sheet(
    ui: &Ui,
    rect: Rect,
    id_source: &str,
    fresh: bool,
    add_contents: impl FnOnce(&mut Ui, Rect),
) -> Close {
    let parent_layer = ui.layer_id();
    let to_local = ui.ctx().layer_transform_from_global(parent_layer);
    let mut close = Close::default();
    Area::new(Id::new(("sheet", id_source)))
        .order(Order::Foreground)
        .fixed_pos(rect.min)
        .constrain(false)
        .show(ui.ctx(), |ui| {
            if let Some(to_global) = ui.ctx().layer_transform_to_global(parent_layer) {
                ui.ctx().set_transform_layer(ui.layer_id(), to_global);
            }
            ui.allocate_rect(rect, Sense::click_and_drag());
            paint_sheet(ui.painter(), rect);

            let handle = Rect::from_center_size(
                Pos2::new(rect.center().x, rect.top() + 11.0),
                vec2(HANDLE_HIT.0, HANDLE_HIT.1),
            );
            let grip = ui.interact(handle, Id::new(("sheet-handle", id_source)), Sense::click());
            let lit = grip.hovered();
            ui.painter().rect_filled(
                Rect::from_center_size(
                    Pos2::new(rect.center().x, rect.top() + 10.0),
                    vec2(HANDLE.0, HANDLE.1),
                ),
                CornerRadius::same(3),
                if lit { CHROME_LIGHT } else { CHROME_DARK },
            );

            add_contents(ui, rect);

            let pointer = ui.input(|input| input.pointer.interact_pos());
            let in_window = pointer.map(|at| to_local.map_or(at, |from_global| from_global * at));
            let on_sheet = in_window.is_some_and(|at| rect.contains(at));
            close.outside = grip.clicked()
                || (!fresh && ui.input(|input| input.pointer.any_click()) && !on_sheet);
            close.escape = ui.input(|input| input.key_pressed(Key::Escape));
        });
    close
}

/// The body of a sheet: its shadow above it, the dark metal with a chrome edge along the top
fn paint_sheet(painter: &Painter, rect: Rect) {
    let top_round =
        CornerRadius { nw: PLATE_RADIUS as u8 + 4, ne: PLATE_RADIUS as u8 + 4, sw: 0, se: 0 };
    painter.rect_filled(rect.translate(vec2(0.0, -5.0)).expand(2.0), top_round, SHADOW);
    painter.rect_filled(rect, top_round, PANEL);
    painter.rect_stroke(rect, top_round, Stroke::new(1.5_f32, CHROME_DARK), StrokeKind::Inside);
    painter.line_segment(
        [
            Pos2::new(rect.left() + 14.0, rect.top() + 2.0),
            Pos2::new(rect.right() - 14.0, rect.top() + 2.0),
        ],
        Stroke::new(1.0_f32, CHROME.gamma_multiply(0.55)),
    );
}

/// A round brass button with a plus engraved in it: what opens the sheet from a gap of the line.
/// `enabled` false draws it dim and dead (the line is full). Returns the click response, which is
/// never a click when it is dead
pub fn plus_button(ui: &Ui, center: Pos2, radius: f32, id_source: &str, enabled: bool) -> Response {
    let rect = Rect::from_center_size(center, vec2(radius * 2.0 + 4.0, radius * 2.0 + 4.0));
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let response = ui.interact(rect, Id::new(("plus", id_source)), sense);
    let painter = ui.painter();
    let (face, rim, ink) = if enabled {
        (BRASS, BRASS_DARK, INK)
    } else {
        (CHROME_DARK.gamma_multiply(0.6), CHROME_DARK.gamma_multiply(0.4), INK)
    };
    painter.circle_filled(center + vec2(0.0, 2.5), radius + 0.5, SHADOW);
    painter.circle_filled(center, radius, face);
    painter.circle_stroke(center, radius, Stroke::new(1.5_f32, rim));
    if enabled && response.hovered() {
        painter.circle_stroke(center, radius + 3.0, Stroke::new(1.5_f32, BRASS));
    }
    let arm = radius * 0.46;
    let stroke = Stroke::new(2.6_f32, ink);
    painter.line_segment([center - vec2(arm, 0.0), center + vec2(arm, 0.0)], stroke);
    painter.line_segment([center - vec2(0.0, arm), center + vec2(0.0, arm)], stroke);
    response
}

/// A small chrome disc with a cross on a block's corner: takes the block out of the chain
pub fn cross_button(ui: &Ui, center: Pos2, radius: f32, id_source: &str) -> Response {
    let rect = Rect::from_center_size(center, vec2(radius * 2.0 + 6.0, radius * 2.0 + 6.0));
    // It stands on the title that a block is dragged by, so it takes the drag as well
    let response = ui.interact(rect, Id::new(("cross", id_source)), Sense::click_and_drag());
    let painter = ui.painter();
    painter.circle_filled(center + vec2(0.0, 2.0), radius + 0.5, SHADOW);
    painter.circle_filled(
        center,
        radius,
        if response.hovered() { CHROME_SHINE } else { CHROME_LIGHT },
    );
    painter.circle_stroke(center, radius, Stroke::new(1.0_f32, Color32::from_black_alpha(110)));
    let arm = radius * 0.38;
    let stroke = Stroke::new(2.0_f32, INK);
    painter.line_segment([center + vec2(-arm, -arm), center + vec2(arm, arm)], stroke);
    painter.line_segment([center + vec2(-arm, arm), center + vec2(arm, -arm)], stroke);
    response
}

/// A dashed outline: the place a block will stand in, or the empty side of the line
pub fn dashed_frame(painter: &Painter, rect: Rect, color: Color32) {
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    painter.extend(Shape::dashed_line(&corners, Stroke::new(2.0_f32, color), 9.0, 6.0));
}

/// A dashed vertical line, from `top` to `bottom` at `x`: the gap the chain is being added to
pub fn dashed_rule(painter: &Painter, x: f32, top: f32, bottom: f32, color: Color32) {
    let points = [Pos2::new(x, top), Pos2::new(x, bottom)];
    painter.extend(Shape::dashed_line(&points, Stroke::new(2.0_f32, color), 7.0, 5.0));
}

/// A card that follows the pointer while it is dragged, above everything, in `rect` (the window's
/// own points): `add_contents` draws it. It takes no clicks
pub fn floating(ui: &Ui, rect: Rect, id_source: &str, add_contents: impl FnOnce(&mut Ui, Rect)) {
    let parent_layer = ui.layer_id();
    Area::new(Id::new(("floating", id_source)))
        .order(Order::Tooltip)
        .fixed_pos(rect.min)
        .constrain(false)
        .interactable(false)
        .show(ui.ctx(), |ui| {
            if let Some(to_global) = ui.ctx().layer_transform_to_global(parent_layer) {
                ui.ctx().set_transform_layer(ui.layer_id(), to_global);
            }
            add_contents(ui, rect);
        });
}

/// A caption in the rig's small print, centred on `center` and dimmed: what the line says about
/// itself ("add here", "all sixteen slots in use")
pub fn aside(painter: &Painter, center: Pos2, text: &str, size: f32) -> Rect {
    silk_label_dim(painter, center, text, size)
}

fn silk_label_dim(painter: &Painter, center: Pos2, text: &str, size: f32) -> Rect {
    super::widgets::print_label(painter, center, text, size, SILK.gamma_multiply(0.7))
}

/// Silk text anchored at its left edge, for the rows of a sheet
pub fn left_label(
    painter: &Painter,
    left_center: Pos2,
    text: &str,
    size: f32,
    color: Color32,
) -> Rect {
    spaced_text(
        painter,
        left_center,
        Align2::LEFT_CENTER,
        &text.to_uppercase(),
        display(size),
        color,
        0.16,
    )
}

/// Silk text centred on `center`, in `color`
pub fn center_label(
    painter: &Painter,
    center: Pos2,
    text: &str,
    size: f32,
    color: Color32,
) -> Rect {
    super::widgets::print_label(painter, center, text, size, color)
}

/// Whether the pointer is over `rect` (in `ui`'s own points) and nothing above the window, a
/// plate or the sheet, is over the pointer: what a hover on the line asks, since the line's layer
/// is scaled and a plate drawn over it takes the pointer
pub fn pointer_over(ui: &Ui, rect: Rect) -> bool {
    let Some(at) = ui.input(|input| input.pointer.hover_pos()) else {
        return false;
    };
    if ui.ctx().layer_id_at(at).is_some_and(|layer| layer.order != Order::Background) {
        return false;
    }
    let local = ui.ctx().layer_transform_from_global(ui.layer_id()).map_or(at, |from| from * at);
    rect.contains(local)
}
