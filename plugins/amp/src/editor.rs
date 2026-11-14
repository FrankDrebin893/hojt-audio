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
use crate::looper::{LooperControl, State as LoopState};
use crate::metronome::{BeatReading, ClickOut, Sound, TapTempo, TimeSignature, BPM_MAX, BPM_MIN};
use crate::order::Zone;
use crate::presets::{self, PRESETS};
use crate::slots::{Block, Chain, EffectKind, Live, SlotParams};
#[cfg(all(test, target_os = "linux"))]
use crate::standalone::Prefs;
use crate::standalone::{self, Applied, AudioChoice};
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

// The icons stand in the middle of the strip, a pitch apart: the metronome, the looper, the tuner
// and the settings. The three with a switch have the lamp and the switch to the right of the icon.
// Four icons take the room three did (524 points from the first icon's left to the last one's
// right), so every other part of the strip stays where it was
const ICON_PITCH: f32 = 160.0;
const PRESET_END: f32 = PRESET_X + PRESET_WIDTH / 2.0;
const ICONS_SPAN: f32 = 3.0 * ICON_PITCH + 2.0 * ui::icons::ICON_REACH;
// The icons stand in the room between the preset display and the output knob, an equal room each side
const METRONOME_X: f32 =
    (PRESET_END + OUTPUT_KNOB.x - KNOB_REACH) / 2.0 - ICONS_SPAN / 2.0 + ui::icons::ICON_REACH;
const LOOPER_X: f32 = METRONOME_X + ICON_PITCH;
const TUNER_X: f32 = LOOPER_X + ICON_PITCH;
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

// The plates open under the strip: the presets' under the display, the metronome's and the
// looper's under their icons, the settings' under theirs
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

// The looper's plate (#280, the Ring picked on the issue): the footswitch stands in a ring of
// lamps that is the loop (it fills red while recording, then a head goes round), the state printed
// in the ring over the switch and what a press does next under it; Stop (it reads Mute while the
// loop plays, Resume while it is muted) and Clear in a column on the left, the Level knob on the
// right. Every place is from the plate's top left corner
const LOOPER_WIDTH: f32 = 500.0;
const LOOPER_HEIGHT: f32 = 214.0;
const LOOPER_RING: Vec2 = vec2(250.0, 116.0);
const LOOPER_RING_RADIUS: f32 = 70.0;
const LOOPER_STATE_DY: f32 = -50.0;
const LOOPER_NEXT_DY: f32 = 54.0;
const LOOPER_BUTTONS_X: f32 = 70.0;
// Caption, button, for Stop and then Clear
const LOOPER_STOP: [f32; 2] = [78.0, 106.0];
const LOOPER_CLEAR: [f32; 2] = [140.0, 168.0];
const LOOPER_LEVEL: Vec2 = vec2(430.0, 118.0);
const LOOPER_LEVEL_RADIUS: f32 = 26.0;

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
/// How often the settings plate asks whether the new audio devices have opened
const OPENING_POLL: std::time::Duration = std::time::Duration::from_millis(50);
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
    /// The looper, under its icon
    Looper,
    /// The settings, under their icon: the cabinet files and, in the standalone program, the
    /// machine's audio, which is the choice held here while the plate is open
    Settings(Option<AudioChoice>),
}

/// Which plate: what each icon and the preset display opens
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Plates {
    Presets,
    Metronome,
    Looper,
    Settings,
}

impl Open {
    fn which(&self) -> Plates {
        match self {
            Open::Presets(_) => Plates::Presets,
            Open::Metronome => Plates::Metronome,
            Open::Looper => Plates::Looper,
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

#[allow(clippy::too_many_arguments)]
pub fn create(
    params: Arc<GuitarAmpParams>,
    editor_state: Arc<EguiState>,
    tuner: Arc<TunerReading>,
    lamp_reading: Arc<LampReading>,
    beats: Arc<BeatReading>,
    looper: Arc<LooperControl>,
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
                looper: &looper,
                cab_loader: &cab_loader,
                load_cabinet: &load_cabinet,
                audio_host: standalone::host(),
                update: &update,
            };
            window(egui_ctx, window_state.as_ref(), setter, &params, view, &shared);
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

/// One frame of the window as the host has it: the line in the resizable window
fn window(
    ctx: &Context,
    state: &EguiState,
    setter: &ParamSetter,
    params: &GuitarAmpParams,
    view: &mut View,
    shared: &Shared,
) {
    ResizableWindow::new(RESIZE_ID).min_size(vec2(MIN_WIDTH as f32, MIN_HEIGHT as f32)).show(
        ctx,
        state,
        |ui| {
            draw(ctx, setter, params, view, shared);
            paint_resize_corner(ui);
        },
    );
}

/// What the editor reads besides the parameters, shared with the audio and the plugin's
/// background tasks
struct Shared<'a> {
    tuner: &'a TunerReading,
    lamp_reading: &'a LampReading,
    beats: &'a BeatReading,
    /// The looper's buttons and what it says of itself
    looper: &'a LooperControl,
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
        // The looper's switch is the plate's big button: the same press. Its lamp: cream while it
        // records, dim amber while it plays, amber while it overdubs, dark otherwise
        let loop_state = shared.looper.state();
        let looper = ui::icon_switch(
            ui,
            origin + vec2(LOOPER_X, STRIP_Y) + middle,
            ui::Icon::Looper,
            view.is_open(Plates::Looper),
            Some(loop_lamp(loop_state)),
            "looper",
        );
        if looper.icon.clicked() {
            view.toggle_plate(Plates::Looper, host, || Open::Looper);
        }
        if looper.switch.is_some_and(|switch| switch.clicked()) {
            shared.looper.press();
        }
        if loop_state == LoopState::Recording {
            // It closes by itself when the buffer is full
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
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
        looper_plate(ui, setter, params, origin + middle, view, shared, looper.icon.rect);
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

/// What the lamp beside the looper's icon shows
fn loop_lamp(state: LoopState) -> Beat {
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
fn loop_ring(state: LoopState, length: u32, position: u32, capacity: u32) -> ui::Ring {
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
fn loop_words(state: LoopState) -> (&'static str, &'static str) {
    match state {
        LoopState::Empty => ("Empty", "Record"),
        LoopState::Recording => ("Recording", "Play"),
        LoopState::Playing => ("Playing", "Overdub"),
        LoopState::Overdubbing => ("Overdub", "Play"),
        LoopState::Stopped => ("Muted", "Play"),
    }
}

/// What the Stop button is called: Mute while the loop sounds, Resume while it is muted
fn stop_word(state: LoopState) -> &'static str {
    match state {
        LoopState::Playing | LoopState::Overdubbing => "Mute",
        LoopState::Stopped => "Resume",
        LoopState::Empty | LoopState::Recording => "Stop",
    }
}

/// The looper's plate under its icon, while it is the plate that is open: the footswitch in its
/// ring, Stop and Clear, and the Level knob
fn looper_plate(
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
    // The devices being opened answer from another thread: the panel looks again until they have
    if audio.as_mut().is_some_and(AudioChoice::poll) {
        ui.ctx().request_repaint_after(OPENING_POLL);
    }
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
            (None, Some(choice)) if choice.is_opening() => "Opening ...",
            (None, Some(choice)) if choice.test_tone => {
                "The tone is on the chosen outputs; mind the volume"
            }
            (None, Some(choice)) if choice.applied => "Playing on the new settings",
            (None, Some(_)) => "Apply opens the new settings",
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
                match choice.apply(host) {
                    // The new program is on its way: this window is done
                    Applied::Restarting => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
                    Applied::Opening => ui.ctx().request_repaint(),
                    Applied::Failed => {}
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
mod tests;
