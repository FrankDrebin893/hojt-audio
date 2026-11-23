use super::*;
use nih_plug_egui::egui::{Align2, FontId, Response, Stroke};

// The looper's plate (#280, the Ring picked on the issue): the footswitch stands in a ring of
// lamps that is the loop (it fills red while recording, then a head goes round), the state printed
// in the ring over the switch and what a press does next under it; Stop (it reads Mute while the
// loop plays, Resume while it is muted) and Clear in a column on the left, the Level knob on the
// right. Every place is from the plate's top left corner
pub(super) const LOOPER_WIDTH: f32 = 500.0;
pub(super) const LOOPER_HEIGHT: f32 = 264.0;
pub(super) const LOOPER_RING: Vec2 = vec2(250.0, 116.0);
pub(super) const LOOPER_RING_RADIUS: f32 = 70.0;
pub(super) const LOOPER_STATE_DY: f32 = -50.0;
pub(super) const LOOPER_NEXT_DY: f32 = 54.0;
pub(super) const LOOPER_BUTTONS_X: f32 = 70.0;
// Caption, button, for Stop and then Clear
pub(super) const LOOPER_STOP: [f32; 2] = [62.0, 86.0];
pub(super) const LOOPER_CLEAR: [f32; 2] = [108.0, 132.0];
pub(super) const LOOPER_LEVEL: Vec2 = vec2(430.0, 76.0);
pub(super) const LOOPER_LEVEL_RADIUS: f32 = 26.0;
// The click's row under the ring (#450): the metronome's own switch with its lamp, its tempo and
// the Click stepper (Always, Rec only). A caption over each
pub(super) const LOOPER_ROW_CAPTION_Y: f32 = 216.0;
pub(super) const LOOPER_ROW_Y: f32 = 240.0;
pub(super) const LOOPER_METRO_LED_X: f32 = 30.0;
pub(super) const LOOPER_METRO_X: f32 = 60.0;
pub(super) const LOOPER_TEMPO_X: f32 = 158.0;
pub(super) const LOOPER_CLICK_X: f32 = 340.0;
pub(super) const LOOPER_CLICK_WIDTH: f32 = 74.0;
// Bars under the Level knob and the Count-in under Clear (#451), a caption over each: the length
// of a loop and what is played before it, either side of the ring
pub(super) const LOOPER_BARS_X: f32 = 430.0;
pub(super) const LOOPER_BARS_WIDTH: f32 = 50.0;
pub(super) const LOOPER_COUNT_X: f32 = 70.0;
pub(super) const LOOPER_COUNT_WIDTH: f32 = 50.0;
pub(super) const LOOPER_SIDE_CAPTION_Y: f32 = 170.0;
pub(super) const LOOPER_SIDE_Y: f32 = 192.0;
// The tabs of the three tracks (#452), on the title's line at the left: the plate has no room to
// grow, and the ring and the click row fill the rest. Centres from the plate's left, the radius of
// a tab and of the brass ring round the selected one
pub(super) const LOOPER_TABS: [f32; 3] = [36.0, 66.0, 96.0];
pub(super) const LOOPER_TAB_Y: f32 = 19.0;
pub(super) const LOOPER_TAB_RADIUS: f32 = 8.5;
// What a dimmed Bars or Click says when it is hovered
pub(super) const LOOPER_NEEDS_CLICK: &str =
    "Needs the click: switch it on with Metro (a set number of bars, and a count-in, count its bars)";

/// Something on the plate changes by itself: a track records, plays or overdubs (also while track
/// 1 is muted and another goes on), one waits for the loop to come round, or a count-in counts
pub(super) fn looper_moves(control: &LooperControl) -> bool {
    (0..crate::looper::TRACKS).any(|track| {
        matches!(
            control.track_state(track),
            LoopState::Recording | LoopState::Playing | LoopState::Overdubbing
        ) || control.track_waiting(track)
    }) || control.count_in().is_some()
}

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
/// while recording, against what it fills against (`LooperControl::fills_against`: the set length
/// of a number of bars, so it is a full circle when the loop is complete, else the buffer), then a
/// head goes round the loop, green while it plays and amber while it overdubs; dark when empty or
/// muted. While a count-in plays (`count`: its beat and the beats of a bar) it is the beats lit in
/// amber, round the circle once a bar
pub(super) fn loop_ring(
    state: LoopState,
    length: u32,
    position: u32,
    capacity: u32,
    count: Option<(u32, u32)>,
) -> ui::Ring {
    let share = |part: u32, whole: u32| if whole == 0 { 0.0 } else { part as f32 / whole as f32 };
    match state {
        // A count-in: the bar's beats light round the circle in amber, a quarter to a beat of four
        LoopState::Empty if count.is_some() => {
            let (beat, beats) = count.unwrap_or_default();
            ui::Ring::Filled { fraction: share(beat, beats), color: ui::RING_AMBER }
        }
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

/// What the ring shows for `track` (0 to 2): track 1 as `loop_ring` does. Track 2 or 3 fills red
/// as the loop goes round while it records its pass, and while it waits for the loop to start
/// again a red head goes round; otherwise it is the loop's own ring, dark while the track is empty
pub(super) fn track_ring(
    track: usize,
    state: LoopState,
    waiting: bool,
    length: u32,
    position: u32,
    count: Option<(u32, u32)>,
    fills: u32,
) -> ui::Ring {
    let share = |part: u32, whole: u32| if whole == 0 { 0.0 } else { part as f32 / whole as f32 };
    match (track, state) {
        (0, _) => loop_ring(state, length, position, fills, count),
        (_, LoopState::Empty) if waiting => {
            ui::Ring::Head { fraction: share(position, length), color: ui::RING_RED }
        }
        (_, LoopState::Recording) => {
            ui::Ring::Filled { fraction: share(position, length), color: ui::RING_RED }
        }
        (_, state) => loop_ring(state, length, position, fills, None),
    }
}

/// What the state of `track` is called in the ring, and what the big button does next. A track 2
/// or 3 needs track 1's loop to be in time with: until there is one it says so
pub(super) fn track_words(
    track: usize,
    state: LoopState,
    waiting: bool,
    master: LoopState,
    count: Option<(u32, u32)>,
) -> (&'static str, &'static str) {
    if track == 0 {
        return loop_words(state, count);
    }
    match state {
        LoopState::Empty if waiting => ("Waiting", "Cancel"),
        LoopState::Empty if master == LoopState::Empty || master == LoopState::Recording => {
            ("Empty", "Record 1 first")
        }
        LoopState::Empty => ("Empty", "Record"),
        state => loop_words(state, None),
    }
}

/// The colour of a track's tab: its lamp, as the strip's icon has one
pub(super) fn tab_lamp(state: LoopState, waiting: bool) -> Color32 {
    match state {
        LoopState::Empty if waiting => ui::RING_RED.gamma_multiply(0.55),
        LoopState::Empty | LoopState::Stopped => Color32::from_rgb(40, 34, 30),
        LoopState::Recording => ui::RING_RED,
        LoopState::Playing => ui::RING_GREEN,
        LoopState::Overdubbing => ui::RING_AMBER,
    }
}

/// One tab: a round lamp in the track's colour with its number on it, outlined in brass while it
/// is the selected one
fn track_tab(
    ui: &mut Ui,
    centre: Pos2,
    number: usize,
    lamp: Color32,
    selected: bool,
    id: &str,
) -> Response {
    let reach = LOOPER_TAB_RADIUS + 3.0;
    let response = ui.interact(
        Rect::from_center_size(centre, Vec2::splat(2.0 * reach)),
        Id::new(id),
        Sense::click(),
    );
    let painter = ui.painter();
    if selected {
        painter.circle_stroke(centre, LOOPER_TAB_RADIUS + 2.5, Stroke::new(1.5_f32, ui::BRASS));
    }
    painter.circle_filled(centre, LOOPER_TAB_RADIUS, lamp);
    painter.circle_stroke(centre, LOOPER_TAB_RADIUS, Stroke::new(1.0_f32, Color32::BLACK));
    let lit = lamp.r() as u32 + lamp.g() as u32 > 200;
    painter.text(
        centre,
        Align2::CENTER_CENTER,
        number.to_string(),
        FontId::proportional(11.0),
        if lit { theme::INK } else { theme::SILK },
    );
    response
}

/// What the state is called in the ring, and what the big button does next; while a count-in plays
/// (`count`: its beat, zero before it begins, and the beats of a bar) it counts the beats
pub(super) fn loop_words(
    state: LoopState,
    count: Option<(u32, u32)>,
) -> (&'static str, &'static str) {
    const BEATS: [&str; 9] = [
        "Count in", "Count 1", "Count 2", "Count 3", "Count 4", "Count 5", "Count 6", "Count 7",
        "Count 8",
    ];
    match state {
        LoopState::Empty if count.is_some() => {
            let (beat, _) = count.unwrap_or_default();
            (BEATS[(beat as usize).min(BEATS.len() - 1)], "Cancel")
        }
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
    let track = control.selected();
    let master = control.state();
    let state = control.track_state(track);
    let waiting = control.track_waiting(track);
    if looper_moves(control) {
        // The head goes round, the fill grows and the beats of a count-in light: look again 30
        // times a second
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
    let close = ui::plate(ui, rect, "looper", anchor, |ui, rect| {
        ui::plate::plate_title(ui.painter(), rect, "Looper");
        let at = |x: f32, y: f32| rect.min + vec2(x, y);
        let centre = rect.min + LOOPER_RING;
        let count = control.count_in();
        // Track 1 fills against what it fills against; a track 2 or 3 against the loop it is a pass of
        let fills = if track == 0 { control.fills_against() } else { control.length() };
        let ring =
            track_ring(track, state, waiting, control.length(), control.position(), count, fills);
        ui::lamp_ring(ui.painter(), centre, LOOPER_RING_RADIUS, ring);
        if ui::footswitch(ui, centre, "looper-press").clicked() {
            control.press_track(track);
        }
        let (named, next) = track_words(track, state, waiting, master, count);
        for (index, x) in LOOPER_TABS.into_iter().enumerate() {
            let lamp = tab_lamp(control.track_state(index), control.track_waiting(index));
            let id = format!("looper-track-{}", index + 1);
            if track_tab(ui, at(x, LOOPER_TAB_Y), index + 1, lamp, index == track, &id).clicked() {
                control.select(index);
            }
        }
        silk_label(ui.painter(), centre + vec2(0.0, LOOPER_STATE_DY), named, 12.0);
        silk_label(ui.painter(), centre + vec2(0.0, LOOPER_NEXT_DY), next, 12.0);

        silk_label(ui.painter(), at(LOOPER_BUTTONS_X, LOOPER_STOP[0]), stop_word(state), 13.0);
        if chrome_button(ui, at(LOOPER_BUTTONS_X, LOOPER_STOP[1]), "looper-stop").clicked() {
            control.stop_track(track);
        }
        silk_label(ui.painter(), at(LOOPER_BUTTONS_X, LOOPER_CLEAR[0]), "Clear", 13.0);
        if chrome_button(ui, at(LOOPER_BUTTONS_X, LOOPER_CLEAR[1]), "looper-clear").clicked() {
            control.clear_track(track);
        }
        param_knob(
            ui,
            setter,
            [&params.looper_level, &params.looper_level_2, &params.looper_level_3][track],
            rect.min + LOOPER_LEVEL,
            LOOPER_LEVEL_RADIUS,
            "Level",
            None,
        );

        // The click's row: the metronome's own parameters, so this plate and the metronome's agree
        let click_on = params.metronome_on.value();
        let [metro, tempo, click] =
            [LOOPER_METRO_X, LOOPER_TEMPO_X, LOOPER_CLICK_X].map(|x| at(x, LOOPER_ROW_Y));
        let caption = |x: f32| at(x, LOOPER_ROW_CAPTION_Y);
        silk_label(ui.painter(), caption(LOOPER_METRO_X), "Metro", 13.0);
        rig_led(ui.painter(), at(LOOPER_METRO_LED_X, LOOPER_ROW_Y), click_on);
        if chrome_button(ui, metro, "looper-metro").clicked() {
            set(setter, &params.metronome_on, !click_on);
        }
        silk_label(ui.painter(), caption(LOOPER_TEMPO_X), "Tempo", 13.0);
        tempo_display(ui, setter, params, tempo, "looper-tempo");

        // Click, Bars and Count-in count on the click: with it off they are dimmed, and Bars and
        // Count-in read Free and Off
        silk_label(ui.painter(), caption(LOOPER_CLICK_X), "Click", 13.0);
        let side = |x: f32| at(x, LOOPER_SIDE_Y);
        silk_label(ui.painter(), at(LOOPER_BARS_X, LOOPER_SIDE_CAPTION_Y), "Bars", 13.0);
        silk_label(ui.painter(), at(LOOPER_COUNT_X, LOOPER_SIDE_CAPTION_Y), "Count-in", 13.0);
        let shown_bars = if click_on { params.looper_bars.value() } else { LoopBars::Free };
        let shown_count = if click_on { params.looper_count_in.value() } else { CountIn::Off };
        let way = params.looper_click.value();
        ui.add_enabled_ui(click_on, |ui| {
            if let Some(step) =
                stepper(ui, click, LOOPER_CLICK_WIDTH, way.name(), "looper-click-way")
            {
                set(setter, &params.looper_click, neighbour(&LoopClick::ALL, way, step));
            }
            if let Some(step) = stepper(
                ui,
                side(LOOPER_BARS_X),
                LOOPER_BARS_WIDTH,
                shown_bars.name(),
                "looper-bars",
            ) {
                set(setter, &params.looper_bars, neighbour(&LoopBars::ALL, shown_bars, step));
            }
            if let Some(step) = stepper(
                ui,
                side(LOOPER_COUNT_X),
                LOOPER_COUNT_WIDTH,
                shown_count.name(),
                "looper-count-in",
            ) {
                set(setter, &params.looper_count_in, neighbour(&CountIn::ALL, shown_count, step));
            }
        });
        if !click_on {
            for (centre, width, id) in [
                (click, LOOPER_CLICK_WIDTH, "looper-click-way-hint"),
                (side(LOOPER_BARS_X), LOOPER_BARS_WIDTH, "looper-bars-hint"),
                (side(LOOPER_COUNT_X), LOOPER_COUNT_WIDTH, "looper-count-in-hint"),
            ] {
                let reach = width / 2.0 + STEPPER_REACH;
                let area = Rect::from_center_size(centre, vec2(2.0 * reach, LED_HEIGHT));
                ui.interact(area, Id::new(id), Sense::hover()).on_hover_text(LOOPER_NEEDS_CLICK);
            }
        }
    });
    if close.asked() {
        view.close_plate(host);
    }
}
