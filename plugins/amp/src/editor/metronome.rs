use super::*;

// The metronome's plate: the tempo display (drag or scroll it, or tap the Tap button), the sound
// over the time signature and the click's output, each on a stepper, the click's volume, and the
// Sync button. Every one is placed from the end of the one before it, from the plate's top left
// corner
pub(super) const METRONOME_PAD: f32 = 26.0;
pub(super) const METRONOME_GAP: f32 = 38.0;
pub(super) const SAVE_REACH: f32 = 25.0;
pub(super) const STEPPER_REACH: f32 = 33.0;
pub(super) const TEMPO_CHARS: usize = 3;
pub(super) const TEMPO_WIDTH: f32 = led_width(TEMPO_CHARS);
pub(super) const TEMPO_X: f32 = METRONOME_PAD + TEMPO_WIDTH / 2.0 + STEPPER_REACH;
pub(super) const TAP_X: f32 =
    TEMPO_X + TEMPO_WIDTH / 2.0 + STEPPER_REACH + METRONOME_GAP + SAVE_REACH;
// The sound over the time signature, on one column of steppers
pub(super) const SOUND_WIDTH: f32 = 100.0;
pub(super) const SIGNATURE_WIDTH: f32 = 54.0;
// and under the sound, side by side, the time signature and where the click is heard
pub(super) const OUT_WIDTH: f32 = 70.0;
pub(super) const LOWER_GAP: f32 = 40.0;
pub(super) const METER_STEP_REACH: f32 = SIGNATURE_WIDTH / 2.0 + STEPPER_REACH;
pub(super) const OUT_REACH: f32 = OUT_WIDTH / 2.0 + STEPPER_REACH;
// The column is as wide as its lower row, or as the sound's stepper if that is wider
pub(super) const COLUMN_WIDTH: f32 = {
    let lower = 2.0 * METER_STEP_REACH + LOWER_GAP + 2.0 * OUT_REACH;
    let upper = SOUND_WIDTH + 2.0 * STEPPER_REACH;
    if lower > upper {
        lower
    } else {
        upper
    }
};
pub(super) const STEPPERS_X: f32 = TAP_X + SAVE_REACH + METRONOME_GAP + COLUMN_WIDTH / 2.0;
pub(super) const SIGNATURE_X: f32 = STEPPERS_X - COLUMN_WIDTH / 2.0 + METER_STEP_REACH;
pub(super) const CLICK_OUT_X: f32 = STEPPERS_X + COLUMN_WIDTH / 2.0 - OUT_REACH;
// Rows, from the plate's top: the title is on the first, then the captions over each control
pub(super) const TEMPO_CAPTION_Y: f32 = 52.0;
pub(super) const TEMPO_Y: f32 = 86.0;
pub(super) const SOUND_CAPTION_Y: f32 = 52.0;
pub(super) const SOUND_Y: f32 = 71.0;
pub(super) const SIGNATURE_CAPTION_Y: f32 = 99.0;
pub(super) const SIGNATURE_Y: f32 = 123.0;
pub(super) const CLICK_RADIUS: f32 = 20.0;
pub(super) const CLICK_REACH: f32 = CLICK_RADIUS + 13.0;
pub(super) const CLICK_VOLUME: Vec2 =
    vec2(STEPPERS_X + COLUMN_WIDTH / 2.0 + METRONOME_GAP + CLICK_REACH, 71.0);
pub(super) const METRONOME_SYNC_X: f32 = CLICK_VOLUME.x + CLICK_REACH + METRONOME_GAP + SAVE_REACH;
pub(super) const METRONOME_END: f32 = METRONOME_SYNC_X + SAVE_REACH;
// The Sync button stands lowest, under its LED and caption
pub(super) const SYNC_STACK: [f32; 3] = [TEMPO_CAPTION_Y, 78.0, 110.0];
pub(super) const METRONOME_WIDTH: f32 = METRONOME_END + METRONOME_PAD;
pub(super) const METRONOME_HEIGHT: f32 = 162.0;
// How long the lamp stays lit on a beat, and how often the window is drawn anew to see it
pub(super) const BEAT_LIT_S: f64 = 0.09;
pub(super) const BEAT_POLL_S: f64 = 0.02;
// A turn of the tempo display: this many points of drag are one beat per minute
pub(super) const TEMPO_DRAG_POINTS: f32 = 3.0;

/// What the metronome's lamp shows: dim amber while it runs, amber on a beat, cream on the first
/// of the bar, lit for a moment. The window asks to be drawn again for as long as the metronome
/// runs
pub(super) fn beat_shown(
    ui: &Ui,
    params: &GuitarAmpParams,
    view: &mut View,
    shared: &Shared,
    now: f64,
) -> Beat {
    let running = params.metronome_on.value() && !params.tuner_on.value();
    let (count, downbeat) = shared.beats.read();
    if count != view.beat.seen {
        view.beat.seen = count;
        view.beat.lit_until = now + BEAT_LIT_S;
        view.beat.downbeat = downbeat;
    }
    let lit = running && now < view.beat.lit_until;
    if running {
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(BEAT_POLL_S));
    }
    match (running, lit, view.beat.downbeat) {
        (false, ..) => Beat::Dark,
        (true, false, _) => Beat::Armed,
        (true, true, false) => Beat::Regular,
        (true, true, true) => Beat::Downbeat,
    }
}

/// The tempo as a display on `centre` to drag up and down or to scroll, with a button at each
/// end: the metronome's own plate and the looper's both carry it, and both set `metronome_bpm`.
/// `id` keeps the two apart
pub(super) fn tempo_display(
    ui: &mut Ui,
    setter: &ParamSetter,
    params: &GuitarAmpParams,
    centre: Pos2,
    id: &str,
) {
    let bpm = params.metronome_bpm.value();
    let window = Rect::from_center_size(centre, vec2(TEMPO_WIDTH, LED_HEIGHT));
    let step = led_stepper(ui, centre, TEMPO_CHARS, &format!("{}", bpm.round() as i32), id);
    let turn = ui.interact(window, Id::new((id, "turn")), Sense::click_and_drag());
    let mut wanted = None;
    if let Some(Pick::Step(step)) = step {
        let by = if matches!(step, Step::Next) { 1.0 } else { -1.0 };
        wanted = Some((bpm.round() + by).clamp(BPM_MIN, BPM_MAX));
    }
    if turn.drag_started() {
        setter.begin_set_parameter(&params.metronome_bpm);
        ui.data_mut(|data| data.insert_temp(turn.id, bpm));
    }
    if turn.dragged() {
        let fine = ui.input(|input| input.modifiers.shift);
        let points = if fine { TEMPO_DRAG_POINTS * 4.0 } else { TEMPO_DRAG_POINTS };
        let held = ui.data_mut(|data| {
            let held = data.get_temp_mut_or(turn.id, bpm);
            *held = (*held - turn.drag_delta().y / points).clamp(BPM_MIN, BPM_MAX);
            *held
        });
        setter.set_parameter(&params.metronome_bpm, held.round());
    }
    if turn.drag_stopped() {
        setter.end_set_parameter(&params.metronome_bpm);
    }
    if turn.hovered() {
        let scrolled = ui.input(|input| input.raw_scroll_delta.y);
        if scrolled != 0.0 {
            wanted = Some((bpm.round() + scrolled.signum()).clamp(BPM_MIN, BPM_MAX));
        }
    }
    if turn.double_clicked() {
        wanted = Some(crate::metronome::BPM_DEFAULT);
    }
    if let Some(value) = wanted {
        set(setter, &params.metronome_bpm, value);
    }
}

/// The metronome's plate under its icon, while it is the plate that is open: the tempo, the tap,
/// the sound, the time signature, where the click is heard, its volume and Sync
#[allow(clippy::too_many_arguments)]
pub(super) fn metronome_plate(
    ui: &mut Ui,
    setter: &ParamSetter,
    params: &GuitarAmpParams,
    origin: Pos2,
    view: &mut View,
    shared: &Shared,
    now: f64,
    anchor: Rect,
) {
    if !view.is_open(Plates::Metronome) {
        return;
    }
    let (has_own_output, host) = (shared.audio_host.is_none(), shared.audio_host);
    let rect = Rect::from_min_size(
        origin + vec2(METRONOME_X - ui::icons::ICON_REACH, PLATE_TOP),
        vec2(METRONOME_WIDTH, METRONOME_HEIGHT),
    );
    let close = ui::plate(ui, rect, "metronome", anchor, |ui, rect| {
        ui::plate::plate_title(ui.painter(), rect, "Metronome");
        let at = |x: f32, y: f32| rect.min + vec2(x, y);

        // The tempo: a display to drag up and down or to scroll, with a button at each end
        silk_label(ui.painter(), at(TEMPO_X, TEMPO_CAPTION_Y), "Tempo", 13.0);
        tempo_display(ui, setter, params, at(TEMPO_X, TEMPO_Y), "tempo");

        // Tap: the tempo of the last few taps
        silk_label(ui.painter(), at(TAP_X, TEMPO_CAPTION_Y), "Tap", 13.0);
        if chrome_button(ui, at(TAP_X, TEMPO_Y), "metronome-tap").clicked() {
            if let Some(tapped) = view.beat.taps.tap(now) {
                set(setter, &params.metronome_bpm, tapped.round());
            }
        }

        // The time signature and the sound
        let signature = params.metronome_time_sig.value();
        silk_label(ui.painter(), at(SIGNATURE_X, SIGNATURE_CAPTION_Y), "Meter", 13.0);
        if let Some(step) = stepper(
            ui,
            at(SIGNATURE_X, SIGNATURE_Y),
            SIGNATURE_WIDTH,
            signature.name(),
            "metronome-meter",
        ) {
            set(
                setter,
                &params.metronome_time_sig,
                neighbour(&TimeSignature::ALL, signature, step),
            );
        }
        let sound = params.metronome_sound.value();
        silk_label(ui.painter(), at(STEPPERS_X, SOUND_CAPTION_Y), "Sound", 13.0);
        if let Some(step) =
            stepper(ui, at(STEPPERS_X, SOUND_Y), SOUND_WIDTH, sound.name(), "metronome-sound")
        {
            set(setter, &params.metronome_sound, neighbour(&Sound::ALL, sound, step));
        }

        // Where the click is heard: with the amp, or alone on the "Metronome" output. The
        // standalone program has no such output: the click is always with the amp there, so the
        // stepper is left out
        if has_own_output {
            let out = params.metronome_out.value();
            silk_label(ui.painter(), at(CLICK_OUT_X, SIGNATURE_CAPTION_Y), "Click out", 13.0);
            if let Some(step) =
                stepper(ui, at(CLICK_OUT_X, SIGNATURE_Y), OUT_WIDTH, out.name(), "metronome-out")
            {
                set(setter, &params.metronome_out, neighbour(&ClickOut::ALL, out, step));
            }
        }

        // The click's volume
        param_knob(
            ui,
            setter,
            &params.metronome_volume,
            rect.min + CLICK_VOLUME,
            CLICK_RADIUS,
            "Volume",
            None,
        );

        // Sync: follow the host's tempo and transport
        let sync = params.metronome_sync.value();
        let [caption, led, button] = SYNC_STACK;
        silk_label(ui.painter(), at(METRONOME_SYNC_X, caption), "Sync", 13.0);
        rig_led(ui.painter(), at(METRONOME_SYNC_X, led), sync);
        if chrome_button(ui, at(METRONOME_SYNC_X, button), "metronome-sync").clicked() {
            set(setter, &params.metronome_sync, !sync);
        }
    });
    if close.asked() {
        view.close_plate(host);
    }
}
