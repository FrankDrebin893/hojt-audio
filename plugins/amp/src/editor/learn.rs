use super::*;
use crate::midi_map::{Learn, Outcome};
use nih_plug_egui::egui::Key;

// MIDI learn on the window (#479): the controllers that came in move the knobs bound to them, and
// the Learn button of the settings plate, a click on a knob and a right-click on a bound one
// make the bindings (`crate::midi_map`; the controls report their clicks through `ui::learn`)

/// What the display says while learn waits for a knob, and while a knob waits for a controller
const LEARN_CLICK: &str = "LEARN: CLICK A KNOB";
const LEARN_MOVE: &str = "MOVE THE PEDAL";

/// Takes the control changes that came in since the last frame: binds the first to the knob learn
/// picked, moves the knobs the others are bound to
pub(super) fn follow_controllers(
    ctx: &Context,
    setter: &ParamSetter,
    params: &GuitarAmpParams,
    view: &mut View,
    shared: &Shared,
    now: f64,
) {
    let channel = standalone::launch().midi_channel;
    let mut seen = Vec::new();
    shared.midi.drain_controllers(channel, |cc, value| seen.push((cc, value)));
    for (cc, value) in seen {
        if view.midi_map.control(params, setter, cc, value) == Outcome::Learned {
            kept(ctx, view, shared, "LEARNED", now);
        }
    }
}

/// Tells the knobs and switches what learn wants of them this frame
pub(super) fn begin(ctx: &Context, params: &GuitarAmpParams, view: &View) {
    ui::learn::begin(
        ctx,
        view.midi_map.is_learning(),
        view.midi_map.armed(params),
        view.midi_map.bound(),
    );
}

/// The end of the frame: what a knob was clicked or right-clicked, the Escape key, and what the
/// display says of learn
pub(super) fn finish(
    ctx: &Context,
    params: &GuitarAmpParams,
    view: &mut View,
    shared: &Shared,
    now: f64,
) {
    let asked = ui::learn::finish(ctx);
    if let Some(place) = asked.clicked {
        if view.midi_map.pick(params, place) {
            ctx.request_repaint();
        }
    }
    if let Some(place) = asked.forgotten {
        if view.midi_map.forget(params, place) {
            kept(ctx, view, shared, "FORGOTTEN", now);
        }
    }
    // Escape leaves learn where no plate is open to take it
    if view.midi_map.is_learning()
        && view.plate.is_none()
        && ctx.input(|input| input.key_pressed(Key::Escape))
    {
        view.midi_map.learn = Learn::Off;
    }
    say(ctx, view);
}

/// The Learn button of the settings plate: starts learn, or ends it. The plate closes so the knobs
/// can be reached
pub(super) fn press(ctx: &Context, view: &mut View) {
    view.midi_map.toggle_learn();
    say(ctx, view);
}

/// The bindings changed: kept with the standalone's settings, and the display says `note` for a moment
fn kept(ctx: &Context, view: &mut View, shared: &Shared, note: &'static str, now: f64) {
    if let Some(host) = shared.audio_host {
        host.save_bindings(&view.midi_map.bindings);
    }
    view.note = Some((note, now + SAVE_NOTE_S));
    ctx.request_repaint();
}

/// While learn is on the display says what it waits for, until it ends
fn say(ctx: &Context, view: &mut View) {
    let waiting = match view.midi_map.learn {
        Learn::Off => None,
        Learn::Waiting => Some(LEARN_CLICK),
        Learn::Armed(_) => Some(LEARN_MOVE),
    };
    match (waiting, view.note) {
        (Some(text), shown) if shown.map(|(note, _)| note) != Some(text) => {
            view.note = Some((text, f64::INFINITY));
            ctx.request_repaint();
        }
        (None, Some((_, until))) if until == f64::INFINITY => {
            view.note = None;
            ctx.request_repaint();
        }
        _ => {}
    }
}
