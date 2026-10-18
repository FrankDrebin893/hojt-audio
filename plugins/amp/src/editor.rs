use nih_plug::prelude::*;
use nih_plug_egui::egui::{vec2, Color32, Id, Pos2, Rect, Sense, Ui, Vec2};
use nih_plug_egui::{create_egui_editor, EguiState};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};
use suite_common::ui::{
    self, chrome_button, drop_panel, level_meter, menu_stepper, param_knob, param_switch, rig_led,
    signal_lamp, silk_label, small_footswitch, stepper, theme, tuner_display, type_into, Column,
    MiniStyle, Motif, Ornament, PedalStyle, Pick, Row, Step, Typed, BENCH_MARGIN, SLIM_KNOBS,
    SLIM_KNOB_RADIUS, SLIM_LED, SLIM_SWITCH,
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

// The bank display left of the preset display: which set of presets the preset display
// picks from. Its caption is on the preset caption's line and its tape on the preset
// tape's line; its buttons go round the banks and its tape drops the whole list open
const BANK_X: f32 = PRESET_X - 210.0;
const BANK_WIDTH: f32 = 120.0;
const BANK_NAME_CHARS: usize = 10;

// The list that drops out of either display, under the preset tape: the banks in the first
// column and the presets of the bank that is listed in the second
const DROP_Y: f32 = PRESET_Y + 17.0;
const BANK_COLUMN: usize = 0;
const DROP_BANK_WIDTH: f32 = 132.0;
const DROP_PRESET_WIDTH: f32 = 186.0;
const DROP_BANK_CHARS: usize = 12;
const DROP_PRESET_CHARS: usize = 17;
// The row at the end of the banks that makes one, and how long a name typed on it may be
const NEW_BANK_ROW: &str = "+ New Bank";
const NEW_BANK_CHARS: usize = 14;

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
    /// The presets of the bank showing, as its folder was when it was last looked at: when
    /// the window opened, and with every step, pick and save
    bank: Bank,
    /// The list of banks and presets while it is dropped open
    menu: Option<Menu>,
    /// What the preset display says instead of the name, and until when
    note: Option<(&'static str, f64)>,
}

/// The list of banks and presets while it is dropped open
struct Menu {
    /// The bank whose presets it lists: the one showing, until another is clicked in it
    listed: String,
    /// The file names in `listed`, as its folder was when it was listed. Empty for the
    /// factory bank, whose presets are built in
    files: Vec<String>,
    /// The name being typed for a new bank
    naming: Option<String>,
    /// Opened in this frame: the click that opened it is no click outside it
    opened: bool,
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

    /// The bank showing, which the preset display picks from
    fn bank(&self) -> String {
        user_presets::bank_of(self.params)
    }

    /// Writes down which bank is showing
    fn set_bank(&self, bank: &str) {
        bank.clone_into(
            &mut self.params.preset_bank.lock().unwrap_or_else(PoisonError::into_inner),
        );
    }

    /// Shows `bank`, with nothing loaded out of it: the sound stays as it stands, the
    /// display falls back to the name of the preset it came from, and Save makes a new
    /// preset in the bank
    fn show_bank(&self, bank: &str, view: &mut View) {
        self.set_bank(bank);
        self.set_user_preset("");
        view.bank = Bank::read(self.params, user_presets::bank_dir(bank).as_deref());
    }

    /// The names of the presets in `bank` without reading them: nothing for the factory
    /// bank, whose presets are built in
    fn bank_files(&self, bank: &str) -> Vec<String> {
        user_presets::bank_dir(bank)
            .map(|dir| user_cab::list_with_extension(&dir, user_presets::EXTENSION))
            .unwrap_or_default()
    }

    /// The caption with the folder behind it, the Bank and Preset displays and the Save
    /// button. The list they open is drawn afterwards, by `menu`
    fn draw(&self, ui: &mut Ui, origin: Pos2, view: &mut View, now: f64) {
        let params = self.params;
        let bank = self.bank();
        let dir = user_presets::bank_dir(&bank);
        let factory = presets::index_or_first(params.preset.load(Ordering::Relaxed));
        let user = self.user_preset();

        let caption = origin + vec2(PRESET_X, PRESET_CAPTION_Y);
        silk_label(ui.painter(), caption, "Preset", 13.0);
        // Where the presets of the bank showing are kept: shown when the caption is
        // hovered, and opened by a double-click on it, to rename, move, copy or throw them
        // away. The factory bank has no folder, so it names the presets folder itself
        let folder = dir.clone().or_else(user_presets::presets_dir);
        if let Some(folder) = &folder {
            let response = ui
                .interact(
                    Rect::from_center_size(caption, vec2(60.0, 18.0)),
                    Id::new("preset-folder"),
                    Sense::click(),
                )
                .on_hover_text(format!("Your presets are kept in {}", folder.display()));
            if response.double_clicked() {
                let _ = std::fs::create_dir_all(folder);
                user_cab::open_folder(folder);
            }
        }

        silk_label(ui.painter(), origin + vec2(BANK_X, PRESET_CAPTION_Y), "Bank", 13.0);
        let bank_shown = user_presets::bank_name(&bank, BANK_NAME_CHARS);
        match menu_stepper(ui, origin + vec2(BANK_X, PRESET_Y), BANK_WIDTH, &bank_shown, "bank") {
            Some(Pick::Open) => self.toggle_menu(view, &bank),
            Some(Pick::Step(step)) => {
                let banks = user_presets::banks(user_presets::presets_dir().as_deref());
                let next = user_presets::neighbour_bank(&banks, &bank, step == Step::Next);
                self.show_bank(&next, view);
                view.note = None;
            }
            None => {}
        }

        let shown = match view.note {
            Some((note, until)) if now < until => note.to_owned(),
            _ => {
                view.note = None;
                self.shown_name(factory, &user, &view.bank)
            }
        };
        match menu_stepper(ui, origin + vec2(PRESET_X, PRESET_Y), PRESET_WIDTH, &shown, "preset") {
            Some(Pick::Open) => self.toggle_menu(view, &bank),
            Some(Pick::Step(step)) => {
                view.note = None;
                self.step(step == Step::Next, &bank, factory, &user, view, now);
            }
            None => {}
        }

        silk_label(ui.painter(), origin + vec2(SAVE_X, PRESET_CAPTION_Y), "Save", 13.0);
        if chrome_button(ui, origin + vec2(SAVE_X, PRESET_Y), "save-preset").clicked() {
            // Nothing is written into the factory bank: a sound saved there goes into the
            // presets folder, which is the bank showing from then on
            let into = dir.clone().or_else(user_presets::presets_dir);
            let saved = into.as_deref().is_some_and(|into| self.save(into, factory, &user, view));
            if saved && dir.is_none() {
                self.set_bank(user_presets::MINE);
            }
            view.note = Some((if saved { "SAVED" } else { "NOT SAVED" }, now + SAVE_NOTE_S));
        }
        // A note is only up for a while: draw once more when it is over
        if view.note.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(SAVE_NOTE_S));
        }
    }

    /// Drops the list open on the bank showing, or closes it if it is open already
    fn toggle_menu(&self, view: &mut View, bank: &str) {
        view.menu = match view.menu {
            Some(_) => None,
            None => Some(Menu {
                listed: bank.to_owned(),
                files: self.bank_files(bank),
                naming: None,
                opened: true,
            }),
        };
    }

    /// The list the displays drop open: the banks in the first column, the presets of the
    /// bank that is listed in the second. It is drawn after everything else in the window,
    /// in a layer above it, so nothing behind it answers a click while it is open
    fn menu(&self, ui: &mut Ui, origin: Pos2, view: &mut View, now: f64) {
        let Some(mut menu) = view.menu.take() else {
            return;
        };
        let params = self.params;
        let presets_dir = user_presets::presets_dir();
        let banks = user_presets::banks(presets_dir.as_deref());

        // What is typed for a new bank is taken before the list is drawn, so the letter is
        // on the tape in the same frame. Escape ends the naming, not the list
        let typed = menu.naming.as_mut().and_then(|name| type_into(ui, name, NEW_BANK_CHARS));
        match typed {
            Some(Typed::Done) => self.make_bank(&mut menu, presets_dir.as_deref(), view, now, ui),
            Some(Typed::Cancelled) => menu.naming = None,
            None => {}
        }

        let showing = self.bank();
        let user = self.user_preset();
        let factory = presets::index_or_first(params.preset.load(Ordering::Relaxed));
        let listed_is_factory = user_presets::bank_dir(&menu.listed).is_none();
        // Which preset is lit: only the bank that is listed can hold the one loaded
        let loaded = match (menu.listed == showing, listed_is_factory) {
            (false, _) => Loaded::Nothing,
            (true, true) if user.is_empty() => Loaded::Factory(factory),
            (true, true) => Loaded::Nothing,
            (true, false) => Loaded::File(&user),
        };
        let bank_rows = bank_rows(&banks, &menu.listed, menu.naming.as_deref());
        let preset_rows =
            preset_rows((!listed_is_factory).then_some(menu.files.as_slice()), loaded);

        let columns = [
            Column { title: "Bank", key: "banks", width: DROP_BANK_WIDTH, rows: &bank_rows },
            Column {
                title: "Preset",
                key: &menu.listed,
                width: DROP_PRESET_WIDTH,
                rows: &preset_rows,
            },
        ];
        let picked = drop_panel(ui, origin + vec2(PRESET_X, DROP_Y), &columns, "presets");
        // Neither the click that opened it nor the escape that ended a name closes it
        let mut open = !(picked.closed && !menu.opened && typed.is_none());
        menu.opened = false;

        match picked.row {
            // The row at the end of the banks takes a name for a new one. Clicked again it
            // keeps the name as it stands, so a bank can be made without the keyboard
            Some((BANK_COLUMN, row)) if row == banks.len() => {
                if menu.naming.is_some() {
                    self.make_bank(&mut menu, presets_dir.as_deref(), view, now, ui);
                } else {
                    menu.naming = Some(String::new());
                }
            }
            Some((BANK_COLUMN, row)) => {
                menu.naming = None;
                if let Some(bank) = banks.get(row) {
                    menu.files = self.bank_files(bank);
                    menu.listed = bank.clone();
                }
            }
            Some((_, row)) => {
                // The bank a preset is picked from is the bank showing from then on
                self.set_bank(&menu.listed);
                view.bank = Bank::read(params, user_presets::bank_dir(&menu.listed).as_deref());
                if listed_is_factory {
                    self.load_factory(row);
                } else {
                    self.load_user(row, view);
                }
                view.note = None;
                open = false;
            }
            None => {}
        }
        if open {
            view.menu = Some(menu);
        }
    }

    /// Makes the bank of the name that was typed, and lists it. A name nothing was typed
    /// for is `Bank 1`, `Bank 2` and so on, which is renamed in the folder
    fn make_bank(
        &self,
        menu: &mut Menu,
        presets_dir: Option<&Path>,
        view: &mut View,
        now: f64,
        ui: &Ui,
    ) {
        let name = menu.naming.take().unwrap_or_default();
        match presets_dir.map(|dir| user_presets::make_bank(dir, &name)) {
            Some(Ok(made)) => {
                self.show_bank(&made, view);
                menu.files = self.bank_files(&made);
                menu.listed = made;
            }
            _ => {
                // The display says so where the name was typed, so it is drawn again
                view.note = Some(("NO BANK", now + SAVE_NOTE_S));
                ui.ctx().request_repaint();
            }
        }
    }

    /// The preset a button of the preset display asks for: the one after or before it in
    /// the bank showing, which it comes round in without leaving it
    fn step(
        &self,
        forward: bool,
        bank: &str,
        factory: usize,
        user: &str,
        view: &mut View,
        now: f64,
    ) {
        let Some(dir) = user_presets::bank_dir(bank) else {
            self.load_factory(user_presets::neighbour_factory(factory, forward));
            return;
        };
        view.bank = Bank::read(self.params, Some(&dir));
        match user_presets::neighbour_user(&view.bank, user, forward) {
            Some(at) => self.load_user(at, view),
            // A bank of the player's with nothing saved in it yet
            None => view.note = Some(("NO PRESETS", now + SAVE_NOTE_S)),
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

    /// Loads the factory preset `index`
    fn load_factory(&self, index: usize) {
        let index = index.min(PRESETS.len() - 1);
        PRESETS[index].load(self.params, self.setter);
        self.params.preset.store(index as u32, Ordering::Relaxed);
        self.set_user_preset("");
    }

    /// Loads the preset at `at` of the bank the window has read, and the cabinet with it
    fn load_user(&self, at: usize, view: &View) {
        let Some((file, preset)) = view.bank.get(at) else {
            return;
        };
        if preset.load(self.params, self.setter) {
            self.cab_loader.chosen();
            self.async_executor.execute_background(Task::LoadCabinet);
        }
        self.set_user_preset(file);
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

/// Which preset of the bank a column lists is loaded, and so lit
#[derive(Clone, Copy, PartialEq, Eq)]
enum Loaded<'a> {
    /// None of them: the sound came from another bank
    Nothing,
    Factory(usize),
    File(&'a str),
}

/// The rows of the bank column: every bank with the one that is listed lit, and the row
/// that makes a bank at the end, which holds the name while it is being typed
fn bank_rows(banks: &[String], listed: &str, naming: Option<&str>) -> Vec<Row> {
    let mut rows: Vec<Row> = banks
        .iter()
        .map(|bank| Row {
            text: user_presets::bank_name(bank, DROP_BANK_CHARS),
            lit: bank == listed,
            dim: false,
        })
        .collect();
    rows.push(match naming {
        // The name as it is typed, with the caret after it
        Some(name) => Row { text: format!("{name}_"), lit: true, dim: false },
        None => Row { text: NEW_BANK_ROW.to_owned(), lit: false, dim: false },
    });
    rows
}

/// The rows of the preset column: the factory presets when `files` is nothing, else the
/// files of the bank that is listed, with the one that is loaded lit. A bank with nothing
/// in it has one row that says so and is not picked
fn preset_rows(files: Option<&[String]>, loaded: Loaded) -> Vec<Row> {
    let mut rows: Vec<Row> = match files {
        None => PRESETS
            .iter()
            .enumerate()
            .map(|(at, preset)| Row {
                text: preset.name.to_owned(),
                lit: loaded == Loaded::Factory(at),
                dim: false,
            })
            .collect(),
        Some(files) => files
            .iter()
            .map(|file| Row {
                text: user_presets::display_name(file, DROP_PRESET_CHARS),
                lit: loaded == Loaded::File(file),
                dim: false,
            })
            .collect(),
    };
    if rows.is_empty() {
        rows.push(Row { text: "No presets".to_owned(), lit: false, dim: true });
    }
    rows
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
        View { lamps: LampView::new(&lamp_reading), bank: Bank::default(), menu: None, note: None },
        move |egui_ctx, view| {
            ui::install(egui_ctx);
            // What was clipped while the window was closed is not news
            view.lamps = LampView::new(&opened_reading);
            // The player may have renamed, moved or added presets in the folder since
            let bank = user_presets::bank_of(&opened_params);
            view.bank = Bank::read(&opened_params, user_presets::bank_dir(&bank).as_deref());
            view.menu = None;
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

                let presets = PresetDisplay {
                    params: &params,
                    setter,
                    cab_loader: &cab_loader,
                    async_executor: &async_executor,
                };
                let tuning = params.tuner_on.value();
                if tuning {
                    // The tuner takes the place of the preset display, list and all
                    view.menu = None;
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

                // The preset list after everything else, in a layer above the window
                presets.menu(ui, origin, view, now);
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

    /// How far a stepper's buttons stand out from the middle of its tape
    fn reach(width: f32) -> f32 {
        width / 2.0 + 22.0
    }

    /// The bottom edge of a display's tape, from the top of the window
    fn tape_bottom(y: f32) -> f32 {
        y + 11.0
    }

    #[test]
    fn test_the_bank_display_stands_clear_of_the_preset_display() {
        assert!(
            BANK_X + reach(BANK_WIDTH) < PRESET_X - reach(PRESET_WIDTH),
            "the bank display runs into the preset display"
        );
        // Both stand clear of the knob ticks, and the list drops out under their tapes
        assert!(tape_bottom(PRESET_Y) < KNOB_Y - KNOB_RADIUS - 12.0, "they run into the dials");
        assert!(DROP_Y > tape_bottom(PRESET_Y), "the list covers the display it drops from");
    }

    #[test]
    fn test_the_list_holds_every_bank_and_the_row_that_makes_one() {
        let banks = vec![user_presets::FACTORY.to_owned(), user_presets::MINE.to_owned()];
        let rows = bank_rows(&banks, user_presets::MINE, None);
        let texts: Vec<&str> = rows.iter().map(|row| row.text.as_str()).collect();
        assert_eq!(texts, ["FACTORY", "MINE", NEW_BANK_ROW]);
        // The bank that is listed is the lit one, and nothing else
        assert_eq!(rows.iter().filter(|row| row.lit).count(), 1);
        assert!(rows[1].lit);
        // While a bank is being named, its row holds what is typed with the caret after it
        let naming = bank_rows(&banks, user_presets::MINE, Some("Jaz"));
        assert_eq!(naming[2].text, "Jaz_");
        assert!(naming[2].lit);
    }

    #[test]
    fn test_the_list_holds_the_presets_of_the_bank_it_lists_with_the_one_loaded_lit() {
        // The factory bank: every built-in preset, the one loaded lit
        let rows = preset_rows(None, Loaded::Factory(3));
        assert_eq!(rows.len(), PRESETS.len());
        assert_eq!(rows[3].text, PRESETS[3].name);
        assert_eq!(rows.iter().filter(|row| row.lit).count(), 1);
        assert!(rows[3].lit);

        // A bank of the player's: its files, without the extension and in capitals
        let files = vec!["Smooth.preset".to_owned(), "Tåge 2.preset".to_owned()];
        let rows = preset_rows(Some(&files), Loaded::File("Tåge 2.preset"));
        assert_eq!(rows[0].text, "SMOOTH");
        assert!(!rows[0].lit && rows[1].lit);

        // Nothing is lit while the sound came out of another bank
        let rows = preset_rows(Some(&files), Loaded::Nothing);
        assert!(rows.iter().all(|row| !row.lit && !row.dim));
        // A bank with nothing in it says so, and that row is not picked
        let empty = preset_rows(Some(&[]), Loaded::Nothing);
        assert_eq!(empty.len(), 1);
        assert!(empty[0].dim);
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
