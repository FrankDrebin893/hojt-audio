//! The icons of a rig's strip: a small drawing in screen print that opens a plate, with a lamp and
//! a switch beside it where the thing can be on or off.
//!
//! The icon and its switch are two targets: the icon opens the plate (or is the whole control,
//! for a tuner), the switch turns the thing on and off.

use nih_plug_egui::egui::{
    vec2, Color32, Id, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui,
};

use super::theme::*;
use super::widgets::{beat_lamp_sized, chrome_button, Beat};

/// What an icon draws
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// A metronome: the tapering case with its pendulum
    Metronome,
    /// A tuning fork
    Tuner,
    /// A sun with rays: the settings
    Settings,
    /// A small ring of dots round a dot: the looper
    Looper,
}

/// The room an icon takes, across and down, from its middle: the click target
pub const ICON_SIZE: f32 = 44.0;
/// How far the lamp stands right of the icon's middle, and the switch
pub const LAMP_DX: f32 = 38.0;
pub const SWITCH_DX: f32 = 68.0;
/// How far an icon with a switch reaches right of its middle: the switch and its button
pub const SWITCH_REACH: f32 = SWITCH_DX + 14.0;
/// How far an icon reaches left of its middle
pub const ICON_REACH: f32 = ICON_SIZE / 2.0;
const LAMP_RADIUS: f32 = 5.5;
/// Half the height of the drawing
const HALF: f32 = 15.0;

/// What a frame of an `icon_switch` was asked
pub struct IconSwitch {
    /// The icon
    pub icon: Response,
    /// The switch, for an icon that has one
    pub switch: Option<Response>,
}

/// An icon centred on `center` with, when `lamp` is given, a lamp and a switch to its right. `open`
/// says its plate is open: a bar is printed under it. The lamp shows what the thing does (a
/// `Beat` for a metronome, `Regular` for a thing that is just on).
pub fn icon_switch(
    ui: &mut Ui,
    center: Pos2,
    icon: Icon,
    open: bool,
    lamp: Option<Beat>,
    id_source: &str,
) -> IconSwitch {
    let rect = Rect::from_center_size(center, vec2(ICON_SIZE, ICON_SIZE));
    let response = ui.interact(rect, Id::new(("icon", id_source)), Sense::click());
    let color = if response.hovered() || open { Color32::WHITE } else { SILK };
    paint_icon(ui.painter(), center, icon, color);
    if open {
        ui.painter().rect_filled(
            Rect::from_center_size(center + vec2(0.0, HALF + 7.0), vec2(30.0, 3.5)),
            1.75,
            CHROME,
        );
    }
    let switch = lamp.map(|beat| {
        beat_lamp_sized(ui.painter(), center + vec2(LAMP_DX, 0.0), beat, LAMP_RADIUS);
        chrome_button(ui, center + vec2(SWITCH_DX, 0.0), &format!("{id_source}-switch"))
    });
    IconSwitch { icon: response, switch }
}

/// The drawing of `icon`, about 30 points high, in `color`
pub fn paint_icon(painter: &Painter, center: Pos2, icon: Icon, color: Color32) {
    let stroke = Stroke::new(2.4_f32, color);
    let at = |x: f32, y: f32| center + vec2(x, y);
    match icon {
        Icon::Metronome => {
            // The case, a tall trapezoid, with its foot
            painter.add(Shape::closed_line(
                vec![at(-5.0, -HALF), at(5.0, -HALF), at(11.0, HALF), at(-11.0, HALF)],
                stroke,
            ));
            // The pendulum, leaning right, and its weight
            painter.line_segment([at(0.0, 9.0), at(6.0, -9.0)], stroke);
            painter.rect_filled(Rect::from_center_size(at(3.4, -2.5), vec2(7.0, 4.0)), 1.0, color);
        }
        Icon::Tuner => {
            // The two tines, joined in a U, and the stem under it
            painter.line_segment([at(-6.0, -HALF), at(-6.0, 1.0)], stroke);
            painter.line_segment([at(6.0, -HALF), at(6.0, 1.0)], stroke);
            painter.add(Shape::line(
                vec![at(-6.0, 1.0), at(-5.0, 5.0), at(0.0, 7.0), at(5.0, 5.0), at(6.0, 1.0)],
                stroke,
            ));
            painter.line_segment([at(0.0, 7.0), at(0.0, HALF)], Stroke::new(3.2_f32, color));
        }
        Icon::Looper => {
            // Twelve dots on a ring, the first eight bigger (a loop part way round), and a dot
            // in the middle: the plate's ring and its footswitch
            for dot in 0..12 {
                let angle = dot as f32 * std::f32::consts::TAU / 12.0;
                let radius = if dot < 8 { 2.4 } else { 1.6 };
                painter.circle_filled(at(angle.cos() * 12.5, angle.sin() * 12.5), radius, color);
            }
            painter.circle_filled(center, 4.5, color);
        }
        Icon::Settings => {
            painter.circle_stroke(center, 5.5, stroke);
            for ray in 0..8 {
                let angle = ray as f32 * std::f32::consts::TAU / 8.0;
                let (sin, cos) = angle.sin_cos();
                painter
                    .line_segment([at(cos * 9.0, sin * 9.0), at(cos * HALF, sin * HALF)], stroke);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_the_lamp_and_the_switch_stand_clear_of_the_icon_and_of_each_other() {
        // The icon's drawing is inside its target, the lamp after it, the switch after the lamp
        const {
            assert!(HALF + 2.0 < ICON_SIZE / 2.0);
            assert!(ICON_SIZE / 2.0 < LAMP_DX - LAMP_RADIUS);
            assert!(LAMP_DX + LAMP_RADIUS + 4.0 < SWITCH_DX - 13.0);
            assert!(SWITCH_REACH >= SWITCH_DX + 13.0);
        }
    }
}
