//! The hardware plate: how a rig configures anything that is not on show at all times.
//!
//! A plate drops down from the icon or display that opens it, dark with a chrome edge, and what
//! stands on it is the rig's own hardware (label tape, steppers, knobs, lit lamps), never an egui
//! widget. It is drawn in a layer above the window and takes every click in its own area, so
//! nothing behind it answers while it is open. It never closes by itself: the caller owns which
//! plate is open (one at a time) and closes it when [`Close`] says so, or when its icon is
//! clicked again (see `icon_switch`).
//!
//! The effects bank that rises from below is a sheet, not a plate (`sheet.rs`).

use nih_plug_egui::egui::{
    vec2, Area, Id, Key, Order, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui,
};

use super::theme::*;
use super::widgets::silk_label;

/// The rounding of a plate's corners
pub const PLATE_RADIUS: f32 = 6.0;
/// Height the plate's title takes at its top, when it has one (`plate_title`)
pub const PLATE_TITLE_HEIGHT: f32 = 26.0;
/// Size of a plate's title
const TITLE_SIZE: f32 = 14.0;

/// What a plate was asked: to close because of a click outside it, or because of escape. A plate
/// with a name being typed on it takes escape for the name first (as `drop_panel`'s callers do),
/// and then leaves the plate open
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Close {
    /// A click landed outside the plate and outside the icon that opens it
    pub outside: bool,
    /// Escape was pressed
    pub escape: bool,
}

impl Close {
    /// Either of the two
    pub fn asked(self) -> bool {
        self.outside || self.escape
    }
}

/// The body of a plate: its shadow, the dark metal and the chrome edge round it, with a lit hairline
/// along the top
pub fn paint_body(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect.translate(vec2(0.0, 5.0)).expand(2.0), PLATE_RADIUS + 1.0, SHADOW);
    painter.rect_filled(rect, PLATE_RADIUS, PANEL);
    painter.rect_stroke(rect, PLATE_RADIUS, Stroke::new(1.5_f32, CHROME_DARK), StrokeKind::Inside);
    painter.line_segment(
        [
            Pos2::new(rect.left() + PLATE_RADIUS, rect.top() + 2.0),
            Pos2::new(rect.right() - PLATE_RADIUS, rect.top() + 2.0),
        ],
        Stroke::new(1.0_f32, CHROME.gamma_multiply(0.55)),
    );
}

/// A plate's title, silk-printed in the middle of the top of `rect`
pub fn plate_title(painter: &Painter, rect: Rect, text: &str) {
    silk_label(
        painter,
        Pos2::new(rect.center().x, rect.top() + 6.0 + PLATE_TITLE_HEIGHT / 2.0),
        text,
        TITLE_SIZE,
    );
}

/// Draws a plate filling `rect` (in the window's own points) in a layer above everything else,
/// and runs `add_contents` on it with the plate's rectangle. `anchor` is the icon or display
/// that opens it: a click there is not a click outside, so that it can close the plate by
/// itself. `id_source` tells this plate from another.
///
/// The plate is kept inside the window: where `rect` would run past an edge it is moved back.
pub fn plate(
    ui: &Ui,
    rect: Rect,
    id_source: &str,
    anchor: Rect,
    add_contents: impl FnOnce(&mut Ui, Rect),
) -> Close {
    let parent_layer = ui.layer_id();
    let clip = ui.clip_rect();
    let to_local = ui.ctx().layer_transform_from_global(parent_layer);
    let mut close = Close::default();
    let rect = keep_inside(rect, clip);
    Area::new(Id::new(("plate", id_source)))
        .order(Order::Foreground)
        .fixed_pos(rect.min)
        .constrain_to(clip)
        .show(ui.ctx(), |ui| {
            if let Some(to_global) = ui.ctx().layer_transform_to_global(parent_layer) {
                ui.ctx().set_transform_layer(ui.layer_id(), to_global);
            }
            // The plate takes the whole area up: every click in it is the plate's, whether it
            // lands on a control or not
            ui.allocate_rect(rect, Sense::click_and_drag());
            paint_body(ui.painter(), rect);
            add_contents(ui, rect);

            let pointer = ui.input(|input| input.pointer.interact_pos());
            let in_window = pointer.map(|at| to_local.map_or(at, |from_global| from_global * at));
            let on_anchor = in_window.is_some_and(|at| anchor.contains(at));
            close.outside = ui.input(|input| input.pointer.any_click())
                && !ui.rect_contains_pointer(rect)
                && !on_anchor;
            close.escape = ui.input(|input| input.key_pressed(Key::Escape));
        });
    close
}

/// `rect` moved, not resized, to lie inside `bounds` where it can
fn keep_inside(rect: Rect, bounds: Rect) -> Rect {
    let dx = (bounds.left() - rect.left()).max(0.0) + (bounds.right() - rect.right()).min(0.0);
    let dy = (bounds.top() - rect.top()).max(0.0) + (bounds.bottom() - rect.bottom()).min(0.0);
    rect.translate(vec2(dx, dy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_a_plate_is_moved_back_inside_the_window_not_resized() {
        let window = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 500.0));
        let size = vec2(300.0, 200.0);
        let past_the_right = Rect::from_min_size(Pos2::new(900.0, 80.0), size);
        let kept = keep_inside(past_the_right, window);
        assert_eq!(kept.size(), size);
        assert_eq!(kept.right(), 1000.0);
        let inside = Rect::from_min_size(Pos2::new(100.0, 80.0), size);
        assert_eq!(keep_inside(inside, window), inside);
        let before_the_left = Rect::from_min_size(Pos2::new(-40.0, 80.0), size);
        assert_eq!(keep_inside(before_the_left, window).left(), 0.0);
    }

    #[test]
    fn test_close_asks_for_either() {
        assert!(!Close::default().asked());
        assert!(Close { outside: true, escape: false }.asked());
        assert!(Close { outside: false, escape: true }.asked());
    }
}
