//! Layout mock-ups, drawn with the editor's own pieces in new places. Not a part of the plugin:
//! `cargo test -p amp --release --lib mockup -- --nocapture` writes the pictures to
//! `$MOCKUP_DIR` (or the temp folder).

use super::*;
use std::path::PathBuf;
use suite_common::ui::snapshot::{pedal_harness, NoHost};
use suite_common::ui::PLUG_Y;

// The slots of `Pedals::pedal`
const GATE: usize = 0;
const DRIVE: usize = 1;
const DELAY: usize = 2;
const REVERB: usize = 3;
const COMP: usize = 4;
const CHORUS: usize = 5;
const PHASER: usize = 6;
const FLANGER: usize = 7;
const TREMOLO: usize = 8;
const FUZZ: usize = 9;
const PITCH: usize = PITCH_SLOT;
const EQ: usize = EQ_SLOT;

const PLATE_GAP: f32 = 22.0;

fn out_dir() -> PathBuf {
    std::env::var_os("MOCKUP_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
}

fn dual_params() -> GuitarAmpParams {
    let params = GuitarAmpParams {
        amp: EnumParam::new("Amp", Amp::Torden),
        amp2_on: BoolParam::new("Amp 2", true),
        ..GuitarAmpParams::default()
    };
    params.cabinet_second.put(crate::amp::model::speaker_index("1x15-deep"));
    params
}

fn render(name: &str, width: u32, height: u32, mut paint: impl FnMut(&mut Ui, Pos2, &Pedals)) {
    let params = dual_params();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let cab_loader = CabLoader::new(Arc::new(crate::cab_stage::CabStage::new()), 48_000.0);
    let mut harness = pedal_harness(width, height, |ctx| {
        ui::rig(ctx, |ui, window| {
            let pedals = Pedals {
                params: &params,
                setter: &setter,
                gate_open: false,
                cab_loader: &cab_loader,
                load_cabinet: &|| {},
            };
            paint(ui, window, &pedals);
        });
    });
    let image = harness.render().expect("render");
    let path = out_dir().join(format!("{name}.png"));
    image.save(&path).expect("save");
    println!("wrote {}", path.display());
}

/// A plate on a strip, from `left` to `right`, between `top` and `bottom`
fn plate(ui: &Ui, origin: Pos2, left: f32, right: f32, top: f32, bottom: f32) {
    ui::strip_plate(
        ui.painter(),
        Rect::from_min_max(origin + vec2(left, top), origin + vec2(right, bottom)),
    );
}

/// A switch with its lamp and caption to its left, as the strip has them. `x` is the switch,
/// `dy` how far below the strip's own row
fn switch_group(ui: &mut Ui, origin: Pos2, x: f32, dy: f32, caption: &str, lit: bool, id: &str) {
    rig_led(ui.painter(), origin + vec2(x + CAPTION_OFFSET, LED_Y + dy), lit);
    silk_label(
        ui.painter(),
        origin + vec2(x + CAPTION_OFFSET, CAPTION_Y + dy),
        caption,
        CAPTION_SIZE,
    );
    small_footswitch(ui, origin + vec2(x, SWITCH_Y + dy), id);
}
/// How far a switch group reaches left and right of its switch
const SWITCH_LEFT: f32 = -CAPTION_OFFSET + 28.0;
const SWITCH_RIGHT: f32 = 25.0;

/// A captioned LED display with its buttons, centred at `x`, `y`
fn led_with_caption(ui: &mut Ui, at: Pos2, chars: usize, text: &str, caption: &str, id: &str) {
    silk_label(ui.painter(), at + vec2(0.0, PRESET_CAPTION_Y - PRESET_Y), caption, 13.0);
    led_stepper(ui, at, chars, text, id);
}

fn stepper_reach(chars: usize) -> f32 {
    led_width(chars) / 2.0 + STEPPER_REACH
}

/// The metronome on two rows, `left` its left edge, `row` the centre of the upper row's
/// controls and `row2` of the lower. Returns its right edge
fn metronome_block(
    ui: &mut Ui,
    p: &GuitarAmpParams,
    s: &ParamSetter,
    o: Pos2,
    left: f32,
    rows: [f32; 2],
    dy: f32,
) -> f32 {
    let [row, row2] = rows;
    let cap = |r: f32| r - (PRESET_Y - PRESET_CAPTION_Y);
    // The switch, as on the strip
    let x = left + SWITCH_LEFT;
    beat_lamp(ui.painter(), o + vec2(x + CAPTION_OFFSET, LED_Y + dy), Beat::Armed);
    silk_label(ui.painter(), o + vec2(x + CAPTION_OFFSET, CAPTION_Y + dy), "Metro", CAPTION_SIZE);
    small_footswitch(ui, o + vec2(x, SWITCH_Y + dy), "m-metro");
    // Tempo over Meter
    let x = x + SWITCH_RIGHT + 30.0 + stepper_reach(TEMPO_CHARS);
    led_with_caption(ui, o + vec2(x, row), TEMPO_CHARS, "120", "Tempo", "m-tempo");
    silk_label(ui.painter(), o + vec2(x, cap(row2) + 4.0), "Meter", 13.0);
    stepper(ui, o + vec2(x, row2 + 4.0), SIGNATURE_WIDTH, "4/4", "m-meter");
    // Tap over Sync
    let x = x + stepper_reach(TEMPO_CHARS) + 12.0 + SAVE_REACH;
    silk_label(ui.painter(), o + vec2(x, cap(row)), "Tap", 13.0);
    chrome_button(ui, o + vec2(x, row), "m-tap");
    silk_label(ui.painter(), o + vec2(x, cap(row2) + 4.0), "Sync", 13.0);
    chrome_button(ui, o + vec2(x, row2 + 4.0), "m-sync");
    rig_led(ui.painter(), o + vec2(x + 27.0, row2 + 4.0), false);
    // Sound over Click out
    let x = x + SAVE_REACH + 30.0 + SOUND_WIDTH / 2.0 + STEPPER_REACH;
    silk_label(ui.painter(), o + vec2(x, cap(row)), "Sound", 13.0);
    stepper(ui, o + vec2(x, row), SOUND_WIDTH, p.metronome_sound.value().name(), "m-sound");
    silk_label(ui.painter(), o + vec2(x, cap(row2) + 4.0), "Click out", 13.0);
    stepper(ui, o + vec2(x, row2 + 4.0), OUT_WIDTH, p.metronome_out.value().name(), "m-out");
    // Volume
    let x = x + SOUND_WIDTH / 2.0 + STEPPER_REACH + 26.0 + CLICK_REACH;
    param_knob(
        ui,
        s,
        &p.metronome_volume,
        o + vec2(x, CLICK_VOLUME.y + dy),
        CLICK_RADIUS,
        "Volume",
        None,
    );
    x + CLICK_REACH
}

/// Output knob, level meter and clip lamp from `left`. Returns the right edge
fn output_block(
    ui: &mut Ui,
    p: &GuitarAmpParams,
    s: &ParamSetter,
    o: Pos2,
    left: f32,
    dy: f32,
) -> f32 {
    let x = left + KNOB_REACH;
    param_knob(ui, s, &p.out_level, o + vec2(x, OUTPUT_KNOB.y + dy), OUTPUT_RADIUS, "Output", None);
    let x = x + KNOB_REACH + 40.0 + METER_REACH;
    level_meter(ui.painter(), o + vec2(x, OUTPUT_METER.y + dy), METER_HEIGHT, -18.0);
    silk_label(
        ui.painter(),
        o + vec2(x, OUTPUT_METER.y + dy) + METER_CAPTION,
        "Level",
        LAMP_CAPTION_SIZE,
    );
    let x = x + METER_REACH + 30.0 + LAMP_REACH;
    signal_lamp(ui.painter(), o + vec2(x, CLIP_LAMP.y + dy), false);
    silk_label(
        ui.painter(),
        o + vec2(x, CLIP_LAMP.y + dy) + CLIP_CAPTION,
        "Clip",
        LAMP_CAPTION_SIZE,
    );
    x + LAMP_REACH
}

/// The input meter at `x`. Returns its right edge
fn input_block(ui: &Ui, o: Pos2, x: f32, dy: f32) {
    level_meter(ui.painter(), o + vec2(x, INPUT_METER.y + dy), METER_HEIGHT, -24.0);
    silk_label(
        ui.painter(),
        o + vec2(x, INPUT_METER.y + dy) + METER_CAPTION,
        "Input",
        LAMP_CAPTION_SIZE,
    );
}

/// How wide `metronome_block` is, from its left edge
fn metro_width() -> f32 {
    SWITCH_LEFT
        + SWITCH_RIGHT
        + 30.0
        + 2.0 * stepper_reach(TEMPO_CHARS)
        + 12.0
        + 2.0 * SAVE_REACH
        + 30.0
        + SOUND_WIDTH
        + 2.0 * STEPPER_REACH
        + 26.0
        + 2.0 * CLICK_REACH
}
/// How wide `output_block` is
fn output_width() -> f32 {
    2.0 * KNOB_REACH + 40.0 + 2.0 * METER_REACH + 30.0 + 2.0 * LAMP_REACH
}
const INPUT_PLATE: f32 = 2.0 * (METER_REACH + 24.0);
/// A plate holding one footswitch group
const SWITCH_PLATE: f32 = SWITCH_LEFT + SWITCH_RIGHT + 28.0;

/// The left edges of plates `widths` wide, from `from` to `to` with the same gap between them
fn spread(from: f32, to: f32, widths: &[f32]) -> Vec<f32> {
    let gap = (to - from - widths.iter().sum::<f32>()) / (widths.len() - 1) as f32;
    let mut x = from;
    widths
        .iter()
        .map(|w| {
            let left = x;
            x += w + gap;
            left
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// A: the rack comes down into the black. The stack rises to the top of the window, and the
// strip is two red rack panels beside its heads, over the pedals: in on the left, out on the
// right. The window is 140 points shorter

#[test]
fn mockup_a_split_rack() {
    const RISE: f32 = STACK_TOP - BENCH_MARGIN;
    let height = (WINDOW_HEIGHT as f32 - RISE) as u32;
    render("A-split-rack", WINDOW_WIDTH, height, |ui, window, pedals| {
        let (p, s) = (pedals.params, pedals.setter);
        let o = window;
        // Where the stack stands with the default order: six pedals before it
        let gap =
            (WINDOW_WIDTH as f32 - 2.0 * BENCH_MARGIN - (12.0 * PEDAL_WIDTH + STACK_WIDTH)) / 12.0;
        let stack_left = BENCH_MARGIN + 6.0 * (PEDAL_WIDTH + gap);
        let stack_right = stack_left + STACK_WIDTH;
        let top = BENCH_MARGIN;
        let panel_bottom = PEDAL_TOP - RISE - BENCH_MARGIN - 2.0;
        // The strip's row, moved down to the middle of the panel
        let dy = (panel_bottom - top - STRIP_HEIGHT) / 2.0;
        let (pt, pb) = (top + 10.0, panel_bottom - 11.0);
        // Two rows inside the plates
        let row1 = top + 62.0;
        let row2 = top + 130.0;

        // The left panel: what comes in, what is picked, the tuner and On
        let left_panel = Rect::from_min_max(
            o + vec2(BENCH_MARGIN, top),
            o + vec2(stack_left - gap, panel_bottom),
        );
        ui::strip(ui, left_panel, &PedalStyle { jacks: ["In", ""], ..STYLE });
        let preset_plate =
            2.0 * PLATE_PAD + 2.0 * stepper_reach(PRESET_CHARS) + 30.0 + 2.0 * SAVE_REACH;
        let ix = INPUT_METER.x;
        let lefts = spread(
            ix - INPUT_PLATE / 2.0,
            left_panel.right() - o.x - 14.0,
            &[INPUT_PLATE, preset_plate, SWITCH_PLATE, SWITCH_PLATE],
        );
        plate(ui, o, lefts[0], lefts[0] + INPUT_PLATE, pt, pb);
        input_block(ui, o, ix, dy);
        // Bank over Preset, Save beside the preset
        let pl = lefts[1];
        plate(ui, o, pl, pl + preset_plate, pt, pb);
        let px = pl + PLATE_PAD + stepper_reach(PRESET_CHARS);
        let save_x = px + stepper_reach(PRESET_CHARS) + 30.0 + SAVE_REACH;
        let bank_x = px - stepper_reach(PRESET_CHARS) + stepper_reach(BANK_CHARS);
        led_with_caption(ui, o + vec2(bank_x, row1), BANK_CHARS, "Factory", "Bank", "a-bank");
        led_with_caption(ui, o + vec2(px, row2), PRESET_CHARS, "Init*", "Preset", "a-preset");
        silk_label(
            ui.painter(),
            o + vec2(save_x, row2 - (PRESET_Y - PRESET_CAPTION_Y)),
            "Save",
            13.0,
        );
        chrome_button(ui, o + vec2(save_x, row2), "a-save");
        for (left, caption, lit) in [(lefts[2], "Tuner", false), (lefts[3], "On", true)] {
            plate(ui, o, left, left + SWITCH_PLATE, pt, pb);
            switch_group(ui, o, left + SWITCH_LEFT, dy, caption, lit, caption);
        }

        // The right panel: the metronome and what goes out
        let right_panel = Rect::from_min_max(
            o + vec2(stack_right + gap, top),
            o + vec2(WINDOW_WIDTH as f32 - BENCH_MARGIN, panel_bottom),
        );
        ui::strip(ui, right_panel, &PedalStyle { jacks: ["", "Stereo Out"], ..STYLE });
        let metro_plate = metro_width() + 40.0;
        let out_plate = output_width() + 2.0 * PLATE_PAD;
        let lefts = spread(
            right_panel.left() - o.x + 14.0,
            right_panel.right() - o.x - 120.0,
            &[metro_plate, out_plate],
        );
        plate(ui, o, lefts[0], lefts[0] + metro_plate, pt, pb);
        metronome_block(ui, p, s, o, lefts[0] + 20.0, [row1, row2], dy);
        plate(ui, o, lefts[1], lefts[1] + out_plate, pt, pb);
        output_block(ui, p, s, o, lefts[1] + PLATE_PAD, dy);

        // The pedals and the stack, all of it 140 points higher
        crate::board::board(ui, o - vec2(0.0, RISE), pedals, &p.order);
    });
}

// ---------------------------------------------------------------------------------------------
// B: a square board. The front pedals stand in two rows of three left of the stack and those
// behind it in two rows of three on its right, so the window is 1150 points narrower; the
// metronome has its own black plate under the amps

const B_MARGIN: f32 = 40.0;
const B_GAP: f32 = 24.0;
const B_ROW_GAP: f32 = 34.0;
const B_SIDE_GAP: f32 = 46.0;
const B_BLOCK: f32 = 3.0 * PEDAL_WIDTH + 2.0 * B_GAP;
const B_WIDTH: f32 = 2.0 * B_MARGIN + 2.0 * B_BLOCK + 2.0 * B_SIDE_GAP + STACK_WIDTH;
const B_ROW1: f32 = STACK_TOP + 6.0;
const B_ROW2: f32 = B_ROW1 + PEDAL_HEIGHT + B_ROW_GAP;
const B_HEIGHT: f32 = B_ROW2 + PEDAL_HEIGHT + BENCH_MARGIN + 6.0;

/// A pedal-to-pedal cable from the right side of one pedal (`from`, its plug's height) to the
/// left side of another in the row under it, along the gap between the rows at `lane_y`
fn wrap_cable(ui: &Ui, from: Pos2, to: Pos2, lane_y: f32) {
    let a = from + vec2(12.0, 0.0);
    let b = to - vec2(12.0, 0.0);
    let bend = 34.0;
    link_cable(ui, [a, a + vec2(0.0, 60.0), Pos2::new(a.x, lane_y), Pos2::new(a.x - bend, lane_y)]);
    link_cable(
        ui,
        [
            Pos2::new(a.x - bend, lane_y),
            Pos2::new(a.x - bend - 1.0, lane_y),
            Pos2::new(b.x + bend + 1.0, lane_y),
            Pos2::new(b.x + bend, lane_y),
        ],
    );
    link_cable(ui, [Pos2::new(b.x + bend, lane_y), Pos2::new(b.x, lane_y), b - vec2(0.0, 60.0), b]);
    for (edge, at) in [(from.x, Pos2::new(a.x, from.y)), (to.x, Pos2::new(b.x, to.y))] {
        ui::angled_plug(ui.painter(), edge, Rect::from_center_size(at, vec2(13.0, 17.0)));
    }
}

#[test]
fn mockup_b_square_board() {
    render("B-square-board", B_WIDTH as u32, B_HEIGHT as u32, |ui, window, pedals| {
        let (p, s) = (pedals.params, pedals.setter);
        let o = window;

        // The strip, as wide as the window, its controls closer together
        let strip = Rect::from_min_size(
            o + vec2(BENCH_MARGIN, BENCH_MARGIN),
            vec2(B_WIDTH - 2.0 * BENCH_MARGIN, STRIP_HEIGHT),
        );
        ui::strip(ui, strip, &STYLE);
        let (pt, pb) = (PLATE_TOP, PLATE_BOTTOM);
        let ix = INPUT_METER.x;
        let preset_plate = 2.0 * PLATE_PAD
            + 2.0 * stepper_reach(BANK_CHARS)
            + 30.0
            + 2.0 * stepper_reach(PRESET_CHARS)
            + 30.0
            + 2.0 * SAVE_REACH;
        let out_plate = output_width() + 2.0 * PLATE_PAD;
        let lefts = spread(
            ix - INPUT_PLATE / 2.0,
            B_WIDTH - 200.0,
            &[INPUT_PLATE, preset_plate, SWITCH_PLATE, out_plate, SWITCH_PLATE],
        );
        plate(ui, o, lefts[0], lefts[0] + INPUT_PLATE, pt, pb);
        input_block(ui, o, ix, 0.0);
        let pl = lefts[1];
        plate(ui, o, pl, pl + preset_plate, pt, pb);
        let bank_x = pl + PLATE_PAD + stepper_reach(BANK_CHARS);
        let px = bank_x + stepper_reach(BANK_CHARS) + 30.0 + stepper_reach(PRESET_CHARS);
        let save_x = px + stepper_reach(PRESET_CHARS) + 30.0 + SAVE_REACH;
        led_with_caption(ui, o + vec2(bank_x, PRESET_Y), BANK_CHARS, "Factory", "Bank", "b-bank");
        led_with_caption(ui, o + vec2(px, PRESET_Y), PRESET_CHARS, "Init*", "Preset", "b-preset");
        silk_label(ui.painter(), o + vec2(save_x, PRESET_CAPTION_Y), "Save", 13.0);
        chrome_button(ui, o + vec2(save_x, PRESET_Y), "b-save");
        // Tuner beside the presets: its display takes the preset's place
        plate(ui, o, lefts[2], lefts[2] + SWITCH_PLATE, pt, pb);
        switch_group(ui, o, lefts[2] + SWITCH_LEFT, 0.0, "Tuner", false, "b-tuner");
        plate(ui, o, lefts[3], lefts[3] + out_plate, pt, pb);
        output_block(ui, p, s, o, lefts[3] + PLATE_PAD, 0.0);
        plate(ui, o, lefts[4], lefts[4] + SWITCH_PLATE, pt, pb);
        switch_group(ui, o, lefts[4] + SWITCH_LEFT, 0.0, "On", true, "b-on");

        // The stack in the middle, its top with the upper row's
        let stack_left = B_MARGIN + B_BLOCK + B_SIDE_GAP;
        let jacks = pedals.stack(ui, o + vec2(stack_left, B_ROW1));

        // The metronome's plate under the amps, down to the lower row's feet
        let mtop = B_ROW1 + 2.0 * LANE_HEIGHT + LANE_GAP + 18.0;
        let mplate = Rect::from_min_max(
            o + vec2(stack_left, mtop),
            o + vec2(stack_left + STACK_WIDTH, B_ROW2 + PEDAL_HEIGHT),
        );
        ui::amp::mounted_plate(ui.painter(), mplate, Plate::BlackAnodised);
        let mdy = mtop - BENCH_MARGIN + (mplate.height() - STRIP_HEIGHT) / 2.0 - 4.0;
        metronome_block(
            ui,
            p,
            s,
            o + vec2(0.0, mdy),
            stack_left + (STACK_WIDTH - metro_width()) / 2.0,
            [PRESET_Y - 6.0, PRESET_Y + 62.0],
            0.0,
        );

        // The pedals: three by two on each side, read like a page
        let front = [[GATE, PITCH, FUZZ], [COMP, DRIVE, PHASER]];
        let back = [[CHORUS, FLANGER, TREMOLO], [EQ, DELAY, REVERB]];
        let back_left = stack_left + STACK_WIDTH + B_SIDE_GAP;
        for (block, left) in [(front, B_MARGIN), (back, back_left)] {
            for (r, row) in block.iter().enumerate() {
                let y = if r == 0 { B_ROW1 } else { B_ROW2 };
                for (c, &slot) in row.iter().enumerate() {
                    let x = left + c as f32 * (PEDAL_WIDTH + B_GAP);
                    pedals.pedal(
                        ui,
                        slot,
                        Rect::from_min_size(o + vec2(x, y), vec2(PEDAL_WIDTH, PEDAL_HEIGHT)),
                    );
                    if c > 0 {
                        ui::coupler(ui.painter(), o.x + x - B_GAP, o.x + x, o.y + y + PLUG_Y);
                    }
                }
            }
            let lane_y = o.y + B_ROW2 - B_ROW_GAP / 2.0;
            wrap_cable(
                ui,
                o + vec2(left + B_BLOCK, B_ROW1 + PLUG_Y),
                o + vec2(left, B_ROW2 + PLUG_Y),
                lane_y,
            );
        }
        // Into the amp from the last front pedal, and out of it to the first one behind
        ui::patch_cable(
            ui.painter(),
            jacks.input,
            o + vec2(B_MARGIN + B_BLOCK, B_ROW2 + PLUG_Y),
            1.0,
        );
        ui::patch_cable(ui.painter(), jacks.output, o + vec2(back_left, B_ROW1 + PLUG_Y), -1.0);
    });
}
