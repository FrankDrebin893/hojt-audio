//! MIDI learn's hook on the controls (#479): which knob or switch the player clicked to bind to a
//! controller, which one to forget, and the ring that shows what is waiting for a controller.
//!
//! The editor tells the controls what it wants once a frame with [`begin`], through the egui
//! context; every knob and `param_switch` then reports a click or a right-click on itself and
//! draws its ring, and the editor reads what was asked with [`finish`] at the end of the frame.
//! A window that never calls `begin` (every other plugin) draws and answers as it always did.
//! A control is told apart by [`address`], the place of its parameter.

use std::sync::Arc;

use nih_plug::prelude::ParamPtr;
use nih_plug_egui::egui::{Context, Id, Pos2, Response, Stroke, Ui};

use super::ring::RING_AMBER;

/// How far outside a control's cap its ring stands, and how thick it is
const RING_GAP: f32 = 4.0;
const ARMED_WIDTH: f32 = 3.0;
const BOUND_WIDTH: f32 = 1.5;

/// What the editor asks of the controls for one frame
#[derive(Clone, Default)]
struct Tap {
    /// Learn is on: a click on a control picks it
    on: bool,
    /// The control that waits for a controller
    armed: Option<usize>,
    /// The controls that have a controller
    bound: Arc<Vec<usize>>,
    /// The control clicked this frame, while learn is on
    clicked: Option<usize>,
    /// The bound control that was right-clicked this frame
    forgotten: Option<usize>,
}

/// What the controls were asked this frame
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asked {
    /// A control was clicked while learn was on
    pub clicked: Option<usize>,
    /// A bound control was right-clicked: forget its controller
    pub forgotten: Option<usize>,
}

fn tap_id() -> Id {
    Id::new("midi-learn-tap")
}

/// The place of a parameter, which tells its control from the others
pub fn address(param: ParamPtr) -> usize {
    match param {
        ParamPtr::FloatParam(ptr) => ptr as usize,
        ParamPtr::IntParam(ptr) => ptr as usize,
        ParamPtr::BoolParam(ptr) => ptr as usize,
        ParamPtr::EnumParam(ptr) => ptr as usize,
    }
}

/// Starts a frame of learn: `on` while a click should pick a control, `armed` the control that
/// waits for a controller, `bound` the controls that have one
pub fn begin(ctx: &Context, on: bool, armed: Option<usize>, bound: Arc<Vec<usize>>) {
    ctx.data_mut(|data| {
        data.insert_temp(tap_id(), Tap { on, armed, bound, clicked: None, forgotten: None });
    });
}

/// Ends the frame: what the controls were asked
pub fn finish(ctx: &Context) -> Asked {
    ctx.data_mut(|data| {
        let tap = data.get_temp_mut_or_default::<Tap>(tap_id());
        Asked { clicked: tap.clicked.take(), forgotten: tap.forgotten.take() }
    })
}

/// A control with `key` (its `address`) answered with `response`: reports a click or a right-click,
/// and rings it, `radius` round `center`, while it waits for a controller (bright) or, with learn
/// on, has one (dim)
pub(super) fn touch(ui: &Ui, key: usize, response: &Response, center: Pos2, radius: f32) {
    let ctx = ui.ctx();
    let Some(tap) = ctx.data(|data| data.get_temp::<Tap>(tap_id())) else {
        return;
    };
    let bound = tap.bound.contains(&key);
    let clicked = tap.on && response.clicked();
    let forgotten = bound && response.secondary_clicked();
    if clicked || forgotten {
        ctx.data_mut(|data| {
            let tap = data.get_temp_mut_or_default::<Tap>(tap_id());
            if clicked {
                tap.clicked = Some(key);
            }
            if forgotten {
                tap.forgotten = Some(key);
            }
        });
        ctx.request_repaint();
    }
    let ring = if tap.armed == Some(key) {
        Stroke::new(ARMED_WIDTH, RING_AMBER)
    } else if tap.on && bound {
        Stroke::new(BOUND_WIDTH, RING_AMBER.gamma_multiply(0.55))
    } else {
        return;
    };
    ui.painter().circle_stroke(center, radius + RING_GAP, ring);
}

#[cfg(test)]
mod tests {
    use super::*;
    use nih_plug::prelude::{FloatParam, FloatRange, Param};

    #[test]
    fn test_a_parameter_has_its_own_address() {
        let a = FloatParam::new("A", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 });
        let b = FloatParam::new("B", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 });
        assert_eq!(address(a.as_ptr()), address(a.as_ptr()));
        assert_ne!(address(a.as_ptr()), address(b.as_ptr()));
    }

    #[test]
    fn test_nothing_is_asked_of_a_window_that_never_began() {
        let ctx = Context::default();
        assert_eq!(finish(&ctx), Asked::default());
    }
}
