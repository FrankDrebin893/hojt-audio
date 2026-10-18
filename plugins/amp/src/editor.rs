use nih_plug::prelude::*;
use nih_plug_egui::egui::{vec2, Color32, Id, Pos2, Rect, Sense, Ui, Vec2};
use nih_plug_egui::{create_egui_editor, EguiState};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};
use suite_common::ui::{
    self, chrome_button, level_meter, param_knob, param_switch, rig_led, signal_lamp, silk_label,
    small_footswitch, stepper, theme, tuner_display, MiniStyle, Motif, Ornament, PedalStyle, Step,
    BENCH_MARGIN, SLIM_KNOBS, SLIM_KNOB_RADIUS, SLIM_LED, SLIM_SWITCH,
};

use crate::cab::MicType;
use crate::cab_stage::{CabLoader, CabStatus, NO_FILES_SHOWN};
use crate::dial_memory;
use crate::lamps::{LampReading, LampView};
use crate::presets::{self, PRESETS};
use crate::tuner::{Note, TunerReading};
use crate::user_cab;
use crate::user_presets::{self, Bank};
use crate::{Amp, GuitarAmpParams, GuitarAmpPlugin, Task};

// The width the head's controls are placed in, and five of the wide pedals
const RIG_WIDTH: u32 = 960;
const WINDOW_WIDTH: u32 = crate::board::WINDOW_WIDTH;
const WINDOW_HEIGHT: u32 = 694;

// The head across the top, the pedalboard in the rest of the window below it
pub(crate) const HEAD_HEIGHT: f32 = 354.0;
// On a head wider than the rig, its controls keep their places among each other and sit
// in the middle
const HEAD_X: f32 = (WINDOW_WIDTH - RIG_WIDTH) as f32 / 2.0;

pub(crate) const STYLE: PedalStyle = PedalStyle {
    name: "Guitar Amp",
    model: "GA-3",
    paint: ui::theme::PAINT_OXBLOOD,
    ornament: Ornament::Bolts,
    jacks: ["Stereo Out", "In"],
    band_y: 236.0,
    art: Some(ui::artwork::AMP),
};

// The preset display, top centre between the jack captions: its caption, and under it the
// tape with a button at each end. Wide enough for the longest name and the edited mark.
// While the tuner is on, its display is there instead, under its own caption
const PRESET_X: f32 = RIG_WIDTH as f32 / 2.0;
const PRESET_CAPTION_Y: f32 = 25.0;
const PRESET_Y: f32 = 47.0;
const PRESET_WIDTH: f32 = 156.0;
// The Save button right of the preset display, its caption on the line of the display's
const SAVE_X: f32 = PRESET_X + 150.0;
// The longest name of a player's preset the display shows, with room for the edited mark
const USER_NAME_CHARS: usize = 13;
// How long the display says what Save did
const SAVE_NOTE_S: f64 = 1.5;
// Room for the note and seventeen lamps
const TUNER_WIDTH: f32 = 286.0;

// The amp's own six dials in an even row, and the output level set apart from them
const KNOB_Y: f32 = 114.0;
const KNOB_RADIUS: f32 = 30.0;
const DIAL_X: f32 = 150.0;
const DIAL_SPACING: f32 = 100.0;
const OUTPUT_X: f32 = 790.0;

// The clip lamp, to the right of the output knob: lit while the output is rounded off by
// the safety clip. A signal lamp has its caption this far under it
const CLIP_LAMP: Vec2 = vec2(872.0, 104.0);
const LAMP_CAPTION: Vec2 = vec2(0.0, 18.0);
const LAMP_CAPTION_SIZE: f32 = 13.0;

// The level meters, one in each wing of the head beside the dials: the input's under the
// jack it comes in at, the output's next to the output knob and the clip lamp. A column of
// lamps on label tape, with its caption under it
const INPUT_METER: Vec2 = vec2(60.0, 126.0);
const OUTPUT_METER: Vec2 = vec2(915.0, 126.0);
const METER_HEIGHT: f32 = 104.0;
const METER_CAPTION: Vec2 = vec2(0.0, 64.0);

// Below the band. Small switches, left of each its LED above its caption: the bypass under
// the output knob, and the tuner beside it on the side away from the amp selector. The amp
// selector is a stepper under the dials, so any number of amps fit. A stepper's caption is
// above its tape, on the line of the LEDs, and its tape on the line of the switch captions
const SWITCH_Y: f32 = 320.0;
const STEPPER_Y: f32 = 335.0;
const AMP_STEPPER_X: f32 = 385.0;
const AMP_STEPPER_WIDTH: f32 = 150.0;
// The microphone of the cabinet, beside the amp selector: three letters on its tape
const MIC_STEPPER_X: f32 = 590.0;
const MIC_STEPPER_WIDTH: f32 = 54.0;
const BYPASS_X: f32 = 790.0;
const TUNER_X: f32 = 910.0;
const CAPTION_OFFSET: f32 = -58.0;
const LED_Y: f32 = 309.0;
const CAPTION_Y: f32 = 335.0;
const CAPTION_SIZE: f32 = 17.0;

// The pedals: five wide ones, as many as fit in the width of the rig, and five slim ones
const WIDE_PEDALS: usize = 5;
const PEDAL_SLOTS: usize = 10;
pub(crate) const PEDAL_TOP: f32 = BENCH_MARGIN + HEAD_HEIGHT + BENCH_MARGIN;
pub(crate) const PEDAL_WIDTH: f32 =
    (RIG_WIDTH as f32 - BENCH_MARGIN) / WIDE_PEDALS as f32 - BENCH_MARGIN;
pub(crate) const PEDAL_HEIGHT: f32 = WINDOW_HEIGHT as f32 - BENCH_MARGIN - PEDAL_TOP;

// On every pedal, from its top centre: the LED under the title, the knobs, the switch at the bottom.
// Two knobs sit side by side. Three do not fit in a row, so they are smaller and in a triangle,
// with the LED between the upper two
const PEDAL_LED: Vec2 = vec2(0.0, 71.0);
const PEDAL_SWITCH: Vec2 = vec2(0.0, 273.0);
const PAIR_RADIUS: f32 = 24.0;
const PAIR_KNOBS: [Vec2; 2] = [vec2(-45.0, 144.0), vec2(45.0, 144.0)];
const TRIO_RADIUS: f32 = 17.0;
const TRIO_KNOBS: [Vec2; 3] = [vec2(-45.0, 71.0), vec2(45.0, 71.0), vec2(0.0, 169.0)];

// The Cab pedal has the display for picking the cabinet where a third knob would be: the
// amp's own or one of the player's files. Its two knobs are the upper two of a trio. The
// tape is as wide as fits between the buttons, and holds this many letters
pub(crate) const CAB_SLOT: usize = 2;
const CAB_CAPTION: Vec2 = vec2(0.0, 174.0);
const CAB_STEPPER: Vec2 = vec2(0.0, 198.0);
const CAB_STEPPER_WIDTH: f32 = 100.0;
const CAB_NAME_CHARS: usize = 10;

// The Gate pedal has a lamp beside its lowest knob that is lit while the gate lets the
// guitar through. The pedal's LED says that the gate is switched on, like every other
pub(crate) const GATE_SLOT: usize = 0;
const GATE_LAMP: Vec2 = vec2(60.0, 164.0);

/// One pedal of the board: its look, its switch and up to three knobs with their captions
type Pedal<'a> = (MiniStyle, &'a BoolParam, &'a [(&'a FloatParam, &'a str)]);

/// Title, paint and art of a pedal: each has a paint and a piece of northern weather of its own
const fn look(title: &'static str, paint: Color32, motif: Motif) -> MiniStyle {
    MiniStyle { title, paint, motif }
}

/// What the pedals of the board are drawn from
pub(crate) struct Pedals<'a> {
    params: &'a GuitarAmpParams,
    setter: &'a ParamSetter<'a>,
    gate_open: bool,
    cab_loader: &'a CabLoader,
    async_executor: &'a AsyncExecutor<GuitarAmpPlugin>,
}

impl Pedals<'_> {
    /// Whether the pedal of `slot` is a slim one: two knobs, one above the other
    pub(crate) fn is_slim(slot: usize) -> bool {
        slot >= WIDE_PEDALS
    }

    /// Draws the pedal of `slot` in `rect`: the five wide ones first, as the chain had
    /// them before there were more, then the slim ones. Returns its top face
    pub(crate) fn pedal(&self, ui: &mut Ui, slot: usize, rect: Rect) -> Rect {
        let (params, setter) = (self.params, self.setter);
        let pedals: [Pedal; PEDAL_SLOTS] = [
            (
                look("Gate", theme::PAINT_GRANITE, Motif::Sluice),
                &params.gate_on,
                &[
                    (&params.in_gain, "Input"),
                    (&params.gate_thresh, "Thresh"),
                    (&params.gate_release, "Release"),
                ],
            ),
            (
                look("Drive", theme::PAINT_OCHRE, Motif::Bonfire),
                &params.drive_on,
                &[
                    (&params.drive_gain, "Drive"),
                    (&params.drive_tone, "Tone"),
                    (&params.drive_level, "Level"),
                ],
            ),
            (
                look("Cab", theme::PAINT_BARK, Motif::Timber),
                &params.cab_on,
                &[(&params.cab_mic, "Mic"), (&params.cab_res, "Res")],
            ),
            (
                look("Delay", theme::PAINT_PETROL, Motif::Ridges),
                &params.delay_on,
                &[
                    (&params.delay_time, "Time"),
                    (&params.delay_feedback, "Repeat"),
                    (&params.delay_mix, "Mix"),
                ],
            ),
            (
                look("Reverb", theme::PAINT_FJORD, Motif::Fjord),
                &params.reverb_on,
                &[(&params.reverb_decay, "Decay"), (&params.reverb_mix, "Mix")],
            ),
            (
                look("Comp", theme::PAINT_MOSS, Motif::Isobars),
                &params.comp_on,
                &[(&params.comp_sustain, "Sustain"), (&params.comp_level, "Level")],
            ),
            (
                look("Chorus", theme::PAINT_SEA, Motif::Swell),
                &params.chorus_on,
                &[(&params.chorus_rate, "Rate"), (&params.chorus_depth, "Depth")],
            ),
            (
                look("Phaser", theme::PAINT_VIOLET, Motif::Moons),
                &params.phaser_on,
                &[(&params.phaser_rate, "Rate"), (&params.phaser_depth, "Depth")],
            ),
            (
                look("Flanger", theme::PAINT_AURORA, Motif::Aurora),
                &params.flanger_on,
                &[(&params.flanger_rate, "Rate"), (&params.flanger_depth, "Depth")],
            ),
            (
                look("Tremolo", theme::PAINT_HEATHER, Motif::Rain),
                &params.trem_on,
                &[(&params.trem_rate, "Rate"), (&params.trem_depth, "Depth")],
            ),
        ];
        let (style, on, knobs) = pedals[slot];
        if Self::is_slim(slot) {
            let face = ui::slim_pedal(ui, rect, &style);
            let top = face.center_top();
            for (&(param, caption), offset) in knobs.iter().zip(SLIM_KNOBS) {
                param_knob(ui, setter, param, top + offset, SLIM_KNOB_RADIUS, caption, None);
            }
            param_switch(ui, setter, on, top + SLIM_SWITCH, top + SLIM_LED);
            return face;
        }
        let face = ui::mini_pedal(ui, rect, &style);
        let top = face.center_top();

        let (radius, places) = if knobs.len() > PAIR_KNOBS.len() || slot == CAB_SLOT {
            (TRIO_RADIUS, &TRIO_KNOBS[..])
        } else {
            (PAIR_RADIUS, &PAIR_KNOBS[..])
        };
        for (&(param, caption), &offset) in knobs.iter().zip(places) {
            param_knob(ui, setter, param, top + offset, radius, caption, None);
        }
        param_switch(ui, setter, on, top + PEDAL_SWITCH, top + PEDAL_LED);

        if slot == GATE_SLOT {
            signal_lamp(ui.painter(), top + GATE_LAMP, self.gate_open);
            silk_label(ui.painter(), top + GATE_LAMP + LAMP_CAPTION, "Open", LAMP_CAPTION_SIZE);
        }

        if slot == CAB_SLOT {
            // The cabinet that plays: the amp's own, or a file from the cabinets
            // folder. The buttons step through them
            let chosen = params.cabinet.lock().unwrap_or_else(PoisonError::into_inner).clone();
            let shown = self.cab_loader.display_name(&chosen, CAB_NAME_CHARS);
            silk_label(ui.painter(), top + CAB_CAPTION, "Speaker", 13.0);
            // Where the files go: shown when the caption is hovered, and opened by a
            // double-click on it
            if let Some(dir) = user_cab::cabinets_dir() {
                let caption = Rect::from_center_size(top + CAB_CAPTION, vec2(70.0, 18.0));
                let response = ui
                    .interact(caption, Id::new("cabinet-folder"), Sense::click())
                    .on_hover_text(format!("Cabinet files go in {}", dir.display()));
                if response.double_clicked() {
                    let _ = std::fs::create_dir_all(&dir);
                    user_cab::open_folder(&dir);
                }
            }
            if self.cab_loader.no_files_showing() {
                // Draw once more when the note is over
                ui.ctx().request_repaint_after(NO_FILES_SHOWN);
            }
            if let Some(step) = stepper(ui, top + CAB_STEPPER, CAB_STEPPER_WIDTH, &shown, "cabinet")
            {
                step_cabinet(params, self.cab_loader, self.async_executor, step == Step::Next);
            }
        }
        face
    }
}

/// Moves the choice of cabinet one on in the round of the amp's own, the built-in ones and
/// the files in the cabinets folder as they are now, and has what it arrives at read away
/// from this thread and the audio. The folder is made here, the first time someone looks
/// for cabinets in it, and not by opening the plugin
fn step_cabinet(
    params: &GuitarAmpParams,
    cab_loader: &CabLoader,
    async_executor: &AsyncExecutor<GuitarAmpPlugin>,
    forward: bool,
) {
    let files = match user_cab::cabinets_dir() {
        Some(dir) => {
            let _ = std::fs::create_dir_all(&dir);
            user_cab::list(&dir)
        }
        None => Vec::new(),
    };
    let mut chosen = params.cabinet.lock().unwrap_or_else(PoisonError::into_inner);
    *chosen = user_cab::neighbour(&files, &chosen, forward);
    // Come round to the amp's own past where the files would be: say there are none
    if files.is_empty() && chosen.is_empty() && forward {
        cab_loader.found_no_files();
    }
    drop(chosen);
    cab_loader.chosen();
    async_executor.execute_background(Task::LoadCabinet);
}

/// What the editor keeps from one frame to the next
pub(crate) struct View {
    lamps: LampView,
    /// The player's presets, as the folder was when it was last looked at: when the window
    /// opened, and with every step and save
    bank: Bank,
    /// What the preset display says instead of the name, and until when
    note: Option<(&'static str, f64)>,
}

/// The preset display and the Save button beside it
struct PresetDisplay<'a> {
    params: &'a GuitarAmpParams,
    setter: &'a ParamSetter<'a>,
    cab_loader: &'a CabLoader,
    async_executor: &'a AsyncExecutor<GuitarAmpPlugin>,
}

impl PresetDisplay<'_> {
    fn user_preset(&self) -> String {
        self.params.user_preset.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    fn set_user_preset(&self, file: &str) {
        file.clone_into(
            &mut self.params.user_preset.lock().unwrap_or_else(PoisonError::into_inner),
        );
    }

    /// The caption with the folder behind it, the display that steps through the presets,
    /// and the Save button
    fn draw(&self, ui: &mut Ui, origin: Pos2, view: &mut View, now: f64) {
        let params = self.params;
        let dir = user_presets::presets_dir();
        let factory = presets::index_or_first(params.preset.load(Ordering::Relaxed));
        let user = self.user_preset();

        let caption = origin + vec2(PRESET_X, PRESET_CAPTION_Y);
        silk_label(ui.painter(), caption, "Preset", 13.0);
        // Where the player's presets are kept: shown when the caption is hovered, and opened
        // by a double-click on it, to rename, copy or throw them away
        if let Some(dir) = &dir {
            let response = ui
                .interact(
                    Rect::from_center_size(caption, vec2(60.0, 18.0)),
                    Id::new("preset-folder"),
                    Sense::click(),
                )
                .on_hover_text(format!("Your presets are kept in {}", dir.display()));
            if response.double_clicked() {
                let _ = std::fs::create_dir_all(dir);
                user_cab::open_folder(dir);
            }
        }

        let shown = match view.note {
            Some((note, until)) if now < until => note.to_owned(),
            _ => {
                view.note = None;
                self.shown_name(factory, &user, &view.bank)
            }
        };
        if let Some(step) =
            stepper(ui, origin + vec2(PRESET_X, PRESET_Y), PRESET_WIDTH, &shown, "preset")
        {
            view.note = None;
            view.bank = Bank::read(params, dir.as_deref());
            let place = user_presets::place(factory, &user, &view.bank);
            let next = user_presets::neighbour(place, &view.bank, step == Step::Next);
            self.load(next, &view.bank);
        }

        silk_label(ui.painter(), origin + vec2(SAVE_X, PRESET_CAPTION_Y), "Save", 13.0);
        if chrome_button(ui, origin + vec2(SAVE_X, PRESET_Y), "save-preset").clicked() {
            let saved = dir.as_deref().is_some_and(|dir| self.save(dir, factory, &user, view));
            view.note = Some((if saved { "SAVED" } else { "NOT SAVED" }, now + SAVE_NOTE_S));
            // Draw once more when the note is over
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(SAVE_NOTE_S));
        }
    }

    /// The name of the preset loaded last, marked once something was moved away from it. A
    /// player's preset that is no longer in the folder has the cabinets' missing mark
    fn shown_name(&self, factory: usize, user: &str, bank: &Bank) -> String {
        if user.is_empty() {
            return PRESETS[factory].display_name(self.params);
        }
        let name = user_presets::display_name(user, USER_NAME_CHARS);
        match bank.find(user) {
            Some(preset) if preset.matches(self.params) => name,
            Some(_) => format!("{name}{}", presets::EDITED_MARK),
            None => format!("{}{name}", user_cab::MISSING_MARK),
        }
    }

    /// Loads the preset at `place` in the round
    fn load(&self, place: usize, bank: &Bank) {
        let (params, setter) = (self.params, self.setter);
        match place.checked_sub(PRESETS.len()).and_then(|index| bank.get(index)) {
            Some((file, preset)) => {
                if preset.load(params, setter) {
                    self.cab_loader.chosen();
                    self.async_executor.execute_background(Task::LoadCabinet);
                }
                self.set_user_preset(file);
            }
            None => {
                let index = place.min(PRESETS.len() - 1);
                PRESETS[index].load(params, setter);
                params.preset.store(index as u32, Ordering::Relaxed);
                self.set_user_preset("");
            }
        }
    }

    /// Writes the sound as it stands: over the player's preset loaded last, or, after a
    /// factory preset, as a new one named after it. Returns whether it was written
    fn save(&self, dir: &Path, factory: usize, user: &str, view: &mut View) -> bool {
        view.bank = Bank::read(self.params, Some(dir));
        let file = if user.is_empty() {
            user_presets::new_file_name(PRESETS[factory].name, &view.bank.files)
        } else {
            user.to_owned()
        };
        if user_presets::save(self.params, dir, &file).is_err() {
            return false;
        }
        self.set_user_preset(&file);
        view.bank = Bank::read(self.params, Some(dir));
        true
    }
}

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(WINDOW_WIDTH, WINDOW_HEIGHT)
}

/// The amp a step of the selector arrives at, round the list
pub(crate) fn neighbour_amp(amp: Amp, step: Step) -> Amp {
    let count = Amp::ALL.len();
    let index = match step {
        Step::Next => (amp.index() + 1) % count,
        Step::Previous => (amp.index() + count - 1) % count,
    };
    Amp::ALL[index]
}

/// The microphone a step of its selector arrives at, round the list
pub(crate) fn neighbour_mic(mic: MicType, step: Step) -> MicType {
    let count = MicType::ALL.len();
    let index = MicType::ALL.iter().position(|&each| each == mic).unwrap_or(0);
    MicType::ALL[match step {
        Step::Next => (index + 1) % count,
        Step::Previous => (index + count - 1) % count,
    }]
}

pub fn create(
    params: Arc<GuitarAmpParams>,
    editor_state: Arc<EguiState>,
    tuner: Arc<TunerReading>,
    lamp_reading: Arc<LampReading>,
    cab_loader: Arc<CabLoader>,
    async_executor: AsyncExecutor<GuitarAmpPlugin>,
) -> Option<Box<dyn Editor>> {
    let (opened_loader, opened_executor) = (cab_loader.clone(), async_executor.clone());
    let opened_reading = lamp_reading.clone();
    let opened_params = params.clone();
    create_egui_editor(
        editor_state,
        View { lamps: LampView::new(&lamp_reading), bank: Bank::default(), note: None },
        move |egui_ctx, view| {
            ui::install(egui_ctx);
            // What was clipped while the window was closed is not news
            view.lamps = LampView::new(&opened_reading);
            // The player may have renamed or added presets in the folder since
            view.bank = Bank::read(&opened_params, user_presets::presets_dir().as_deref());
            // A cabinet file that was missing or unusable may have been put right since:
            // look again whenever the window opens
            if opened_loader.status() != CabStatus::Fine {
                opened_executor.execute_background(Task::LoadCabinet);
            }
        },
        move |egui_ctx, setter, view| {
            ui::rig(egui_ctx, |ui, window| {
                // The update runs with every frame of the window whether it is painted
                // or not, so this costs two atomic reads per frame, and a repaint only
                // when a lamp changes
                let now = ui.input(|input| input.time);
                if view.lamps.look(&lamp_reading, params.gate_on.value(), now) {
                    ui.ctx().request_repaint();
                }
                let (gate_open, clipping) = view.lamps.shown();
                let [input_db, output_db] = view.lamps.levels();

                let head = Rect::from_min_size(
                    window + vec2(BENCH_MARGIN, BENCH_MARGIN),
                    vec2(WINDOW_WIDTH as f32 - 2.0 * BENCH_MARGIN, HEAD_HEIGHT),
                );
                ui::head(ui, head, &STYLE);
                // What is on the head is placed from here
                let origin = window + vec2(HEAD_X, 0.0);

                let tuning = params.tuner_on.value();
                if tuning {
                    // The note the tuner hears and how far off it is, where the preset was
                    let note = tuner.hz().map(Note::nearest);
                    let name = note.map(|note| format!("{}{}", note.name, note.octave));
                    silk_label(
                        ui.painter(),
                        origin + vec2(PRESET_X, PRESET_CAPTION_Y),
                        "Tuner",
                        13.0,
                    );
                    tuner_display(
                        ui.painter(),
                        origin + vec2(PRESET_X, PRESET_Y),
                        TUNER_WIDTH,
                        name.as_deref(),
                        note.map(|note| note.cents),
                    );
                    // What it hears changes without anyone touching the window
                    ui.ctx().request_repaint();
                } else {
                    let presets = PresetDisplay {
                        params: &params,
                        setter,
                        cab_loader: &cab_loader,
                        async_executor: &async_executor,
                    };
                    presets.draw(ui, origin, view, now);
                }

                let dials = ["Gain", "Bass", "Mid", "Treble", "Presence", "Master"];
                for (index, (param, label)) in params.amp_dials().into_iter().zip(dials).enumerate()
                {
                    let x = DIAL_X + DIAL_SPACING * index as f32;
                    param_knob(
                        ui,
                        setter,
                        param,
                        origin + vec2(x, KNOB_Y),
                        KNOB_RADIUS,
                        label,
                        None,
                    );
                }
                param_knob(
                    ui,
                    setter,
                    &params.out_level,
                    origin + vec2(OUTPUT_X, KNOB_Y),
                    KNOB_RADIUS,
                    "Output",
                    None,
                );
                for (place, level, caption) in
                    [(INPUT_METER, input_db, "Input"), (OUTPUT_METER, output_db, "Output")]
                {
                    level_meter(ui.painter(), origin + place, METER_HEIGHT, level);
                    silk_label(
                        ui.painter(),
                        origin + place + METER_CAPTION,
                        caption,
                        LAMP_CAPTION_SIZE,
                    );
                }
                signal_lamp(ui.painter(), origin + CLIP_LAMP, clipping);
                silk_label(
                    ui.painter(),
                    origin + CLIP_LAMP + LAMP_CAPTION,
                    "Clip",
                    LAMP_CAPTION_SIZE,
                );

                // The amp selector: the amp that is left keeps its dials, and the one picked
                // gets its own back
                let amp_shown = params.amp.value();
                silk_label(ui.painter(), origin + vec2(AMP_STEPPER_X, LED_Y), "Amp", CAPTION_SIZE);
                if let Some(step) = stepper(
                    ui,
                    origin + vec2(AMP_STEPPER_X, STEPPER_Y),
                    AMP_STEPPER_WIDTH,
                    amp_shown.model().name,
                    "amp",
                ) {
                    dial_memory::switch_amp(&params, setter, neighbour_amp(amp_shown, step));
                }

                // The microphone the cabinet is heard with, as the selector of the amp
                let mic_shown = params.cab_mic_type.value();
                silk_label(ui.painter(), origin + vec2(MIC_STEPPER_X, LED_Y), "Mic", CAPTION_SIZE);
                if let Some(step) = stepper(
                    ui,
                    origin + vec2(MIC_STEPPER_X, STEPPER_Y),
                    MIC_STEPPER_WIDTH,
                    mic_shown.short_name(),
                    "mic",
                ) {
                    setter.begin_set_parameter(&params.cab_mic_type);
                    setter.set_parameter(&params.cab_mic_type, neighbour_mic(mic_shown, step));
                    setter.end_set_parameter(&params.cab_mic_type);
                }

                // Tuner switch, lit while tuning: the amp is silent then
                rig_led(ui.painter(), origin + vec2(TUNER_X + CAPTION_OFFSET, LED_Y), tuning);
                silk_label(
                    ui.painter(),
                    origin + vec2(TUNER_X + CAPTION_OFFSET, CAPTION_Y),
                    "Tuner",
                    CAPTION_SIZE,
                );
                if small_footswitch(ui, origin + vec2(TUNER_X, SWITCH_Y), "tuner").clicked() {
                    setter.begin_set_parameter(&params.tuner_on);
                    setter.set_parameter(&params.tuner_on, !tuning);
                    setter.end_set_parameter(&params.tuner_on);
                }

                // Bypass switch, lit while the amp is on
                let bypassed = params.bypass.value();
                rig_led(ui.painter(), origin + vec2(BYPASS_X + CAPTION_OFFSET, LED_Y), !bypassed);
                silk_label(
                    ui.painter(),
                    origin + vec2(BYPASS_X + CAPTION_OFFSET, CAPTION_Y),
                    "On",
                    CAPTION_SIZE,
                );
                if small_footswitch(ui, origin + vec2(BYPASS_X, SWITCH_Y), "bypass").clicked() {
                    setter.begin_set_parameter(&params.bypass);
                    setter.set_parameter(&params.bypass, !bypassed);
                    setter.end_set_parameter(&params.bypass);
                }

                let pedals = Pedals {
                    params: &params,
                    setter,
                    gate_open,
                    cab_loader: &cab_loader,
                    async_executor: &async_executor,
                };
                crate::board::board(ui, window, &pedals, &params.order);
            });
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // The picture on the face is compiled in: a build without it is a build of other sources
    #[test]
    fn test_the_pedal_is_drawn_with_its_art() {
        assert!(STYLE.art == Some(ui::artwork::AMP), "the pedal has lost its art");
    }

    #[test]
    fn test_mic_stepper_goes_round_the_list_both_ways() {
        let mut mic = MicType::ALL[0];
        for expected in [MicType::Ribbon, MicType::Condenser, MicType::Dynamic] {
            mic = neighbour_mic(mic, Step::Next);
            assert_eq!(mic, expected);
        }
        for one in MicType::ALL {
            assert_eq!(neighbour_mic(neighbour_mic(one, Step::Previous), Step::Next), one);
        }
    }

    #[test]
    fn test_amp_stepper_goes_round_the_list_both_ways() {
        let mut amp = Amp::ALL[0];
        let mut seen = vec![amp];
        for _ in 0..Amp::ALL.len() {
            amp = neighbour_amp(amp, Step::Next);
            seen.push(amp);
        }
        // Every amp once, and back where it started
        assert_eq!(seen.first(), seen.last());
        for one in Amp::ALL {
            assert_eq!(
                seen.iter().filter(|&&seen| seen == one).count(),
                if one == seen[0] { 2 } else { 1 }
            );
        }
        for one in Amp::ALL {
            assert_eq!(neighbour_amp(neighbour_amp(one, Step::Next), Step::Previous), one);
        }
    }
}
