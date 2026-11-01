use nih_plug::prelude::*;
use nih_plug_egui::egui::{vec2, Color32, Context, Id, Pos2, Rect, Sense, Ui, Vec2};
use nih_plug_egui::{create_egui_editor, EguiState};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError};
use suite_common::ui::{
    self, chrome_button, drop_panel, led_stepper, led_width, level_meter, nameplate_stepper,
    panel_knob, param_knob, param_switch, print_label, rig_led, settings_panel, signal_lamp,
    silk_label, small_footswitch, stepper, theme, tuner_display, type_into, type_on_tape, Column,
    HeadFaces, MiniStyle, Motif, Ornament, PedalStyle, Pick, Row, SettingRow, SettingsEvent,
    SettingsText, Step, Typed, BENCH_MARGIN, LED_HEIGHT, SLIM_KNOBS, SLIM_KNOB_RADIUS, SLIM_LED,
    SLIM_SWITCH,
};

use crate::board::STACK_WIDTH;
use crate::cab::MicType;
use crate::cab_stage::{CabLoader, CabStatus, NO_FILES_SHOWN};
use crate::dial_memory;
use crate::lamps::{LampReading, LampView};
use crate::looks;
use crate::presets::{self, PRESETS};
#[cfg(test)]
use crate::standalone::Prefs;
use crate::standalone::{self, AudioChoice};
use crate::tuner::{Note, TunerReading};
use crate::user_cab;
use crate::user_presets::{self, Bank};
use crate::{Amp, GuitarAmpParams, GuitarAmpPlugin, Task};

const WINDOW_WIDTH: u32 = crate::board::WINDOW_WIDTH;
const WINDOW_HEIGHT: u32 = 650;

// The strip across the top, and under it the line: the pedals, and among them the stack, the
// head standing on the cabinet. The cabinet is as tall as the pedals and on the floor with them
const STRIP_HEIGHT: f32 = 130.0;
pub(crate) const STACK_TOP: f32 = BENCH_MARGIN + STRIP_HEIGHT + BENCH_MARGIN;
const HEAD_HEIGHT: f32 = 180.0;
pub(crate) const PEDAL_TOP: f32 = STACK_TOP + HEAD_HEIGHT;
pub(crate) const PEDAL_HEIGHT: f32 = WINDOW_HEIGHT as f32 - BENCH_MARGIN - PEDAL_TOP;
pub(crate) const PEDAL_WIDTH: f32 = 180.0;

pub(crate) const STYLE: PedalStyle = PedalStyle {
    name: "Guitar Amp",
    model: "GA-3",
    paint: ui::theme::PAINT_OXBLOOD,
    ornament: Ornament::Bolts,
    jacks: ["In", "Stereo Out"],
    band_y: 62.0,
    art: Some(ui::artwork::AMP),
};

// Everything on the strip is placed from the window's top left corner, in one row across it
// with the same gap between neighbours and the same margin at both ends. What comes in and what
// is picked is on the left: the input meter, then the bank and preset displays and the Save
// button. What goes out is on the right: the output knob, the output meter, the clip lamp, and
// the On and Tuner switches. Each one's place is the end of the one before it and the gap
const STRIP_GAP: f32 = 134.0;

// The bank display and the preset display, each an LED display with a button at each end and
// its caption over it: what set of presets the preset display picks from, and the preset.
// Wide enough for the longest name and the edited mark. While the tuner is on, its display is
// where the preset display is, under its own caption
const PRESET_CAPTION_Y: f32 = 38.0;
const PRESET_Y: f32 = 72.0;
const PRESET_CHARS: usize = 16;
const PRESET_WIDTH: f32 = led_width(PRESET_CHARS);
// The longest name of a player's preset the display shows, with room for the edited mark
const USER_NAME_CHARS: usize = PRESET_CHARS - 1;
// How long the display says what Save did
const SAVE_NOTE_S: f64 = 1.5;

// The bank display left of the preset display. Its caption is on the preset caption's line and
// its window on the preset window's line; its buttons go round the banks and its window drops
// the whole list open
const BANK_CHARS: usize = 10;
const BANK_WIDTH: f32 = led_width(BANK_CHARS);
const BANK_NAME_CHARS: usize = BANK_CHARS;

// The list that drops out of either display, under the preset window, over the line below:
// the banks in the first column and the presets of the bank that is listed in the second
const DROP_Y: f32 = PRESET_Y + LED_HEIGHT / 2.0 + 6.0;
const BANK_COLUMN: usize = 0;
const DROP_BANK_WIDTH: f32 = 132.0;
const DROP_PRESET_WIDTH: f32 = 186.0;
const DROP_BANK_CHARS: usize = 12;
const DROP_PRESET_CHARS: usize = 17;
// The row at the end of the banks that makes one, and how long a name typed on it may be
const NEW_BANK_ROW: &str = "+ New Bank";
const NEW_BANK_CHARS: usize = 14;
// How long a name typed for a new preset may be, and how much of it the display has room for
// (the caret takes one place)
const NEW_PRESET_CHARS: usize = 24;
const NEW_PRESET_SHOWN: usize = PRESET_CHARS - 1;

// What each control of the strip takes across, from its middle: a level meter with its caption,
// the Save button, the output knob with its ticks, a signal lamp with its caption, and a small
// switch with the LED and caption left of it. A stepper's buttons reach this far past its window
const METER_REACH: f32 = 28.0;
const SAVE_REACH: f32 = 25.0;
const KNOB_REACH: f32 = OUTPUT_RADIUS + 13.0;
const LAMP_REACH: f32 = 20.0;
const SWITCH_REACH: f32 = 25.0;
const SWITCH_GROUP: f32 = SWITCH_REACH + 86.0;
const STEPPER_REACH: f32 = 33.0;

// The level meters, one in each wing: the input's under the jack it comes in at, the
// output's between the output knob and the clip lamp. A column of lamps on label tape,
// with its caption under it
const INPUT_METER: Vec2 = vec2(135.0, 66.0);
const METER_HEIGHT: f32 = 80.0;
const METER_CAPTION: Vec2 = vec2(0.0, 52.0);

const BANK_X: f32 = INPUT_METER.x + METER_REACH + STRIP_GAP + BANK_WIDTH / 2.0 + STEPPER_REACH;
const PRESET_X: f32 =
    BANK_X + BANK_WIDTH / 2.0 + STEPPER_REACH + STRIP_GAP + PRESET_WIDTH / 2.0 + STEPPER_REACH;
// The Save button right of the preset display, its caption on the line of the display's
const SAVE_X: f32 = PRESET_X + PRESET_WIDTH / 2.0 + STEPPER_REACH + STRIP_GAP + SAVE_REACH;

// The output level, the plugin's own and not the amp's
const OUTPUT_KNOB: Vec2 = vec2(SAVE_X + SAVE_REACH + STRIP_GAP + KNOB_REACH, 56.0);
const OUTPUT_RADIUS: f32 = 22.0;
const OUTPUT_METER: Vec2 = vec2(OUTPUT_KNOB.x + KNOB_REACH + STRIP_GAP + METER_REACH, 66.0);

// The clip lamp, right of the output meter: lit while the output is rounded off by the
// safety clip. A signal lamp has its caption this far under it
const CLIP_LAMP: Vec2 = vec2(OUTPUT_METER.x + METER_REACH + STRIP_GAP + LAMP_REACH, 58.0);
const LAMP_CAPTION: Vec2 = vec2(0.0, 18.0);
const LAMP_CAPTION_SIZE: f32 = 13.0;

// Small switches, left of each its LED above its caption: the bypass, and the tuner beside
// it at the end of the strip
const SWITCH_Y: f32 = 72.0;
const BYPASS_X: f32 = CLIP_LAMP.x + LAMP_REACH + STRIP_GAP + SWITCH_GROUP - SWITCH_REACH;
const TUNER_X: f32 = BYPASS_X + SWITCH_REACH + STRIP_GAP + SWITCH_GROUP - SWITCH_REACH;
const CAPTION_OFFSET: f32 = -58.0;
const LED_Y: f32 = 61.0;
const CAPTION_Y: f32 = 87.0;
const CAPTION_SIZE: f32 = 17.0;

// On the head, which looks as its amp does (`looks`): the nameplate on the grille, the amp's
// name between two buttons that step to the next amp, and the panel under it. On the panel,
// from its left edge, the input jack the cable from the pedals goes into, the amp's six dials
// in an even row, and at its right end the pilot lamp, lit while the amp is on
const NAMEPLATE_WIDTH: f32 = 140.0;
// The middle of the jack, the first dial and the line of the dials, from the panel's top left
// corner; captions are on one line above them
const HEAD_JACK_X: f32 = 18.0;
const DIAL_FIRST_X: f32 = 59.0;
const DIAL_Y: f32 = 52.0;
const DIAL_RADIUS: f32 = 18.0;
const DIAL_SPACING: f32 = 61.0;
const DIALS: [&str; 6] = ["Gain", "Bass", "Mid", "Treble", "Presence", "Master"];
// On the cabinet, which looks as the cabinet that plays (`looks::cab_look`), from its top left
// corner: on the control plate across its top, the cabinet that plays (the amp's own, a
// built-in one or a file) over the microphone that hears it, the Mic and Res knobs, and the
// cabinet's switch with its LED and caption above it. The output jack the cable to the pedals
// behind it leaves from is under the plate
const SPEAKER_CAPTION: Vec2 = vec2(110.0, 38.0);
const SPEAKER_STEPPER: Vec2 = vec2(110.0, 60.0);
// The tape is as wide as fits beside the knobs, and holds this many letters
const SPEAKER_WIDTH: f32 = 100.0;
const CAB_NAME_CHARS: usize = 10;
// The microphone: three letters on its tape
const MIC_CAPTION: Vec2 = vec2(110.0, 88.0);
const MIC_STEPPER: Vec2 = vec2(110.0, 110.0);
const MIC_STEPPER_WIDTH: f32 = 54.0;
const CAB_KNOBS: [Vec2; 2] = [vec2(240.0, 62.0), vec2(310.0, 62.0)];
const CAB_KNOB_RADIUS: f32 = 17.0;
const CAB_SWITCH_CAPTION: Vec2 = vec2(385.0, 38.0);
const CAB_LED: Vec2 = vec2(385.0, 60.0);
const CAB_SWITCH: Vec2 = vec2(385.0, 104.0);
const CAB_JACK: Vec2 = vec2(425.0, 160.0);
const CAB_JACK_CAPTION: Vec2 = vec2(425.0, 182.0);
// The jack and its caption are on a small plate of their own over the grille's corner, so the
// caption reads on every cloth
const CAB_JACK_PLATE: Rect = Rect::from_min_max(Pos2::new(400.0, 150.0), Pos2::new(448.0, 196.0));

// The pedals, all as wide as each other: four with a trio or a pair of knobs side by side and
// six with two knobs on a diagonal; the pitch shifter, last, is a trio like the first three
const WIDE_PEDALS: usize = 4;
const PEDAL_SLOTS: usize = 10;
pub(crate) const PITCH_SLOT: usize = PEDAL_SLOTS;

// On every pedal, from its top centre: the LED under the title, the knobs, the switch at the bottom.
// Two knobs sit side by side. Three do not fit in a row, so they are smaller and in a triangle,
// with the LED between the upper two
const PEDAL_LED: Vec2 = vec2(0.0, 71.0);
const PEDAL_SWITCH: Vec2 = vec2(0.0, 273.0);
const PAIR_RADIUS: f32 = 24.0;
const PAIR_KNOBS: [Vec2; 2] = [vec2(-45.0, 144.0), vec2(45.0, 144.0)];
const TRIO_RADIUS: f32 = 17.0;
const TRIO_KNOBS: [Vec2; 3] = [vec2(-45.0, 71.0), vec2(45.0, 71.0), vec2(0.0, 169.0)];

// The Gate pedal has a lamp beside its lowest knob that is lit while the gate lets the
// guitar through. The pedal's LED says that the gate is switched on, like every other
pub(crate) const GATE_SLOT: usize = 0;
const GATE_LAMP: Vec2 = vec2(60.0, 164.0);

// While the Delay follows the song's tempo (Sync), its Time knob, the first, picks the note
// value in the same place, and its tape shows the note
pub(crate) const DELAY_SLOT: usize = 2;
// Sync is a small push button right of the Mix knob, where the Gate has its lamp: a stomp
// switch beside the On switch would run off the face. Its LED above it, its caption above that
const SYNC_CAPTION: Vec2 = vec2(60.0, 156.0);
const SYNC_LED: Vec2 = vec2(60.0, 180.0);
const SYNC_BUTTON: Vec2 = vec2(60.0, 212.0);

/// Where the patch cables go into the stack: the head's input and the cabinet's output
#[derive(Clone, Copy)]
pub(crate) struct Jacks {
    pub(crate) input: Pos2,
    pub(crate) output: Pos2,
}

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
    /// Has the chosen cabinet read in the background
    load_cabinet: &'a dyn Fn(),
}

impl Pedals<'_> {
    /// Whether the pedal of `slot` is a slim one: two knobs, one above the other
    pub(crate) fn is_slim(slot: usize) -> bool {
        (WIDE_PEDALS..PEDAL_SLOTS).contains(&slot)
    }

    /// The pitch shifter: a trio of knobs, the first of which is a whole number of semitones
    /// and so not a `FloatParam` like the rest of them. Returns its top face
    fn pitch_pedal(&self, ui: &mut Ui, rect: Rect) -> Rect {
        let (params, setter) = (self.params, self.setter);
        let face = ui::mini_pedal(ui, rect, &look("Pitch", theme::PAINT_ORANGE, Motif::Horizon));
        let top = face.center_top();
        let [semitones, blend, tracking] = TRIO_KNOBS;
        let radius = TRIO_RADIUS;
        param_knob(ui, setter, &params.pitch_semitones, top + semitones, radius, "Semitones", None);
        param_knob(ui, setter, &params.pitch_blend, top + blend, radius, "Blend", None);
        param_knob(ui, setter, &params.pitch_latency, top + tracking, radius, "Tracking", None);
        param_switch(ui, setter, &params.pitch_on, top + PEDAL_SWITCH, top + PEDAL_LED);
        face
    }

    /// Draws the pedal of `slot` in `rect`: the wide ones first, as the chain had them
    /// before there were more, then the slim ones. Returns its top face
    pub(crate) fn pedal(&self, ui: &mut Ui, slot: usize, rect: Rect) -> Rect {
        if slot == PITCH_SLOT {
            return self.pitch_pedal(ui, rect);
        }
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
            (
                look("Fuzz", theme::PAINT_EMBER, Motif::Geyser),
                &params.fuzz_on,
                &[(&params.fuzz_fuzz, "Fuzz"), (&params.fuzz_tone, "Tone")],
            ),
        ];
        let (style, on, knobs) = pedals[slot];
        if Self::is_slim(slot) {
            let face = ui::mini_pedal(ui, rect, &style);
            let top = face.center_top();
            for (&(param, caption), offset) in knobs.iter().zip(SLIM_KNOBS) {
                param_knob(ui, setter, param, top + offset, SLIM_KNOB_RADIUS, caption, None);
            }
            param_switch(ui, setter, on, top + SLIM_SWITCH, top + SLIM_LED);
            return face;
        }
        let face = ui::mini_pedal(ui, rect, &style);
        let top = face.center_top();

        let (radius, places) = if knobs.len() > PAIR_KNOBS.len() {
            (TRIO_RADIUS, &TRIO_KNOBS[..])
        } else {
            (PAIR_RADIUS, &PAIR_KNOBS[..])
        };
        for (index, (&(param, caption), &offset)) in knobs.iter().zip(places).enumerate() {
            if slot == DELAY_SLOT && index == 0 && params.delay_sync.value() {
                param_knob(ui, setter, &params.delay_note, top + offset, radius, caption, None);
            } else {
                param_knob(ui, setter, param, top + offset, radius, caption, None);
            }
        }
        param_switch(ui, setter, on, top + PEDAL_SWITCH, top + PEDAL_LED);

        if slot == DELAY_SLOT {
            let sync = params.delay_sync.value();
            silk_label(ui.painter(), top + SYNC_CAPTION, "Sync", LAMP_CAPTION_SIZE);
            rig_led(ui.painter(), top + SYNC_LED, sync);
            if chrome_button(ui, top + SYNC_BUTTON, "delay-sync").clicked() {
                setter.begin_set_parameter(&params.delay_sync);
                setter.set_parameter(&params.delay_sync, !sync);
                setter.end_set_parameter(&params.delay_sync);
            }
        }
        if slot == GATE_SLOT {
            signal_lamp(ui.painter(), top + GATE_LAMP, self.gate_open);
            silk_label(ui.painter(), top + GATE_LAMP + LAMP_CAPTION, "Open", LAMP_CAPTION_SIZE);
        }

        face
    }

    /// Draws the stack with its top left corner at `top_left`: the head of the amp that is
    /// picked, and the cabinet it stands on, as tall as the pedals and looking as the cabinet
    /// that plays. Returns where the cables go in and out
    pub(crate) fn stack(&self, ui: &mut Ui, top_left: Pos2) -> Jacks {
        let (params, setter) = (self.params, self.setter);
        let amp_shown = params.amp.value();
        let look = looks::look(amp_shown);
        let (panel, print) = (look.head.panel, look.head.print);
        let head = Rect::from_min_size(top_left, vec2(STACK_WIDTH, HEAD_HEIGHT));
        let faces = ui::amp_head(ui, head, &look.head, !params.bypass.value());
        let places = HeadPlaces::new(faces);
        let cab = Rect::from_min_size(
            Pos2::new(top_left.x, top_left.y + HEAD_HEIGHT),
            vec2(STACK_WIDTH, PEDAL_HEIGHT),
        );
        // The cabinet that plays: the amp's own, a built-in one or a file from the
        // cabinets folder. The buttons of its display step through them
        let chosen = params.cabinet.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let cab_look = looks::cab_look(amp_shown, &chosen, self.cab_loader.status());
        ui::cabinet(ui, cab, cab_look);
        let cab = cab.min;
        ui::amp::mounted_plate(
            ui.painter(),
            CAB_JACK_PLATE.translate(cab.to_vec2()),
            cab_look.plate,
        );

        // The nameplate picks the amp: the amp that is left keeps its dials, and the one
        // picked gets its own back
        let name = amp_shown.model().name;
        if let Some(step) =
            nameplate_stepper(ui, places.nameplate, NAMEPLATE_WIDTH, panel, print, name, "amp")
        {
            dial_memory::switch_amp(params, setter, neighbour_amp(amp_shown, step));
        }
        for ((param, label), at) in params.amp_dials().into_iter().zip(DIALS).zip(places.dials) {
            panel_knob(ui, setter, param, at, DIAL_RADIUS, label, look.knobs, print);
        }
        print_label(ui.painter(), places.jack_caption, "In", LAMP_CAPTION_SIZE, print);

        let shown = self.cab_loader.display_name(&chosen, CAB_NAME_CHARS);
        silk_label(ui.painter(), cab + SPEAKER_CAPTION, "Speaker", LAMP_CAPTION_SIZE);
        // Where the files go: shown when the caption is hovered, and opened by a
        // double-click on it
        if let Some(dir) = user_cab::cabinets_dir() {
            let caption = Rect::from_center_size(cab + SPEAKER_CAPTION, vec2(70.0, 18.0));
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
        if let Some(step) = stepper(ui, cab + SPEAKER_STEPPER, SPEAKER_WIDTH, &shown, "cabinet") {
            step_cabinet(params, self.cab_loader, self.load_cabinet, step == Step::Next);
        }

        // The microphone the cabinet is heard with
        let mic_shown = params.cab_mic_type.value();
        silk_label(ui.painter(), cab + MIC_CAPTION, "Mic Type", LAMP_CAPTION_SIZE);
        if let Some(step) =
            stepper(ui, cab + MIC_STEPPER, MIC_STEPPER_WIDTH, mic_shown.short_name(), "mic")
        {
            setter.begin_set_parameter(&params.cab_mic_type);
            setter.set_parameter(&params.cab_mic_type, neighbour_mic(mic_shown, step));
            setter.end_set_parameter(&params.cab_mic_type);
        }

        for ((param, caption), offset) in
            [(&params.cab_mic, "Mic"), (&params.cab_res, "Res")].into_iter().zip(CAB_KNOBS)
        {
            param_knob(ui, setter, param, cab + offset, CAB_KNOB_RADIUS, caption, None);
        }
        silk_label(ui.painter(), cab + CAB_SWITCH_CAPTION, "Cab", LAMP_CAPTION_SIZE);
        param_switch(ui, setter, &params.cab_on, cab + CAB_SWITCH, cab + CAB_LED);
        silk_label(ui.painter(), cab + CAB_JACK_CAPTION, "Out", LAMP_CAPTION_SIZE);

        Jacks { input: places.jack, output: cab + CAB_JACK }
    }
}

/// Where the controls of a head go, from its faces
struct HeadPlaces {
    nameplate: Pos2,
    jack: Pos2,
    jack_caption: Pos2,
    dials: [Pos2; DIALS.len()],
}

impl HeadPlaces {
    fn new(faces: HeadFaces) -> Self {
        let corner = faces.panel.left_top();
        let dials = std::array::from_fn(|index| {
            corner + vec2(DIAL_FIRST_X + DIAL_SPACING * index as f32, DIAL_Y)
        });
        let jack = corner + vec2(HEAD_JACK_X, DIAL_Y);
        // On the line of the dials' captions
        let caption = ui::panel_caption(dials[0], DIAL_RADIUS).y;
        Self {
            nameplate: faces.grille.center(),
            jack,
            jack_caption: Pos2::new(jack.x, caption),
            dials,
        }
    }
}

/// Moves the choice of cabinet one on in the round of the amp's own, the built-in ones and
/// the files in the cabinets folder as they are now, and has what it arrives at read away
/// from this thread and the audio. The folder is made here, the first time someone looks
/// for cabinets in it, and not by opening the plugin
fn step_cabinet(
    params: &GuitarAmpParams,
    cab_loader: &CabLoader,
    load_cabinet: &dyn Fn(),
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
    load_cabinet();
}

/// What the editor keeps from one frame to the next
pub(crate) struct View {
    lamps: LampView,
    /// The presets of the bank showing, as its folder was when it was last looked at: when
    /// the window opened, and with every step, pick and save
    bank: Bank,
    /// The list of banks and presets while it is dropped open
    menu: Option<Menu>,
    /// The name being typed on the preset display's tape for a new preset, after Save
    saving: Option<String>,
    /// What the preset display says instead of the name, and until when
    note: Option<(&'static str, f64)>,
    /// What the cabinet showed when the window was last drawn
    cab_status: CabStatus,
    /// The Audio panel while it is open (the standalone program only)
    audio: Option<AudioChoice>,
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
    /// Has the chosen cabinet read in the background
    load_cabinet: &'a dyn Fn(),
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
        match led_stepper(ui, origin + vec2(BANK_X, PRESET_Y), BANK_CHARS, &bank_shown, "bank") {
            Some(Pick::Open) => {
                view.saving = None;
                self.toggle_menu(view, &bank);
            }
            Some(Pick::Step(step)) => {
                view.saving = None;
                let banks = user_presets::banks(user_presets::presets_dir().as_deref());
                let next = user_presets::neighbour_bank(&banks, &bank, step == Step::Next);
                self.show_bank(&next, view);
                view.note = None;
            }
            None => {}
        }

        // A name typed for a new preset is taken before the tape is drawn, so the letter is
        // on it in the same frame. Escape drops the name, and the sound is not saved
        let typed = view.saving.as_mut().and_then(|name| {
            type_on_tape(
                ui,
                origin + vec2(PRESET_X, PRESET_Y),
                PRESET_WIDTH,
                name,
                NEW_PRESET_CHARS,
            )
        });
        match typed {
            Some(Typed::Done) => self.finish_save(&dir, factory, &user, view, now),
            Some(Typed::Cancelled) => view.saving = None,
            None => {}
        }

        let shown = match (&view.saving, view.note) {
            (Some(name), _) => {
                let skip = name.chars().count().saturating_sub(NEW_PRESET_SHOWN);
                format!("{}_", name.chars().skip(skip).collect::<String>())
            }
            (None, Some((note, until))) if now < until => note.to_owned(),
            (None, _) => {
                view.note = None;
                self.shown_name(factory, &user, &view.bank)
            }
        };
        match led_stepper(ui, origin + vec2(PRESET_X, PRESET_Y), PRESET_CHARS, &shown, "preset") {
            // The tape clicked while a name is typed keeps it as it stands
            Some(Pick::Open) if view.saving.is_some() => {
                self.finish_save(&dir, factory, &user, view, now);
            }
            Some(Pick::Open) => self.toggle_menu(view, &bank),
            Some(Pick::Step(step)) => {
                view.saving = None;
                view.note = None;
                self.step(step == Step::Next, &bank, factory, &user, view, now);
            }
            None => {}
        }

        silk_label(ui.painter(), origin + vec2(SAVE_X, PRESET_CAPTION_Y), "Save", 13.0);
        if chrome_button(ui, origin + vec2(SAVE_X, PRESET_Y), "save-preset").clicked() {
            if view.saving.is_some() {
                // Pressed again, the name stands as it is: the mouse alone is enough
                self.finish_save(&dir, factory, &user, view, now);
            } else {
                // Every save asks for a name on the tape, which starts as the name of the
                // preset loaded out of this bank, so enter keeps it and writes over it;
                // nothing is written until the name is kept
                let loaded = dir.is_some().then_some(user.as_str()).filter(|user| !user.is_empty());
                view.saving = Some(user_presets::name_to_keep(loaded, NEW_PRESET_CHARS));
                view.note = None;
            }
        }
        // A note is only up for a while: draw once more when it is over
        if view.note.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(SAVE_NOTE_S));
        }
    }

    /// Keeps the name that was typed for a new preset and saves the sound under it
    fn finish_save(
        &self,
        dir: &Option<std::path::PathBuf>,
        factory: usize,
        user: &str,
        view: &mut View,
        now: f64,
    ) {
        let name = view.saving.take().unwrap_or_default();
        // The name of the preset loaded out of this bank, kept as it is, writes over it
        let loaded = dir.is_some().then_some(user).filter(|user| !user.is_empty());
        let named = (!user_presets::keeps_name(loaded, &name, NEW_PRESET_CHARS)).then_some(name);
        self.finish_save_as(dir, factory, user, view, now, named.as_deref());
    }

    /// Saves the sound into the bank showing and says how it went. `named` is the name typed
    /// for a new preset; none is the preset loaded being written over
    fn finish_save_as(
        &self,
        dir: &Option<std::path::PathBuf>,
        factory: usize,
        user: &str,
        view: &mut View,
        now: f64,
        named: Option<&str>,
    ) {
        // Nothing is written into the factory bank: a sound saved there goes into the
        // presets folder, which is the bank showing from then on
        let into = dir.clone().or_else(user_presets::presets_dir);
        let saved = into.as_deref().is_some_and(|into| self.save(into, factory, user, named, view));
        if saved && dir.is_none() {
            self.set_bank(user_presets::MINE);
        }
        view.note = Some((if saved { "SAVED" } else { "NOT SAVED" }, now + SAVE_NOTE_S));
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
            (self.load_cabinet)();
        }
        self.set_user_preset(file);
    }

    /// Writes the sound as it stands: over the player's preset loaded last, or, as a
    /// new one, under the name typed, or after the preset it started as with none. Returns whether it was written
    fn save(
        &self,
        dir: &Path,
        factory: usize,
        user: &str,
        named: Option<&str>,
        view: &mut View,
    ) -> bool {
        view.bank = Bank::read(self.params, Some(dir));
        let file =
            user_presets::save_file_name(named, user, PRESETS[factory].name, &view.bank.files);
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
            ..Row::default()
        })
        .collect();
    rows.push(match naming {
        // The name as it is typed, with the caret after it. That row takes the keyboard
        Some(name) => Row { text: format!("{name}_"), lit: true, dim: false, typing: true },
        None => Row { text: NEW_BANK_ROW.to_owned(), ..Row::default() },
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
                ..Row::default()
            })
            .collect(),
        Some(files) => files
            .iter()
            .map(|file| Row {
                text: user_presets::display_name(file, DROP_PRESET_CHARS),
                lit: loaded == Loaded::File(file),
                ..Row::default()
            })
            .collect(),
    };
    if rows.is_empty() {
        rows.push(Row { text: "No presets".to_owned(), dim: true, ..Row::default() });
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
        View {
            lamps: LampView::new(&lamp_reading),
            bank: Bank::default(),
            menu: None,
            saving: None,
            note: None,
            cab_status: CabStatus::Fine,
            audio: None,
        },
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
            let load_cabinet = || async_executor.execute_background(Task::LoadCabinet);
            let shared = Shared {
                tuner: &tuner,
                lamp_reading: &lamp_reading,
                cab_loader: &cab_loader,
                load_cabinet: &load_cabinet,
                audio_host: standalone::host(),
            };
            draw(egui_ctx, setter, &params, view, &shared);
        },
    )
}

/// What the editor reads besides the parameters, shared with the audio and the plugin's
/// background tasks
struct Shared<'a> {
    tuner: &'a TunerReading,
    lamp_reading: &'a LampReading,
    cab_loader: &'a CabLoader,
    /// Has the chosen cabinet read in the background
    load_cabinet: &'a dyn Fn(),
    /// The machine's audio, in the standalone program; a plugin in a DAW has none
    audio_host: Option<&'a dyn standalone::Host>,
}

/// The whole window, once the fonts are in: what the plugin shows and its snapshot tests draw
fn draw(
    ctx: &Context,
    setter: &ParamSetter,
    params: &GuitarAmpParams,
    view: &mut View,
    shared: &Shared,
) {
    ui::rig(ctx, |ui, window| {
        // The update runs with every frame of the window whether it is painted
        // or not, so this costs two atomic reads per frame, and a repaint only
        // when a lamp changes
        let now = ui.input(|input| input.time);
        if view.lamps.look(shared.lamp_reading, params.gate_on.value(), now) {
            ui.ctx().request_repaint();
        }
        // A cabinet file is read in the background, and the cabinet shows whether it
        // could be used
        let cab_status = shared.cab_loader.status();
        if cab_status != view.cab_status {
            view.cab_status = cab_status;
            ui.ctx().request_repaint();
        }
        let (gate_open, clipping) = view.lamps.shown();
        let [input_db, output_db] = view.lamps.levels();

        let strip = Rect::from_min_size(
            window + vec2(BENCH_MARGIN, BENCH_MARGIN),
            vec2(WINDOW_WIDTH as f32 - 2.0 * BENCH_MARGIN, STRIP_HEIGHT),
        );
        ui::strip(ui, strip, &STYLE);
        // What is on the strip is placed from here
        let origin = window;

        let presets = PresetDisplay {
            params,
            setter,
            cab_loader: shared.cab_loader,
            load_cabinet: shared.load_cabinet,
        };
        let tuning = params.tuner_on.value();
        if tuning {
            // The tuner takes the place of the preset display, list and all
            view.menu = None;
            // The note the tuner hears and how far off it is, where the preset was
            let note = shared.tuner.hz().map(Note::nearest);
            let name = note.map(|note| format!("{}{}", note.name, note.octave));
            silk_label(ui.painter(), origin + vec2(PRESET_X, PRESET_CAPTION_Y), "Tuner", 13.0);
            tuner_display(
                ui.painter(),
                origin + vec2(PRESET_X, PRESET_Y),
                PRESET_CHARS,
                name.as_deref(),
                note.map(|note| note.cents),
            );
            // What it hears changes without anyone touching the window
            ui.ctx().request_repaint();
        } else {
            presets.draw(ui, origin, view, now);
        }

        param_knob(
            ui,
            setter,
            &params.out_level,
            origin + OUTPUT_KNOB,
            OUTPUT_RADIUS,
            "Output",
            None,
        );
        for (place, level, caption) in
            [(INPUT_METER, input_db, "Input"), (OUTPUT_METER, output_db, "Output")]
        {
            level_meter(ui.painter(), origin + place, METER_HEIGHT, level);
            silk_label(ui.painter(), origin + place + METER_CAPTION, caption, LAMP_CAPTION_SIZE);
        }
        signal_lamp(ui.painter(), origin + CLIP_LAMP, clipping);
        silk_label(ui.painter(), origin + CLIP_LAMP + LAMP_CAPTION, "Clip", LAMP_CAPTION_SIZE);

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
            params,
            setter,
            gate_open,
            cab_loader: shared.cab_loader,
            load_cabinet: shared.load_cabinet,
        };
        crate::board::board(ui, window, &pedals, &params.order);

        // The preset list after everything else, in a layer above the window
        presets.menu(ui, origin, view, now);

        // The standalone program has the machine's audio to set
        if let Some(host) = shared.audio_host {
            audio_button(ui, origin, view, host);
            audio_panel(ui, origin, view, host);
        }
    });
}

// The Audio button of the standalone program, in the gap between the output meter and the clip
// lamp, with its caption under it
const AUDIO_BUTTON: Vec2 =
    vec2((OUTPUT_METER.x + METER_REACH + CLIP_LAMP.x - LAMP_REACH) / 2.0, CLIP_LAMP.y - 4.0);
const AUDIO_CAPTION_Y: f32 = 26.0;
// Where the Audio panel stands: in the middle of the window, over the stack
const AUDIO_PANEL: Vec2 = vec2(WINDOW_WIDTH as f32 / 2.0, 330.0);

fn audio_button(ui: &mut Ui, origin: Pos2, view: &mut View, host: &dyn standalone::Host) {
    let at = origin + AUDIO_BUTTON;
    silk_label(ui.painter(), at + vec2(0.0, AUDIO_CAPTION_Y), "Audio", CAPTION_SIZE);
    if chrome_button(ui, at, "audio").clicked() {
        view.audio = match view.audio {
            Some(_) => None,
            None => Some(AudioChoice::open(host, standalone::launch())),
        };
    }
}

fn audio_panel(ui: &mut Ui, origin: Pos2, view: &mut View, host: &dyn standalone::Host) {
    let Some(choice) = view.audio.as_mut() else {
        return;
    };
    let rows = choice.rows();
    let shown: Vec<SettingRow> =
        rows.iter().map(|(caption, shown)| SettingRow { caption, shown }).collect();
    let note = choice.error.as_deref().unwrap_or("Apply starts the amp again on the new settings");
    let text = SettingsText { title: "Audio Settings", note, apply: "Apply", cancel: "Cancel" };
    let event = settings_panel(ui, origin + AUDIO_PANEL, &text, &shown, "audio");
    match event {
        Some(SettingsEvent::Step(row, step)) => {
            choice.step(host, row, step == Step::Next);
        }
        Some(SettingsEvent::Apply) => match host.restart(&choice.prefs) {
            // The new program is on its way: this window is done
            Ok(()) => ui.ctx().send_viewport_cmd(nih_plug_egui::egui::ViewportCommand::Close),
            Err(error) => choice.error = Some(format!("Could not restart: {error}")),
        },
        Some(SettingsEvent::Cancel) => view.audio = None,
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    use suite_common::ui::snapshot::{pedal_harness, Harness, NoHost};

    // The picture on the face is compiled in: a build without it is a build of other sources
    #[test]
    fn test_the_pedal_is_drawn_with_its_art() {
        assert!(STYLE.art == Some(ui::artwork::AMP), "the pedal has lost its art");
    }

    /// The window as a snapshot test draws it: with `params`, the tuner hearing `tuner_hz`,
    /// and `view` as it is after the window opened and `open` did what a click would. The
    /// lamps and meters are at rest, the cabinet the amp's own
    #[cfg(target_os = "linux")]
    fn harness_of<'a>(
        params: &'a GuitarAmpParams,
        setter: &'a ParamSetter<'a>,
        tuner_hz: Option<f32>,
        open: impl FnOnce(&mut View),
    ) -> Harness<'a> {
        harness_with(params, setter, tuner_hz, None, open)
    }

    /// `harness_of` in the standalone program, if there is a `host` for the machine's audio
    #[cfg(target_os = "linux")]
    fn harness_with<'a>(
        params: &'a GuitarAmpParams,
        setter: &'a ParamSetter<'a>,
        tuner_hz: Option<f32>,
        audio_host: Option<&'a dyn standalone::Host>,
        open: impl FnOnce(&mut View),
    ) -> Harness<'a> {
        let tuner = TunerReading::new();
        tuner.set(tuner_hz);
        let lamp_reading = LampReading::new();
        let cab_loader = CabLoader::new(Arc::new(crate::cab_stage::CabStage::new()), 48_000.0);
        let mut view = View {
            lamps: LampView::new(&lamp_reading),
            bank: Bank::read(params, None),
            menu: None,
            saving: None,
            note: None,
            cab_status: CabStatus::Fine,
            audio: None,
        };
        open(&mut view);
        pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, move |ctx| {
            let shared = Shared {
                tuner: &tuner,
                lamp_reading: &lamp_reading,
                cab_loader: &cab_loader,
                load_cabinet: &|| {},
                audio_host,
            };
            draw(ctx, setter, params, &mut view, &shared);
        })
    }

    // What the editor draws in its window, every parameter at its default, against the
    // picture in tests/snapshots/editor.png. On a change it leaves editor.new.png and
    // editor.diff.png there to look at (see suite_common::ui::snapshot)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_editor_looks_as_it_did() {
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        harness_of(&params, &setter, None, |_| {}).snapshot("editor");
    }

    /// A machine with two interfaces, for the standalone window's tests
    #[cfg(target_os = "linux")]
    struct TwoInterfaces;
    #[cfg(target_os = "linux")]
    impl standalone::Host for TwoInterfaces {
        fn drivers(&self) -> Vec<String> {
            ["auto", "alsa", "jack"].map(String::from).to_vec()
        }
        fn devices(&self, _: &str) -> standalone::DriverDevices {
            standalone::DriverDevices {
                inputs: vec!["Scarlett 2i2 USB".into(), "Built-in Microphone".into()],
                outputs: vec!["Scarlett 2i2 USB".into(), "Speakers".into()],
            }
        }
        fn restart(&self, _: &Prefs) -> std::io::Result<()> {
            Ok(())
        }
    }

    // The standalone program's window: the Audio button in the strip's corner, and the panel
    // open over the stack. A plugin in a DAW (the other snapshots) has neither
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_standalone_audio_panel_looks_as_it_did() {
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let machine = TwoInterfaces;
        let prefs = Prefs {
            backend: "alsa".into(),
            input_device: Some("Scarlett 2i2 USB".into()),
            ..Prefs::default()
        };
        harness_with(&params, &setter, None, Some(&machine), |view| {
            view.audio = Some(AudioChoice::open(&machine, prefs));
        })
        .snapshot("audio_panel");
    }

    // The tuner in the place of the preset display, hearing a low E a little flat
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_tuner_looks_as_it_did() {
        let params = GuitarAmpParams {
            tuner_on: BoolParam::new("Tuner", true).non_automatable(),
            ..GuitarAmpParams::default()
        };
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        harness_of(&params, &setter, Some(81.0), |_| {}).snapshot("tuner");
    }

    // The preset list dropped open on the factory bank. The presets folder is one that is
    // not there, so the banks are the two every player has, whatever is on this machine
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_preset_menu_looks_as_it_did() {
        let nowhere = std::env::temp_dir().join("hojt-snapshot-no-presets");
        std::env::set_var(user_presets::FOLDER_ENV, &nowhere);
        assert!(!nowhere.exists(), "{nowhere:?} is there: the list would show its banks");
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let open = |view: &mut View| {
            view.menu = Some(Menu {
                listed: user_presets::FACTORY.to_owned(),
                files: Vec::new(),
                naming: None,
                opened: true,
            });
        };
        harness_of(&params, &setter, None, open).snapshot("preset_menu");
    }

    /// How far a stepper's buttons stand out from the middle of its tape
    fn reach(width: f32) -> f32 {
        width / 2.0 + 22.0
    }

    /// The bottom edge of a stepper's tape, from the top of the window
    fn tape_bottom(y: f32) -> f32 {
        y + 11.0
    }

    /// The bottom edge of an LED display's window, from the top of the window
    fn display_bottom(y: f32) -> f32 {
        y + LED_HEIGHT / 2.0
    }

    // A stepper's button and a small footswitch, half across; how far a knob's ticks reach
    // out from its rim, and how far a lamp of a meter or a signal lamp reaches from its middle
    const BUTTON: f32 = 11.0;
    const SWITCH: f32 = 25.0;
    const TICKS: f32 = 13.0;
    const LAMP: f32 = 11.0;

    /// From left to right, the room each control of a wing takes across, from the window's
    /// left edge: each one must end before the next begins
    fn in_a_row(wing: &[(&str, f32, f32)]) {
        for pair in wing.windows(2) {
            let ((a, _, a_right), (b, b_left, _)) = (pair[0], pair[1]);
            assert!(a_right + 4.0 < b_left, "{a} runs into {b}");
        }
    }

    /// What each control of the strip takes across, from the window's left edge: the left end
    /// and the right end, in the order of the strip
    fn strip_controls() -> Vec<(&'static str, f32, f32)> {
        let display = |x: f32, width: f32| (x - reach(width) - BUTTON, x + reach(width) + BUTTON);
        let (bank, preset) = (display(BANK_X, BANK_WIDTH), display(PRESET_X, PRESET_WIDTH));
        let knob = OUTPUT_RADIUS + TICKS;
        vec![
            ("the input meter", INPUT_METER.x - METER_REACH, INPUT_METER.x + METER_REACH),
            ("the bank display", bank.0, bank.1),
            ("the preset display", preset.0, preset.1),
            ("Save", SAVE_X - SAVE_REACH, SAVE_X + SAVE_REACH),
            ("the output knob", OUTPUT_KNOB.x - knob, OUTPUT_KNOB.x + knob),
            ("the output meter", OUTPUT_METER.x - METER_REACH, OUTPUT_METER.x + METER_REACH),
            ("the clip lamp", CLIP_LAMP.x - LAMP_REACH, CLIP_LAMP.x + LAMP_REACH),
            ("On", BYPASS_X + CAPTION_OFFSET - 28.0, BYPASS_X + SWITCH),
            ("Tuner", TUNER_X + CAPTION_OFFSET - 28.0, TUNER_X + SWITCH),
        ]
    }

    #[test]
    fn test_the_strip_controls_stand_clear_of_each_other_and_of_its_edges() {
        let width = WINDOW_WIDTH as f32;
        let mut row = vec![("the strip's edge", 0.0, BENCH_MARGIN)];
        row.extend(strip_controls());
        row.push(("the strip's edge", width - BENCH_MARGIN, width));
        in_a_row(&row);
        // The tuner's display takes the place of the preset display, so it is the same size
        let tuner = (PRESET_X - PRESET_WIDTH / 2.0, PRESET_X + PRESET_WIDTH / 2.0);
        let preset = strip_controls()[2];
        assert!(preset.1 < tuner.0 && tuner.1 < preset.2);
        // Every one of them on the strip, from its top edge to the top of its lower edge
        let strip = BENCH_MARGIN..=BENCH_MARGIN + STRIP_HEIGHT - 5.0;
        let knob = OUTPUT_RADIUS + TICKS;
        for y in [
            PRESET_CAPTION_Y - 7.0,
            display_bottom(PRESET_Y),
            OUTPUT_KNOB.y - knob,
            INPUT_METER.y - METER_HEIGHT / 2.0,
            INPUT_METER.y + METER_CAPTION.y + 7.0,
            SWITCH_Y + SWITCH,
        ] {
            assert!(strip.contains(&y), "{y} is off the strip");
        }
    }

    // The standalone program's Audio button and its caption stand between the output meter
    // and the clip lamp, clear of both
    #[test]
    fn test_the_audio_button_stands_clear_of_the_meter_and_the_clip_lamp() {
        let reach = 18.0;
        assert!(AUDIO_BUTTON.x - reach > OUTPUT_METER.x + METER_REACH);
        assert!(AUDIO_BUTTON.x + reach < CLIP_LAMP.x - LAMP_REACH);
        const { assert!(AUDIO_BUTTON.y + AUDIO_CAPTION_Y + 10.0 < STRIP_HEIGHT) };
    }

    #[test]
    fn test_the_strip_is_used_evenly_with_no_hole_in_the_middle() {
        let controls = strip_controls();
        let gaps: Vec<f32> = controls.windows(2).map(|pair| pair[1].1 - pair[0].2).collect();
        let (least, most) =
            gaps.iter().fold((f32::MAX, 0.0_f32), |(l, m), &g| (l.min(g), m.max(g)));
        assert!(most - least < 12.0, "the gaps are not even: {gaps:?}");
        // The ends are about as far from the strip's edges as the controls are from each other.
        // The right end keeps a little more room: the logo stands there
        let (left, right) =
            (controls[0].1 - BENCH_MARGIN, WINDOW_WIDTH as f32 - BENCH_MARGIN - controls[8].2);
        assert!((0.0..30.0).contains(&(right - left)), "{left} on the left, {right} on the right");
        assert!(left < most && most < 160.0, "{left} {most}");
    }

    #[test]
    fn test_the_list_drops_out_under_its_display() {
        assert!(DROP_Y > display_bottom(PRESET_Y), "the list covers the display it drops from");
        assert!(BANK_X + reach(BANK_WIDTH) < PRESET_X - reach(PRESET_WIDTH));
    }

    /// The control plate of every cabinet's look, from the cabinet's top left corner
    fn cab_plates() -> impl Iterator<Item = Rect> {
        let cab = Rect::from_min_size(Pos2::ZERO, vec2(STACK_WIDTH, PEDAL_HEIGHT));
        let files = ["", "room.wav"].map(str::to_owned);
        let speakers = (0..crate::amp::model::SPEAKERS.len()).map(user_cab::speaker_choice);
        let choices: Vec<String> = files.into_iter().chain(speakers).collect();
        Amp::ALL.into_iter().flat_map(move |amp| {
            let choices = choices.clone();
            choices.into_iter().map(move |chosen| {
                ui::cab_faces(cab, looks::cab_look(amp, &chosen, CabStatus::Fine)).plate
            })
        })
    }

    #[test]
    fn test_every_cab_has_its_plate_in_one_place() {
        let mut plates = cab_plates();
        let first = plates.next().unwrap();
        assert!(plates.all(|plate| plate == first));
    }

    /// Where the cabinet's controls go
    fn plate() -> Rect {
        cab_plates().next().unwrap()
    }

    #[test]
    fn test_the_delays_sync_button_stands_clear_of_the_knobs_and_on_the_face() {
        let knob = TRIO_RADIUS + TICKS;
        let mix = TRIO_KNOBS[2];
        // Across, right of the Mix knob's scale and its tape, inside the face
        for (name, y) in
            [("caption", SYNC_CAPTION.y), ("LED", SYNC_LED.y), ("button", SYNC_BUTTON.y)]
        {
            if (y - mix.y).abs() < knob + 4.0 {
                in_a_row(&[
                    ("the Mix knob", mix.x - knob, mix.x + knob),
                    (name, SYNC_LED.x - 20.0, SYNC_LED.x + 20.0),
                    ("the face's edge", PEDAL_WIDTH / 2.0, PEDAL_WIDTH),
                ]);
            }
        }
        in_a_row(&[
            ("the Mix knob's tape", mix.x - 24.0, mix.x + 24.0),
            ("the Sync button", SYNC_BUTTON.x - BUTTON, SYNC_BUTTON.x + BUTTON),
            ("the face's edge", PEDAL_WIDTH / 2.0, PEDAL_WIDTH),
        ]);
        assert_eq!(SYNC_CAPTION.x, SYNC_BUTTON.x);
        assert_eq!(SYNC_LED.x, SYNC_BUTTON.x);
        // From top to bottom: the Repeat knob's tape, the caption, the LED, the button, and
        // the On switch's place
        let repeat_tape_bottom = TRIO_KNOBS[1].y + TRIO_RADIUS + 18.0 + 15.0 + 2.0 + 16.0;
        in_a_row(&[
            ("the Repeat knob's tape", 0.0, repeat_tape_bottom),
            ("the Sync caption", SYNC_CAPTION.y - 7.0, SYNC_CAPTION.y + 7.0),
            ("the Sync LED", SYNC_LED.y - LAMP, SYNC_LED.y + LAMP),
            ("the Sync button", SYNC_BUTTON.y - BUTTON, SYNC_BUTTON.y + BUTTON + 2.5),
            ("the On switch", PEDAL_SWITCH.y - SWITCH, PEDAL_SWITCH.y + SWITCH),
        ]);
    }

    #[test]
    fn test_the_cab_plate_controls_stand_clear_of_their_neighbours() {
        let plate = plate();
        let display =
            |at: Vec2, width: f32| (at.x - reach(width) - BUTTON, at.x + reach(width) + BUTTON);
        let knob = CAB_KNOB_RADIUS + TICKS;
        for stepper in
            [display(SPEAKER_STEPPER, SPEAKER_WIDTH), display(MIC_STEPPER, MIC_STEPPER_WIDTH)]
        {
            in_a_row(&[
                ("the plate's edge", 0.0, plate.left()),
                ("a stepper", stepper.0, stepper.1),
                ("the Mic knob", CAB_KNOBS[0].x - knob, CAB_KNOBS[0].x + knob),
                ("the Res knob", CAB_KNOBS[1].x - knob, CAB_KNOBS[1].x + knob),
                ("the Cab switch", CAB_SWITCH.x - SWITCH, CAB_SWITCH.x + SWITCH),
                ("the plate's edge", plate.right(), STACK_WIDTH),
            ]);
        }
        // The speaker's tape over the microphone's caption, and both on the plate
        assert!(tape_bottom(SPEAKER_STEPPER.y) + 4.0 < MIC_CAPTION.y - 7.0);
        assert!(tape_bottom(MIC_STEPPER.y) < plate.bottom());
        assert!(CAB_SWITCH.y + SWITCH < plate.bottom());
        // From top to bottom, the switch's LED over the switch
        in_a_row(&[
            ("the Cab LED", CAB_LED.y - LAMP, CAB_LED.y + LAMP),
            ("the Cab switch", CAB_SWITCH.y - SWITCH, CAB_SWITCH.y + SWITCH),
        ]);
        // The output jack under the plate, at the height of the pedals' plugs
        assert!(CAB_JACK.y - 8.0 > plate.bottom());
        assert!((CAB_JACK.y - ui::PLUG_Y).abs() < 20.0);
        // On its own plate with its caption, clear of the plate's screws, on the cabinet
        let cab = Rect::from_min_size(Pos2::ZERO, vec2(STACK_WIDTH, PEDAL_HEIGHT));
        assert!(cab.contains_rect(CAB_JACK_PLATE));
        assert!(CAB_JACK_PLATE.top() > plate.bottom());
        let screws = CAB_JACK_PLATE.shrink(7.0);
        let caption = Rect::from_center_size(CAB_JACK_CAPTION.to_pos2(), vec2(24.0, 14.0));
        let jack = Rect::from_center_size(CAB_JACK.to_pos2(), Vec2::splat(2.0 * JACK));
        for (name, rect) in [("the jack", jack), ("its caption", caption)] {
            assert!(CAB_JACK_PLATE.contains_rect(rect), "{name} is off its plate");
            assert!(screws.left() + 2.0 < rect.left() && rect.right() < screws.right() - 2.0);
        }
    }

    /// The faces of every amp's head, as the window has them
    fn heads() -> impl Iterator<Item = (Amp, Rect, HeadFaces)> {
        let head = Rect::from_min_size(Pos2::new(0.0, STACK_TOP), vec2(STACK_WIDTH, HEAD_HEIGHT));
        Amp::ALL
            .into_iter()
            .map(move |amp| (amp, head, ui::head_faces(head, &looks::look(amp).head)))
    }

    // The jack's nut, and the pilot lamp in its bezel, half across
    const JACK: f32 = 8.0;
    const PILOT: f32 = 9.0;

    #[test]
    fn test_the_head_panel_holds_the_jack_the_dials_and_the_lamp_in_a_row() {
        for (amp, _, faces) in heads() {
            let places = HeadPlaces::new(faces);
            let jewel = faces.panel.right() - ui::amp::JEWEL_INSET;
            let mut row = vec![
                ("the panel's edge", 0.0, faces.panel.left()),
                ("the jack", places.jack.x - JACK, places.jack.x + JACK),
            ];
            for (dial, label) in places.dials.iter().zip(DIALS) {
                let room = ui::panel_knob_rect(*dial, DIAL_RADIUS);
                // Its scale, caption and tape all on the panel
                assert!(faces.panel.contains_rect(room), "{amp:?}: {label} is off the panel");
                row.push((label, room.left(), room.right()));
            }
            row.push(("the pilot lamp", jewel - PILOT, jewel + PILOT));
            row.push(("the panel's edge", faces.panel.right(), STACK_WIDTH));
            in_a_row(&row);
            // The jack's caption over the jack, on the panel
            assert!(places.jack_caption.y + 7.0 < places.jack.y - JACK);
            assert!(places.jack_caption.y - 7.0 > faces.panel.top(), "{amp:?}");
        }
        assert_eq!(PEDAL_TOP + PEDAL_HEIGHT + BENCH_MARGIN, WINDOW_HEIGHT as f32);
    }

    #[test]
    fn test_the_nameplate_and_its_buttons_sit_on_the_grille_clear_of_the_handle() {
        for (amp, head, faces) in heads() {
            let places = HeadPlaces::new(faces);
            let plate = Rect::from_center_size(
                places.nameplate,
                vec2(NAMEPLATE_WIDTH, ui::widgets::NAMEPLATE_HEIGHT),
            );
            assert!(faces.grille.contains_rect(plate), "{amp:?}: {plate:?}");
            let reach = reach(NAMEPLATE_WIDTH) - 2.0;
            let buttons = Rect::from_center_size(
                places.nameplate,
                vec2(2.0 * (reach + BUTTON), 2.0 * BUTTON),
            );
            // Inside the grille, away from its edge and the piping round it
            assert!(faces.grille.shrink(4.0).contains_rect(buttons), "{amp:?}: {buttons:?}");
            // The handle stands on the box, over the grille
            assert!(buttons.top() > head.top() + ui::amp::HANDLE_HEIGHT + 4.0, "{amp:?}");
        }
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
        for expected in
            [MicType::Ribbon, MicType::Condenser, MicType::Close, MicType::Air, MicType::Dynamic]
        {
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
