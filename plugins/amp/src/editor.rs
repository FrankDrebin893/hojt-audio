use nih_plug::prelude::*;
use nih_plug_egui::egui::{
    emath::TSTransform, vec2, Color32, Context, Id, LayerId, Order, Pos2, Rect, Sense, Ui,
    UiBuilder, Vec2, ViewportCommand,
};
use nih_plug_egui::resizable_window::ResizableWindow;
use nih_plug_egui::{create_egui_editor, EguiState};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, PoisonError};
use suite_common::ui::{
    self, chrome_button, drop_panel_with, led_display, led_stepper, led_width, nameplate_stepper,
    panel_knob, param_knob, param_switch, print_label, rig_led, signal_lamp, silk_frame,
    silk_label, small_footswitch, stepper, theme, tuner_display, type_into, type_on_tape, Beat,
    Column, MiniStyle, Motif, Ornament, PedalStyle, Pick, Plate, Row, Step, Typed, BENCH_MARGIN,
    LED_HEIGHT, SLIM_JACK_X, SLIM_KNOBS, SLIM_KNOB_RADIUS, SLIM_LED, SLIM_PRINT_REACH,
    SLIM_STRIP_HEIGHT, SLIM_SWITCH,
};

use crate::board::{Line, STACK_WIDTH};
use crate::cab::MicType;
use crate::cab_stage::{CabLoader, CabStatus, NO_FILES_SHOWN};
use crate::dial_memory;
use crate::lamps::{LampReading, LampView};
use crate::looks;
use crate::metronome::{BeatReading, ClickOut, Sound, TapTempo, TimeSignature, BPM_MAX, BPM_MIN};
use crate::order::Zone;
use crate::presets::{self, PRESETS};
use crate::slots::{Block, Chain, EffectKind, Live, SlotParams};
#[cfg(all(test, target_os = "linux"))]
use crate::standalone::Prefs;
use crate::standalone::{self, AudioChoice};
use crate::tuner::{Note, TunerReading};
use crate::user_cab;
use crate::user_presets::{self, Bank};
use crate::wah::WahMode;
use crate::{Amp, GuitarAmpParams, GuitarAmpPlugin, Task};
use suite_common::update::UpdateCheck;

/// The width of the line's design layer for the chain a project had before it could be built:
/// the gate, the twelve pedals and the stack. The strip's places are made at this width and move
/// with the layer's (`Fit`); the line's own width follows its chain (`board::Line::width`)
const WINDOW_WIDTH: u32 = 3411;
const WINDOW_HEIGHT: u32 = (STACK_TOP + HEAD_HEIGHT + PEDAL_HEIGHT + BENCH_MARGIN) as u32;

// The line is drawn in a layer of its own width and `WINDOW_HEIGHT` high, and the layer is
// scaled onto the window by a zoom that is what fits (`Fit`), from `MIN_ZOOM` to `MAX_ZOOM`. A
// line too long to fit at `MIN_ZOOM` is scrolled sideways, with the strip staying where it is.
// A window opens sized to the screen (up to the full 3411 x 584, or 1876 x 321 on a 1920 x 1080
// screen) and is dragged by its corner to any size from `MIN_WIDTH` x `MIN_HEIGHT` up
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 1.0;
const DEFAULT_ZOOM: f32 = 0.55;
const DEFAULT_WIDTH: u32 = (WINDOW_WIDTH as f32 * DEFAULT_ZOOM).round() as u32;
/// The narrowest the strip may be: what the controls on it need to stand clear of each other.
/// It stretches with the window from there, and so does the layer
const STRIP_MIN_WIDTH: f32 = 2600.0;
/// How far the scroll bar stands in from the window's edges
const SCROLL_INSET: f32 = 40.0;
const MIN_WIDTH: u32 = (STRIP_MIN_WIDTH * MIN_ZOOM) as u32 + 1;
const MIN_HEIGHT: u32 = (WINDOW_HEIGHT as f32 * MIN_ZOOM) as u32 + 1;
const FALLBACK_SCREEN_WIDTH: u32 = 1920;
const SCREEN_MARGIN: u32 = FALLBACK_SCREEN_WIDTH - DEFAULT_WIDTH;
const RESIZE_ID: &str = "amp-resize";

// The strip across the top, and under it the line: the pedals, and among them the stack, the
// head standing on the cabinet. The cabinet is as tall as the pedals and on the floor with them
const STRIP_HEIGHT: f32 = SLIM_STRIP_HEIGHT;
pub(crate) const STACK_TOP: f32 = BENCH_MARGIN + STRIP_HEIGHT + BENCH_MARGIN;
const HEAD_HEIGHT: f32 = 180.0;
pub(crate) const PEDAL_TOP: f32 = STACK_TOP + HEAD_HEIGHT;
// The pedals and the stack keep the size they had under the tall strip: the slimmer strip made the
// window lower, not them taller
pub(crate) const PEDAL_HEIGHT: f32 = 310.0;
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

// The strip is slim (`SLIM_STRIP_HEIGHT`), and what is on it stands on one line, `STRIP_Y` down
// from the window's top, in three groups. From the left: the input (its jack, its meter with the
// Input gain beside it) and the preset display, which opens the list of banks and presets as a
// plate under it. In the middle the icons of the metronome, the tuner and the settings, the first
// two with a lamp and a switch. From the right: the logo with the model number, the Clip lamp,
// and the output (its meter with the Output knob before it). What is printed by the strip itself
// (`ui::slim_strip`: the jacks with their captions, the model number, the logo) is at its ends
const STRIP_Y: f32 = BENCH_MARGIN + 30.0;
const STRIP_FACE_RIGHT: f32 = WINDOW_WIDTH as f32 - BENCH_MARGIN;

// A level meter lies down on the strip: its caption over it, the knob of its gain beside it
const METER_WIDTH: f32 = 260.0;
const METER_Y: f32 = STRIP_Y + 6.0;
const METER_CAPTION_Y: f32 = BENCH_MARGIN + 15.0;
const METER_CAPTION_SIZE: f32 = 13.0;
// The knobs of the strip are small and have no caption of their own: the meter's caption names
// both. Their scale reaches `KNOB_TICKS` past the rim
const STRIP_KNOB_RADIUS: f32 = 14.0;
const KNOB_TICKS: f32 = 13.0;
const KNOB_REACH: f32 = STRIP_KNOB_RADIUS + KNOB_TICKS;
const METER_REACH: f32 = METER_WIDTH / 2.0;
const STRIP_GAP: f32 = 16.0;

// The input: the meter right of the jack and the Input gain right of the meter
const INPUT_METER: Vec2 = vec2(BENCH_MARGIN + SLIM_JACK_X + 3.0 * STRIP_GAP + METER_REACH, METER_Y);
const INPUT_KNOB: Vec2 = vec2(INPUT_METER.x + METER_REACH + STRIP_GAP + KNOB_REACH, STRIP_Y);

// The preset display: the bank and the name of the preset in one window, with a marker at its
// right end that says it opens. Wide enough for the longest bank and name. A click on it opens
// the plate of banks and presets. While the tuner is on, its display is where this one is
const PRESET_CHARS: usize = 26;
const PRESET_WIDTH: f32 = led_width(PRESET_CHARS);
const PRESET_X: f32 = INPUT_KNOB.x + KNOB_REACH + 2.0 * STRIP_GAP + PRESET_WIDTH / 2.0;
// The longest name of a player's preset the display shows, with room for the edited mark
const USER_NAME_CHARS: usize = 15;
// The bank's name in front of it, cut to this many letters
const BANK_SHOWN_CHARS: usize = 9;
// How long the display says what Save did
const SAVE_NOTE_S: f64 = 1.5;

// The icons stand in the middle of the strip, a pitch apart. The two with a switch have the lamp
// and the switch to the right of the icon
const ICON_PITCH: f32 = 240.0;
const PRESET_END: f32 = PRESET_X + PRESET_WIDTH / 2.0;
const ICONS_SPAN: f32 = 2.0 * ICON_PITCH + 2.0 * ui::icons::ICON_REACH;
// The icons stand in the room between the preset display and the output knob, an equal room each side
const METRONOME_X: f32 =
    (PRESET_END + OUTPUT_KNOB.x - KNOB_REACH) / 2.0 - ICONS_SPAN / 2.0 + ui::icons::ICON_REACH;
const TUNER_X: f32 = METRONOME_X + ICON_PITCH;
const SETTINGS_X: f32 = TUNER_X + ICON_PITCH;

// The update line (`ui::update_line`) stands on the strip in the room between the settings icon
// and the output knob, which is the widest empty room on it, and moves with both ends of it
const UPDATE_ROOM_LEFT: f32 = SETTINGS_X + ui::icons::ICON_REACH;
const UPDATE_ROOM_RIGHT: f32 = OUTPUT_KNOB.x - KNOB_REACH;

/// Where the update line is centred on the strip, from the window's top left corner, for a
/// strip whose right end and middle have moved by `shift` and half of it
fn update_line_at(shift: f32) -> Vec2 {
    let (left, right) = (UPDATE_ROOM_LEFT + shift / 2.0, UPDATE_ROOM_RIGHT + shift);
    vec2((left + right) / 2.0, STRIP_Y)
}

// The output: the knob, the meter and the Clip lamp, lit while the output is rounded off by the
// safety clip, with its caption under it. The model number and the logo are right of them
const MODEL_LEFT: f32 = STRIP_FACE_RIGHT - SLIM_PRINT_REACH;
const CLIP_LAMP_REACH: f32 = 20.0;
const CLIP_LAMP: Vec2 = vec2(MODEL_LEFT - 4.0 * STRIP_GAP - CLIP_LAMP_REACH, METER_Y);
const CLIP_CAPTION: Vec2 = vec2(0.0, METER_CAPTION_Y - METER_Y);
const LAMP_CAPTION: Vec2 = vec2(0.0, 18.0);
const LAMP_CAPTION_SIZE: f32 = 13.0;
const OUTPUT_METER: Vec2 =
    vec2(CLIP_LAMP.x - CLIP_LAMP_REACH - 3.0 * STRIP_GAP - METER_REACH, METER_Y);
const OUTPUT_KNOB: Vec2 = vec2(OUTPUT_METER.x - METER_REACH - STRIP_GAP - KNOB_REACH, STRIP_Y);

// The plates open under the strip: the presets' under the display, the metronome's under its
// icon, the settings' under theirs
const PLATE_TOP: f32 = BENCH_MARGIN + SLIM_STRIP_HEIGHT + 4.0;
// The list the preset plate holds: the banks in the first column and the presets of the bank that
// is listed in the second
const BANK_COLUMN: usize = 0;
const DROP_BANK_WIDTH: f32 = 132.0;
const DROP_PRESET_WIDTH: f32 = 186.0;
const DROP_BANK_CHARS: usize = 12;
const DROP_PRESET_CHARS: usize = 17;
// The row at the end of the banks that makes one, and how long a name typed on it may be
const NEW_BANK_ROW: &str = "+ New Bank";
const NEW_BANK_CHARS: usize = 14;
// The foot of the preset plate: the name display, where a new preset is named, and Save beside it
const FOOT_HEIGHT: f32 = 46.0;
const FOOT_CHARS: usize = 18;
const FOOT_SAVE_REACH: f32 = 25.0;
const FOOT_BUTTON_SPREAD: f32 = 40.0;
// How long a name typed for a new preset may be, and how much of it the display has room for
// (the caret takes one place)
const NEW_PRESET_CHARS: usize = 24;
const NEW_PRESET_SHOWN: usize = FOOT_CHARS - 1;

// The metronome's plate: the tempo display (drag or scroll it, or tap the Tap button), the sound
// over the time signature and the click's output, each on a stepper, the click's volume, and the
// Sync button. Every one is placed from the end of the one before it, from the plate's top left
// corner
const METRONOME_PAD: f32 = 26.0;
const METRONOME_GAP: f32 = 38.0;
const SAVE_REACH: f32 = 25.0;
const STEPPER_REACH: f32 = 33.0;
const TEMPO_CHARS: usize = 3;
const TEMPO_WIDTH: f32 = led_width(TEMPO_CHARS);
const TEMPO_X: f32 = METRONOME_PAD + TEMPO_WIDTH / 2.0 + STEPPER_REACH;
const TAP_X: f32 = TEMPO_X + TEMPO_WIDTH / 2.0 + STEPPER_REACH + METRONOME_GAP + SAVE_REACH;
// The sound over the time signature, on one column of steppers
const SOUND_WIDTH: f32 = 100.0;
const SIGNATURE_WIDTH: f32 = 54.0;
// and under the sound, side by side, the time signature and where the click is heard
const OUT_WIDTH: f32 = 70.0;
const LOWER_GAP: f32 = 40.0;
const METER_STEP_REACH: f32 = SIGNATURE_WIDTH / 2.0 + STEPPER_REACH;
const OUT_REACH: f32 = OUT_WIDTH / 2.0 + STEPPER_REACH;
// The column is as wide as its lower row, or as the sound's stepper if that is wider
const COLUMN_WIDTH: f32 = {
    let lower = 2.0 * METER_STEP_REACH + LOWER_GAP + 2.0 * OUT_REACH;
    let upper = SOUND_WIDTH + 2.0 * STEPPER_REACH;
    if lower > upper {
        lower
    } else {
        upper
    }
};
const STEPPERS_X: f32 = TAP_X + SAVE_REACH + METRONOME_GAP + COLUMN_WIDTH / 2.0;
const SIGNATURE_X: f32 = STEPPERS_X - COLUMN_WIDTH / 2.0 + METER_STEP_REACH;
const CLICK_OUT_X: f32 = STEPPERS_X + COLUMN_WIDTH / 2.0 - OUT_REACH;
// Rows, from the plate's top: the title is on the first, then the captions over each control
const TEMPO_CAPTION_Y: f32 = 52.0;
const TEMPO_Y: f32 = 86.0;
const SOUND_CAPTION_Y: f32 = 52.0;
const SOUND_Y: f32 = 71.0;
const SIGNATURE_CAPTION_Y: f32 = 99.0;
const SIGNATURE_Y: f32 = 123.0;
const CLICK_RADIUS: f32 = 20.0;
const CLICK_REACH: f32 = CLICK_RADIUS + 13.0;
const CLICK_VOLUME: Vec2 =
    vec2(STEPPERS_X + COLUMN_WIDTH / 2.0 + METRONOME_GAP + CLICK_REACH, 71.0);
const METRONOME_SYNC_X: f32 = CLICK_VOLUME.x + CLICK_REACH + METRONOME_GAP + SAVE_REACH;
const METRONOME_END: f32 = METRONOME_SYNC_X + SAVE_REACH;
// The Sync button stands lowest, under its LED and caption
const SYNC_STACK: [f32; 3] = [TEMPO_CAPTION_Y, 78.0, 110.0];
const METRONOME_WIDTH: f32 = METRONOME_END + METRONOME_PAD;
const METRONOME_HEIGHT: f32 = 162.0;
// How long the lamp stays lit on a beat, and how often the window is drawn anew to see it
const BEAT_LIT_S: f64 = 0.09;
const BEAT_POLL_S: f64 = 0.02;
// A turn of the tempo display: this many points of drag are one beat per minute
const TEMPO_DRAG_POINTS: f32 = 3.0;

// The settings plate: a row to the cabinet files and, in the standalone program, a row to each
// of the audio's choices, with Apply and Cancel under them
const SETTINGS_PAD: f32 = 16.0;
const SETTINGS_CAPTION_WIDTH: f32 = 130.0;
const SETTINGS_TAPE_WIDTH: f32 = 330.0;
const SETTINGS_ROW_HEIGHT: f32 = 34.0;
const SETTINGS_NOTE_HEIGHT: f32 = 26.0;
const SETTINGS_FOOT_HEIGHT: f32 = 58.0;
const SETTINGS_BUTTON_SPREAD: f32 = 110.0;
// A stepper reaches its buttons this far past its tape on each side
const SETTINGS_OVERHANG: f32 = 31.0;
// The longest cabinet name the settings' tape has room for
const SETTINGS_CAB_CHARS: usize = 26;
const SETTINGS_WIDTH: f32 =
    2.0 * SETTINGS_PAD + SETTINGS_CAPTION_WIDTH + SETTINGS_TAPE_WIDTH + 2.0 * SETTINGS_OVERHANG;

// The wah's Toe, Heel and Rocker captions, and the rocker's value on its tape
const ROCKER_END_SIZE: f32 = 12.0;
const ROCKER_TAPE_SIZE: f32 = 11.0;
const LANE_CAPTION_SIZE: f32 = 15.0;

// On the head, which looks as its amp does (`looks`): the nameplate on the grille, the amp's
// name between two buttons that step to the next amp, and the panel under it. On the panel,
// from its left edge, the input jack the cable from the pedals goes into, the amp's six dials
// in an even row, and at its right end the pilot lamp, lit while the amp is on
const NAMEPLATE_WIDTH: f32 = 140.0;
// The middle of the jack, the first dial and the line of the dials, from the panel's top left
// corner; captions are on one line above them
const DIAL_Y: f32 = 52.0;
const DIALS: [&str; 6] = ["Gain", "Bass", "Mid", "Treble", "Presence", "Master"];
// The stack is two lanes, one per amp, one above the other: a head with the cabinet beside it,
// that play together. Each lane's head looks as its amp (`looks`), its cabinet as the cabinet
// that plays through it (`looks::cab_look`; no plate: the lane's own choice is on the head's
// lower line). The lanes end at a black plate where the signal meets, with the Blend fader
// between the two, and what both share: Mic, Res, the microphone, the cabinets' switch and the
// one output jack. Positions are from the stack's top left corner
const LANE_GAP: f32 = 18.0;
const LANE_HEIGHT: f32 = (HEAD_HEIGHT + PEDAL_HEIGHT - LANE_GAP) / 2.0;
/// The stack's height: both lanes and the gap between them
pub(crate) const STACK_HEIGHT: f32 = HEAD_HEIGHT + PEDAL_HEIGHT;
const LANE_HEAD_WIDTH: f32 = 420.0;
const LANE_CAB_WIDTH: f32 = 120.0;
const LANE_CAB_GAP: f32 = 6.0;
// The plate starts after the cabinets, with room for a coupler between
const MIX_GAP: f32 = 12.0;
const MIX_X: f32 = LANE_HEAD_WIDTH + LANE_CAB_WIDTH + LANE_CAB_GAP + MIX_GAP - LANE_CAB_GAP;
const MIX_WIDTH: f32 = STACK_WIDTH - MIX_X;
// On the plate, from its top, in three zones: the Blend frame (the fader with its end captions
// and its value beside the slot, to the left of the middle), the Cabinets frame (Mic and Res,
// Mic Type, and the Cab switch with its LED, all of them for both cabinets), and under them the
// output jack. Across, from the plate's middle
const PLATE_FRAME_INSET: f32 = 10.0;
const BLEND_FRAME_TOP: f32 = 14.0;
const BLEND_FRAME_BOTTOM: f32 = 192.0;
const FADER_DX: f32 = -12.0;
const MIX_AMP1_CAPTION_Y: f32 = 40.0;
const FADER_TOP: f32 = 66.0;
const FADER_LENGTH: f32 = 80.0;
const MIX_AMP2_CAPTION_Y: f32 = 176.0;
const MIX_END_CAPTION_SIZE: f32 = 15.0;
const MIX_BLEND_TAPE_DX: f32 = 38.0;
const MIX_BLEND_TAPE_Y: f32 = FADER_TOP + FADER_LENGTH / 2.0 - 8.0;
const CABS_FRAME_TOP: f32 = 206.0;
const CABS_FRAME_BOTTOM: f32 = 444.0;
const MIX_KNOB_Y: f32 = 251.0;
const MIX_KNOB_DX: f32 = 31.0;
const MIX_KNOB_RADIUS: f32 = 15.0;
const MIC_CAPTION_Y: f32 = 331.0;
const MIC_STEPPER_Y: f32 = 355.0;
const MIC_STEPPER_WIDTH: f32 = 44.0;
const MIX_SWITCH_CAPTION_Y: f32 = 386.0;
const MIX_LED_Y: f32 = 410.0;
const MIX_LED_DX: f32 = -30.0;
const MIX_SWITCH_Y: f32 = 407.0;
const MIX_SWITCH_DX: f32 = 22.0;
const MIX_JACK_Y: f32 = 462.0;
const MIX_JACK_DX: f32 = 16.0;
const MIX_JACK_CAPTION_DX: f32 = -24.0;

// On a lane's head, from the panel's top left corner: the dials in a row, the lane's name,
// its cabinet's stepper and, on the second lane, its switch, on the line under the dials
const LANE_JACK_X: f32 = 16.0;
const LANE_DIAL_FIRST_X: f32 = 54.0;
const LANE_DIAL_SPACING: f32 = 55.0;
const LANE_DIAL_RADIUS: f32 = 15.0;
const LANE_LINE_DY: f32 = 72.0;
const LANE_NAME_X: f32 = 48.0;
const LANE_CAB_X: f32 = 170.0;
const LANE_CAB_CAPTION_DY: f32 = -24.0;
const LANE_CAB_STEPPER_WIDTH: f32 = 90.0;
// From the panel's right edge
const LANE_ON_CAPTION_DX: f32 = -116.0;
const LANE_ON_LED_DX: f32 = -88.0;
const LANE_ON_SWITCH_DX: f32 = -46.0;
const CAB_NAME_CHARS: usize = 10;
const JACK_RADIUS: f32 = 8.0;

// The blocks of the line, all as wide as each other: the gate, drive, delay and reverb with a trio
// or a pair of knobs side by side, six with two knobs on a diagonal (`is_slim`); the pitch shifter
// is a trio, the EQ has four knobs in two rows and the wah the rocker

// On every pedal, from its top centre: the LED under the title, the knobs, the switch at the bottom.
// Two knobs sit side by side. Three do not fit in a row, so they are smaller and in a triangle,
// with the LED between the upper two
const PEDAL_LED: Vec2 = vec2(0.0, 71.0);
const PEDAL_SWITCH: Vec2 = vec2(0.0, 273.0);
const PAIR_RADIUS: f32 = 24.0;
const PAIR_KNOBS: [Vec2; 2] = [vec2(-45.0, 144.0), vec2(45.0, 144.0)];
const TRIO_RADIUS: f32 = 17.0;
// The EQ's four: Low and High, the shelves, in the left column, and Mid with its Freq under it
// in the right one, the LED between the upper two as on a trio
const QUAD_KNOBS: [Vec2; 4] =
    [vec2(-45.0, 68.0), vec2(45.0, 68.0), vec2(45.0, 174.0), vec2(-45.0, 174.0)];
// Smaller than a trio's, so the two rows' ticks and tapes leave room between them and for the switch
const QUAD_RADIUS: f32 = 15.0;
// The wah, a pedal of its own kind: the rocker's tall slot in the middle (`top` is the middle of
// its upper end, toe at the top), Bite over Sens in a column on the left, and on the right the
// LED under the title, the rocker's value on tape under its name and the Auto push button with
// its lamp above and its caption under it. Toe and Heel are printed at the slot's ends
const ROCKER_TOP: Vec2 = vec2(0.0, 80.0);
const ROCKER_LENGTH: f32 = 116.0;
const ROCKER_TOE: Vec2 = vec2(0.0, 50.0);
const ROCKER_HEEL: Vec2 = vec2(0.0, 224.0);
const WAH_KNOBS: [Vec2; 2] = [vec2(-62.0, 80.0), vec2(-62.0, 176.0)];
const WAH_KNOB_RADIUS: f32 = 14.0;
const WAH_LED: Vec2 = vec2(62.0, 76.0);
const ROCKER_CAPTION: Vec2 = vec2(62.0, 98.0);
const ROCKER_TAPE: Vec2 = vec2(62.0, 110.0);
const AUTO_LED: Vec2 = vec2(56.0, 232.0);
const AUTO_BUTTON: Vec2 = vec2(56.0, 262.0);
const AUTO_CAPTION: Vec2 = vec2(56.0, 287.0);
const TRIO_KNOBS: [Vec2; 3] = [vec2(-45.0, 71.0), vec2(45.0, 71.0), vec2(0.0, 169.0)];

// The Gate pedal has a lamp beside its LED, under the title, that is lit while the gate lets the
// guitar through. The pedal's LED says that the gate is switched on, like every other
const GATE_LAMP: Vec2 = vec2(58.0, 71.0);

// While the Delay follows the song's tempo (Sync), its Time knob, the first, picks the note
// value in the same place, and its tape shows the note
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

/// Where a block's values are kept: in its pedal's own parameters (the gate that is always
/// first, and each of the twelve pedals on its own slot: what hosts, old projects and the audio
/// read as they always did) or in the parameters of the slot it stands on, whose knobs mean what
/// the same knob of the pedal means, at the same position
#[derive(Clone, Copy)]
pub(crate) enum Src<'a> {
    Own,
    Slot(&'a SlotParams, u8),
}

impl Src<'_> {
    /// What tells this source's widgets from another's of the same kind
    fn key(self) -> String {
        match self {
            Src::Own => "own".to_owned(),
            Src::Slot(_, slot) => slot.to_string(),
        }
    }
}

/// Title, paint and art of a pedal: each has a paint and a piece of northern weather of its own
const fn look(title: &'static str, paint: Color32, motif: Motif) -> MiniStyle {
    MiniStyle { title, paint, motif }
}

/// The look of the pedal of `kind`
pub(crate) fn kind_look(kind: EffectKind) -> MiniStyle {
    match kind {
        EffectKind::Gate => look("Gate", theme::PAINT_GRANITE, Motif::Sluice),
        EffectKind::Drive => look("Drive", theme::PAINT_OCHRE, Motif::Bonfire),
        EffectKind::Delay => look("Delay", theme::PAINT_PETROL, Motif::Ridges),
        EffectKind::Reverb => look("Reverb", theme::PAINT_FJORD, Motif::Fjord),
        EffectKind::Comp => look("Comp", theme::PAINT_MOSS, Motif::Isobars),
        EffectKind::Chorus => look("Chorus", theme::PAINT_SEA, Motif::Swell),
        EffectKind::Phaser => look("Phaser", theme::PAINT_VIOLET, Motif::Moons),
        EffectKind::Flanger => look("Flanger", theme::PAINT_AURORA, Motif::Aurora),
        EffectKind::Tremolo => look("Tremolo", theme::PAINT_HEATHER, Motif::Rain),
        EffectKind::Fuzz => look("Fuzz", theme::PAINT_EMBER, Motif::Geyser),
        EffectKind::Pitch => look("Pitch", theme::PAINT_ORANGE, Motif::Horizon),
        EffectKind::Eq => look("EQ", theme::PAINT_BARK, Motif::Columns),
        EffectKind::Wah => look("Wah", theme::PAINT_BRASS, Motif::Foss),
    }
}

/// Whether the pedal of `kind` is a slim one: two knobs, one above the other
pub(crate) fn is_slim(kind: EffectKind) -> bool {
    matches!(
        kind,
        EffectKind::Comp
            | EffectKind::Chorus
            | EffectKind::Phaser
            | EffectKind::Flanger
            | EffectKind::Tremolo
            | EffectKind::Fuzz
    )
}

/// What the blocks of the line are drawn from
pub(crate) struct Pedals<'a> {
    params: &'a GuitarAmpParams,
    setter: &'a ParamSetter<'a>,
    gate_open: bool,
    cab_loader: &'a CabLoader,
    /// Has the chosen cabinet read in the background
    load_cabinet: &'a dyn Fn(),
}

impl Pedals<'_> {
    /// A knob of the block at `src`: its `index`th (counted from 0) knob. `own` is the pedal's
    /// own parameter for it, which a slot's knob is told in the terms of
    #[allow(clippy::too_many_arguments)]
    fn knob<P: Param>(
        &self,
        ui: &mut Ui,
        src: Src,
        index: usize,
        own: &P,
        at: Pos2,
        radius: f32,
        caption: &str,
    ) {
        match src {
            Src::Own => {
                param_knob(ui, self.setter, own, at, radius, caption, None);
            }
            Src::Slot(slot, _) => {
                let knob = slot.knobs()[index];
                ui::param_knob_as(ui, self.setter, knob, own, at, radius, caption, None);
            }
        }
    }

    /// The switch and lamp of the block at `src`
    fn on_switch(&self, ui: &mut Ui, src: Src, own: &BoolParam, switch: Pos2, led: Pos2) {
        let on = match src {
            Src::Own => own,
            Src::Slot(slot, _) => &slot.on,
        };
        param_switch(ui, self.setter, on, switch, led);
    }

    /// The pitch shifter: a trio of knobs, the first of which is a whole number of semitones
    /// and so not a `FloatParam` like the rest of them. Returns its top face
    fn pitch_pedal(&self, ui: &mut Ui, src: Src, rect: Rect) -> Rect {
        let params = self.params;
        let face = ui::mini_pedal(ui, rect, &kind_look(EffectKind::Pitch));
        let top = face.center_top();
        let [semitones, blend, tracking] = TRIO_KNOBS;
        let radius = TRIO_RADIUS;
        self.knob(ui, src, 0, &params.pitch_semitones, top + semitones, radius, "Semitones");
        self.knob(ui, src, 1, &params.pitch_blend, top + blend, radius, "Blend");
        self.knob(ui, src, 2, &params.pitch_latency, top + tracking, radius, "Tracking");
        self.on_switch(ui, src, &params.pitch_on, top + PEDAL_SWITCH, top + PEDAL_LED);
        face
    }

    /// The EQ: four knobs in two rows. Returns its top face
    fn eq_pedal(&self, ui: &mut Ui, src: Src, rect: Rect) -> Rect {
        let params = self.params;
        let face = ui::mini_pedal(ui, rect, &kind_look(EffectKind::Eq));
        let top = face.center_top();
        let [low, mid, freq, high] = QUAD_KNOBS;
        let radius = QUAD_RADIUS;
        self.knob(ui, src, 0, &params.eq_low, top + low, radius, "Low");
        self.knob(ui, src, 1, &params.eq_mid, top + mid, radius, "Mid");
        self.knob(ui, src, 2, &params.eq_freq, top + freq, radius, "Freq");
        self.knob(ui, src, 3, &params.eq_high, top + high, radius, "High");
        self.on_switch(ui, src, &params.eq_on, top + PEDAL_SWITCH, top + PEDAL_LED);
        face
    }

    /// The wah: the rocker, Bite and Sens, the Auto button and the switch. Returns its top face
    fn wah_pedal(&self, ui: &mut Ui, src: Src, rect: Rect) -> Rect {
        let (params, setter) = (self.params, self.setter);
        let face = ui::mini_pedal(ui, rect, &kind_look(EffectKind::Wah));
        let top = face.center_top();
        // A slot's knobs are: the rocker, Bite, the mode and Sens
        let auto = match src {
            Src::Own => params.wah_mode.value(),
            Src::Slot(slot, _) => params.wah_mode.preview_plain(slot.k3.value()),
        } == WahMode::Auto;

        let (rocker, shown) = match src {
            Src::Own => (&params.wah_pedal, params.wah_pedal.unmodulated_normalized_value()),
            Src::Slot(slot, _) => (&slot.k1, slot.k1.unmodulated_normalized_value()),
        };
        ui::rocker(ui, setter, rocker, top + ROCKER_TOP, ROCKER_LENGTH, theme::SILK);
        silk_label(ui.painter(), top + ROCKER_TOE, "Toe", ROCKER_END_SIZE);
        silk_label(ui.painter(), top + ROCKER_HEEL, "Heel", ROCKER_END_SIZE);
        // In Auto the dial is where the pedal rests, and says so
        silk_label(
            ui.painter(),
            top + ROCKER_CAPTION,
            if auto { "Rest" } else { "Rocker" },
            ROCKER_END_SIZE,
        );
        let value = params.wah_pedal.normalized_value_to_string(shown, true);
        ui::widgets::label_tape(ui.painter(), top + ROCKER_TAPE, &value, ROCKER_TAPE_SIZE);

        let [bite, sens] = WAH_KNOBS;
        self.knob(ui, src, 1, &params.wah_q, top + bite, WAH_KNOB_RADIUS, "Bite");
        self.knob(ui, src, 3, &params.wah_sens, top + sens, WAH_KNOB_RADIUS, "Sens");

        // The Auto push button, the Delay's Sync idiom: a lamp above it, the caption under it
        silk_label(ui.painter(), top + AUTO_CAPTION, "Auto", LAMP_CAPTION_SIZE);
        rig_led(ui.painter(), top + AUTO_LED, auto);
        if chrome_button(ui, top + AUTO_BUTTON, &format!("wah-auto-{}", src.key())).clicked() {
            let mode = if auto { WahMode::Pedal } else { WahMode::Auto };
            match src {
                Src::Own => set(setter, &params.wah_mode, mode),
                Src::Slot(slot, _) => {
                    set(setter, &slot.k3, params.wah_mode.preview_normalized(mode))
                }
            }
        }
        self.on_switch(ui, src, &params.wah_on, top + PEDAL_SWITCH, top + WAH_LED);
        face
    }

    /// The gate that is always first, on its own parameters, with the lamp that says it lets the
    /// guitar through. Returns its top face
    pub(crate) fn gate(&self, ui: &mut Ui, rect: Rect) -> Rect {
        let face = self.plain_pedal(ui, Src::Own, EffectKind::Gate, rect);
        let top = face.center_top();
        signal_lamp(ui.painter(), top + GATE_LAMP, self.gate_open);
        silk_label(ui.painter(), top + GATE_LAMP + LAMP_CAPTION, "Open", LAMP_CAPTION_SIZE);
        face
    }

    /// Draws the block in `rect`, with its values where its slot keeps them: the pedal's own
    /// parameters on its own slot, the slot's otherwise. Returns its top face
    pub(crate) fn block(&self, ui: &mut Ui, block: Block, rect: Rect) -> Rect {
        let src = if Chain::is_home(block) {
            Src::Own
        } else {
            Src::Slot(self.params.slot(block.slot as usize), block.slot)
        };
        self.plain_pedal(ui, src, block.kind, rect)
    }

    /// The chain as it stands
    pub(crate) fn chain(&self) -> Chain {
        self.params.chain.get()
    }

    /// Takes the block on `slot` out of the chain, keeping its settings (`StoredChain::take_out`)
    pub(crate) fn take_out(&self, slot: u8) -> Option<Block> {
        let live = Live { params: self.params, setter: self.setter };
        self.params.chain.take_out(&live, slot)
    }

    /// Puts a block of `kind` into the chain as number `place` of `zone`, with the settings the
    /// last one of its kind had when it was taken out. None when the chain is full
    pub(crate) fn put_in(&self, kind: EffectKind, zone: Zone, place: usize) -> Option<Block> {
        let live = Live { params: self.params, setter: self.setter };
        self.params.chain.put_in(&live, kind, zone, place).ok()
    }

    /// The pedals with a pair or a trio of knobs (and the Delay's Sync): the gate, drive,
    /// delay, reverb, and the slim ones. Returns the top face
    fn plain_pedal(&self, ui: &mut Ui, src: Src, kind: EffectKind, rect: Rect) -> Rect {
        let (params, setter) = (self.params, self.setter);
        let (on, knobs): (&BoolParam, &[(&FloatParam, &str)]) = match kind {
            EffectKind::Gate => (
                &params.gate_on,
                &[(&params.gate_thresh, "Thresh"), (&params.gate_release, "Release")],
            ),
            EffectKind::Drive => (
                &params.drive_on,
                &[
                    (&params.drive_gain, "Drive"),
                    (&params.drive_tone, "Tone"),
                    (&params.drive_level, "Level"),
                ],
            ),
            EffectKind::Delay => (
                &params.delay_on,
                &[
                    (&params.delay_time, "Time"),
                    (&params.delay_feedback, "Repeat"),
                    (&params.delay_mix, "Mix"),
                ],
            ),
            EffectKind::Reverb => {
                (&params.reverb_on, &[(&params.reverb_decay, "Decay"), (&params.reverb_mix, "Mix")])
            }
            EffectKind::Comp => (
                &params.comp_on,
                &[(&params.comp_sustain, "Sustain"), (&params.comp_level, "Level")],
            ),
            EffectKind::Chorus => (
                &params.chorus_on,
                &[(&params.chorus_rate, "Rate"), (&params.chorus_depth, "Depth")],
            ),
            EffectKind::Phaser => (
                &params.phaser_on,
                &[(&params.phaser_rate, "Rate"), (&params.phaser_depth, "Depth")],
            ),
            EffectKind::Flanger => (
                &params.flanger_on,
                &[(&params.flanger_rate, "Rate"), (&params.flanger_depth, "Depth")],
            ),
            EffectKind::Tremolo => {
                (&params.trem_on, &[(&params.trem_rate, "Rate"), (&params.trem_depth, "Depth")])
            }
            EffectKind::Fuzz => {
                (&params.fuzz_on, &[(&params.fuzz_fuzz, "Fuzz"), (&params.fuzz_tone, "Tone")])
            }
            // Pedals of their own kind
            EffectKind::Pitch => return self.pitch_pedal(ui, src, rect),
            EffectKind::Eq => return self.eq_pedal(ui, src, rect),
            EffectKind::Wah => return self.wah_pedal(ui, src, rect),
        };
        let face = ui::mini_pedal(ui, rect, &kind_look(kind));
        let top = face.center_top();
        if is_slim(kind) {
            for (index, (&(param, caption), offset)) in knobs.iter().zip(SLIM_KNOBS).enumerate() {
                self.knob(ui, src, index, param, top + offset, SLIM_KNOB_RADIUS, caption);
            }
            self.on_switch(ui, src, on, top + SLIM_SWITCH, top + SLIM_LED);
            return face;
        }

        let (radius, places) = if knobs.len() > PAIR_KNOBS.len() {
            (TRIO_RADIUS, &TRIO_KNOBS[..])
        } else {
            (PAIR_RADIUS, &PAIR_KNOBS[..])
        };
        // Whether the Delay follows the song's tempo (Sync): its Time knob, the first, then
        // picks the note value in the same place, and its tape shows the note. A slot's Sync
        // is its fourth knob, the note its fifth
        let sync = kind == EffectKind::Delay
            && match src {
                Src::Own => params.delay_sync.value(),
                Src::Slot(slot, _) => params.delay_sync.preview_plain(slot.k4.value()),
            };
        for (index, (&(param, caption), &offset)) in knobs.iter().zip(places).enumerate() {
            if sync && index == 0 {
                self.knob(ui, src, 4, &params.delay_note, top + offset, radius, caption);
            } else {
                self.knob(ui, src, index, param, top + offset, radius, caption);
            }
        }
        self.on_switch(ui, src, on, top + PEDAL_SWITCH, top + PEDAL_LED);

        if kind == EffectKind::Delay {
            silk_label(ui.painter(), top + SYNC_CAPTION, "Sync", LAMP_CAPTION_SIZE);
            rig_led(ui.painter(), top + SYNC_LED, sync);
            if chrome_button(ui, top + SYNC_BUTTON, &format!("delay-sync-{}", src.key())).clicked()
            {
                match src {
                    Src::Own => set(setter, &params.delay_sync, !sync),
                    Src::Slot(slot, _) => set(setter, &slot.k4, if sync { 0.0 } else { 1.0 }),
                }
            }
        }
        face
    }

    /// Draws the stack with its top left corner at `top_left`: two lanes, the first amp's
    /// head with its cabinet beside it and the second amp's under it, and the plate where
    /// they meet. Returns where the cables go in and out
    pub(crate) fn stack(&self, ui: &mut Ui, top_left: Pos2) -> Jacks {
        let mut jacks = [Pos2::ZERO; 2];
        let mut ends = [Pos2::ZERO; 2];
        for lane in 0..2 {
            let origin = top_left + vec2(0.0, (LANE_HEIGHT + LANE_GAP) * lane as f32);
            (jacks[lane], ends[lane]) = self.lane(ui, origin, lane == 1);
        }
        // The guitar goes into both heads: a link from the first jack to the second's, round
        // the left edge of the panels
        let bow = vec2(-70.0, 50.0);
        link_cable(ui, [jacks[1], jacks[1] + vec2(bow.x, -bow.y), jacks[0] + bow, jacks[0]]);
        for jack in jacks {
            jack_socket(ui, jack);
        }

        let plate = Rect::from_min_size(
            top_left + vec2(MIX_X, 0.0),
            vec2(MIX_WIDTH, 2.0 * LANE_HEIGHT + LANE_GAP),
        );
        ui::amp::mounted_plate(ui.painter(), plate, Plate::BlackAnodised);
        for end in ends {
            ui::coupler(ui.painter(), end.x, plate.left(), end.y);
        }
        Jacks { input: jacks[1], output: self.mix_plate(ui, plate) }
    }

    /// The first amp's cabinet choice: the amp's own, a built-in one or a file
    fn chosen(&self) -> String {
        self.params.cabinet.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// One lane at `origin`: the head of its amp with its nameplate and six dials, the cabinet
    /// it plays through beside it, and the line under the dials with its name, its cabinet's
    /// stepper and, for the second, its On switch. Returns the middle of the head's input jack
    /// and where its cabinet ends on the right
    fn lane(&self, ui: &mut Ui, origin: Pos2, second: bool) -> (Pos2, Pos2) {
        let (params, setter) = (self.params, self.setter);
        let amp = if second { params.amp2.value() } else { params.amp.value() };
        let look = looks::look(amp);
        let (panel, print) = (look.head.panel, look.head.print);
        let on = if second { params.amp2_on.value() } else { true } && !params.bypass.value();

        // The head
        let head = Rect::from_min_size(origin, vec2(LANE_HEAD_WIDTH, LANE_HEIGHT));
        let faces = ui::amp_head(ui, head, &look.head, on);
        let corner = faces.panel.left_top();
        let dials = std::array::from_fn::<Pos2, 6, _>(|index| {
            corner + vec2(LANE_DIAL_FIRST_X + LANE_DIAL_SPACING * index as f32, DIAL_Y)
        });
        let id = if second { "amp2" } else { "amp" };
        if let Some(step) = nameplate_stepper(
            ui,
            faces.grille.center(),
            NAMEPLATE_WIDTH,
            panel,
            print,
            amp.model().name,
            id,
        ) {
            if second {
                dial_memory::switch_second_amp(params, setter, neighbour_amp(amp, step));
            } else {
                dial_memory::switch_amp(params, setter, neighbour_amp(amp, step));
            }
        }
        let own_dials = if second { params.amp2_dials() } else { params.amp_dials() };
        for ((param, label), at) in own_dials.into_iter().zip(DIALS).zip(dials) {
            panel_knob(ui, setter, param, at, LANE_DIAL_RADIUS, label, look.knobs, print);
        }
        let jack = corner + vec2(LANE_JACK_X, DIAL_Y);
        let caption = ui::panel_caption(dials[0], LANE_DIAL_RADIUS).y;
        print_label(ui.painter(), Pos2::new(jack.x, caption), "In", LAMP_CAPTION_SIZE, print);

        // The line under the dials
        let line = corner.y + DIAL_Y + LANE_LINE_DY;
        print_label(
            ui.painter(),
            Pos2::new(corner.x + LANE_NAME_X, line),
            if second { "Amp 2" } else { "Amp 1" },
            18.0,
            print,
        );
        let cab_at = Pos2::new(corner.x + LANE_CAB_X, line);
        print_label(
            ui.painter(),
            cab_at + vec2(0.0, LANE_CAB_CAPTION_DY),
            "Cabinet",
            LANE_CAPTION_SIZE,
            print,
        );
        if second {
            let right = faces.panel.right();
            print_label(
                ui.painter(),
                Pos2::new(right + LANE_ON_CAPTION_DX, line),
                "On",
                15.0,
                print,
            );
            rig_led(ui.painter(), Pos2::new(right + LANE_ON_LED_DX, line), params.amp2_on.value());
            if small_footswitch(ui, Pos2::new(right + LANE_ON_SWITCH_DX, line), "amp2_on").clicked()
            {
                let now = params.amp2_on.value();
                setter.begin_set_parameter(&params.amp2_on);
                setter.set_parameter(&params.amp2_on, !now);
                setter.end_set_parameter(&params.amp2_on);
            }
        }

        // The cabinet it plays through
        let chosen = if second { params.cabinet_second.choice() } else { self.chosen() };
        let status = if second { CabStatus::Fine } else { self.cab_loader.status() };
        let mut style = *looks::cab_look(amp, &chosen, status);
        style.plate_height = 0.0;
        let cab = Rect::from_min_size(
            origin + vec2(LANE_HEAD_WIDTH + LANE_CAB_GAP, 0.0),
            vec2(LANE_CAB_WIDTH, LANE_HEIGHT),
        );
        ui::cabinet(ui, cab, &style);

        // Its stepper: the amp's own, the built-in ones and, for the first, the files too
        if second {
            let shown = match params.cabinet_second.get() {
                Some(speaker) => crate::amp::model::SPEAKERS[speaker].name.to_owned(),
                None => "OWN".to_owned(),
            };
            if let Some(step) = stepper(ui, cab_at, LANE_CAB_STEPPER_WIDTH, &shown, "cabinet2") {
                params.cabinet_second.step(step == Step::Next);
            }
        } else {
            // Where the files go: shown when the caption is hovered, and opened by a
            // double-click on it
            if let Some(dir) = user_cab::cabinets_dir() {
                let caption = Rect::from_center_size(
                    cab_at + vec2(0.0, LANE_CAB_CAPTION_DY),
                    vec2(70.0, 18.0),
                );
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
            let shown = self.cab_loader.display_name(&chosen, CAB_NAME_CHARS);
            if let Some(step) = stepper(ui, cab_at, LANE_CAB_STEPPER_WIDTH, &shown, "cabinet") {
                step_cabinet(params, self.cab_loader, self.load_cabinet, step == Step::Next);
            }
        }
        (jack, cab.right_center())
    }

    /// The plate where the lanes meet: the Blend fader between the two amps, then what both
    /// share. Returns the output jack
    fn mix_plate(&self, ui: &mut Ui, plate: Rect) -> Pos2 {
        let (params, setter) = (self.params, self.setter);
        let x = plate.center().x;
        let at = |dy: f32| Pos2::new(x, plate.top() + dy);
        let frame = |top: f32, bottom: f32| {
            Rect::from_min_max(
                Pos2::new(plate.left() + PLATE_FRAME_INSET, plate.top() + top),
                Pos2::new(plate.right() - PLATE_FRAME_INSET, plate.top() + bottom),
            )
        };
        let paint = Plate::BlackAnodised.color();

        // Blend, the plate's lead: its frame, the fader and the value on a tape beside it
        silk_frame(ui.painter(), frame(BLEND_FRAME_TOP, BLEND_FRAME_BOTTOM), "Blend", paint);
        let fader = at(0.0) + vec2(FADER_DX, 0.0);
        let end = |ui: &Ui, dy: f32, caption: &str| {
            silk_label(ui.painter(), fader + vec2(0.0, dy), caption, MIX_END_CAPTION_SIZE);
        };
        end(ui, MIX_AMP1_CAPTION_Y, "Amp 1");
        ui::crossfader(
            ui,
            setter,
            &params.amp_blend,
            fader + vec2(0.0, FADER_TOP),
            FADER_LENGTH,
            theme::SILK,
        );
        end(ui, MIX_AMP2_CAPTION_Y, "Amp 2");
        let blend = params.amp_blend.unmodulated_normalized_value();
        ui::widgets::label_tape(
            ui.painter(),
            at(MIX_BLEND_TAPE_Y) + vec2(MIX_BLEND_TAPE_DX, 0.0),
            &params.amp_blend.normalized_value_to_string(blend, true),
            12.0,
        );

        // What both cabinets share
        silk_frame(ui.painter(), frame(CABS_FRAME_TOP, CABS_FRAME_BOTTOM), "Cabinets", paint);
        for ((param, caption), dx) in [(&params.cab_mic, "Mic"), (&params.cab_res, "Res")]
            .into_iter()
            .zip([-MIX_KNOB_DX, MIX_KNOB_DX])
        {
            param_knob(
                ui,
                setter,
                param,
                at(MIX_KNOB_Y) + vec2(dx, 0.0),
                MIX_KNOB_RADIUS,
                caption,
                None,
            );
        }
        let mic_shown = params.cab_mic_type.value();
        silk_label(ui.painter(), at(MIC_CAPTION_Y), "Mic Type", LAMP_CAPTION_SIZE);
        if let Some(step) =
            stepper(ui, at(MIC_STEPPER_Y), MIC_STEPPER_WIDTH, mic_shown.short_name(), "mic")
        {
            setter.begin_set_parameter(&params.cab_mic_type);
            setter.set_parameter(&params.cab_mic_type, neighbour_mic(mic_shown, step));
            setter.end_set_parameter(&params.cab_mic_type);
        }
        silk_label(
            ui.painter(),
            at(MIX_SWITCH_CAPTION_Y) + vec2(MIX_LED_DX, 0.0),
            "Cabs",
            LAMP_CAPTION_SIZE,
        );
        param_switch(
            ui,
            setter,
            &params.cab_on,
            at(MIX_SWITCH_Y) + vec2(MIX_SWITCH_DX, 0.0),
            at(MIX_LED_Y) + vec2(MIX_LED_DX, 0.0),
        );

        // The output: the plate's own jack, under the frames
        let out = at(MIX_JACK_Y) + vec2(MIX_JACK_DX, 0.0);
        jack_socket(ui, out);
        silk_label(ui.painter(), out + vec2(MIX_JACK_CAPTION_DX, 0.0), "Out", MIX_END_CAPTION_SIZE);
        out
    }
}

/// A jack socket: the nut a patch cable's plug goes into
fn jack_socket(ui: &Ui, at: Pos2) {
    use ui::theme::{CHROME, CHROME_DARK};
    ui.painter().circle_filled(at, JACK_RADIUS, CHROME_DARK);
    ui.painter().circle_filled(at, JACK_RADIUS - 2.0, CHROME);
    ui.painter().circle_filled(at, JACK_RADIUS - 4.0, CHROME_DARK);
}

/// The cable between the two heads' input jacks, with its shadow
fn link_cable(ui: &Ui, points: [Pos2; 4]) {
    use nih_plug_egui::egui::{epaint::CubicBezierShape, Stroke};
    let shadow = points.map(|point| point + vec2(2.0, 3.0));
    for (points, color) in [(shadow, Color32::from_black_alpha(90)), (points, theme::CABLE)] {
        ui.painter().add(CubicBezierShape::from_points_stroke(
            points,
            false,
            Color32::TRANSPARENT,
            Stroke::new(5.0_f32, color),
        ));
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
    /// the window opened, and with every pick and save
    bank: Bank,
    /// The plate open under the strip, if one is. It is one field, so there are never two
    plate: Option<Open>,
    /// The name being typed on the preset plate's foot for a new preset, after Save
    saving: Option<String>,
    /// What the preset display says instead of the name, and until when
    note: Option<(&'static str, f64)>,
    /// What the cabinet showed when the window was last drawn
    cab_status: CabStatus,
    /// The metronome's lamp, and the taps on its Tap button
    beat: BeatView,
    /// How far the line is scrolled sideways, in points of the layer
    scroll: f32,
    /// The effects bank, rising from below the line while it is open. One surface is open at a
    /// time, a plate or the sheet
    effects: Option<crate::bank::Sheet>,
}

/// A plate open under the strip: configuration is a plate that opens from an icon or a display,
/// and one is open at a time
pub(crate) enum Open {
    /// The list of banks and presets, with Save, under the preset display
    Presets(Menu),
    /// The metronome's settings, under its icon
    Metronome,
    /// The settings, under their icon: the cabinet files and, in the standalone program, the
    /// machine's audio, which is the choice held here while the plate is open
    Settings(Option<AudioChoice>),
}

/// Which plate: what each icon and the preset display opens
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Plates {
    Presets,
    Metronome,
    Settings,
}

impl Open {
    fn which(&self) -> Plates {
        match self {
            Open::Presets(_) => Plates::Presets,
            Open::Metronome => Plates::Metronome,
            Open::Settings(_) => Plates::Settings,
        }
    }
}

impl View {
    /// Whether the plate `which` is the one that is open
    fn is_open(&self, which: Plates) -> bool {
        self.plate.as_ref().is_some_and(|open| open.which() == which)
    }

    /// Closes the plate that is open, if one is: the audio choice of the settings is let go (a test
    /// tone it started stops), and a name typed for a preset is dropped
    fn close_plate(&mut self, host: Option<&dyn standalone::Host>) {
        if let (Some(Open::Settings(Some(mut choice))), Some(host)) = (self.plate.take(), host) {
            choice.close(host);
        }
        self.saving = None;
    }

    /// What a click on the icon or display of `which` does: closes its plate if it is the one
    /// open, otherwise closes whichever is open and opens it, made by `open`
    fn toggle_plate(
        &mut self,
        which: Plates,
        host: Option<&dyn standalone::Host>,
        open: impl FnOnce() -> Open,
    ) {
        let was_open = self.is_open(which);
        self.close_plate(host);
        if !was_open {
            self.plate = Some(open());
            self.effects = None;
        }
    }
}

/// What the metronome's lamp shows: the last beat the audio counted that the window has seen
#[derive(Default)]
struct BeatView {
    seen: u32,
    lit_until: f64,
    downbeat: bool,
    taps: TapTempo,
}

/// The list of banks and presets while it is dropped open
pub(crate) struct Menu {
    /// The bank whose presets it lists: the one showing, until another is clicked in it
    listed: String,
    /// The file names in `listed`, as its folder was when it was listed. Empty for the
    /// factory bank, whose presets are built in
    files: Vec<String>,
    /// The name being typed for a new bank
    naming: Option<String>,
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

    /// What the preset display says: the bank and the preset loaded last (marked once something was
    /// moved away from it), or for a moment what Save did
    fn shown(&self, view: &mut View, now: f64) -> String {
        match view.note {
            Some((note, until)) if now < until => note.to_owned(),
            _ => {
                view.note = None;
                let factory = presets::index_or_first(self.params.preset.load(Ordering::Relaxed));
                let user = self.user_preset();
                let bank = user_presets::bank_name(&self.bank(), BANK_SHOWN_CHARS);
                format!("{bank} \u{b7} {}", self.shown_name(factory, &user, &view.bank))
            }
        }
    }

    /// The preset display on the strip, with a marker at its end that says it opens: a click on it
    /// opens the plate of banks and presets under it, or closes it. There are no arrows: the plate
    /// is where a preset is picked
    fn display(
        &self,
        ui: &mut Ui,
        origin: Pos2,
        view: &mut View,
        now: f64,
        host: Option<&dyn standalone::Host>,
    ) {
        let at = origin + vec2(PRESET_X, STRIP_Y);
        // The marker takes the place of the last two characters
        let shown: String = self.shown(view, now).chars().take(PRESET_CHARS - 2).collect();
        let window = Rect::from_center_size(at, vec2(PRESET_WIDTH, LED_HEIGHT));
        let response = ui
            .interact(window, Id::new("preset-display"), Sense::click())
            .on_hover_text("Banks and presets");
        led_display(ui.painter(), at, PRESET_CHARS, &shown);
        let open = view.is_open(Plates::Presets);
        if response.hovered() || open {
            ui.painter().rect_filled(window, 6.0, Color32::from_white_alpha(14));
        }
        // The marker: a small triangle, pointing down while it is shut and up while it is open
        let tip = if open { -1.0 } else { 1.0 };
        let mark = at + vec2(PRESET_WIDTH / 2.0 - 20.0, 0.0);
        ui.painter().add(nih_plug_egui::egui::Shape::convex_polygon(
            vec![
                mark + vec2(-5.0, -2.5 * tip),
                mark + vec2(5.0, -2.5 * tip),
                mark + vec2(0.0, 3.5 * tip),
            ],
            theme::TAPE_TEXT,
            nih_plug_egui::egui::Stroke::NONE,
        ));
        if response.clicked() {
            let bank = self.bank();
            view.toggle_plate(Plates::Presets, host, || {
                Open::Presets(Menu {
                    listed: bank.clone(),
                    files: self.bank_files(&bank),
                    naming: None,
                })
            });
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

    /// Starts naming a new preset on the plate's foot, which starts as the name of the preset
    /// loaded out of this bank, so enter keeps it and writes over it; nothing is written until the
    /// name is kept. With a name being typed already, it is kept as it stands: the mouse alone
    /// is enough
    fn save_pressed(
        &self,
        dir: &Option<std::path::PathBuf>,
        factory: usize,
        user: &str,
        view: &mut View,
        now: f64,
    ) {
        if view.saving.is_some() {
            self.finish_save(dir, factory, user, view, now);
        } else {
            let loaded = dir.is_some().then_some(user).filter(|user| !user.is_empty());
            view.saving = Some(user_presets::name_to_keep(loaded, NEW_PRESET_CHARS));
            view.note = None;
        }
    }

    /// The plate under the preset display: the banks in the first column, the presets of the bank
    /// that is listed in the second, and under them the foot, where a preset is named and saved.
    /// It is drawn after everything else in the window, in a layer above it, so nothing behind it
    /// answers a click while it is open
    fn plate(&self, ui: &mut Ui, origin: Pos2, view: &mut View, now: f64) {
        let mut menu = match view.plate.take() {
            Some(Open::Presets(menu)) => menu,
            other => {
                view.plate = other;
                return;
            }
        };
        let params = self.params;
        let presets_dir = user_presets::presets_dir();
        let banks = user_presets::banks(presets_dir.as_deref());

        // What is typed for a new bank is taken before the list is drawn, so the letter is
        // on the tape in the same frame. Escape ends the naming, not the plate
        let typed = menu.naming.as_mut().and_then(|name| type_into(ui, name, NEW_BANK_CHARS));
        match typed {
            Some(Typed::Done) => self.make_bank(&mut menu, presets_dir.as_deref(), view, now, ui),
            Some(Typed::Cancelled) => menu.naming = None,
            None => {}
        }

        let showing = self.bank();
        let user = self.user_preset();
        let factory = presets::index_or_first(params.preset.load(Ordering::Relaxed));
        let dir = user_presets::bank_dir(&showing);
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
        let anchor = Rect::from_center_size(
            origin + vec2(PRESET_X, STRIP_Y),
            vec2(PRESET_WIDTH, LED_HEIGHT),
        );
        let top = origin + vec2(PRESET_X, PLATE_TOP);
        // What is typed on the foot is taken in it, and whether it ended a name is kept
        let save_typed = std::cell::Cell::new(None);
        let picked =
            drop_panel_with(ui, top, &columns, "presets", anchor, FOOT_HEIGHT, |ui, foot| {
                let width = led_width(FOOT_CHARS);
                let name = Pos2::new(foot.left() + width / 2.0, foot.top() + 20.0);
                // A name being typed is taken before the display is drawn, so the letter is on it in
                // the same frame. Escape drops the name, and the sound is not saved
                if let Some(typing) = view.saving.as_mut() {
                    save_typed.set(type_on_tape(ui, name, width, typing, NEW_PRESET_CHARS));
                }
                match save_typed.get() {
                    Some(Typed::Done) => self.finish_save(&dir, factory, &user, view, now),
                    Some(Typed::Cancelled) => view.saving = None,
                    None => {}
                }
                let shown = match (&view.saving, view.note) {
                    (Some(typing), _) => {
                        let skip = typing.chars().count().saturating_sub(NEW_PRESET_SHOWN);
                        format!("{}_", typing.chars().skip(skip).collect::<String>())
                    }
                    (None, Some((note, until))) if now < until => note.to_owned(),
                    (None, _) => {
                        view.note = None;
                        self.shown_name(factory, &user, &view.bank)
                    }
                };
                let window = Rect::from_center_size(name, vec2(width, LED_HEIGHT));
                let click = ui.interact(window, Id::new("foot-name"), Sense::click());
                led_display(ui.painter(), name, FOOT_CHARS, &shown);
                if click.hovered() {
                    ui.painter().rect_filled(window, 6.0, Color32::from_white_alpha(14));
                }
                let save = Pos2::new(foot.right() - FOOT_SAVE_REACH + 3.0, foot.top() + 14.0);
                silk_label(ui.painter(), save + vec2(0.0, 22.0), "Save", 11.0);
                let pressed = chrome_button(ui, save, "save-preset").clicked();
                if click.clicked() || pressed {
                    self.save_pressed(&dir, factory, &user, view, now);
                }
                // The folder the presets of the bank are kept in: hovered it says where, and a click
                // opens it, to rename, move, copy or throw them away. The factory bank has no folder,
                // so it names the presets folder itself
                if let Some(folder) = dir.clone().or_else(user_presets::presets_dir) {
                    let at = save - vec2(FOOT_BUTTON_SPREAD, 0.0);
                    silk_label(ui.painter(), at + vec2(0.0, 22.0), "Folder", 11.0);
                    let button = chrome_button(ui, at, "preset-folder")
                        .on_hover_text(format!("Your presets are kept in {}", folder.display()));
                    if button.clicked() {
                        let _ = std::fs::create_dir_all(&folder);
                        user_cab::open_folder(&folder);
                    }
                }
            });
        // Neither the escape that ended a name nor the one that ended a bank's closes the plate
        let named = typed.is_some() || save_typed.get().is_some();
        let mut open = !(picked.closed && !named);
        // A note is only up for a while: draw once more when it is over
        if view.note.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(SAVE_NOTE_S));
        }

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
                    menu.listed = bank.to_owned();
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
            view.plate = Some(Open::Presets(menu));
        } else {
            view.saving = None;
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
    let screen = physical_screen_width().unwrap_or(FALLBACK_SCREEN_WIDTH);
    let (width, height) = size_for_screen(screen);
    EguiState::from_size(width, height)
}

/// The window size `(width, height)` that fits inside a screen `screen_width` logical points wide,
/// at the zoom the twelve-pedal line fits at, clamped to `0.4..=1.0`
fn size_for_screen(screen_width: u32) -> (u32, u32) {
    let available = screen_width.saturating_sub(SCREEN_MARGIN) as f32;
    let zoom = if (available - DEFAULT_WIDTH as f32).abs() < 0.5 {
        DEFAULT_ZOOM
    } else {
        (available / WINDOW_WIDTH as f32).clamp(MIN_ZOOM, MAX_ZOOM)
    };
    // Never wider than the screen has room for: a line that does not fit at `MIN_ZOOM` in it is
    // scrolled
    let width = (WINDOW_WIDTH as f32 * zoom).round().min(available).max(MIN_WIDTH as f32) as u32;
    let height = (WINDOW_HEIGHT as f32 * zoom).round().max(MIN_HEIGHT as f32) as u32;
    (width, height)
}

/// The primary display's width in pixels (or logical points on macOS), or `None` when no display
/// is open (as in headless tests)
fn physical_screen_width() -> Option<u32> {
    #[cfg(target_os = "windows")]
    {
        const DESKTOPHORZRES: i32 = 118;
        const SM_CXMAXTRACK: i32 = 59;
        #[link(name = "user32")]
        extern "system" {
            fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
            fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
            fn GetSystemMetrics(index: i32) -> i32;
        }
        #[link(name = "gdi32")]
        extern "system" {
            fn GetDeviceCaps(hdc: *mut std::ffi::c_void, index: i32) -> i32;
        }
        // SAFETY: `GetDC(null)` borrows the screen DC for the call and `ReleaseDC` returns it;
        // `GetSystemMetrics(SM_CXMAXTRACK)` queries the current thread's maximum window tracking
        // width (which is smaller than `DESKTOPHORZRES` when the host thread is DPI-virtualized).
        let width = unsafe {
            let hdc = GetDC(std::ptr::null_mut());
            let phys = if hdc.is_null() {
                0
            } else {
                let w = GetDeviceCaps(hdc, DESKTOPHORZRES);
                ReleaseDC(std::ptr::null_mut(), hdc);
                w
            };
            let max_track = GetSystemMetrics(SM_CXMAXTRACK);
            if phys > 0 && max_track > 0 {
                phys.min(max_track)
            } else {
                phys.max(max_track)
            }
        };
        (width > 0).then_some(width as u32)
    }
    #[cfg(target_os = "macos")]
    {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGMainDisplayID() -> u32;
            fn CGDisplayPixelsWide(display: u32) -> usize;
        }
        // SAFETY: queries the main display's logical width from CoreGraphics without side effects.
        let width = unsafe { CGDisplayPixelsWide(CGMainDisplayID()) };
        (width > 0).then_some(width as u32)
    }
    #[cfg(target_os = "linux")]
    {
        #[link(name = "X11")]
        extern "C" {
            fn XOpenDisplay(display_name: *const std::ffi::c_char) -> *mut std::ffi::c_void;
            fn XDefaultScreen(display: *mut std::ffi::c_void) -> i32;
            fn XDisplayWidth(display: *mut std::ffi::c_void, screen_number: i32) -> i32;
            fn XCloseDisplay(display: *mut std::ffi::c_void) -> i32;
        }
        // SAFETY: opens the default X11 display if present, reads the default screen's width, and
        // closes the connection before returning.
        let width = unsafe {
            let display = XOpenDisplay(std::ptr::null());
            if display.is_null() {
                0
            } else {
                let screen = XDefaultScreen(display);
                let w = XDisplayWidth(display, screen);
                XCloseDisplay(display);
                w
            }
        };
        (width > 0).then_some(width as u32)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// Keeps `editor_state` no wider than the current screen at `scale_factor` before the window opens
fn clamp_state_to_screen(editor_state: &Arc<EguiState>, scale_factor: f32) {
    if editor_state.is_open() {
        return;
    }
    if let Some(screen_px) = physical_screen_width() {
        let factor = if scale_factor > 0.0 { scale_factor } else { 1.0 };
        let logical_screen = (screen_px as f32 / factor).round() as u32;
        let (max_w, max_h) = size_for_screen(logical_screen);
        if editor_state.size().0 > max_w {
            if let Ok(fitting) = Arc::try_unwrap(EguiState::from_size(max_w, max_h)) {
                nih_plug::params::persist::PersistentField::set(editor_state, fitting);
            }
        }
    }
}

struct ScreenEditor {
    inner: Box<dyn Editor>,
    editor_state: Arc<EguiState>,
    scale_factor: AtomicU32,
}

impl Editor for ScreenEditor {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn std::any::Any + Send> {
        let factor = f32::from_bits(self.scale_factor.load(Ordering::Relaxed));
        clamp_state_to_screen(&self.editor_state, factor);
        self.inner.spawn(parent, context)
    }

    fn size(&self) -> (u32, u32) {
        self.inner.size()
    }

    fn set_scale_factor(&self, factor: f32) -> bool {
        let ok = self.inner.set_scale_factor(factor);
        if ok && factor > 0.0 {
            self.scale_factor.store(factor.to_bits(), Ordering::Relaxed);
            clamp_state_to_screen(&self.editor_state, factor);
        }
        ok
    }

    fn param_value_changed(&self, id: &str, normalized_value: f32) {
        self.inner.param_value_changed(id, normalized_value);
    }

    fn param_modulation_changed(&self, id: &str, modulation_offset: f32) {
        self.inner.param_modulation_changed(id, modulation_offset);
    }

    fn param_values_changed(&self) {
        self.inner.param_values_changed();
    }
}

/// What the window shows of the line: the zoom the layer is scaled onto the window by, the
/// layer's width and how much of it the window shows. The zoom is what fits the whole layer in
/// the window, wide and high, kept between `MIN_ZOOM` and `MAX_ZOOM`. Where even `MIN_ZOOM` does
/// not fit the line, the window shows part of it and the player scrolls the rest into view; a
/// block never shrinks below what `MIN_ZOOM` makes it
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fit {
    zoom: f32,
    /// The layer: the line's width, but never narrower than the strip needs
    layer: f32,
    /// How many points of the layer the window shows, at that zoom
    visible: f32,
}

impl Fit {
    /// The fit of a window `window` points large for a line `line_width` wide
    fn of(window: Vec2, line_width: f32) -> Fit {
        let layer = line_width.max(STRIP_MIN_WIDTH);
        // A window's height is a whole number of points, rounded from the zoom that made it: half
        // a point of leeway keeps the zoom it was made at
        let high = (window.y + 0.5) / WINDOW_HEIGHT as f32;
        let mut zoom = (window.x / layer).min(high).clamp(MIN_ZOOM, MAX_ZOOM);
        // The size the window opens at on a 1920 screen is the default zoom, to the pixel
        if (zoom - DEFAULT_ZOOM).abs() < 0.001 {
            zoom = DEFAULT_ZOOM;
        }
        // A point short of the whole layer, or over it, is the whole layer: a window whose width
        // was rounded to whole points does not scroll for the sake of the rounding
        let visible = window.x / zoom;
        let visible = if (visible - layer).abs() < 0.5 / zoom + 0.5 { layer } else { visible };
        Fit { zoom, layer, visible }
    }

    /// How far the line can be scrolled: 0 when the window shows all of it
    fn furthest(&self) -> f32 {
        (self.layer - self.visible).max(0.0)
    }

    /// The strip's width: the window's, though never under what its controls need
    fn strip_width(&self) -> f32 {
        self.visible.max(STRIP_MIN_WIDTH)
    }

    /// How far what the strip has at its right end has moved from where it stands at the
    /// default width, and half of that for what stands in the middle of it
    fn shift(&self) -> f32 {
        self.strip_width() - WINDOW_WIDTH as f32
    }
}

/// Paints the corner handle's three lines on top of the line. The corner is dragged freely: the
/// window has no fixed proportions, since a chain that grows is a line that grows
fn paint_resize_corner(ui: &mut Ui) {
    *ui = ui.new_child(
        UiBuilder::new()
            .max_rect(ui.max_rect())
            .layer_id(LayerId::new(Order::Foreground, Id::new("amp-resize-corner"))),
    );
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
    beats: Arc<BeatReading>,
    cab_loader: Arc<CabLoader>,
    async_executor: AsyncExecutor<GuitarAmpPlugin>,
) -> Option<Box<dyn Editor>> {
    let (opened_loader, opened_executor) = (cab_loader.clone(), async_executor.clone());
    let opened_reading = lamp_reading.clone();
    let opened_beats = beats.clone();
    let opened_params = params.clone();
    let window_state = editor_state.clone();
    let screen_state = editor_state.clone();
    // Where the window is made, never in `process`: at most one request a day, on its own thread
    let update = UpdateCheck::start(params.update.clone());
    let opened_update = update.clone();
    create_egui_editor(
        editor_state,
        View {
            lamps: LampView::new(&lamp_reading),
            bank: Bank::default(),
            plate: None,
            saving: None,
            note: None,
            cab_status: CabStatus::Fine,
            beat: BeatView::default(),
            scroll: 0.0,
            effects: None,
        },
        move |egui_ctx, view| {
            ui::install(egui_ctx);
            // A result arrives on the check's own thread: the window is painted only when
            // asked, so it asks
            let ctx = egui_ctx.clone();
            opened_update.on_result(move || ctx.request_repaint());
            // What was clipped while the window was closed is not news
            view.lamps = LampView::new(&opened_reading);
            // The player may have renamed, moved or added presets in the folder since
            let bank = user_presets::bank_of(&opened_params);
            view.bank = Bank::read(&opened_params, user_presets::bank_dir(&bank).as_deref());
            view.close_plate(standalone::host());
            // Beats counted while the window was closed are not news
            view.beat.seen = opened_beats.read().0;
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
                beats: &beats,
                cab_loader: &cab_loader,
                load_cabinet: &load_cabinet,
                audio_host: standalone::host(),
                update: &update,
            };
            ResizableWindow::new(RESIZE_ID)
                .min_size(vec2(MIN_WIDTH as f32, MIN_HEIGHT as f32))
                .show(egui_ctx, window_state.as_ref(), |ui| {
                    draw(egui_ctx, setter, &params, view, &shared);
                    paint_resize_corner(ui);
                });
        },
    )
    .map(|inner| -> Box<dyn Editor> {
        Box::new(ScreenEditor {
            inner,
            editor_state: screen_state,
            scale_factor: AtomicU32::new(1.0_f32.to_bits()),
        })
    })
}

/// What the editor reads besides the parameters, shared with the audio and the plugin's
/// background tasks
struct Shared<'a> {
    tuner: &'a TunerReading,
    lamp_reading: &'a LampReading,
    beats: &'a BeatReading,
    cab_loader: &'a CabLoader,
    /// Has the chosen cabinet read in the background
    load_cabinet: &'a dyn Fn(),
    /// The machine's audio, in the standalone program; a plugin in a DAW has none
    audio_host: Option<&'a dyn standalone::Host>,
    /// Whether a newer version is out: the line on the strip
    update: &'a UpdateCheck,
}

/// The whole window, once the fonts are in: what the plugin shows and its snapshot tests draw
fn draw(
    ctx: &Context,
    setter: &ParamSetter,
    params: &GuitarAmpParams,
    view: &mut View,
    shared: &Shared,
) {
    let line_width = Line::of_chain(&params.chain.get()).width();
    let fit = Fit::of(ctx.screen_rect().size(), line_width);
    ctx.set_transform_layer(ui::rig_layer(), TSTransform::from_scaling(fit.zoom));
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

        // The strip is as wide as the window shows. What stands at its right end and in its
        // middle moves with that from where it stands at the default width
        let (right, middle) = (vec2(fit.shift(), 0.0), vec2(fit.shift() / 2.0, 0.0));
        let strip = Rect::from_min_size(
            window + vec2(BENCH_MARGIN, BENCH_MARGIN),
            vec2(fit.strip_width() - 2.0 * BENCH_MARGIN, STRIP_HEIGHT),
        );
        ui::slim_strip(ui, strip, &STYLE);
        ui::update_line(ui, window + update_line_at(fit.shift()), shared.update);
        // What is on the strip is placed from here
        let origin = window;
        let host = shared.audio_host;

        let presets = PresetDisplay {
            params,
            setter,
            cab_loader: shared.cab_loader,
            load_cabinet: shared.load_cabinet,
        };
        let tuning = params.tuner_on.value();
        if tuning {
            // The tuner takes the place of the preset display, plate and all
            if view.is_open(Plates::Presets) {
                view.close_plate(host);
            }
            // The note the tuner hears and how far off it is, where the preset was
            let note = shared.tuner.hz().map(Note::nearest);
            let name = note.map(|note| format!("{}{}", note.name, note.octave));
            tuner_display(
                ui.painter(),
                origin + vec2(PRESET_X, STRIP_Y),
                PRESET_CHARS,
                name.as_deref(),
                note.map(|note| note.cents),
            );
            // What it hears changes without anyone touching the window
            ui.ctx().request_repaint();
        } else {
            presets.display(ui, origin, view, now, host);
        }

        // The input: its meter under its caption, and the gain beside it
        // The output: the knob, the meter under its caption, and the clip lamp
        for (place, level, caption) in
            [(INPUT_METER, input_db, "Input"), (OUTPUT_METER + right, output_db, "Output")]
        {
            ui::level_meter_wide(ui.painter(), origin + place, METER_WIDTH, level);
            silk_label(
                ui.painter(),
                origin + vec2(place.x, METER_CAPTION_Y),
                caption,
                METER_CAPTION_SIZE,
            );
        }
        ui::bare_param_knob(ui, setter, &params.in_gain, origin + INPUT_KNOB, STRIP_KNOB_RADIUS);
        ui::bare_param_knob(
            ui,
            setter,
            &params.out_level,
            origin + OUTPUT_KNOB + right,
            STRIP_KNOB_RADIUS,
        );
        signal_lamp(ui.painter(), origin + CLIP_LAMP + right, clipping);
        silk_label(
            ui.painter(),
            origin + CLIP_LAMP + CLIP_CAPTION + right,
            "Clip",
            METER_CAPTION_SIZE,
        );

        // The icons, in the middle. The metronome's opens its plate and its switch starts it; the
        // tuner's is its switch, the display above taking its place; the settings have a plate only
        let beat = beat_shown(ui, params, view, shared, now);
        let metronome = ui::icon_switch(
            ui,
            origin + vec2(METRONOME_X, STRIP_Y) + middle,
            ui::Icon::Metronome,
            view.is_open(Plates::Metronome),
            Some(beat),
            "metronome",
        );
        if metronome.icon.clicked() {
            view.toggle_plate(Plates::Metronome, host, || Open::Metronome);
        }
        if metronome.switch.is_some_and(|switch| switch.clicked()) {
            set(setter, &params.metronome_on, !params.metronome_on.value());
        }
        let tuner = ui::icon_switch(
            ui,
            origin + vec2(TUNER_X, STRIP_Y) + middle,
            ui::Icon::Tuner,
            tuning,
            Some(if tuning { Beat::Regular } else { Beat::Dark }),
            "tuner",
        );
        if tuner.icon.clicked() || tuner.switch.is_some_and(|switch| switch.clicked()) {
            set(setter, &params.tuner_on, !tuning);
        }
        let settings = ui::icon_switch(
            ui,
            origin + vec2(SETTINGS_X, STRIP_Y) + middle,
            ui::Icon::Settings,
            view.is_open(Plates::Settings),
            None,
            "settings",
        );
        if settings.icon.clicked() {
            view.toggle_plate(Plates::Settings, host, || {
                Open::Settings(host.map(|host| AudioChoice::open(host, standalone::launch())))
            });
        }

        let pedals = Pedals {
            params,
            setter,
            gate_open,
            cab_loader: shared.cab_loader,
            load_cabinet: shared.load_cabinet,
        };

        // The line, scrolled sideways where the window shows only part of it. The wheel scrolls it
        // while the pointer is over it and no plate is open, and so does the bar along the foot
        view.scroll = view.scroll.clamp(0.0, fit.furthest());
        if fit.furthest() > 0.0 && view.plate.is_none() {
            // Not over the sheet of the bank, which stands on the line's lower part
            let sheet_top = view.effects.as_ref().map_or(f32::MAX, |bank| {
                bank.top(ui.max_rect().bottom() - window.y, now) + window.y
            });
            let over_line = ui.input(|input| input.pointer.hover_pos()).is_some_and(|at| {
                at.y / fit.zoom > STACK_TOP - BENCH_MARGIN && at.y / fit.zoom < sheet_top
            });
            let wheel = ui.input(|input| input.raw_scroll_delta);
            if over_line && (wheel.x != 0.0 || wheel.y != 0.0) {
                // Down and right bring what is further along into view
                view.scroll =
                    (view.scroll - (wheel.x + wheel.y) / fit.zoom).clamp(0.0, fit.furthest());
            }
        }
        crate::board::board(
            ui,
            window - vec2(view.scroll, 0.0),
            &pedals,
            &params.chain,
            &mut view.effects,
            view.scroll,
            now,
        );
        // Opening the bank closes the plate that was open
        if view.effects.as_ref().is_some_and(|bank| bank.fresh) && view.plate.is_some() {
            view.close_plate(host);
        }
        let bottom = ui.max_rect().bottom();
        let track = Rect::from_min_max(
            Pos2::new(window.x + SCROLL_INSET, bottom - 1.0 - ui::SCROLL_BAR_THICKNESS),
            Pos2::new(window.x + fit.visible - SCROLL_INSET, bottom - 1.0),
        );
        view.scroll = ui::scroll_bar(ui, "line", track, fit.visible, fit.layer, view.scroll);

        // The bank and the plates after everything else, each in a layer above the window; one is
        // open at most
        crate::bank::show(ui, window, fit.visible, &pedals, &mut view.effects, view.scroll, now);
        presets.plate(ui, origin, view, now);
        metronome_plate(
            ui,
            setter,
            params,
            origin + middle,
            view,
            shared,
            now,
            metronome.icon.rect,
        );
        settings_plate(ui, setter, params, origin + middle, view, shared, settings.icon.rect);
    });
}

/// The enum value a step of its stepper arrives at, round the list
fn neighbour<T: Copy + PartialEq>(all: &[T], current: T, step: Step) -> T {
    let index = all.iter().position(|&each| each == current).unwrap_or(0);
    all[match step {
        Step::Next => (index + 1) % all.len(),
        Step::Previous => (index + all.len() - 1) % all.len(),
    }]
}

/// Sets a parameter as one gesture
fn set<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

/// What the metronome's lamp shows: dim amber while it runs, amber on a beat, cream on the first
/// of the bar, lit for a moment. The window asks to be drawn again for as long as the metronome
/// runs
fn beat_shown(
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

/// The metronome's plate under its icon, while it is the plate that is open: the tempo, the tap,
/// the sound, the time signature, where the click is heard, its volume and Sync
#[allow(clippy::too_many_arguments)]
fn metronome_plate(
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
        let bpm = params.metronome_bpm.value();
        let tempo = at(TEMPO_X, TEMPO_Y);
        silk_label(ui.painter(), at(TEMPO_X, TEMPO_CAPTION_Y), "Tempo", 13.0);
        let window = Rect::from_center_size(tempo, vec2(TEMPO_WIDTH, LED_HEIGHT));
        let step = led_stepper(ui, tempo, TEMPO_CHARS, &format!("{}", bpm.round() as i32), "tempo");
        let turn = ui.interact(window, Id::new("tempo-turn"), Sense::click_and_drag());
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

/// The settings plate under its icon, while it is the plate that is open: the cabinet files and,
/// in the standalone program, the machine's audio, which Apply restarts the amp on
fn settings_plate(
    ui: &mut Ui,
    _setter: &ParamSetter,
    params: &GuitarAmpParams,
    origin: Pos2,
    view: &mut View,
    shared: &Shared,
    anchor: Rect,
) {
    let host = shared.audio_host;
    let Some(Open::Settings(audio)) = view.plate.as_mut() else {
        return;
    };
    let audio_rows = audio.as_ref().map(AudioChoice::rows);
    let rows = 1 + audio_rows.as_ref().map_or(0, |rows| rows.len());
    let height = settings_height(audio_rows.is_some(), rows);
    let rect = Rect::from_min_size(
        origin + vec2(SETTINGS_X + ui::icons::ICON_REACH - SETTINGS_WIDTH, PLATE_TOP),
        vec2(SETTINGS_WIDTH, height),
    );
    let mut cancelled = false;
    let mut event = None;
    let close = ui::plate(ui, rect, "settings", anchor, |ui, rect| {
        ui::plate::plate_title(ui.painter(), rect, "Settings");
        let painter = ui.painter().clone();
        let mut y = rect.top() + SETTINGS_PAD + ui::plate::PLATE_TITLE_HEIGHT;
        let caption_x = rect.left() + SETTINGS_PAD + SETTINGS_CAPTION_WIDTH / 2.0;
        let tape_x = rect.left()
            + SETTINGS_PAD
            + SETTINGS_CAPTION_WIDTH
            + SETTINGS_OVERHANG
            + SETTINGS_TAPE_WIDTH / 2.0;

        // The cabinet: the amp's own, a built-in one or a file, picked as on the amp's head. Where
        // the files go is shown when the caption is hovered, and opened by a double-click on it
        let middle = y + SETTINGS_ROW_HEIGHT / 2.0;
        silk_label(&painter, Pos2::new(caption_x, middle), "Cabinet", 13.0);
        if let Some(dir) = user_cab::cabinets_dir() {
            let caption = Rect::from_center_size(Pos2::new(caption_x, middle), vec2(90.0, 20.0));
            let response = ui
                .interact(caption, Id::new("settings-cabinet-folder"), Sense::click())
                .on_hover_text(format!("Cabinet files go in {}", dir.display()));
            if response.double_clicked() {
                let _ = std::fs::create_dir_all(&dir);
                user_cab::open_folder(&dir);
            }
        }
        if shared.cab_loader.no_files_showing() {
            // Draw once more when the note is over
            ui.ctx().request_repaint_after(NO_FILES_SHOWN);
        }
        let chosen = params.cabinet.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let shown = shared.cab_loader.display_name(&chosen, SETTINGS_CAB_CHARS);
        if let Some(step) =
            stepper(ui, Pos2::new(tape_x, middle), SETTINGS_TAPE_WIDTH, &shown, "settings-cabinet")
        {
            step_cabinet(params, shared.cab_loader, shared.load_cabinet, step == Step::Next);
        }
        y += SETTINGS_ROW_HEIGHT;

        if let Some(rows) = &audio_rows {
            for (index, (caption, shown)) in rows.iter().enumerate() {
                let middle = y + SETTINGS_ROW_HEIGHT / 2.0;
                silk_label(&painter, Pos2::new(caption_x, middle), caption, 13.0);
                if let Some(step) = stepper(
                    ui,
                    Pos2::new(tape_x, middle),
                    SETTINGS_TAPE_WIDTH,
                    shown,
                    &format!("audio-{index}"),
                ) {
                    event = Some(SettingsEvent::Step(index, step));
                }
                y += SETTINGS_ROW_HEIGHT;
            }
        }
        // The line under the rows, and for the audio the two buttons
        let choice = audio.as_ref();
        let note = match (choice.and_then(|choice| choice.error.as_deref()), choice) {
            (Some(error), _) => error,
            (None, Some(choice)) if choice.test_tone => {
                "The tone is on the chosen outputs; mind the volume"
            }
            (None, Some(_)) => "Apply starts the amp again on the new settings",
            (None, None) => "Cabinet files are read from the cabinets folder",
        };
        silk_label(
            &painter,
            Pos2::new(rect.center().x, y + SETTINGS_NOTE_HEIGHT / 2.0),
            note,
            12.0,
        );
        y += SETTINGS_NOTE_HEIGHT;
        if audio.is_some() {
            let buttons = y + 16.0;
            for (side, caption, id, wanted) in [
                (-1.0, "Apply", "apply", SettingsEvent::Apply),
                (1.0, "Cancel", "cancel", SettingsEvent::Cancel),
            ] {
                let at = Pos2::new(rect.center().x + side * SETTINGS_BUTTON_SPREAD, buttons);
                silk_label(&painter, at + vec2(0.0, 25.0), caption, 12.0);
                if chrome_button(ui, at, &format!("settings-{id}")).clicked() {
                    event = Some(wanted);
                }
            }
        }
    });
    match event {
        Some(SettingsEvent::Step(row, step)) => {
            if let (Some(choice), Some(host)) = (audio.as_mut(), host) {
                choice.step(host, row, step == Step::Next);
            }
        }
        Some(SettingsEvent::Apply) => {
            if let (Some(choice), Some(host)) = (audio.as_mut(), host) {
                match host.restart(&choice.prefs) {
                    // The new program is on its way: this window is done
                    Ok(()) => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
                    Err(error) => choice.error = Some(format!("Could not restart: {error}")),
                }
            }
        }
        Some(SettingsEvent::Cancel) => cancelled = true,
        None => {}
    }
    if close.asked() || cancelled {
        view.close_plate(host);
    }
}

/// What a row of the audio part of the settings plate asked for
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsEvent {
    /// A row was stepped to the previous or the next entry of its list
    Step(usize, Step),
    /// Take the settings: the amp starts again on them
    Apply,
    /// Leave them as they were
    Cancel,
}

/// How high the settings plate is: its title, a row for the cabinet and for each of the audio's
/// choices, the note, and the buttons of the audio
fn settings_height(with_audio: bool, rows: usize) -> f32 {
    2.0 * SETTINGS_PAD
        + ui::plate::PLATE_TITLE_HEIGHT
        + rows as f32 * SETTINGS_ROW_HEIGHT
        + SETTINGS_NOTE_HEIGHT
        + if with_audio { SETTINGS_FOOT_HEIGHT } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    use std::cell::RefCell;
    #[cfg(target_os = "linux")]
    use std::rc::Rc;
    #[cfg(target_os = "linux")]
    use suite_common::ui::snapshot::{pedal_harness, Harness, NoHost};

    /// The tests that point the presets at a folder of their own, one at a time: the folder is
    /// named in the environment, which every test shares
    #[cfg(target_os = "linux")]
    static FOLDER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    // The picture on the face is compiled in: a build without it is a build of other sources
    #[test]
    fn test_the_pedal_is_drawn_with_its_art() {
        assert!(STYLE.art == Some(ui::artwork::AMP), "the pedal has lost its art");
    }

    /// A view as the window has it after it opened: nothing open, the bank the params name
    fn fresh_view(params: &GuitarAmpParams, lamp_reading: &LampReading) -> View {
        View {
            lamps: LampView::new(lamp_reading),
            bank: Bank::read(params, None),
            plate: None,
            saving: None,
            note: None,
            cab_status: CabStatus::Fine,
            beat: BeatView::default(),
            scroll: 0.0,
            effects: None,
        }
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
        harness_shared(params, setter, tuner_hz, audio_host, open).0
    }

    /// `harness_with` that also hands back the view the window draws from, to look at what a click
    /// did to it
    #[cfg(target_os = "linux")]
    fn harness_shared<'a>(
        params: &'a GuitarAmpParams,
        setter: &'a ParamSetter<'a>,
        tuner_hz: Option<f32>,
        audio_host: Option<&'a dyn standalone::Host>,
        open: impl FnOnce(&mut View),
    ) -> (Harness<'a>, Rc<RefCell<View>>) {
        let tuner = TunerReading::new();
        tuner.set(tuner_hz);
        let lamp_reading = LampReading::new();
        let cab_loader = CabLoader::new(Arc::new(crate::cab_stage::CabStage::new()), 48_000.0);
        let mut view = fresh_view(params, &lamp_reading);
        open(&mut view);
        let view = Rc::new(RefCell::new(view));
        let harness = harness_at_size(
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            params,
            setter,
            tuner,
            lamp_reading,
            cab_loader,
            audio_host,
            view.clone(),
        );
        (harness, view)
    }

    #[cfg(target_os = "linux")]
    #[allow(clippy::too_many_arguments)]
    fn harness_at_size<'a>(
        width: u32,
        height: u32,
        params: &'a GuitarAmpParams,
        setter: &'a ParamSetter<'a>,
        tuner: TunerReading,
        lamp_reading: LampReading,
        cab_loader: CabLoader,
        audio_host: Option<&'a dyn standalone::Host>,
        view: Rc<RefCell<View>>,
    ) -> Harness<'a> {
        pedal_harness(width, height, move |ctx| {
            let shared = Shared {
                tuner: &tuner,
                lamp_reading: &lamp_reading,
                beats: &BeatReading::default(),
                cab_loader: &cab_loader,
                load_cabinet: &|| {},
                audio_host,
                update: &UpdateCheck::resting(params.update.clone()),
            };
            draw(ctx, setter, params, &mut view.borrow_mut(), &shared);
        })
    }

    /// A click of the primary button at `at`, in window points at full size: the pointer arrives,
    /// presses and lets go, a frame each
    #[cfg(target_os = "linux")]
    fn click(harness: &mut Harness, at: Pos2) {
        use nih_plug_egui::egui::{Event, Modifiers, PointerButton};
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        for event in [Event::PointerMoved(at), button(true), button(false)] {
            harness.input_mut().events.push(event);
            harness.run_ok();
        }
    }

    /// Text typed, and keys pressed, in the next frame
    #[cfg(target_os = "linux")]
    fn type_text(harness: &mut Harness, text: &str) {
        use nih_plug_egui::egui::Event;
        harness.input_mut().events.push(Event::Text(text.to_owned()));
        harness.run_ok();
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

    // The update line is drawn when a newer version is known, and only then: with the check off,
    // or the line closed for that version, the window is the one it was (pixel for pixel)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_update_line_is_drawn_when_a_newer_version_is_known() {
        type Settings = suite_common::update::UpdateSettings;
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let picture = |change: &dyn Fn(&mut Settings)| {
            let params = GuitarAmpParams::default();
            change(&mut params.update.lock().unwrap());
            let mut harness = harness_of(&params, &setter, None, |_| {});
            // Not returned directly: the harness must go before the params it borrows
            #[allow(clippy::let_and_return)]
            let raw = harness.render().unwrap().into_raw();
            raw
        };
        let none = picture(&|_| {});
        let newer = |settings: &mut Settings| settings.latest = "99.0.0".into();
        assert_ne!(picture(&newer), none, "no line with a newer version known");
        assert!(
            picture(&|settings| {
                newer(settings);
                settings.enabled = false;
            }) == none,
            "a line with the check off"
        );
        assert!(
            picture(&|settings| {
                newer(settings);
                settings.dismissed = settings.latest.clone();
            }) == none,
            "a line after it was closed"
        );
    }

    // The window with the line, against tests/snapshots/editor_update.png
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_editor_with_the_update_line_looks_as_it_did() {
        let params = GuitarAmpParams::default();
        params.update.lock().unwrap().latest = "99.0.0".into();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        harness_of(&params, &setter, None, |_| {}).snapshot("editor_update");
    }

    // The line has the room it needs on the strip at its narrowest and at the widest it opens,
    // clear of the settings icon and of the output knob's scale
    #[test]
    fn test_the_update_line_has_room_on_the_strip() {
        let needed = ui::update_line::parts(Pos2::ZERO, [96.0, 51.0, 19.0]).0.width();
        for width in [STRIP_MIN_WIDTH, WINDOW_WIDTH as f32, 3840.0] {
            let shift = width - WINDOW_WIDTH as f32;
            let room = (UPDATE_ROOM_RIGHT + shift) - (UPDATE_ROOM_LEFT + shift / 2.0) - STRIP_GAP;
            assert!(room >= needed, "{room} of the strip's room at {width} wide");
        }
    }

    // The window at the size it opens at (0.55 zoom, 1876 x 321): every pedal, label and the
    // strip scaled down together, nothing clipped
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_editor_at_default_zoom_looks_as_it_did() {
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let tuner = TunerReading::new();
        let lamp_reading = LampReading::new();
        let cab_loader = CabLoader::new(Arc::new(crate::cab_stage::CabStage::new()), 48_000.0);
        let view = Rc::new(RefCell::new(fresh_view(&params, &lamp_reading)));
        let (width, height) = size_for_screen(FALLBACK_SCREEN_WIDTH);
        harness_at_size(
            width,
            height,
            &params,
            &setter,
            tuner,
            lamp_reading,
            cab_loader,
            None,
            view,
        )
        .snapshot("editor_default_zoom");
    }

    /// A window `width` by `height` points drawing `params`, scrolled `scroll` points, the way
    /// a window of that size does, with everything else at rest
    #[cfg(target_os = "linux")]
    fn line_harness<'a>(
        width: u32,
        height: u32,
        params: &'a GuitarAmpParams,
        setter: &'a ParamSetter<'a>,
        scroll: f32,
    ) -> (Harness<'a>, Rc<RefCell<View>>) {
        let lamp_reading = LampReading::new();
        let cab_loader = CabLoader::new(Arc::new(crate::cab_stage::CabStage::new()), 48_000.0);
        let mut view = fresh_view(params, &lamp_reading);
        view.scroll = scroll;
        let view = Rc::new(RefCell::new(view));
        let harness = harness_at_size(
            width,
            height,
            params,
            setter,
            TunerReading::new(),
            lamp_reading,
            cab_loader,
            None,
            view.clone(),
        );
        (harness, view)
    }

    /// A chain of `kinds` in front of the amps then `kinds` behind them, each on the slot its
    /// pedal's own parameters are on if that is free, a slot of its own otherwise
    fn chain_of(front: &[EffectKind], back: &[EffectKind]) -> Chain {
        let mut chain = Chain::empty();
        for &kind in front {
            chain.add(kind, crate::order::Zone::Front, 99).unwrap();
        }
        for &kind in back {
            chain.add(kind, crate::order::Zone::Back, 99).unwrap();
        }
        chain
    }

    /// The chain of the longest line: sixteen blocks, a second of eight of the kinds among them,
    /// so that half of them keep their values in the parameters of their slot
    fn sixteen_blocks() -> Chain {
        use EffectKind::*;
        chain_of(
            &[Wah, Pitch, Fuzz, Comp, Comp, Drive, Eq, Phaser],
            &[Chorus, Chorus, Flanger, Tremolo, Delay, Delay, Reverb, Reverb],
        )
    }

    // A short chain, a compressor in front of the amps and a chorus behind them: the line is as
    // long as its blocks, the strip as wide as the window
    #[test]
    #[cfg(target_os = "linux")]
    fn test_a_short_chain_looks_as_it_did() {
        use EffectKind::*;
        let params = GuitarAmpParams::default();
        params.chain.put(chain_of(&[Comp], &[Chorus]));
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        line_harness(1430, 321, &params, &setter, 0.0).0.snapshot("line_short");
        let _ = (Comp, Chorus);
    }

    // Today's chain in the smallest window: the zoom the line is at when it is the least readable
    // there is, and the line still all there
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_twelve_pedals_in_the_smallest_window_look_as_they_did() {
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        line_harness(MIN_WIDTH, MIN_HEIGHT, &params, &setter, 0.0).0.snapshot("line_small");
    }

    // Today's chain on a large screen: full size, the strip as wide as the window
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_twelve_pedals_in_a_large_window_look_as_they_did() {
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        line_harness(3840, 640, &params, &setter, 0.0).0.snapshot("line_large");
    }

    // Sixteen blocks in the window that shows them whole at the least zoom (4239 points at 0.5): the
    // second of each kind, eight of them, on the parameters of their slots
    #[test]
    #[cfg(target_os = "linux")]
    fn test_sixteen_blocks_look_as_they_did() {
        let params = GuitarAmpParams::default();
        params.chain.put(sixteen_blocks());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        line_harness(2120, 292, &params, &setter, 0.0).0.snapshot("line_sixteen");
    }

    // The same sixteen in the smallest window: they do not fit at the zoom it stops at, so the
    // line is scrolled, here to its start and to its end
    #[test]
    #[cfg(target_os = "linux")]
    fn test_sixteen_blocks_scroll_in_the_smallest_window() {
        let params = GuitarAmpParams::default();
        params.chain.put(sixteen_blocks());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (width, height) = (MIN_WIDTH, MIN_HEIGHT);
        line_harness(width, height, &params, &setter, 0.0).0.snapshot("line_scrolled_start");
        line_harness(width, height, &params, &setter, 1.0e6).0.snapshot("line_scrolled_end");
    }

    // The longest chain there is: sixteen blocks (and the gate in front of them) and both amps on (Torden and Klar), in the window
    // that shows them whole and in the smallest one scrolled to the amps. Both lanes stay readable
    // at the least zoom, and the line at rest still asks for no repaint
    #[test]
    #[cfg(target_os = "linux")]
    fn test_sixteen_blocks_and_both_amps_look_as_they_did() {
        let params = GuitarAmpParams {
            amp: EnumParam::new("Amp", Amp::Torden),
            amp2_on: BoolParam::new("Amp 2", true),
            ..GuitarAmpParams::default()
        };
        params.chain.put(sixteen_blocks());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        line_harness(2120, 292, &params, &setter, 0.0).0.snapshot("line_sixteen_two_amps");
        // The stack stands after the eight blocks in front of it: scrolled so that it is in view
        let stack_at = Line::of_chain(&sixteen_blocks()).places_for_test()[8 + 1];
        let (mut harness, _) =
            line_harness(MIN_WIDTH, MIN_HEIGHT, &params, &setter, stack_at - 200.0);
        harness.snapshot("line_two_amps_scrolled");
        for _ in 0..3 {
            harness.step();
        }
        let delays: Vec<_> =
            harness.output().viewport_output.values().map(|v| v.repaint_delay).collect();
        assert!(!delays.is_empty());
        assert!(delays.iter().all(|delay| *delay == std::time::Duration::MAX), "{delays:?}");
    }

    /// The wheel turned over `at`, by `delta` points
    #[cfg(target_os = "linux")]
    fn wheel(harness: &mut Harness, at: Pos2, delta: Vec2) {
        use nih_plug_egui::egui::{Event, Modifiers, MouseWheelUnit};
        for event in [
            Event::PointerMoved(at),
            Event::MouseWheel { unit: MouseWheelUnit::Point, delta, modifiers: Modifiers::NONE },
        ] {
            harness.input_mut().events.push(event);
            harness.run_ok();
        }
    }

    // A line that is wider than the window scrolls with the wheel over it, either way the wheel
    // is turned, and stops at both ends; a window that shows it all has nothing to scroll
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_wheel_scrolls_a_line_that_does_not_fit_and_stops_at_its_ends() {
        let params = GuitarAmpParams::default();
        params.chain.put(sixteen_blocks());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = line_harness(MIN_WIDTH, MIN_HEIGHT, &params, &setter, 0.0);
        let over_the_line = Pos2::new(300.0, 200.0);
        wheel(&mut harness, over_the_line, vec2(0.0, -100.0));
        // 100 points of wheel at 0.5 is 200 points of the layer
        assert!((view.borrow().scroll - 200.0).abs() < 0.01, "{}", view.borrow().scroll);
        wheel(&mut harness, over_the_line, vec2(-40.0, 0.0));
        assert!((view.borrow().scroll - 280.0).abs() < 0.01);
        wheel(&mut harness, over_the_line, vec2(0.0, 1000.0));
        assert_eq!(view.borrow().scroll, 0.0);
        let furthest = Fit::of(vec2(MIN_WIDTH as f32, MIN_HEIGHT as f32), 4239.0).furthest();
        wheel(&mut harness, over_the_line, vec2(0.0, -100_000.0));
        assert_eq!(view.borrow().scroll, furthest);
        // Over the strip the wheel is left alone
        wheel(&mut harness, Pos2::new(300.0, 10.0), vec2(0.0, 1000.0));
        assert_eq!(view.borrow().scroll, furthest);

        // The twelve-pedal line in a window that shows it all does not scroll
        let params = GuitarAmpParams::default();
        let (mut harness, view) = line_harness(WINDOW_WIDTH, WINDOW_HEIGHT, &params, &setter, 0.0);
        wheel(&mut harness, over_the_line, vec2(0.0, -500.0));
        assert_eq!(view.borrow().scroll, 0.0);
    }

    // A press in the groove of the scroll bar brings its thumb there, and the thumb is dragged
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_scroll_bar_takes_the_line_where_it_is_pressed_and_dragged() {
        let params = GuitarAmpParams::default();
        params.chain.put(sixteen_blocks());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = line_harness(MIN_WIDTH, MIN_HEIGHT, &params, &setter, 0.0);
        let furthest = Fit::of(vec2(MIN_WIDTH as f32, MIN_HEIGHT as f32), 4239.0).furthest();
        let bar_y = WINDOW_HEIGHT as f32 * MIN_ZOOM - 2.0;
        // Pressed at the groove's far end: the line is at its end
        click(&mut harness, Pos2::new(MIN_WIDTH as f32 - 25.0, bar_y));
        assert_eq!(view.borrow().scroll, furthest);
        // At its near end: back at the start
        click(&mut harness, Pos2::new(25.0, bar_y));
        assert_eq!(view.borrow().scroll, 0.0);
    }

    /// The pointer pressed at `from`, carried to `to` and let go there, a frame at each step
    #[cfg(target_os = "linux")]
    fn drag(harness: &mut Harness, from: Pos2, to: Pos2) {
        use nih_plug_egui::egui::{Event, Modifiers, PointerButton};
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        let middle = from + (to - from) * 0.5;
        let events = [
            Event::PointerMoved(from),
            button(from, true),
            Event::PointerMoved(from + vec2(4.0, 0.0)),
            Event::PointerMoved(middle),
            Event::PointerMoved(to),
            button(to, false),
        ];
        for event in events {
            harness.input_mut().events.push(event);
            harness.run_ok();
        }
    }

    // A block is dragged by its title to another place in the line and lands there, across the
    // amps if it is carried across them, with its slot
    #[test]
    #[cfg(target_os = "linux")]
    fn test_a_block_dragged_by_its_title_lands_in_the_nearest_place() {
        use crate::order::Zone;
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, _) = line_harness(WINDOW_WIDTH, WINDOW_HEIGHT, &params, &setter, 0.0);
        let line = Line::of_chain(&params.chain.get());
        let title = |at: usize| {
            let left = line.places_for_test()[at];
            Pos2::new(left + PEDAL_WIDTH / 2.0, PEDAL_TOP + 30.0)
        };
        // The chorus, first behind the amps (nine pieces in), to the far end of the window
        let chorus = line.pieces_for_test().iter().position(|piece| {
            matches!(piece, crate::board::Piece::Block(block) if block.kind == EffectKind::Chorus)
        });
        let chorus = chorus.unwrap();
        drag(&mut harness, title(chorus), Pos2::new(WINDOW_WIDTH as f32 - 60.0, PEDAL_TOP + 30.0));
        for _ in 0..3 {
            harness.run_ok();
        }
        let chain = params.chain.get();
        assert_eq!(chain.back().last().map(|block| block.kind), Some(EffectKind::Chorus));
        assert_eq!(chain.back().len(), 6);
        assert_eq!(chain.front().len(), 6);
        // It is on its own slot still
        assert!(chain.blocks().iter().all(|&block| Chain::is_home(block)));
        // And back across the amps to right behind the gate
        let line = Line::of_chain(&chain);
        let last = line.pieces_for_test().len() - 1;
        drag(
            &mut harness,
            title(last),
            Pos2::new(BENCH_MARGIN + PEDAL_WIDTH + 40.0, PEDAL_TOP + 30.0),
        );
        for _ in 0..3 {
            harness.run_ok();
        }
        let chain = params.chain.get();
        assert_eq!(chain.front().first().map(|block| block.kind), Some(EffectKind::Chorus));
        assert_eq!(chain.front().len(), 7);
        let _ = Zone::Front;
    }

    // Nothing asks for a repaint while nobody touches the window: a drawn line is painted only
    // when something changes (STYLE.md)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_a_line_at_rest_asks_for_no_repaint() {
        let params = GuitarAmpParams::default();
        params.chain.put(sixteen_blocks());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, _) = line_harness(MIN_WIDTH, MIN_HEIGHT, &params, &setter, 300.0);
        for _ in 0..3 {
            harness.step();
        }
        let delays: Vec<_> = harness
            .output()
            .viewport_output
            .values()
            .map(|viewport| viewport.repaint_delay)
            .collect();
        assert!(!delays.is_empty());
        assert!(delays.iter().all(|delay| *delay == std::time::Duration::MAX), "{delays:?}");
    }

    // The second amp on beside the first: Torden and Klar, each through a cabinet of its own
    // (Torden's own, Klar through a 1x15), a Blend of 5.0 and the amps on their way
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_second_amp_looks_as_it_did() {
        let params = GuitarAmpParams {
            amp: EnumParam::new("Amp", Amp::Torden),
            amp2_on: BoolParam::new("Amp 2", true),
            ..GuitarAmpParams::default()
        };
        params.cabinet_second.put(crate::amp::model::speaker_index("1x15-deep"));
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        harness_of(&params, &setter, None, |_| {}).snapshot("dual");
    }

    // The wah on, in Auto, with the rocker at the toe: the lamp, the Auto lamp and the cap at the
    // top of its slot (the default picture has them at rest)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_wah_looks_as_it_did() {
        let params = GuitarAmpParams {
            wah_on: BoolParam::new("Wah", true),
            wah_mode: EnumParam::new("Wah Mode", WahMode::Auto),
            wah_pedal: crate::dial_param("Wah Pedal", 1.0),
            ..GuitarAmpParams::default()
        };
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        harness_of(&params, &setter, None, |_| {}).snapshot("wah");
    }

    #[test]
    fn test_the_wahs_controls_stand_clear_of_each_other_and_on_the_face() {
        let knob = WAH_KNOB_RADIUS + TICKS;
        let [bite, sens] = WAH_KNOBS;
        // Across the rocker's row: Bite's scale, the rocker with its ticks, the tape of its value
        // and the face's edge
        in_a_row(&[
            ("Bite's scale", bite.x - knob, bite.x + knob),
            ("the rocker's ticks", -ui::widgets::ROCKER_REACH, ui::widgets::ROCKER_REACH),
            ("the rocker's tape", ROCKER_TAPE.x - 22.0, ROCKER_TAPE.x + 22.0),
            ("the face's edge", PEDAL_WIDTH / 2.0, PEDAL_WIDTH),
        ]);
        // The cap at either end of its slot stays inside the face between the captions
        let cap = ui::widgets::ROCKER_CAP_HEIGHT / 2.0;
        in_a_row(&[
            ("Toe", ROCKER_TOE.y - 6.0, ROCKER_TOE.y + 6.0),
            ("the rocker", ROCKER_TOP.y - cap, ROCKER_TOP.y + ROCKER_LENGTH + cap),
            ("Heel", ROCKER_HEEL.y - 6.0, ROCKER_HEEL.y + 6.0),
            ("the On switch", PEDAL_SWITCH.y - SWITCH, PEDAL_SWITCH.y + SWITCH),
        ]);
        // Down the right: the lamp, the rocker's name, its tape, then the Auto button with its
        // lamp above and its caption under it
        in_a_row(&[
            ("the lamp", WAH_LED.y - LAMP, WAH_LED.y + LAMP),
            ("the rocker's caption", ROCKER_CAPTION.y - 6.0, ROCKER_CAPTION.y + 6.0),
            ("the rocker's tape", ROCKER_TAPE.y, ROCKER_TAPE.y + 17.0),
            ("the Auto lamp", AUTO_LED.y - LAMP, AUTO_LED.y + LAMP),
            ("the Auto button", AUTO_BUTTON.y - BUTTON, AUTO_BUTTON.y + BUTTON + 2.5),
            ("the Auto caption", AUTO_CAPTION.y - 7.0, AUTO_CAPTION.y + 7.0),
        ]);
        assert_eq!(AUTO_LED.x, AUTO_BUTTON.x);
        assert_eq!(AUTO_CAPTION.x, AUTO_BUTTON.x);
        // The Auto button stands beside the On switch, not over it, and inside the face
        in_a_row(&[
            ("the On switch", -SWITCH, SWITCH),
            ("the Auto button", AUTO_BUTTON.x - BUTTON, AUTO_BUTTON.x + BUTTON),
            ("the face's edge", PEDAL_WIDTH / 2.0 - 10.0, PEDAL_WIDTH / 2.0),
        ]);
        // Down the left: Bite's tape is over Sens's scale
        assert!(bite.y + 68.0 <= sens.y - knob, "Bite's tape runs into Sens");
        assert_eq!(bite.x, sens.x);
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
        fn channels(&self, _: &str, _: bool, device: Option<&str>) -> u32 {
            // The interface has four of each, the machine's own devices a pair
            if device == Some("Scarlett 2i2 USB") {
                4
            } else {
                2
            }
        }
    }

    // The settings plate in a plugin: the cabinet files alone, under the icon
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_settings_plate_looks_as_it_did() {
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        harness_of(&params, &setter, None, |view| view.plate = Some(Open::Settings(None)))
            .snapshot("settings");
    }

    // The standalone program's settings plate: the cabinet files and the machine's audio
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_standalone_audio_settings_look_as_they_did() {
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let machine = TwoInterfaces;
        let prefs = Prefs {
            backend: "alsa".into(),
            input_device: Some("Scarlett 2i2 USB".into()),
            input_channel: 3,
            ..Prefs::default()
        };
        harness_with(&params, &setter, None, Some(&machine), |view| {
            view.plate = Some(Open::Settings(Some(AudioChoice::open(&machine, prefs))));
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

    // The metronome running, its plate open: its lamp dim between beats, Sync on, six eighths, the
    // Click and its output apart
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_metronome_looks_as_it_did() {
        let params = GuitarAmpParams {
            metronome_on: BoolParam::new("Metronome", true),
            metronome_sync: BoolParam::new("Metronome Sync", true),
            metronome_time_sig: EnumParam::new("Metronome Time Signature", TimeSignature::SixEight),
            metronome_sound: EnumParam::new("Metronome Sound", Sound::Click),
            // The longer of the click output's two names, the one that must fit its tape
            metronome_out: EnumParam::new("Metronome Output", ClickOut::Separate),
            ..GuitarAmpParams::default()
        };
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        harness_of(&params, &setter, None, |view| view.plate = Some(Open::Metronome))
            .snapshot("metronome");
    }

    /// The presets folder of a test: one that is not there, so the banks are the two every player
    /// has, whatever is on this machine
    #[cfg(target_os = "linux")]
    fn no_presets_folder() {
        let nowhere = std::env::temp_dir().join("hojt-snapshot-no-presets");
        std::env::set_var(user_presets::FOLDER_ENV, &nowhere);
        assert!(!nowhere.exists(), "{nowhere:?} is there: the list would show its banks");
    }

    /// The preset plate on the factory bank
    fn presets_open(view: &mut View) {
        view.plate = Some(Open::Presets(Menu {
            listed: user_presets::FACTORY.to_owned(),
            files: Vec::new(),
            naming: None,
        }));
    }

    // The preset plate open on the factory bank, under the display, with its foot
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_preset_menu_looks_as_it_did() {
        let _folder = FOLDER_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        no_presets_folder();
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        harness_of(&params, &setter, None, presets_open).snapshot("preset_menu");
    }

    /// How far a stepper's buttons stand out from the middle of its tape
    fn reach(width: f32) -> f32 {
        width / 2.0 + 22.0
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

    /// The top face of the strip, in window points
    fn strip_face() -> Rect {
        strip_face_shifted(0.0)
    }

    /// The top face of a strip `shift` points wider than the default
    fn strip_face_shifted(shift: f32) -> Rect {
        Rect::from_min_max(
            Pos2::new(BENCH_MARGIN, BENCH_MARGIN),
            Pos2::new(
                WINDOW_WIDTH as f32 + shift - BENCH_MARGIN,
                BENCH_MARGIN + SLIM_STRIP_HEIGHT - 5.0,
            ),
        )
    }

    /// The strips of the widths a window may give it: the narrowest, today's, a very wide one
    fn strip_shifts() -> [f32; 3] {
        [STRIP_MIN_WIDTH - WINDOW_WIDTH as f32, 0.0, 2600.0]
    }

    /// What stands on the strip, and the room each part takes, in points from the window's top
    /// left corner, from the left to the right of the strip: the print of `ui::slim_strip` (the
    /// jacks with their captions, the model number, the logo) and every control
    /// `strip_parts` of a strip `shift` points wider than the default: what is at its right end
    /// moves with it, and what is in the middle half as far
    fn strip_parts_shifted(shift: f32) -> Vec<(&'static str, Rect)> {
        let print = ui::slim_print(strip_face_shifted(shift), &STYLE);
        let (right, middle) = (vec2(shift, 0.0), vec2(shift / 2.0, 0.0));
        let around = |at: Vec2, half: Vec2| Rect::from_center_size(at.to_pos2(), 2.0 * half);
        let meter = |at: Vec2| {
            around(at, vec2(METER_WIDTH / 2.0, ui::widgets::WIDE_METER_HEIGHT / 2.0 + 2.0))
        };
        let caption =
            |at: Vec2, width: f32| around(vec2(at.x, METER_CAPTION_Y), vec2(width / 2.0, 7.0));
        let knob = |at: Vec2| around(at, Vec2::splat(KNOB_REACH));
        let icon = |x: f32, switch: bool| {
            Rect::from_min_max(
                Pos2::new(x - ui::icons::ICON_REACH, STRIP_Y - ui::icons::ICON_SIZE / 2.0),
                Pos2::new(
                    x + if switch { ui::icons::SWITCH_REACH } else { ui::icons::ICON_REACH },
                    STRIP_Y + ui::icons::ICON_SIZE / 2.0,
                ),
            )
        };
        vec![
            ("the input jack", Rect::from_center_size(print.jacks[0], Vec2::splat(16.0))),
            ("the input jack's caption", print.captions[0]),
            ("the input meter", meter(INPUT_METER)),
            ("the input meter's caption", caption(INPUT_METER, 52.0)),
            ("the Input knob", knob(INPUT_KNOB)),
            (
                "the preset display",
                around(vec2(PRESET_X, STRIP_Y), vec2(PRESET_WIDTH / 2.0, LED_HEIGHT / 2.0)),
            ),
            ("the metronome", icon(METRONOME_X + middle.x, true)),
            ("the tuner", icon(TUNER_X + middle.x, true)),
            ("the settings", icon(SETTINGS_X + middle.x, false)),
            ("the output knob", knob(OUTPUT_KNOB + right)),
            ("the output meter", meter(OUTPUT_METER + right)),
            ("the output meter's caption", caption(OUTPUT_METER + right, 62.0)),
            ("the clip lamp", around(CLIP_LAMP + right, Vec2::splat(LAMP))),
            ("the clip lamp's caption", around(CLIP_LAMP + CLIP_CAPTION + right, vec2(24.0, 7.0))),
            ("the model number", print.model),
            ("the logo", print.logo),
            ("the output jack", Rect::from_center_size(print.jacks[1], Vec2::splat(16.0))),
            ("the output jack's caption", print.captions[1]),
        ]
    }

    #[test]
    fn test_nothing_on_the_strip_overlaps_and_it_reads_from_the_left_to_the_right() {
        for shift in strip_shifts() {
            nothing_overlaps_on_a_strip_wider_by(shift);
        }
    }

    fn nothing_overlaps_on_a_strip_wider_by(shift: f32) {
        let parts = strip_parts_shifted(shift);
        for (index, (name, rect)) in parts.iter().enumerate() {
            for (other, other_rect) in &parts[index + 1..] {
                assert!(
                    !rect.expand(2.0).intersects(*other_rect),
                    "{name} {rect:?} runs into {other} {other_rect:?}"
                );
            }
        }
        // The signal runs from left to right: what comes in, the preset, the icons, what goes out
        let at = |name: &str| parts.iter().find(|part| part.0 == name).unwrap().1.center().x;
        let order = [
            "the input jack",
            "the input meter",
            "the Input knob",
            "the preset display",
            "the metronome",
            "the tuner",
            "the settings",
            "the output knob",
            "the output meter",
            "the clip lamp",
            "the model number",
            "the logo",
            "the output jack",
        ];
        for pair in order.windows(2) {
            assert!(at(pair[0]) < at(pair[1]), "{} is not left of {}", pair[0], pair[1]);
        }
        // Room to spare between the groups, as many points before the icons as after them
        let room = |from: &str, to: &str| {
            let find = |name: &str| parts.iter().find(|part| part.0 == name).unwrap().1;
            find(to).left() - find(from).right()
        };
        let (before, after) =
            (room("the preset display", "the metronome"), room("the settings", "the output knob"));
        assert!((before - after).abs() < 0.5 && before > 40.0, "{before} {after} at {shift}");
    }

    #[test]
    fn test_nothing_on_the_strip_reaches_its_edges_or_the_line_under_it() {
        for shift in strip_shifts() {
            let face = strip_face_shifted(shift);
            for (name, rect) in strip_parts_shifted(shift) {
                assert!(
                    face.shrink(2.0).contains_rect(rect),
                    "{name} {rect:?} is not on the strip's face {face:?} wider by {shift}"
                );
            }
        }
        // The strip is slim, and the line starts under it, a bench margin away from its box
        assert_eq!(STRIP_HEIGHT, 64.0);
        assert_eq!(STACK_TOP, BENCH_MARGIN + STRIP_HEIGHT + BENCH_MARGIN);
        // The plates open under the box, never over the strip's controls
        const { assert!(PLATE_TOP > BENCH_MARGIN + STRIP_HEIGHT) };
        // The controls end where the print begins
        let model = ui::slim_print(strip_face(), &STYLE).model;
        assert!((MODEL_LEFT - model.left()).abs() < 0.01);
        // The center line: every row of controls is on it, as the icons are
        assert_eq!(STRIP_Y, INPUT_KNOB.y);
        assert_eq!(STRIP_Y, OUTPUT_KNOB.y);
    }

    #[test]
    fn test_the_strip_holds_the_preset_display_the_tuner_takes_the_place_of() {
        // The tuner's display is the preset display's size, in its place
        assert_eq!(led_width(PRESET_CHARS), PRESET_WIDTH);
        // What the display says is cut two characters short of its end, for the marker
        const { assert!(PRESET_CHARS > BANK_SHOWN_CHARS + 3 + USER_NAME_CHARS + 1 - 3) };
    }

    // The input and the output are made alike: a meter with its caption over it, a knob beside it
    #[test]
    fn test_the_input_and_the_output_are_made_alike() {
        assert_eq!(INPUT_METER.y, OUTPUT_METER.y);
        assert_eq!(INPUT_KNOB.y, OUTPUT_KNOB.y);
        // The input's knob is right of its meter, the output's left of its meter
        const { assert!(INPUT_KNOB.x > INPUT_METER.x && OUTPUT_KNOB.x < OUTPUT_METER.x) };
        let gap = |meter: Vec2, knob: Vec2| (meter.x - knob.x).abs() - METER_REACH - KNOB_REACH;
        assert_eq!(gap(INPUT_METER, INPUT_KNOB), gap(OUTPUT_METER, OUTPUT_KNOB));
    }

    #[test]
    fn test_the_icons_stand_a_pitch_apart_between_the_preset_display_and_the_output() {
        // The room left is shared: as much before the icons as after them
        let before = METRONOME_X - ui::icons::ICON_REACH - PRESET_END;
        let after = OUTPUT_KNOB.x - KNOB_REACH - (SETTINGS_X + ui::icons::ICON_REACH);
        assert!((before - after).abs() < 0.01 && before > 40.0, "{before} {after}");
        assert_eq!(TUNER_X - METRONOME_X, SETTINGS_X - TUNER_X);
        // An icon with a switch reaches the next icon's room, no further
        const { assert!(METRONOME_X + ui::icons::SWITCH_REACH + 4.0 < TUNER_X - ui::icons::ICON_REACH) };
        const { assert!(TUNER_X + ui::icons::SWITCH_REACH + 4.0 < SETTINGS_X - ui::icons::ICON_REACH) };
    }

    /// The plate each of the three opens, where it stands in the window
    fn plate_rects() -> [(&'static str, Rect, Rect); 3] {
        let preset_plate_width =
            DROP_BANK_WIDTH + DROP_PRESET_WIDTH + 2.0 * (5.0 + 4.0) + 12.0 + 20.0;
        let below = |left: f32, width: f32, height: f32| {
            Rect::from_min_size(Pos2::new(left, PLATE_TOP), vec2(width, height))
        };
        [
            (
                "the preset plate",
                below(PRESET_X - preset_plate_width / 2.0, preset_plate_width, 260.0),
                Rect::from_center_size(
                    Pos2::new(PRESET_X, STRIP_Y),
                    vec2(PRESET_WIDTH, LED_HEIGHT),
                ),
            ),
            (
                "the metronome's plate",
                below(METRONOME_X - ui::icons::ICON_REACH, METRONOME_WIDTH, METRONOME_HEIGHT),
                Rect::from_center_size(
                    Pos2::new(METRONOME_X, STRIP_Y),
                    Vec2::splat(ui::icons::ICON_SIZE),
                ),
            ),
            (
                "the settings plate",
                below(
                    SETTINGS_X + ui::icons::ICON_REACH - SETTINGS_WIDTH,
                    SETTINGS_WIDTH,
                    settings_height(true, 9),
                ),
                Rect::from_center_size(
                    Pos2::new(SETTINGS_X, STRIP_Y),
                    Vec2::splat(ui::icons::ICON_SIZE),
                ),
            ),
        ]
    }

    #[test]
    fn test_each_plate_opens_under_the_control_that_opens_it_and_inside_the_window() {
        let window =
            Rect::from_min_size(Pos2::ZERO, vec2(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32));
        for (name, plate, opener) in plate_rects() {
            assert!(window.contains_rect(plate), "{name} {plate:?} is out of the window");
            // Under the strip's box, and its opener is over it, not in its way
            assert!(plate.top() > BENCH_MARGIN + STRIP_HEIGHT, "{name}");
            assert!(opener.bottom() < plate.top(), "{name}");
            // Its edge lines up with the opener's: the display's left edge, or under the icon
            assert!(
                plate.left() <= opener.center().x && opener.center().x <= plate.right(),
                "{name} is not under its opener"
            );
        }
    }

    #[test]
    fn test_the_preset_plates_foot_holds_the_name_the_folder_and_save_apart() {
        let foot = DROP_BANK_WIDTH + DROP_PRESET_WIDTH + 2.0 * (5.0 + 4.0) + 12.0;
        let save = foot - FOOT_SAVE_REACH + 3.0;
        let folder = save - FOOT_BUTTON_SPREAD;
        in_a_row(&[
            ("the name display", 0.0, led_width(FOOT_CHARS)),
            ("the Folder button", folder - BUTTON, folder + BUTTON),
            ("the Save button", save - BUTTON, save + BUTTON),
            ("the plate's edge", foot, foot),
        ]);
        // Their captions are under them and clear of each other
        in_a_row(&[
            ("the Folder caption", folder - 20.0, folder + 20.0),
            ("the Save caption", save - 15.0, save + 15.0),
        ]);
        // The display and the buttons with their captions fit the foot's height
        const { assert!(20.0 + LED_HEIGHT / 2.0 < FOOT_HEIGHT) };
        const { assert!(14.0 + 22.0 + 7.0 < FOOT_HEIGHT) };
        // A name typed fits the display, with the caret
        const { assert!(NEW_PRESET_SHOWN < FOOT_CHARS) };
    }

    #[test]
    fn test_the_metronomes_plate_stands_clear_of_itself_and_of_its_edges() {
        let stepper_reach = |width: f32| width / 2.0 + STEPPER_REACH;
        in_a_row(&[
            ("the plate's edge", 0.0, METRONOME_PAD - 8.0),
            (
                "the tempo",
                TEMPO_X - stepper_reach(TEMPO_WIDTH),
                TEMPO_X + stepper_reach(TEMPO_WIDTH),
            ),
            ("Tap", TAP_X - SAVE_REACH, TAP_X + SAVE_REACH),
            ("the steppers", STEPPERS_X - COLUMN_WIDTH / 2.0, STEPPERS_X + COLUMN_WIDTH / 2.0),
            ("the volume", CLICK_VOLUME.x - CLICK_REACH, CLICK_VOLUME.x + CLICK_REACH),
            ("Sync", METRONOME_SYNC_X - SAVE_REACH, METRONOME_SYNC_X + SAVE_REACH),
            ("the plate's edge", METRONOME_WIDTH - METRONOME_PAD + 8.0, METRONOME_WIDTH),
        ]);
        // The narrower stepper is inside the column the wider one makes
        const { assert!(SIGNATURE_WIDTH < SOUND_WIDTH) };
        // Under the sound, the meter and the click's output side by side, clear of each other
        // and inside the column, which the sound's stepper is centred over
        in_a_row(&[
            (
                "the meter",
                SIGNATURE_X - stepper_reach(SIGNATURE_WIDTH),
                SIGNATURE_X + stepper_reach(SIGNATURE_WIDTH),
            ),
            (
                "the click's output",
                CLICK_OUT_X - stepper_reach(OUT_WIDTH),
                CLICK_OUT_X + stepper_reach(OUT_WIDTH),
            ),
        ]);
        assert!(
            SIGNATURE_X - stepper_reach(SIGNATURE_WIDTH) >= STEPPERS_X - COLUMN_WIDTH / 2.0 - 0.01
        );
        assert!(CLICK_OUT_X + stepper_reach(OUT_WIDTH) <= STEPPERS_X + COLUMN_WIDTH / 2.0 + 0.01);
        // Top to bottom in the column: the title, then caption, tape, caption, tape, each clear
        // of the next, all on the plate
        let tape = 11.0;
        let caption = 7.0;
        let column = [
            ui::plate::PLATE_TITLE_HEIGHT,
            SOUND_CAPTION_Y - caption,
            SOUND_CAPTION_Y + caption,
            SOUND_Y - tape,
            SOUND_Y + tape,
            SIGNATURE_CAPTION_Y - caption,
            SIGNATURE_CAPTION_Y + caption,
            SIGNATURE_Y - tape,
            SIGNATURE_Y + tape,
        ];
        assert!(column.windows(2).all(|pair| pair[0] < pair[1]), "{column:?}");
        assert!(column[8] + 8.0 < METRONOME_HEIGHT);
        // The tempo's display and its caption, and Tap, on the same rows as the sound's caption
        assert_eq!(TEMPO_CAPTION_Y, SOUND_CAPTION_Y);
        const { assert!(TEMPO_CAPTION_Y + 7.0 < TEMPO_Y - LED_HEIGHT / 2.0) };
        const { assert!(TEMPO_Y + LED_HEIGHT / 2.0 < METRONOME_HEIGHT) };
        // The volume knob, with its caption and its tape, ends on the plate
        let tape_bottom = CLICK_VOLUME.y + CLICK_RADIUS + 18.0 + 15.0 + 2.0 + 16.0;
        assert!(tape_bottom + 4.0 < METRONOME_HEIGHT, "{tape_bottom}");
        // The Sync button under its LED under its caption, on the plate
        let [caption, led, button] = SYNC_STACK;
        assert!(caption + 8.0 < led - 12.0 && led + 12.0 < button - BUTTON - 2.0);
        assert!(button + BUTTON < METRONOME_HEIGHT);
    }

    #[test]
    fn test_the_settings_plate_holds_every_row_the_note_and_the_buttons() {
        // A row's stepper reaches its buttons past its tape, and all of it is on the plate
        let row = SETTINGS_PAD
            + SETTINGS_CAPTION_WIDTH
            + SETTINGS_OVERHANG
            + SETTINGS_TAPE_WIDTH
            + SETTINGS_OVERHANG;
        assert!(row + SETTINGS_PAD <= SETTINGS_WIDTH + 0.01);
        // The audio's eight rows and the cabinet's, the note and the buttons grow the plate
        let (plugin, standalone) = (settings_height(false, 1), settings_height(true, 9));
        assert!(plugin < standalone);
        assert_eq!(standalone - plugin, 8.0 * SETTINGS_ROW_HEIGHT + SETTINGS_FOOT_HEIGHT,);
        // The longest cabinet name fits the tape
        assert!(SETTINGS_CAB_CHARS as f32 * 8.5 < SETTINGS_TAPE_WIDTH);
        // The plate is on the window under the icon, with room under it for the pedals' titles
        assert!(PLATE_TOP + standalone < WINDOW_HEIGHT as f32);
    }

    #[test]
    fn test_a_stepper_goes_round_its_list() {
        use TimeSignature::*;
        assert_eq!(neighbour(&TimeSignature::ALL, SixEight, Step::Next), TwoFour);
        assert_eq!(neighbour(&TimeSignature::ALL, TwoFour, Step::Previous), SixEight);
        assert_eq!(neighbour(&Sound::ALL, Sound::Beep, Step::Next), Sound::Click);
    }

    // One plate at a time, by the shape of the view: a field holds the plate that is open, so
    // opening one is closing the other
    #[test]
    fn test_opening_a_plate_closes_the_one_that_was_open() {
        let params = GuitarAmpParams::default();
        let mut view = fresh_view(&params, &LampReading::new());
        assert!(view.plate.is_none());
        for which in [Plates::Presets, Plates::Metronome, Plates::Settings, Plates::Metronome] {
            let open = |which| {
                move || match which {
                    Plates::Presets => Open::Presets(Menu {
                        listed: user_presets::FACTORY.to_owned(),
                        files: Vec::new(),
                        naming: None,
                    }),
                    Plates::Metronome => Open::Metronome,
                    Plates::Settings => Open::Settings(None),
                }
            };
            view.toggle_plate(which, None, open(which));
            // It is the plate that is open, and no other
            for other in [Plates::Presets, Plates::Metronome, Plates::Settings] {
                assert_eq!(view.is_open(other), other == which, "{other:?} after {which:?}");
            }
        }
        // Its own icon again closes it
        view.toggle_plate(Plates::Metronome, None, || Open::Metronome);
        assert!(view.plate.is_none());
        // A name being typed does not outlast the plate it was typed on
        view.saving = Some("Big".to_owned());
        view.close_plate(None);
        assert!(view.saving.is_none());
    }

    /// Where the icon of `plate` is, in window points
    #[cfg(target_os = "linux")]
    fn opener(plate: Plates) -> Pos2 {
        match plate {
            Plates::Presets => Pos2::new(PRESET_X, STRIP_Y),
            Plates::Metronome => Pos2::new(METRONOME_X, STRIP_Y),
            Plates::Settings => Pos2::new(SETTINGS_X, STRIP_Y),
        }
    }

    // The same by clicks in the window: an icon opens its plate and the plate that was open is
    // gone, the same icon closes it, and so do a click outside and escape
    #[test]
    #[cfg(target_os = "linux")]
    fn test_no_plate_opens_beside_another() {
        let _folder = FOLDER_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        no_presets_folder();
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = harness_shared(&params, &setter, None, None, |_| {});
        let open = |view: &Rc<RefCell<View>>| -> Vec<Plates> {
            [Plates::Presets, Plates::Metronome, Plates::Settings]
                .into_iter()
                .filter(|&plate| view.borrow().is_open(plate))
                .collect()
        };
        assert!(open(&view).is_empty());
        for plate in [
            Plates::Metronome,
            Plates::Settings,
            Plates::Presets,
            Plates::Metronome,
            Plates::Presets,
        ] {
            click(&mut harness, opener(plate));
            assert_eq!(open(&view), [plate], "after a click on the icon of {plate:?}");
        }
        // The icon of the plate that is open closes it
        click(&mut harness, opener(Plates::Presets));
        assert!(open(&view).is_empty());
        // A click outside it closes it: on the bench between the strip and the line
        click(&mut harness, opener(Plates::Metronome));
        assert_eq!(open(&view), [Plates::Metronome]);
        click(&mut harness, Pos2::new(3.0, 300.0));
        assert!(open(&view).is_empty(), "a click outside left {:?}", open(&view));
        // And so does escape
        click(&mut harness, opener(Plates::Settings));
        assert_eq!(open(&view), [Plates::Settings]);
        harness.press_key(nih_plug_egui::egui::Key::Escape);
        harness.run_ok();
        assert!(open(&view).is_empty(), "escape left {:?}", open(&view));
    }

    // A click inside a plate is the plate's: it neither closes it nor opens another
    #[test]
    #[cfg(target_os = "linux")]
    fn test_a_click_on_a_plate_leaves_it_open() {
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) =
            harness_shared(&params, &setter, None, None, |view| view.plate = Some(Open::Metronome));
        // On its title, where there is no control
        click(&mut harness, Pos2::new(METRONOME_X + 60.0, PLATE_TOP + 19.0));
        assert!(view.borrow().is_open(Plates::Metronome));
    }

    /// A folder for a test of its own, made empty, which the presets are kept in while `body` runs
    #[cfg(target_os = "linux")]
    fn with_presets_in(name: &str, body: impl FnOnce(&std::path::Path)) {
        let _folder = FOLDER_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let dir = crate::user_cab::tests::test_dir(name);
        std::env::set_var(user_presets::FOLDER_ENV, &dir);
        body(&dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // #264: a name for a new preset may hold spaces. Save is pressed, the name is typed in the
    // plate's foot with its space as the host hands it over, and enter keeps it
    #[test]
    #[cfg(target_os = "linux")]
    fn test_a_new_preset_is_saved_under_a_name_with_spaces() {
        use nih_plug_egui::egui::Key;
        with_presets_in("editor-names-with-spaces", |dir| {
            let params = GuitarAmpParams::default();
            let host = NoHost;
            let setter = ParamSetter::new(&host);
            let (mut harness, view) = harness_shared(&params, &setter, None, None, |view| {
                presets_open(view);
                view.saving = Some(String::new());
            });
            // A space that came as text
            type_text(&mut harness, "Big");
            type_text(&mut harness, " ");
            type_text(&mut harness, "Mud");
            assert_eq!(view.borrow().saving.as_deref(), Some("Big Mud"));
            harness.press_key(Key::Enter);
            harness.run_ok();
            assert!(view.borrow().saving.is_none());
            let saved = crate::user_cab::list_with_extension(dir, user_presets::EXTENSION);
            assert_eq!(saved, ["Big Mud.preset"]);
            // The list of the bank has it, with the space
            assert_eq!(view.borrow().bank.files, ["Big Mud.preset"]);
        });
    }

    // The same for a host that takes the space bar for itself and only tells the window the key
    // was pressed, with no character: the name still has its space
    #[test]
    #[cfg(target_os = "linux")]
    fn test_a_space_the_host_hands_over_as_a_key_press_is_in_the_name() {
        use nih_plug_egui::egui::Key;
        with_presets_in("editor-names-space-key", |dir| {
            let params = GuitarAmpParams::default();
            let host = NoHost;
            let setter = ParamSetter::new(&host);
            let (mut harness, view) = harness_shared(&params, &setter, None, None, |view| {
                presets_open(view);
                view.saving = Some(String::new());
            });
            type_text(&mut harness, "Big");
            harness.press_key(Key::Space);
            harness.run_ok();
            type_text(&mut harness, "Mud");
            assert_eq!(view.borrow().saving.as_deref(), Some("Big Mud"));
            harness.press_key(Key::Enter);
            harness.run_ok();
            let saved = crate::user_cab::list_with_extension(dir, user_presets::EXTENSION);
            assert_eq!(saved, ["Big Mud.preset"]);
        });
    }

    // A name is kept as typed when the Save button is pressed a second time: names with spaces
    // are written under file names with spaces, and read back as they were typed
    #[test]
    fn test_a_name_with_spaces_makes_a_file_with_spaces() {
        let taken = vec!["Big Mud.preset".to_owned()];
        assert_eq!(
            user_presets::named_file_name("Big Mud", &[]).as_deref(),
            Some("Big Mud.preset")
        );
        // A name that is taken gets a number, after its space
        assert_eq!(
            user_presets::named_file_name("Big Mud", &taken).as_deref(),
            Some("Big Mud 2.preset")
        );
        // The spaces at its ends are not part of it, those inside are
        assert_eq!(
            user_presets::named_file_name("  Big  Mud ", &[]).as_deref(),
            Some("Big  Mud.preset")
        );
        assert_eq!(user_presets::display_name("Big Mud.preset", 17), "BIG MUD");
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

    /// The faces of every amp's head in the lane it stands in (the first or the second)
    fn heads() -> impl Iterator<Item = (Amp, Rect, ui::HeadFaces)> {
        let head =
            Rect::from_min_size(Pos2::new(0.0, STACK_TOP), vec2(LANE_HEAD_WIDTH, LANE_HEIGHT));
        Amp::ALL
            .into_iter()
            .map(move |amp| (amp, head, ui::head_faces(head, &looks::look(amp).head)))
    }

    // The jack's nut, and the pilot lamp in its bezel, half across
    const JACK: f32 = 8.0;
    const PILOT: f32 = 9.0;

    #[test]
    fn test_the_lanes_fill_the_stack_and_stand_on_the_floor_with_the_pedals() {
        assert_eq!(2.0 * LANE_HEIGHT + LANE_GAP, HEAD_HEIGHT + PEDAL_HEIGHT);
        assert_eq!(PEDAL_TOP + PEDAL_HEIGHT + BENCH_MARGIN, WINDOW_HEIGHT as f32);
        // Head, cabinet and plate across the stack, each clear of the next, with room for
        // the couplers
        let cab_left = LANE_HEAD_WIDTH + LANE_CAB_GAP;
        in_a_row(&[
            ("the head", 0.0, LANE_HEAD_WIDTH),
            ("the cabinet", cab_left, cab_left + LANE_CAB_WIDTH),
            ("the plate", MIX_X, STACK_WIDTH),
        ]);
        assert_eq!(MIX_X - (cab_left + LANE_CAB_WIDTH), MIX_GAP - LANE_CAB_GAP);
    }

    #[test]
    fn test_the_head_panel_holds_the_jack_the_dials_and_the_lamp_in_a_row() {
        for (amp, _, faces) in heads() {
            let corner = faces.panel.left_top();
            let jack = corner + vec2(LANE_JACK_X, DIAL_Y);
            let jewel = faces.panel.right() - ui::amp::JEWEL_INSET;
            let mut row = vec![
                ("the panel's edge", 0.0, faces.panel.left()),
                ("the jack", jack.x - JACK, jack.x + JACK),
            ];
            for (index, label) in DIALS.iter().enumerate() {
                let at =
                    corner + vec2(LANE_DIAL_FIRST_X + LANE_DIAL_SPACING * index as f32, DIAL_Y);
                let room = ui::panel_knob_rect(at, LANE_DIAL_RADIUS);
                // Its scale, caption and tape all on the panel
                assert!(faces.panel.contains_rect(room), "{amp:?}: {label} is off the panel");
                row.push((label, room.left(), room.right()));
            }
            row.push(("the pilot lamp", jewel - PILOT, jewel + PILOT));
            row.push(("the panel's edge", faces.panel.right(), LANE_HEAD_WIDTH));
            in_a_row(&row);
            // The jack's caption over the jack, on the panel
            let first = corner + vec2(LANE_DIAL_FIRST_X, DIAL_Y);
            let caption = ui::panel_caption(first, LANE_DIAL_RADIUS).y;
            assert!(caption + 7.0 < corner.y + DIAL_Y - JACK);
            assert!(caption - 7.0 > faces.panel.top(), "{amp:?}");
        }
    }

    #[test]
    fn test_the_line_under_the_dials_holds_the_name_the_cabinet_and_the_switch_apart() {
        for (amp, _, faces) in heads() {
            let line = DIAL_Y + LANE_LINE_DY;
            let stepper = reach(LANE_CAB_STEPPER_WIDTH) + BUTTON;
            let right = faces.panel.width();
            // Across: the name, the cabinet's stepper, the switch's caption, LED and switch
            in_a_row(&[
                ("the panel's edge", 0.0, 0.0),
                ("the name", LANE_NAME_X - 32.0, LANE_NAME_X + 32.0),
                ("the cabinet", LANE_CAB_X - stepper, LANE_CAB_X + stepper),
                (
                    "the On caption",
                    right + LANE_ON_CAPTION_DX - 10.0,
                    right + LANE_ON_CAPTION_DX + 10.0,
                ),
                ("the On LED", right + LANE_ON_LED_DX - LAMP, right + LANE_ON_LED_DX + LAMP),
                (
                    "the On switch",
                    right + LANE_ON_SWITCH_DX - SWITCH,
                    right + LANE_ON_SWITCH_DX + SWITCH,
                ),
                ("the panel's edge", right, right),
            ]);
            // Down: the dials' tapes, then the cabinet's caption, then its stepper, and the
            // switch inside the panel
            let tape = ui::panel_knob_rect(Pos2::new(0.0, DIAL_Y), LANE_DIAL_RADIUS).bottom();
            let caption_top = line + LANE_CAB_CAPTION_DY - 7.0;
            let caption_bottom = line + LANE_CAB_CAPTION_DY + 7.0;
            assert!(tape + 2.0 < caption_top, "{amp:?}: the caption is under the dials' tapes");
            assert!(caption_bottom + 4.0 < line - 11.0, "{amp:?}");
            assert!(line + SWITCH < faces.panel.height(), "{amp:?}: the switch is off the panel");
        }
    }

    #[test]
    fn test_the_plate_holds_the_fader_and_what_the_cabinets_share_in_order() {
        let knob = MIX_KNOB_RADIUS + TICKS;
        let half = MIX_WIDTH / 2.0 - PLATE_FRAME_INSET;
        let (cap, ticks) = (ui::widgets::FADER_CAP_HALF_WIDTH, ui::widgets::FADER_REACH);
        let stepper = reach(MIC_STEPPER_WIDTH) + BUTTON;
        // Across, inside the frames: the Blend zone's fader with its tape beside it, the
        // Cabinets zone's knobs, stepper, LED and switch
        let tape = 19.0;
        for (name, from, to) in [
            ("the fader's cap", FADER_DX - cap, FADER_DX + cap),
            ("the fader's ticks", FADER_DX - ticks, FADER_DX + ticks),
            ("the Blend tape", MIX_BLEND_TAPE_DX - tape, MIX_BLEND_TAPE_DX + tape),
            ("the Mic knob", -MIX_KNOB_DX - knob, -MIX_KNOB_DX + knob),
            ("the Res knob", MIX_KNOB_DX - knob, MIX_KNOB_DX + knob),
            ("the microphone's stepper", -stepper, stepper),
            ("the Cabs LED", MIX_LED_DX - LAMP, MIX_LED_DX + LAMP),
            ("the Cabs switch", MIX_SWITCH_DX - SWITCH, MIX_SWITCH_DX + SWITCH),
        ] {
            assert!(-half + 4.0 < from && to < half - 4.0, "{name} is outside its frame");
        }
        // The tape is clear of the fader's ticks, the LED of the switch, the knobs of each other
        assert!(FADER_DX + ticks + 4.0 < MIX_BLEND_TAPE_DX - tape);
        const {
            assert!(MIX_LED_DX + LAMP + 4.0 < MIX_SWITCH_DX - SWITCH);
        }
        assert!(-MIX_KNOB_DX + knob + 4.0 < MIX_KNOB_DX - knob);
        // Down, from the top, each clear of the next; a frame's title is on its top line
        let cap_half = ui::widgets::FADER_CAP_HEIGHT / 2.0;
        let end = MIX_END_CAPTION_SIZE / 2.0;
        let knob_tape = MIX_KNOB_Y + MIX_KNOB_RADIUS + 18.0 + 15.0 + 2.0 + 16.0;
        in_a_row(&[
            ("the plate's top", 0.0, 0.0),
            ("the Blend frame's title", BLEND_FRAME_TOP - 8.0, BLEND_FRAME_TOP + 8.0),
            ("the Amp 1 caption", MIX_AMP1_CAPTION_Y - end, MIX_AMP1_CAPTION_Y + end),
            ("the fader", FADER_TOP - cap_half, FADER_TOP + FADER_LENGTH + cap_half),
            ("the Amp 2 caption", MIX_AMP2_CAPTION_Y - end, MIX_AMP2_CAPTION_Y + end),
            ("the Blend frame", BLEND_FRAME_BOTTOM, BLEND_FRAME_BOTTOM),
            ("the Cabinets frame's title", CABS_FRAME_TOP - 8.0, CABS_FRAME_TOP + 8.0),
            ("the Mic and Res knobs", MIX_KNOB_Y - MIX_KNOB_RADIUS - TICKS, knob_tape),
            ("the microphone's caption", MIC_CAPTION_Y - 7.0, MIC_CAPTION_Y + 7.0),
            ("the microphone's stepper", MIC_STEPPER_Y - 11.0, MIC_STEPPER_Y + 11.0),
            ("the Cabs caption", MIX_SWITCH_CAPTION_Y - 7.0, MIX_SWITCH_CAPTION_Y + 7.0),
            ("the Cabs LED", MIX_LED_Y - LAMP, MIX_LED_Y + LAMP),
            ("the Cabinets frame", CABS_FRAME_BOTTOM, CABS_FRAME_BOTTOM),
            ("the output jack", MIX_JACK_Y - JACK, MIX_JACK_Y + JACK),
            ("the plate's bottom", 2.0 * LANE_HEIGHT + LANE_GAP, 2.0 * LANE_HEIGHT + LANE_GAP),
        ]);
        // The switch beside its LED is under the stepper and inside its frame
        const {
            assert!(MIC_STEPPER_Y + 11.0 + 4.0 < MIX_SWITCH_Y - SWITCH);
        }
        const {
            assert!(MIX_SWITCH_Y + SWITCH + 4.0 < CABS_FRAME_BOTTOM);
        }
        // The jack stays inside the plate's screws, and its caption is beside it
        const {
            assert!(MIX_JACK_CAPTION_DX < -JACK - 12.0);
        }
        assert!(MIX_JACK_DX + JACK < half - 4.0);
        assert!(MIX_JACK_DX + MIX_JACK_CAPTION_DX - 14.0 > -half);
    }

    #[test]
    fn test_the_nameplate_and_its_buttons_sit_on_the_grille_clear_of_the_handle() {
        for (amp, head, faces) in heads() {
            let nameplate = faces.grille.center();
            let plate = Rect::from_center_size(
                nameplate,
                vec2(NAMEPLATE_WIDTH, ui::widgets::NAMEPLATE_HEIGHT),
            );
            assert!(faces.grille.contains_rect(plate), "{amp:?}: {plate:?}");
            let reach = reach(NAMEPLATE_WIDTH) - 2.0;
            let buttons =
                Rect::from_center_size(nameplate, vec2(2.0 * (reach + BUTTON), 2.0 * BUTTON));
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

    #[test]
    fn test_default_window_scales_with_screen_and_fits_inside_each_screen() {
        assert_eq!(size_for_screen(1366), (1322, MIN_HEIGHT));
        assert_eq!(size_for_screen(1920), (1876, 321));
        assert_eq!(size_for_screen(2560), (2516, 431));
        assert_eq!(size_for_screen(2752), (2708, 464));
        assert_eq!(size_for_screen(3440), (3396, 581));
        assert_eq!(size_for_screen(3840), (WINDOW_WIDTH, WINDOW_HEIGHT));
        for screen_w in [1366, 1536, 1920, 2560, 2752, 3440, 3840] {
            let (width, height) = size_for_screen(screen_w);
            assert!(
                (MIN_WIDTH..=WINDOW_WIDTH).contains(&width),
                "width {width} for screen {screen_w} is outside {MIN_WIDTH}..={WINDOW_WIDTH}"
            );
            assert!(
                (MIN_HEIGHT..=WINDOW_HEIGHT).contains(&height),
                "height {height} for screen {screen_w} is outside {MIN_HEIGHT}..={WINDOW_HEIGHT}"
            );
            if screen_w >= MIN_WIDTH + SCREEN_MARGIN {
                assert!(
                    width + SCREEN_MARGIN <= screen_w,
                    "width {width} leaves less than {SCREEN_MARGIN} px on a {screen_w} px screen"
                );
            }
        }
        let (width, height) = default_state().size();
        assert!((MIN_WIDTH..=WINDOW_WIDTH).contains(&width));
        assert!((MIN_HEIGHT..=WINDOW_HEIGHT).contains(&height));
    }

    /// A window of `width` by `height` points for a line `line` wide
    fn fit(width: f32, height: f32, line: f32) -> Fit {
        Fit::of(vec2(width, height), line)
    }

    #[test]
    fn test_the_zoom_fits_the_layer_in_the_window_and_stays_between_its_limits() {
        let default_line = WINDOW_WIDTH as f32;
        // The window that opens on a 1920 screen shows the twelve-pedal line whole, at 0.55
        let opening = fit(1876.0, 321.0, default_line);
        assert!((opening.zoom - DEFAULT_ZOOM).abs() < 0.001);
        assert_eq!(opening.furthest(), 0.0);
        assert_eq!(opening.layer, default_line);
        // Wider than the line needs: no bigger than full size, the strip as wide as the window
        let wide = fit(5000.0, 900.0, default_line);
        assert_eq!(wide.zoom, MAX_ZOOM);
        assert_eq!(wide.strip_width(), 5000.0);
        assert_eq!(wide.furthest(), 0.0);
        // A window as wide as the line wants but too low for it: the height decides
        let low = fit(3000.0, 400.0, default_line);
        assert!((low.zoom - 400.0 / WINDOW_HEIGHT as f32).abs() < 0.001);
        assert!(low.visible > default_line);
        // Nothing is scaled below the minimum, nor made above the maximum
        for width in [100.0, 1301.0, 1500.0, 2400.0, 4000.0, 9000.0] {
            for height in [50.0, 234.0, 584.0, 2000.0] {
                for line in [default_line / 2.0, default_line, 4239.0, 9000.0] {
                    let fit = fit(width, height, line);
                    assert!((MIN_ZOOM..=MAX_ZOOM).contains(&fit.zoom), "{fit:?}");
                    assert!(fit.strip_width() >= STRIP_MIN_WIDTH);
                    assert!(fit.layer >= line.min(STRIP_MIN_WIDTH.max(line)));
                }
            }
        }
    }

    #[test]
    fn test_a_line_too_long_for_the_minimum_zoom_scrolls_and_shorter_ones_do_not() {
        // Sixteen blocks and a gate, the stack and the gaps: 4239 points
        let long = Line::of_chain(&full_chain_of_sixteen()).width();
        assert_eq!(long, 4239.0);
        // A window of 2120 shows it whole, at the least zoom there is
        let fits = fit(2120.0, 292.0, long);
        assert!((fits.zoom - MIN_ZOOM).abs() < 0.001, "{fits:?}");
        assert_eq!(fits.furthest(), 0.0, "{fits:?}");
        // On the window of a 1920 screen it would be under the minimum: scrolled instead, the
        // blocks no smaller than the minimum makes them
        let scrolls = fit(1876.0, 321.0, long);
        assert_eq!(scrolls.zoom, MIN_ZOOM);
        assert!((scrolls.furthest() - (long - 1876.0 / MIN_ZOOM)).abs() < 0.01);
        assert!(scrolls.furthest() > 0.0);
        // Today's twelve-pedal line scrolls only below 1706 points
        assert_eq!(fit(1706.0, 292.0, 3411.0).furthest(), 0.0);
        assert!(fit(1500.0, 292.0, 3411.0).furthest() > 0.0);
        // The strip is what the window shows, never less than its controls need
        assert!(scrolls.strip_width() >= STRIP_MIN_WIDTH);
        // The window's smallest size shows a whole strip
        let smallest = fit(MIN_WIDTH as f32, MIN_HEIGHT as f32, long);
        assert!(smallest.visible >= STRIP_MIN_WIDTH, "{smallest:?}");
        // The strip's places are those of the default width where the layer is that wide
        assert_eq!(fit(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32, 3411.0).shift(), 0.0);
    }

    // Sixteen blocks, eight on each side, with no more than `MAX_PER_KIND` of a kind
    fn full_chain_of_sixteen() -> Chain {
        sixteen_blocks()
    }

    // The effects bank (#272): the plus in a gap, the sheet that rises, the card dragged into a
    // gap, the cross on a block, the empty chain and the full one

    /// The pointer moved to `at` (window points at full size), and a frame drawn
    #[cfg(target_os = "linux")]
    fn hover(harness: &mut Harness, at: Pos2) {
        use nih_plug_egui::egui::Event;
        harness.input_mut().events.push(Event::PointerMoved(at));
        harness.run_ok();
        harness.run_ok();
    }

    /// The button pressed at `at`, the pointer carried to `to`, and not let go
    #[cfg(target_os = "linux")]
    fn carry(harness: &mut Harness, from: Pos2, to: Pos2) {
        use nih_plug_egui::egui::{Event, Modifiers, PointerButton};
        let press = Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        };
        for event in [
            Event::PointerMoved(from),
            press,
            Event::PointerMoved(from + vec2(6.0, -6.0)),
            Event::PointerMoved(from + (to - from) * 0.5),
            Event::PointerMoved(to),
        ] {
            harness.input_mut().events.push(event);
            harness.run_ok();
        }
        harness.run_ok();
    }

    /// The button let go at `at`
    #[cfg(target_os = "linux")]
    fn release(harness: &mut Harness, at: Pos2) {
        use nih_plug_egui::egui::{Event, Modifiers, PointerButton};
        harness.input_mut().events.push(Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        });
        harness.run_ok();
        harness.run_ok();
    }

    #[cfg(target_os = "linux")]
    fn press_escape(harness: &mut Harness) {
        use nih_plug_egui::egui::{Event, Key, Modifiers};
        harness.input_mut().events.push(Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        harness.run_ok();
    }

    /// The bank open from `from`, risen: as it is a moment after a plus was clicked
    #[cfg(target_os = "linux")]
    fn risen(view: &Rc<RefCell<View>>, from: (crate::order::Zone, usize)) {
        let mut sheet = crate::bank::Sheet::opened(from, -10.0, None);
        sheet.fresh = false;
        view.borrow_mut().effects = Some(sheet);
    }

    /// The effects most of the bank's frames are made with: seven of the twelve, the rest not in
    /// the chain
    fn half_chain() -> Chain {
        use EffectKind::*;
        chain_of(&[Fuzz, Comp, Drive, Phaser], &[Chorus, Delay, Reverb])
    }

    /// Where the plus of gap `place` of `zone` stands, in window points at full size
    fn gap_plus(chain: &Chain, zone: crate::order::Zone, place: usize) -> Pos2 {
        let gap = Line::of_chain(chain)
            .gaps()
            .into_iter()
            .find(|gap| gap.zone == zone && gap.place == place)
            .unwrap();
        Pos2::new(gap.x, PEDAL_TOP + ui::PLUG_Y)
    }

    #[cfg(target_os = "linux")]
    fn bank_harness<'a>(
        params: &'a GuitarAmpParams,
        setter: &'a ParamSetter<'a>,
    ) -> (Harness<'a>, Rc<RefCell<View>>) {
        line_harness(WINDOW_WIDTH, WINDOW_HEIGHT, params, setter, 0.0)
    }

    // The pointer over a gap shows its plus and where it would add; a click on the plus raises the
    // bank, which stays open for that gap
    #[test]
    #[cfg(target_os = "linux")]
    fn test_a_gap_shows_its_plus_under_the_pointer_and_the_plus_opens_the_bank() {
        use crate::order::Zone;
        let params = GuitarAmpParams::default();
        params.chain.put(half_chain());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = bank_harness(&params, &setter);
        // Between the drive and the phaser
        let plus = gap_plus(&params.chain.get(), Zone::Front, 3);
        hover(&mut harness, plus + vec2(0.0, -40.0));
        harness.snapshot("bank_hover_gap");
        assert!(view.borrow().effects.is_none());
        click(&mut harness, plus);
        let opened = view.borrow().effects.clone().expect("the plus opens the bank");
        assert_eq!(opened.from, (Zone::Front, 3));
        // The click that opened it did not close it again
        harness.run_ok();
        assert!(view.borrow().effects.is_some());
        // It stays where the pointer left; the gap it adds to keeps its plus
        hover(&mut harness, Pos2::new(300.0, 100.0));
        assert!(view.borrow().effects.is_some());
    }

    // The pointer over a block outlines it and shows the cross on its corner
    #[test]
    #[cfg(target_os = "linux")]
    fn test_a_block_under_the_pointer_shows_its_cross() {
        let params = GuitarAmpParams::default();
        params.chain.put(half_chain());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, _) = bank_harness(&params, &setter);
        let line = Line::of_chain(&params.chain.get());
        // The comp, second of the pieces in front of the amps
        let comp = line.places_for_test()[2];
        hover(&mut harness, Pos2::new(comp + PEDAL_WIDTH / 2.0, PEDAL_TOP + 120.0));
        harness.snapshot("bank_hover_block");
    }

    // The cross takes the block out with its settings kept, raises the bank with the note, and
    // Undo puts it back where it stood
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_cross_takes_a_block_out_and_undo_puts_it_back() {
        use crate::order::Zone;
        let params = GuitarAmpParams::default();
        params.chain.put(half_chain());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = bank_harness(&params, &setter);
        let line = Line::of_chain(&params.chain.get());
        let comp = line.places_for_test()[2];
        let corner = Pos2::new(comp + PEDAL_WIDTH - 24.0, PEDAL_TOP + 24.0);
        hover(&mut harness, Pos2::new(comp + PEDAL_WIDTH / 2.0, PEDAL_TOP + 120.0));
        click(&mut harness, corner);
        let chain = params.chain.get();
        assert_eq!(chain.blocks().len(), 6);
        assert!(!chain.blocks().iter().any(|block| block.kind == EffectKind::Comp));
        let kept = view.borrow().effects.as_ref().and_then(|sheet| sheet.kept);
        assert_eq!(
            kept,
            Some(crate::bank::Kept { kind: EffectKind::Comp, zone: Zone::Front, place: 1 })
        );
        // Risen: the sheet and its note in a picture
        view.borrow_mut().effects.as_mut().unwrap().since = -10.0;
        view.borrow_mut().effects.as_mut().unwrap().fresh = false;
        hover(&mut harness, Pos2::new(1600.0, 60.0));
        harness.snapshot("bank_taken_out");

        // Undo, on the note's right
        let sheet = crate::bank::risen_rect(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
        click(&mut harness, crate::bank::undo_rect(sheet).center());
        let chain = params.chain.get();
        assert_eq!(chain.blocks().len(), 7);
        assert_eq!(chain.front()[1].kind, EffectKind::Comp);
        assert!(view.borrow().effects.as_ref().is_some_and(|sheet| sheet.kept.is_none()));
    }

    // The sheet covers what is under it: the layer under the pointer is the sheet's, a click on
    // its empty metal reaches nothing below, and a click on a card adds that effect where the
    // bank was opened from
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_sheet_covers_the_line_and_a_click_on_a_card_adds_the_effect() {
        use crate::order::Zone;
        use nih_plug_egui::egui::Order;
        let params = GuitarAmpParams::default();
        params.chain.put(half_chain());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = bank_harness(&params, &setter);
        risen(&view, (Zone::Front, 3));
        harness.run_ok();
        harness.run_ok();
        harness.snapshot("bank_open");
        let top = WINDOW_HEIGHT as f32 - crate::bank::SHEET_HEIGHT;
        let order_at =
            |harness: &Harness, at: Pos2| harness.ctx.layer_id_at(at).map(|layer| layer.order);
        assert_eq!(order_at(&harness, Pos2::new(400.0, top + 100.0)), Some(Order::Foreground));
        // The foot switch of a pedal is under the sheet
        assert_eq!(
            order_at(&harness, Pos2::new(400.0, PEDAL_TOP + 273.0)),
            Some(Order::Foreground)
        );
        assert_ne!(order_at(&harness, Pos2::new(400.0, top - 40.0)), Some(Order::Foreground));
        // A click on the bare metal between the two rows of cards, over a foot switch
        click(&mut harness, Pos2::new(400.0, top + 52.0 + 64.0 + 4.0));
        assert!(view.borrow().effects.is_some(), "a click on the sheet leaves it open");
        assert_eq!(params.chain.get().blocks().len(), 7);

        // The first card is the pitch shifter: not in the chain, so it takes the slot of its own
        let sheet = crate::bank::risen_rect(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
        click(&mut harness, crate::bank::card_rect(sheet, 0).center());
        let chain = params.chain.get();
        assert_eq!(chain.blocks().len(), 8);
        assert_eq!(chain.front()[3].kind, EffectKind::Pitch);
        assert!(Chain::is_home(chain.front()[3]));
        // The gap it adds to moves on past it, so the next card follows it
        assert_eq!(view.borrow().effects.as_ref().map(|sheet| sheet.from), Some((Zone::Front, 4)));
    }

    // A card dragged out of the sheet opens a place in the nearest gap and lands there when it is
    // let go over the line; let go over the sheet it lands nowhere
    #[test]
    #[cfg(target_os = "linux")]
    fn test_a_card_dragged_into_a_gap_opens_a_place_and_lands_there() {
        use crate::order::Zone;
        let params = GuitarAmpParams::default();
        params.chain.put(half_chain());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = bank_harness(&params, &setter);
        risen(&view, (Zone::Front, 3));
        harness.run_ok();
        let sheet = crate::bank::risen_rect(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
        // The flanger: second row, third card
        let flanger = crate::bank::card_rect(sheet, 8).center();
        let target = Pos2::new(gap_plus(&params.chain.get(), Zone::Front, 2).x, PEDAL_TOP + 40.0);
        carry(&mut harness, flanger, target);
        harness.snapshot("bank_dragging");
        let dragged = view.borrow().effects.as_ref().and_then(|sheet| sheet.drag);
        assert!(dragged.is_some_and(|drag| drag.kind != EffectKind::Gate));
        release(&mut harness, target);
        for _ in 0..4 {
            harness.run_ok();
        }
        let chain = params.chain.get();
        assert_eq!(chain.blocks().len(), 8, "{:?}", chain.blocks());
        assert!(view.borrow().effects.as_ref().is_some_and(|sheet| sheet.drag.is_none()));
        let kinds: Vec<_> = chain.front().iter().map(|block| block.kind).collect();
        assert_eq!(kinds.len(), 5);
        assert_eq!(kinds[2], dragged.unwrap().kind);

        // Let go over the sheet: nothing is added
        carry(&mut harness, flanger, flanger + vec2(300.0, 20.0));
        release(&mut harness, flanger + vec2(300.0, 20.0));
        assert_eq!(params.chain.get().blocks().len(), 8);
    }

    // Escape, a click outside and the sheet's handle close it; a plate takes its place and the
    // other way round, so one is open at a time; typed text does not close it
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_sheet_closes_on_escape_a_click_outside_and_its_handle() {
        use crate::order::Zone;
        let params = GuitarAmpParams::default();
        params.chain.put(half_chain());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = bank_harness(&params, &setter);
        let top = WINDOW_HEIGHT as f32 - crate::bank::SHEET_HEIGHT;

        risen(&view, (Zone::Front, 3));
        harness.run_ok();
        type_text(&mut harness, "abc");
        assert!(view.borrow().effects.is_some());
        press_escape(&mut harness);
        assert!(view.borrow().effects.is_none());

        risen(&view, (Zone::Front, 3));
        harness.run_ok();
        click(&mut harness, Pos2::new(1500.0, 40.0));
        assert!(view.borrow().effects.is_none());

        risen(&view, (Zone::Front, 3));
        harness.run_ok();
        click(&mut harness, Pos2::new(WINDOW_WIDTH as f32 / 2.0, top + 11.0));
        assert!(view.borrow().effects.is_none());

        // The sheet's own Close
        risen(&view, (Zone::Front, 3));
        harness.run_ok();
        click(
            &mut harness,
            crate::bank::close_rect(crate::bank::risen_rect(
                WINDOW_WIDTH as f32,
                WINDOW_HEIGHT as f32,
            ))
            .center(),
        );
        assert!(view.borrow().effects.is_none());

        // Opening a plate takes the sheet down
        risen(&view, (Zone::Front, 3));
        harness.run_ok();
        click(&mut harness, Pos2::new(SETTINGS_X, STRIP_Y));
        assert!(view.borrow().effects.is_none());
        assert!(view.borrow().is_open(Plates::Settings));
        // And the sheet takes the plate down
        let plus = gap_plus(&params.chain.get(), Zone::Front, 3);
        hover(&mut harness, plus + vec2(0.0, -40.0));
        click(&mut harness, plus);
        assert!(view.borrow().effects.is_some());
        assert!(view.borrow().plate.is_none());
    }

    // The bank is a sheet with no text field: typed text is neither taken nor does it close the
    // sheet, and the place text does go (the preset name) still takes it once the sheet is down
    #[test]
    #[cfg(target_os = "linux")]
    fn test_typed_text_still_works_with_the_bank_in_the_window() {
        use crate::order::Zone;
        with_presets_in("editor-typing-with-the-bank", |_| {
            let params = GuitarAmpParams::default();
            params.chain.put(half_chain());
            let host = NoHost;
            let setter = ParamSetter::new(&host);
            let (mut harness, view) = bank_harness(&params, &setter);
            risen(&view, (Zone::Front, 3));
            harness.run_ok();
            type_text(&mut harness, "Big Mud");
            assert!(view.borrow().effects.is_some(), "typing does not close the sheet");
            assert!(!harness.ctx.wants_keyboard_input(), "the sheet takes no keyboard focus");
            assert_eq!(params.chain.get().blocks().len(), 7);
            // A plate opens under its icon, takes the sheet down, and then takes the typing
            click(&mut harness, Pos2::new(PRESET_X, STRIP_Y));
            assert!(view.borrow().effects.is_none());
            assert!(view.borrow().is_open(Plates::Presets));
            view.borrow_mut().saving = Some(String::new());
            type_text(&mut harness, "Big");
            type_text(&mut harness, " ");
            type_text(&mut harness, "Mud");
            assert_eq!(view.borrow().saving.as_deref(), Some("Big Mud"));
        });
    }

    // The pointer over the amps shows the plus that adds one while the chain has fewer lanes than
    // the engine has (here one of two; with two of two nothing is offered, and amp 1 has no cross)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_amps_show_a_plus_while_the_engine_has_room_for_another() {
        let params = GuitarAmpParams::default();
        params.chain.put(half_chain());
        assert_eq!(params.chain.get().amps(), 1);
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, _) = bank_harness(&params, &setter);
        let line = Line::of_chain(&params.chain.get());
        let stack = line.places_for_test()[5];
        let over_amp = Pos2::new(stack + 200.0, STACK_TOP + 100.0);
        hover(&mut harness, over_amp);
        harness.snapshot("bank_hover_amp");
        // Its plus, near the stack's foot
        let plus = Pos2::new(stack + STACK_WIDTH / 2.0 - 80.0, STACK_TOP + STACK_HEIGHT - 18.0);
        click(&mut harness, plus);
        assert_eq!(params.chain.get().amps(), 2);
        // Two of two: the pointer over the amps offers nothing
        hover(&mut harness, over_amp);
        click(&mut harness, plus);
        assert_eq!(params.chain.get().amps(), 2);
        let mut chain = params.chain.get();
        assert!(!chain.remove_amp(0), "amp 1 cannot be taken out");
    }

    // The line at rest with a chain of seven, the bank down
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_line_at_rest_with_the_bank_down_looks_as_it_did() {
        let params = GuitarAmpParams::default();
        params.chain.put(half_chain());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        bank_harness(&params, &setter).0.snapshot("bank_rest");
    }

    // The print on a card reads as well as on the pedal it stands for
    #[test]
    fn test_the_print_on_every_card_reads_as_well_as_on_a_pedal() {
        for kind in crate::bank::CARDS {
            let ground = crate::bank::card_ground(kind);
            let read = theme::contrast(theme::SILK, ground);
            assert!(read >= theme::least_contrast(), "{kind:?}: {read}");
        }
    }

    // A chain with no blocks is a dashed place on each side of the amps, each with a plus; a
    // card dropped on one lands there
    #[test]
    #[cfg(target_os = "linux")]
    fn test_an_empty_chain_invites_the_first_effect() {
        use crate::order::Zone;
        let params = GuitarAmpParams::default();
        params.chain.put(Chain::empty());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = bank_harness(&params, &setter);
        harness.snapshot("bank_empty");
        let line = Line::of_chain(&params.chain.get());
        let slots: Vec<_> = line.gaps().into_iter().filter(|gap| gap.slot).collect();
        assert_eq!(slots.len(), 2);
        // The plus of the first
        click(&mut harness, Pos2::new(slots[0].x, PEDAL_TOP + PEDAL_HEIGHT / 2.0 - 40.0));
        assert_eq!(view.borrow().effects.as_ref().map(|sheet| sheet.from), Some((Zone::Front, 0)));
        view.borrow_mut().effects.as_mut().unwrap().fresh = false;
        harness.run_ok();
        // A card clicked lands in front of the amps
        view.borrow_mut().effects.as_mut().unwrap().since = -10.0;
        harness.run_ok();
        // The fuzz, fourth card
        let sheet = crate::bank::risen_rect(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
        click(&mut harness, crate::bank::card_rect(sheet, 3).center());
        let chain = params.chain.get();
        assert_eq!(chain.blocks().len(), 1);
        assert_eq!(chain.front().len(), 1);
        assert_eq!(chain.front()[0].kind, EffectKind::Fuzz);
    }

    // Sixteen blocks: the plus is not offered, and the line says why
    #[test]
    #[cfg(target_os = "linux")]
    fn test_sixteen_blocks_offer_no_plus_and_say_why() {
        let params = GuitarAmpParams::default();
        params.chain.put(sixteen_blocks());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = line_harness(2120, 292, &params, &setter, 0.0);
        let plus = gap_plus(&params.chain.get(), crate::order::Zone::Front, 3) * MIN_ZOOM;
        hover(&mut harness, plus + vec2(0.0, -20.0));
        harness.snapshot("bank_full");
        click(&mut harness, plus);
        assert!(view.borrow().effects.is_none());
    }

    // An open sheet, a gap with its plus and a window at rest ask for no repaint: the sheet's
    // rise is the only thing that moves, and it is over once it has risen
    #[test]
    #[cfg(target_os = "linux")]
    fn test_an_open_sheet_at_rest_asks_for_no_repaint() {
        use crate::order::Zone;
        let params = GuitarAmpParams::default();
        params.chain.put(half_chain());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = bank_harness(&params, &setter);
        risen(&view, (Zone::Front, 3));
        // Past the sheet's own fade in
        for _ in 0..20 {
            harness.step();
        }
        let delays: Vec<_> = harness
            .output()
            .viewport_output
            .values()
            .map(|viewport| viewport.repaint_delay)
            .collect();
        assert!(!delays.is_empty());
        assert!(delays.iter().all(|delay| *delay == std::time::Duration::MAX), "{delays:?}");
    }

    // The line the chain makes and the gaps it has: one after the gate, one after each block,
    // one on either side of the stack's neighbours, one at the end; a card lands in the nearest
    #[test]
    fn test_the_gaps_of_a_line_and_the_one_a_card_lands_in() {
        use crate::order::Zone;
        let chain = half_chain();
        let line = Line::of_chain(&chain);
        let gaps = line.gaps();
        // gate + 4 blocks in front, the stack, 3 blocks behind: eight pieces, a gap after each
        assert_eq!(gaps.len(), 9);
        assert_eq!((gaps[0].zone, gaps[0].place), (Zone::Front, 0));
        assert_eq!((gaps[4].zone, gaps[4].place), (Zone::Front, 4));
        assert_eq!((gaps[5].zone, gaps[5].place), (Zone::Back, 0));
        assert_eq!((gaps[8].zone, gaps[8].place), (Zone::Back, 3));
        for pair in gaps.windows(2) {
            assert!(pair[0].x < pair[1].x);
        }
        let nearest =
            |x: f32| line.drop_target(EffectKind::Delay, x).map(|gap| (gap.zone, gap.place));
        assert_eq!(nearest(gaps[2].x + 5.0), Some((Zone::Front, 2)));
        assert_eq!(nearest(-100.0), Some((Zone::Front, 0)));
        assert_eq!(nearest(1.0e6), Some((Zone::Back, 3)));
        // The last gap stays inside the layer
        assert!(gaps[8].x + suite_common::ui::PLUS_RADIUS < line.width());
        // An empty chain has a place on each side of the amps and no other gap
        let empty = Line::of_chain(&Chain::empty());
        let slots = empty.gaps();
        assert_eq!(slots.len(), 2);
        assert!(slots.iter().all(|gap| gap.slot && gap.place == 0));
        assert_eq!(slots[0].zone, Zone::Front);
        assert_eq!(slots[1].zone, Zone::Back);
    }
}
