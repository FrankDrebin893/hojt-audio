//! Throwaway mock-ups for the looper's plate (#280 explore). Not part of the plugin.
#![allow(clippy::all)]
use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, vec2, Color32, Pos2, Rect, Shape, Stroke, Ui};
use suite_common::ui::snapshot::{pedal_harness, NoHost};
use suite_common::ui::theme::*;
use suite_common::ui::{self, chrome_button, led_display, param_knob, rig_led, silk_label, Icon};

use crate::GuitarAmpParams;

// 0 empty, 1 recording, 2 playing, 3 overdubbing
const STATES: [&str; 4] = ["Empty", "Recording", "Playing", "Overdubbing"];

#[derive(Clone, Copy, PartialEq)]
enum Dir {
    Deck,
    Stomp,
    Ring,
}

fn paint_looper_icon(p: &egui::Painter, c: Pos2, dir: Dir, color: Color32) {
    let stroke = Stroke::new(2.4_f32, color);
    match dir {
        Dir::Deck => {
            for dx in [-8.0, 8.0] {
                p.circle_stroke(c + vec2(dx, -2.0), 6.5, stroke);
                p.circle_filled(c + vec2(dx, -2.0), 1.8, color);
            }
            p.line_segment([c + vec2(-8.0, 4.5), c + vec2(8.0, 4.5)], stroke);
            p.line_segment([c + vec2(-14.0, 11.0), c + vec2(14.0, 11.0)], stroke);
        }
        Dir::Stomp => {
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = -0.2 + i as f32 * (std::f32::consts::TAU * 0.82) / 24.0;
                    c + vec2(a.cos(), a.sin()) * 11.0
                })
                .collect();
            p.add(Shape::line(pts.clone(), stroke));
            let tip = pts[24];
            p.add(Shape::convex_polygon(
                vec![tip + vec2(-6.0, -1.0), tip + vec2(6.0, -1.0), tip + vec2(0.0, 7.0)],
                color,
                Stroke::NONE,
            ));
        }
        Dir::Ring => {
            for i in 0..12 {
                let a = i as f32 * std::f32::consts::TAU / 12.0;
                let r = if i < 8 { 2.4 } else { 1.6 };
                p.circle_filled(c + vec2(a.cos(), a.sin()) * 12.5, r, color);
            }
            p.circle_filled(c, 4.5, color);
        }
    }
}

/// The icon with its lamp and switch, as `ui::icon_switch` does for the others
fn icon(ui: &mut Ui, at: Pos2, dir: Dir, state: usize, open: bool) {
    let color = if open { Color32::WHITE } else { SILK };
    paint_looper_icon(ui.painter(), at, dir, color);
    if open {
        ui.painter().rect_filled(
            Rect::from_center_size(at + vec2(0.0, 22.0), vec2(30.0, 3.5)),
            1.75,
            CHROME,
        );
    }
    let beat = match state {
        0 => ui::Beat::Dark,
        1 => ui::Beat::Downbeat,
        2 => ui::Beat::Armed,
        _ => ui::Beat::Regular,
    };
    ui::widgets::beat_lamp_sized(ui.painter(), at + vec2(38.0, 0.0), beat, 5.5);
    chrome_button(ui, at + vec2(68.0, 0.0), &format!("loop-{}-{}", dir as u8, state));
}

fn next_action(state: usize) -> &'static str {
    ["Record", "Play", "Overdub", "Play"][state]
}

fn plate_size(dir: Dir) -> egui::Vec2 {
    match dir {
        Dir::Deck => vec2(560.0, 168.0),
        Dir::Stomp => vec2(500.0, 192.0),
        Dir::Ring => vec2(500.0, 214.0),
    }
}

fn level_knob(ui: &mut Ui, params: &GuitarAmpParams, at: Pos2, r: f32) {
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    param_knob(ui, &setter, &params.looper_level, at, r, "Level", None);
}

fn draw_plate(ui: &mut Ui, params: &GuitarAmpParams, dir: Dir, state: usize, rect: Rect) {
    ui.push_id((dir as u8, state, rect.min.x as i32, rect.min.y as i32), |ui| draw_plate_in(ui, params, dir, state, rect));
}

fn draw_plate_in(ui: &mut Ui, params: &GuitarAmpParams, dir: Dir, state: usize, rect: Rect) {
    ui::plate::paint_body(ui.painter(), rect);
    ui::plate::plate_title(ui.painter(), rect, "Looper");
    let at = |x: f32, y: f32| rect.min + vec2(x, y);
    let id = format!("{}{}", dir as u8, state);
    let red = Color32::from_rgb(230, 60, 40);
    let amber = Color32::from_rgb(255, 168, 28);
    match dir {
        Dir::Deck => {
            silk_label(ui.painter(), at(112.0, 50.0), "Loop", 13.0);
            let text = ["NO LOOP", "REC 3.2S", "PLAY 4B", "DUB 4B"][state];
            led_display(ui.painter(), at(112.0, 76.0), 9, text);
            for i in 0..24usize {
                let x = 30.0 + i as f32 * 9.4;
                let on = match state {
                    1 => i <= 6,
                    2 => i == 11,
                    3 => i == 17,
                    _ => false,
                };
                let color = if on {
                    if state == 1 { red } else { amber }
                } else {
                    Color32::from_rgb(40, 34, 30)
                };
                ui.painter().rect_filled(
                    Rect::from_center_size(at(x, 112.0), vec2(5.0, 12.0)),
                    1.5,
                    color,
                );
            }
            silk_label(ui.painter(), at(112.0, 136.0), if state == 0 { "" } else { "Bar 3 of 4" }, 12.0);
            silk_label(ui.painter(), at(284.0, 50.0), next_action(state), 13.0);
            ui::small_footswitch(ui, at(284.0, 90.0), &id);
            rig_led(ui.painter(), at(284.0, 128.0), matches!(state, 1 | 3));
            silk_label(ui.painter(), at(364.0, 50.0), if state == 2 { "Mute" } else { "Stop" }, 13.0);
            chrome_button(ui, at(364.0, 84.0), &format!("s{id}"));
            silk_label(ui.painter(), at(424.0, 50.0), "Clear", 13.0);
            chrome_button(ui, at(424.0, 84.0), &format!("c{id}"));
            level_knob(ui, params, at(504.0, 84.0), 20.0);
        }
        Dir::Stomp => {
            let names = ["Rec", "Play", "Dub"];
            for (i, name) in names.iter().enumerate() {
                let x = 150.0 + i as f32 * 56.0;
                let on = matches!((state, i), (1, 0) | (2, 1) | (3, 2) | (3, 1));
                silk_label(ui.painter(), at(x, 50.0), name, 13.0);
                rig_led(ui.painter(), at(x, 70.0), on);
            }
            ui::footswitch(ui, at(206.0, 122.0), &id);
            silk_label(ui.painter(), at(206.0, 166.0), next_action(state), 12.0);
            silk_label(ui.painter(), at(56.0, 70.0), if state == 2 { "Mute" } else { "Stop" }, 13.0);
            chrome_button(ui, at(56.0, 98.0), &format!("s{id}"));
            silk_label(ui.painter(), at(56.0, 128.0), "Clear", 13.0);
            chrome_button(ui, at(56.0, 156.0), &format!("c{id}"));
            level_knob(ui, params, at(400.0, 100.0), 26.0);
        }
        Dir::Ring => {
            let centre = at(250.0, 116.0);
            let n = 32usize;
            for i in 0..n {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n as f32;
                let p = centre + vec2(a.cos(), a.sin()) * 70.0;
                let on = match state {
                    0 => false,
                    1 => i <= 12,
                    _ => i == 10 || i == 9,
                };
                let color = if !on {
                    Color32::from_rgb(40, 34, 30)
                } else if state == 1 {
                    red
                } else if state == 3 {
                    amber
                } else {
                    Color32::from_rgb(120, 220, 120)
                };
                ui.painter().circle_filled(p, 3.6, color);
            }
            ui::footswitch(ui, centre, &id);
            silk_label(ui.painter(), centre + vec2(0.0, -50.0), ["Empty", "Recording", "Playing", "Overdub"][state], 12.0);
            silk_label(ui.painter(), centre + vec2(0.0, 54.0), next_action(state), 12.0);
            silk_label(ui.painter(), at(70.0, 78.0), if state == 2 { "Mute" } else { "Stop" }, 13.0);
            chrome_button(ui, at(70.0, 106.0), &format!("s{id}"));
            silk_label(ui.painter(), at(70.0, 140.0), "Clear", 13.0);
            chrome_button(ui, at(70.0, 168.0), &format!("c{id}"));
            level_knob(ui, params, at(430.0, 118.0), 26.0);
        }
    }
}

fn shot(name: &str, size: [u32; 2], draw: impl Fn(&mut Ui) + 'static) {
    let mut harness = pedal_harness(size[0], size[1], move |ctx| {
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(BENCH)).show(ctx, |ui| draw(ui));
    });
    harness.run_ok();
    let img = harness.render().expect("render");
    std::fs::create_dir_all("/tmp/hojt-explore").unwrap();
    img.save(format!("/tmp/hojt-explore/{name}.png")).unwrap();
}

fn full(dir: Dir, state: usize, name: &str) {
    let plate = plate_size(dir);
    shot(name, [840, 82 + plate.y as u32 + 20], move |ui| {
        let params = GuitarAmpParams::default();
        let strip = Rect::from_min_size(Pos2::new(8.0, 8.0), vec2(824.0, 64.0));
        ui.painter().rect_filled(strip, 6.0, PAINT_OXBLOOD);
        let y = 40.0;
        ui::icons::paint_icon(ui.painter(), Pos2::new(70.0, y), Icon::Metronome, SILK);
        ui::widgets::beat_lamp_sized(ui.painter(), Pos2::new(108.0, y), ui::Beat::Armed, 5.5);
        chrome_button(ui, Pos2::new(138.0, y), "m");
        icon(ui, Pos2::new(250.0, y), dir, state, true);
        ui::icons::paint_icon(ui.painter(), Pos2::new(430.0, y), Icon::Tuner, SILK);
        ui::icons::paint_icon(ui.painter(), Pos2::new(560.0, y), Icon::Settings, SILK);
        let rect = Rect::from_min_size(Pos2::new(250.0 - 22.0, 78.0), plate);
        draw_plate(ui, &params, dir, state, rect);
    });
}

fn states(dir: Dir, name: &str) {
    let plate = plate_size(dir);
    let (w, h) = (plate.x as u32 + 20, plate.y as u32 + 66);
    for s in 0..4usize {
        shot(&format!("{name}-{s}"), [w, h], move |ui| {
            let params = GuitarAmpParams::default();
            let (ox, oy) = (10.0, 6.0);
            let strip = Rect::from_min_size(Pos2::new(ox, oy), vec2(plate.x, 46.0));
            ui.painter().rect_filled(strip, 6.0, PAINT_OXBLOOD);
            icon(ui, Pos2::new(ox + 60.0, oy + 22.0), dir, s, true);
            silk_label(ui.painter(), Pos2::new(ox + plate.x - 90.0, oy + 22.0), STATES[s], 16.0);
            draw_plate(ui, &params, dir, s, Rect::from_min_size(Pos2::new(ox, oy + 52.0), plate));
        });
    }
    let files: Vec<String> = (0..4).map(|s| format!("/tmp/hojt-explore/{name}-{s}.png")).collect();
    let status = std::process::Command::new("montage")
        .args(&files)
        .args(["-tile", "2x2", "-geometry", "+0+0", "-background", "#121110"])
        .arg(format!("/tmp/hojt-explore/{name}.png"))
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
#[ignore]
fn render_explore() {
    for (dir, n) in [(Dir::Deck, "a-deck"), (Dir::Stomp, "b-stomp"), (Dir::Ring, "c-ring")] {
        full(dir, 2, &format!("{n}-strip"));
        states(dir, &format!("{n}-states"));
    }
}
