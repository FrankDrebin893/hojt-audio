use super::*;

// The looper's plate (#280, the Ring picked on the issue): the footswitch stands in a ring of
// lamps that is the loop (it fills red while recording, then a head goes round), the state printed
// in the ring over the switch and what a press does next under it; Stop (it reads Mute while the
// loop plays, Resume while it is muted) and Clear in a column on the left, the Level knob on the
// right. Every place is from the plate's top left corner
pub(super) const LOOPER_WIDTH: f32 = 500.0;
pub(super) const LOOPER_HEIGHT: f32 = 214.0;
pub(super) const LOOPER_RING: Vec2 = vec2(250.0, 116.0);
pub(super) const LOOPER_RING_RADIUS: f32 = 70.0;
pub(super) const LOOPER_STATE_DY: f32 = -50.0;
pub(super) const LOOPER_NEXT_DY: f32 = 54.0;
pub(super) const LOOPER_BUTTONS_X: f32 = 70.0;
// Caption, button, for Stop and then Clear
pub(super) const LOOPER_STOP: [f32; 2] = [78.0, 106.0];
pub(super) const LOOPER_CLEAR: [f32; 2] = [140.0, 168.0];
pub(super) const LOOPER_LEVEL: Vec2 = vec2(430.0, 118.0);
pub(super) const LOOPER_LEVEL_RADIUS: f32 = 26.0;

/// What the lamp beside the looper's icon shows
pub(super) fn loop_lamp(state: LoopState) -> Beat {
    match state {
        LoopState::Empty | LoopState::Stopped => Beat::Dark,
        LoopState::Recording => Beat::Downbeat,
        LoopState::Playing => Beat::Armed,
        LoopState::Overdubbing => Beat::Regular,
    }
}

/// What the ring of the looper's plate shows, from what the looper says of itself: it fills red
/// against the buffer while recording, then a head goes round the loop, green while it plays and
/// amber while it overdubs; dark when empty or muted
pub(super) fn loop_ring(state: LoopState, length: u32, position: u32, capacity: u32) -> ui::Ring {
    let share = |part: u32, whole: u32| if whole == 0 { 0.0 } else { part as f32 / whole as f32 };
    match state {
        LoopState::Empty | LoopState::Stopped => ui::Ring::Dark,
        LoopState::Recording => {
            ui::Ring::Filled { fraction: share(length, capacity), color: ui::RING_RED }
        }
        LoopState::Playing => {
            ui::Ring::Head { fraction: share(position, length), color: ui::RING_GREEN }
        }
        LoopState::Overdubbing => {
            ui::Ring::Head { fraction: share(position, length), color: ui::RING_AMBER }
        }
    }
}

/// What the state is called in the ring, and what the big button does next
pub(super) fn loop_words(state: LoopState) -> (&'static str, &'static str) {
    match state {
        LoopState::Empty => ("Empty", "Record"),
        LoopState::Recording => ("Recording", "Play"),
        LoopState::Playing => ("Playing", "Overdub"),
        LoopState::Overdubbing => ("Overdub", "Play"),
        LoopState::Stopped => ("Muted", "Play"),
    }
}

/// What the Stop button is called: Mute while the loop sounds, Resume while it is muted
pub(super) fn stop_word(state: LoopState) -> &'static str {
    match state {
        LoopState::Playing | LoopState::Overdubbing => "Mute",
        LoopState::Stopped => "Resume",
        LoopState::Empty | LoopState::Recording => "Stop",
    }
}

/// The looper's plate under its icon, while it is the plate that is open: the footswitch in its
/// ring, Stop and Clear, and the Level knob
pub(super) fn looper_plate(
    ui: &mut Ui,
    setter: &ParamSetter,
    params: &GuitarAmpParams,
    origin: Pos2,
    view: &mut View,
    shared: &Shared,
    anchor: Rect,
) {
    if !view.is_open(Plates::Looper) {
        return;
    }
    let host = shared.audio_host;
    let rect = Rect::from_min_size(
        origin + vec2(LOOPER_X - ui::icons::ICON_REACH, PLATE_TOP),
        vec2(LOOPER_WIDTH, LOOPER_HEIGHT),
    );
    let control = shared.looper;
    let state = control.state();
    if matches!(state, LoopState::Recording | LoopState::Playing | LoopState::Overdubbing) {
        // The head goes round and the fill grows: look again 30 times a second
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
    let close = ui::plate(ui, rect, "looper", anchor, |ui, rect| {
        ui::plate::plate_title(ui.painter(), rect, "Looper");
        let at = |x: f32, y: f32| rect.min + vec2(x, y);
        let centre = rect.min + LOOPER_RING;
        let ring = loop_ring(state, control.length(), control.position(), control.capacity());
        ui::lamp_ring(ui.painter(), centre, LOOPER_RING_RADIUS, ring);
        if ui::footswitch(ui, centre, "looper-press").clicked() {
            control.press();
        }
        let (named, next) = loop_words(state);
        silk_label(ui.painter(), centre + vec2(0.0, LOOPER_STATE_DY), named, 12.0);
        silk_label(ui.painter(), centre + vec2(0.0, LOOPER_NEXT_DY), next, 12.0);

        silk_label(ui.painter(), at(LOOPER_BUTTONS_X, LOOPER_STOP[0]), stop_word(state), 13.0);
        if chrome_button(ui, at(LOOPER_BUTTONS_X, LOOPER_STOP[1]), "looper-stop").clicked() {
            control.stop();
        }
        silk_label(ui.painter(), at(LOOPER_BUTTONS_X, LOOPER_CLEAR[0]), "Clear", 13.0);
        if chrome_button(ui, at(LOOPER_BUTTONS_X, LOOPER_CLEAR[1]), "looper-clear").clicked() {
            control.clear();
        }
        param_knob(
            ui,
            setter,
            &params.looper_level,
            rect.min + LOOPER_LEVEL,
            LOOPER_LEVEL_RADIUS,
            "Level",
            None,
        );
    });
    if close.asked() {
        view.close_plate(host);
    }
}
