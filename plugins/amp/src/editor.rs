use nih_plug::prelude::*;
use nih_plug_egui::egui::{
    emath::TSTransform, vec2, Color32, Context, CursorIcon, Id, LayerId, Order, Pos2, Rect, Sense,
    Ui, UiBuilder, Vec2, ViewportCommand,
};
use nih_plug_egui::resizable_window::ResizableWindow;
use nih_plug_egui::{create_egui_editor, EguiState};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, PoisonError};
use suite_common::ui::{
    self, chrome_button, drop_panel_with, led_display, led_stepper, led_width, nameplate_opener,
    panel_knob, param_knob, param_switch, print_label, rig_led, signal_lamp, silk_frame,
    silk_label, stepper, theme, tuner_display, type_into, type_on_tape, Beat, Column, MiniStyle,
    Motif, Ornament, PedalStyle, Pick, Plate, Row, Step, Typed, BENCH_MARGIN, LED_HEIGHT,
    SLIM_JACK_X, SLIM_KNOBS, SLIM_KNOB_RADIUS, SLIM_LED, SLIM_PRINT_REACH, SLIM_STRIP_HEIGHT,
    SLIM_SWITCH,
};

use crate::board::{Line, STACK_WIDTH};
use crate::cab::MicType;
use crate::cab_stage::{CabLoader, CabStatus, NO_FILES_SHOWN};
use crate::lamps::{LampReading, LampView};
use crate::looks;
use crate::looper::{CountIn, LoopBars, LoopClick, LooperControl, State as LoopState};
use crate::metronome::{BeatReading, ClickOut, Sound, TapTempo, TimeSignature, BPM_MAX, BPM_MIN};
use crate::midi::MidiInbox;
use crate::midi_map::MidiMap;
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
use crate::{GuitarAmpParams, GuitarAmpPlugin, Task};
use suite_common::update::UpdateCheck;

/// The width of the line's design layer for the chain a project had before it could be built:
/// the gate, the twelve pedals and the stack. The strip's places are made at this width and move
/// with the layer's (`Fit`); the line's own width follows its chain (`board::Line::width`)
const WINDOW_WIDTH: u32 = 3501;
const WINDOW_HEIGHT: u32 = (STACK_TOP + HEAD_HEIGHT + PEDAL_HEIGHT + BENCH_MARGIN) as u32;

// The line is drawn in a layer of its own width and `WINDOW_HEIGHT` high, and the layer is
// scaled onto the window by a zoom that is what fits (`Fit`), from `MIN_ZOOM` to `MAX_ZOOM`. A
// line too long to fit at `MIN_ZOOM` is scrolled sideways, with the strip staying where it is.
// A window opens sized to the screen (up to the full 3501 x 584, or 1876 x 313 on a 1920 x 1080
// screen) and is dragged by its corner to any size from `MIN_WIDTH` x `MIN_HEIGHT` up
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 1.0;
// What a 1920 px screen leaves of the window beside it, as ever (the window opens 1876 wide there)
const SCREEN_MARGIN: u32 = 44;
const DEFAULT_WIDTH: u32 = FALLBACK_SCREEN_WIDTH - SCREEN_MARGIN;
const DEFAULT_ZOOM: f32 = DEFAULT_WIDTH as f32 / WINDOW_WIDTH as f32;
/// The narrowest the strip may be: what the controls on it need to stand clear of each other.
/// It stretches with the window from there, and so does the layer
const STRIP_MIN_WIDTH: f32 = 2600.0;
/// How far the scroll bar stands in from the window's edges
const SCROLL_INSET: f32 = 40.0;
const MIN_WIDTH: u32 = (STRIP_MIN_WIDTH * MIN_ZOOM) as u32 + 1;
const MIN_HEIGHT: u32 = (WINDOW_HEIGHT as f32 * MIN_ZOOM) as u32 + 1;
const FALLBACK_SCREEN_WIDTH: u32 = 1920;
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

// The no-audio lamp (`ui::audio_lamp`) stands right of the settings icon, at the left end of the
// room the update line has, with its caption under it; the update line gives it the room
// (`AUDIO_ROOM`) while the lamp is drawn. The lamp is a little above the strip's line, so that its
// caption stands under it on the strip
const AUDIO_LAMP: Vec2 = vec2(UPDATE_ROOM_LEFT + 46.0, STRIP_Y - 12.0);
const AUDIO_ROOM: f32 = 82.0;
/// The least zoom at which the lamp has its caption: 15 points at 0.7 are 10 pixels
const CAPTION_MIN_ZOOM: f32 = 0.7;

/// Does the update line, at the width the longest message takes, have its room on a strip whose
/// right end and middle have moved by `shift`, beside the no-audio lamp when that is drawn? On the
/// narrowest strips it has not both: the lamp, which says the program is silent, keeps the room
/// and the update line waits until there is audio
fn update_line_fits(shift: f32, lamp: bool) -> bool {
    let needed = ui::update_line::parts(Pos2::ZERO, [96.0, 51.0, 19.0]).0.width();
    let taken = if lamp { AUDIO_ROOM } else { 0.0 };
    (UPDATE_ROOM_RIGHT + shift) - (UPDATE_ROOM_LEFT + taken + shift / 2.0) - STRIP_GAP >= needed
}

/// Where the update line is centred on the strip, from the window's top left corner, for a
/// strip whose right end and middle have moved by `shift` and half of it, and with the room the
/// no-audio lamp takes at its left end when that is drawn
fn update_line_at(shift: f32, lamp: bool) -> Vec2 {
    let taken = if lamp { AUDIO_ROOM } else { 0.0 };
    let (left, right) = (UPDATE_ROOM_LEFT + taken + shift / 2.0, UPDATE_ROOM_RIGHT + shift);
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
// The wah's Toe, Heel and Rocker captions, and the rocker's value on its tape
const ROCKER_END_SIZE: f32 = 12.0;
const ROCKER_TAPE_SIZE: f32 = 11.0;

// On the head, which looks as its amp does (`looks`): the nameplate on the grille, the amp's
// name between two buttons that step to the next amp, and the panel under it. On the panel,
// from its left edge, the input jack the cable from the pedals goes into, the amp's six dials
// in an even row, and at its right end the pilot lamp, lit while the amp is on
const NAMEPLATE_WIDTH: f32 = 140.0;
const NAMEPLATE_TEXT_SIZE: f32 = 22.0;
// The middle of the jack, the first dial and the line of the dials, from the panel's top left
// corner; captions are on one line above them
const DIAL_Y: f32 = 52.0;
const DIALS: [&str; 6] = ["Gain", "Bass", "Mid", "Treble", "Presence", "Master"];
// The stack is two lanes, one per amp, one above the other: a head with the cabinet beside it,
// that play together. Each lane's head looks as its amp (`looks`), its cabinet as the cabinet
// that plays through it (`looks::cab_look`), with the lane's choice on a nameplate at its foot. The lanes end at a black plate where the signal meets, with the Blend fader
// between the two, and what both share: Mic, Res, the microphone, the cabinets' switch and the
// one output jack. Positions are from the stack's top left corner
const LANE_GAP: f32 = 18.0;
const LANE_HEIGHT: f32 = (HEAD_HEIGHT + PEDAL_HEIGHT - LANE_GAP) / 2.0;
/// The stack's height: both lanes and the gap between them
pub(crate) const STACK_HEIGHT: f32 = HEAD_HEIGHT + PEDAL_HEIGHT;
const LANE_HEAD_WIDTH: f32 = 420.0;
const LANE_CAB_WIDTH: f32 = 210.0;
const LANE_CAB_GAP: f32 = 6.0;
// The plate starts after the cabinets, with room for a coupler between
const MIX_GAP: f32 = 12.0;
const MIX_X: f32 = LANE_HEAD_WIDTH + LANE_CAB_WIDTH + LANE_CAB_GAP + MIX_GAP - LANE_CAB_GAP;
const MIX_WIDTH: f32 = STACK_WIDTH - MIX_X;
// On a lane's head, from the panel's top left corner: the dials in a row, the lane's name,
// and, on the second lane, its switch, on the line under the dials
const LANE_JACK_X: f32 = 16.0;
const LANE_DIAL_FIRST_X: f32 = 54.0;
const LANE_DIAL_SPACING: f32 = 55.0;
const LANE_DIAL_RADIUS: f32 = 15.0;
const LANE_LINE_DY: f32 = 72.0;
const LANE_NAME_X: f32 = 48.0;
// A cabinet's nameplate, centred across the cabinet and this far up from its foot: the choice's
// name between two step buttons, on brushed chrome
const CAB_PLATE_WIDTH: f32 = 128.0;
const CAB_PLATE_FOOT: f32 = 28.0;
const CAB_PLATE_TEXT_SIZE: f32 = 15.0;
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

/// The pedal of `kind` as a picture, for a miniature (`ui::miniature`): its paint, art and
/// title, the LED, its knobs at the middle of their travel and the footswitch, drawn as the
/// pedal in the line draws them and answering nothing. Left off, as nothing of it reads at half
/// size: the knobs' captions and value tapes, the Delay's Sync, the Wah's rocker and Auto
/// button (the rocker's slot is drawn bare). `rect` is the pedal's full size; `lit` is its LED
pub(crate) fn paint_miniature(ui: &Ui, rect: Rect, kind: EffectKind, lit: bool) -> Rect {
    let face = ui::mini_pedal(ui, rect, &kind_look(kind));
    let (painter, top) = (ui.painter(), face.center_top());
    let (radius, led) = match kind {
        EffectKind::Pitch => (TRIO_RADIUS, PEDAL_LED),
        EffectKind::Eq => (QUAD_RADIUS, PEDAL_LED),
        EffectKind::Wah => (WAH_KNOB_RADIUS, WAH_LED),
        _ if is_slim(kind) => (SLIM_KNOB_RADIUS, SLIM_LED),
        EffectKind::Drive | EffectKind::Delay => (TRIO_RADIUS, PEDAL_LED),
        _ => (PAIR_RADIUS, PEDAL_LED),
    };
    let knobs: &[Vec2] = match kind {
        EffectKind::Pitch | EffectKind::Drive | EffectKind::Delay => &TRIO_KNOBS,
        EffectKind::Eq => &QUAD_KNOBS,
        EffectKind::Wah => &WAH_KNOBS,
        _ if is_slim(kind) => &SLIM_KNOBS,
        _ => &PAIR_KNOBS,
    };
    if kind == EffectKind::Wah {
        let slot =
            Rect::from_min_size(top + ROCKER_TOP - vec2(7.0, 0.0), vec2(14.0, ROCKER_LENGTH));
        painter.rect_filled(slot, 6.0, Color32::from_black_alpha(190));
        painter.rect_filled(
            Rect::from_center_size(slot.center(), vec2(34.0, 26.0)),
            5.0,
            theme::CHROME_LIGHT,
        );
    }
    for &offset in knobs {
        ui::widgets::knob_with(
            painter,
            top + offset,
            radius,
            0.5,
            None,
            ui::KnobCap::Fluted,
            theme::SILK,
        );
    }
    rig_led(painter, top + led, lit);
    let switch = if is_slim(kind) { SLIM_SWITCH } else { PEDAL_SWITCH };
    ui::paint_small_footswitch(painter, top + switch);
    face
}

/// What the blocks of the line are drawn from
pub(crate) struct Pedals<'a> {
    params: &'a GuitarAmpParams,
    setter: &'a ParamSetter<'a>,
    gate_open: bool,
    cab_loader: &'a CabLoader,
    /// Has the chosen cabinet read in the background
    load_cabinet: &'a dyn Fn(),
    /// The lane whose head or cabinet was double-clicked, or whose nameplate button was, in this
    /// frame, and which: the window opens (or closes) that lane's drawer of amps or cabinets
    drawer_ask: std::cell::Cell<Option<(usize, drawer::What)>>,
    /// Where each lane's head stands this frame, which the amp drawer's click outside spares
    heads: std::cell::Cell<[Rect; 2]>,
    /// The same for the cabinets, which the cabinet drawer's click outside spares
    cabs: std::cell::Cell<[Rect; 2]>,
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
    /// it plays through beside it with its choice on a nameplate at its foot, and the line under
    /// the dials with its name and, for the second, its On switch. Returns the middle of the head's input jack
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
        let lane = usize::from(second);
        let mut heads = self.heads.get();
        heads[lane] = head;
        self.heads.set(heads);
        // The head takes a double-click, under the dials, jack and switch, which are added after
        // it and so answer their own clicks first
        let hit = ui
            .interact(head, Id::new(("head-drawer", id)), Sense::click())
            .on_hover_text("Double-click to choose");
        if hit.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if hit.double_clicked() {
            self.drawer_ask.set(Some((lane, drawer::What::Amp)));
        }
        if nameplate_opener(
            ui,
            faces.grille.center(),
            NAMEPLATE_WIDTH,
            panel,
            print,
            amp.model().name,
            id,
            NAMEPLATE_TEXT_SIZE,
        ) {
            self.drawer_ask.set(Some((lane, drawer::What::Amp)));
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
        if second {
            let right = faces.panel.right();
            print_label(
                ui.painter(),
                Pos2::new(right + LANE_ON_CAPTION_DX, line),
                "On",
                15.0,
                print,
            );
            param_switch(
                ui,
                setter,
                &params.amp2_on,
                Pos2::new(right + LANE_ON_SWITCH_DX, line),
                Pos2::new(right + LANE_ON_LED_DX, line),
            );
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

        // A double-click on the cabinet opens its drawer, under the nameplate's button, which is
        // added after it and so answers its own click first
        let mut cabs = self.cabs.get();
        cabs[lane] = cab;
        self.cabs.set(cabs);
        let hit = ui
            .interact(cab, Id::new(("cabinet-drawer", id)), Sense::click())
            .on_hover_text("Double-click to choose");
        if hit.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if hit.double_clicked() {
            self.drawer_ask.set(Some((lane, drawer::What::Cabinet)));
        }
        // Its nameplate at the foot: what plays, with a button that opens the drawer of the
        // amp's own, the built-in ones and, for the first, the files
        let shown = if second {
            match params.cabinet_second.get() {
                Some(speaker) => crate::amp::model::SPEAKERS[speaker].name.to_owned(),
                None => "OWN".to_owned(),
            }
        } else {
            if self.cab_loader.no_files_showing() {
                // Draw once more when the note is over
                ui.ctx().request_repaint_after(NO_FILES_SHOWN);
            }
            self.cab_loader.display_name(&chosen, CAB_NAME_CHARS)
        };
        if nameplate_opener(
            ui,
            Pos2::new(cab.center().x, cab.bottom() - CAB_PLATE_FOOT),
            CAB_PLATE_WIDTH,
            Plate::BrushedChrome,
            theme::INK,
            &shown,
            if second { "cabinet2" } else { "cabinet" },
            CAB_PLATE_TEXT_SIZE,
        ) {
            self.drawer_ask.set(Some((lane, drawer::What::Cabinet)));
        }
        (jack, cab.right_center())
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
    /// The amp's (or the cabinet's) drawer, rising from below the line while it is open: the
    /// third surface beside the plates and the bank, of which one is open at a time
    drawer: Option<drawer::Drawer>,
    /// Which MIDI controller moves which knob, and where learn stands (#479)
    midi_map: MidiMap,
}

/// A plate open under the strip: configuration is a plate that opens from an icon or a display,
/// and one is open at a time. Only one plate is ever held, so the audio panel's size costs nothing
#[allow(clippy::large_enum_variant)]
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
            self.drawer = None;
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
    midi: Arc<MidiInbox>,
    async_executor: AsyncExecutor<GuitarAmpPlugin>,
) -> Option<Box<dyn Editor>> {
    let opened_midi = midi.clone();
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
            drawer: None,
            midi_map: MidiMap::default(),
        },
        move |egui_ctx, view| {
            ui::install(egui_ctx);
            // A result arrives on the check's own thread: the window is painted only when
            // asked, so it asks
            let ctx = egui_ctx.clone();
            opened_update.on_result(move || ctx.request_repaint());
            // Program changes that came while no window was open are stale; from now on one
            // asks this window to paint (ADR 0006)
            opened_midi.clear();
            opened_midi.attach(egui_ctx.clone());
            // The controllers the player bound to knobs are the standalone's settings
            view.midi_map = MidiMap::new(standalone::launch().midi_bindings, &opened_params);
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
            let audio = ui::AudioNotice::current();
            let shared = Shared {
                tuner: &tuner,
                lamp_reading: &lamp_reading,
                beats: &beats,
                looper: &looper,
                cab_loader: &cab_loader,
                midi: &midi,
                load_cabinet: &load_cabinet,
                audio_host: standalone::host(),
                update: &update,
                audio: audio.as_ref(),
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
    /// Program changes from a MIDI footcontroller, which pick a preset of the bank showing
    midi: &'a MidiInbox,
    /// Has the chosen cabinet read in the background
    load_cabinet: &'a dyn Fn(),
    /// The machine's audio, in the standalone program; a plugin in a DAW has none
    audio_host: Option<&'a dyn standalone::Host>,
    /// Whether a newer version is out: the line on the strip
    update: &'a UpdateCheck,
    /// Set where the standalone program's audio is the dummy backend: the lamp on the strip
    audio: Option<&'a ui::AudioNotice>,
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
        if update_line_fits(fit.shift(), shared.audio.is_some()) {
            ui::update_line(
                ui,
                window + update_line_at(fit.shift(), shared.audio.is_some()),
                shared.update,
            );
        }
        // What is on the strip is placed from here
        let origin = window;
        let host = shared.audio_host;

        let presets = PresetDisplay {
            params,
            setter,
            cab_loader: shared.cab_loader,
            load_cabinet: shared.load_cabinet,
        };
        // A footcontroller's program change loads preset n of the bank showing, as a click does
        // (before anything is drawn, so this frame shows it)
        if !shared.midi.is_empty() {
            let channel = standalone::launch().midi_channel;
            shared.midi.drain(channel, |number| {
                presets.load_program(usize::from(number), view);
            });
            // The controllers move the knobs they are bound to, or bind to the one learn picked
            learn::follow_controllers(ctx, setter, params, view, shared, now);
        }
        learn::begin(ctx, params, view);
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
            let launch = settings_choices(view);
            view.toggle_plate(Plates::Settings, host, || {
                Open::Settings(host.map(|host| AudioChoice::open(host, launch)))
            });
        }
        // No audio device could be opened: the lamp says so, and a click on it opens the Audio
        // panel on the settings plate, where the program's audio is chosen
        // The lamp is no click outside the plate it opens, as the icon is none
        let mut settings_anchor = settings.icon.rect;
        // Its caption is 15 points: below `CAPTION_MIN_ZOOM` it would not read, and the lamp stands
        // alone with its hover
        let lamp_at = origin + AUDIO_LAMP + middle;
        let lamp = if fit.zoom < CAPTION_MIN_ZOOM {
            ui::audio_lamp_bare(ui, lamp_at, shared.audio)
        } else {
            ui::audio_lamp(ui, lamp_at, shared.audio)
        };
        if let Some(lamp) = lamp {
            settings_anchor = settings_anchor.union(lamp.rect);
            if lamp.clicked() && host.is_some() && !view.is_open(Plates::Settings) {
                let launch = settings_choices(view);
                view.toggle_plate(Plates::Settings, host, || {
                    Open::Settings(host.map(|host| AudioChoice::open(host, launch)))
                });
            }
        }

        let pedals = Pedals {
            params,
            setter,
            gate_open,
            cab_loader: shared.cab_loader,
            load_cabinet: shared.load_cabinet,
            drawer_ask: std::cell::Cell::new(None),
            heads: std::cell::Cell::new([Rect::NOTHING; 2]),
            cabs: std::cell::Cell::new([Rect::NOTHING; 2]),
        };

        // The line, scrolled sideways where the window shows only part of it. The wheel scrolls it
        // while the pointer is over it and no plate is open, and so does the bar along the foot
        view.scroll = view.scroll.clamp(0.0, fit.furthest());
        if fit.furthest() > 0.0 && view.plate.is_none() {
            // Not over the sheet of the bank, which stands on the line's lower part
            let below = ui.max_rect().bottom() - window.y;
            let sheet_top = view
                .effects
                .as_ref()
                .map_or(f32::MAX, |bank| bank.top(below, now) + window.y)
                .min(view.drawer.map_or(f32::MAX, |drawer| drawer.top(below, now) + window.y));
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
        // Opening the bank closes the plate that was open, and the drawer
        if view.effects.as_ref().is_some_and(|bank| bank.fresh) {
            view.close_plate(host);
            view.drawer = None;
        }
        // A double-click on a head, or its nameplate's button, opens that lane's drawer, or
        // closes it if it is the one open; it takes the bank and the plate down
        if let Some((lane, what)) = pedals.drawer_ask.take() {
            if view.drawer.is_some_and(|open| open.lane == lane && open.what == what) {
                view.drawer = None;
            } else {
                view.drawer = Some(drawer::Drawer::opened(lane, what, now));
                view.effects = None;
                view.close_plate(host);
                ui.ctx().request_repaint();
            }
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
        drawer::show_amps(ui, window, fit.visible, &pedals, &mut view.drawer, now);
        drawer::show_cabs(ui, window, fit.visible, &pedals, &mut view.drawer, now);
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
        settings_plate(ui, setter, params, origin + middle, view, shared, settings_anchor);
        learn::finish(ctx, params, view, shared, now);
    });
}

/// What the settings plate opens on: what the program plays on, with the controllers bound to knobs
/// since (which Apply saves with the rest, so it must not save older ones)
fn settings_choices(view: &View) -> standalone::Prefs {
    standalone::Prefs { midi_bindings: view.midi_map.bindings.clone(), ..standalone::launch() }
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

mod drawer;
mod learn;
mod looper;
mod metronome;
mod mix;
mod preset_display;
mod settings;

use self::looper::*;
use self::metronome::*;
#[cfg(test)]
use self::mix::*;
use self::preset_display::*;
use self::settings::*;

#[cfg(test)]
mod tests;
