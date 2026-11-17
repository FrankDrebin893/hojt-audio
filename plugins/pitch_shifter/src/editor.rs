use nih_plug::prelude::*;
use nih_plug_egui::egui::{vec2, Context, Vec2};
use nih_plug_egui::{create_egui_editor, EguiState};
use std::sync::Arc;

use suite_common::ui::{self, footswitch, led, param_knob, Ornament, PedalStyle};
use suite_common::update::UpdateCheck;

use crate::GuitarPitchShifterParams;

const UPDATE_LINE_AT: Vec2 = vec2(WINDOW_WIDTH as f32 / 2.0, 33.0);
// The no-audio lamp (`ui::audio_lamp_left`), where the audio is the dummy backend: over the model
// number in the lower left, lamp, caption and model number a line (18 px) apart, the caption and the
// model number on one left edge
const NO_AUDIO_AT: Vec2 = vec2(ui::MODEL_LEFT + 12.5, 496.0);
const WINDOW_WIDTH: u32 = 400;
const WINDOW_HEIGHT: u32 = 600;

// Where the controls sit, from the window's top left corner: three small knobs in a row over
// the big Semitones knob, then the name band, the LED and the footswitch
const LATENCY_AT: Vec2 = vec2(83.0, 80.0);
const BLEND_AT: Vec2 = vec2(200.0, 80.0);
const SMOOTHNESS_AT: Vec2 = vec2(317.0, 80.0);
const SEMITONES_AT: Vec2 = vec2(200.0, 250.0);
const LED_AT: Vec2 = vec2(200.0, 463.0);
const FOOTSWITCH_AT: Vec2 = vec2(200.0, 523.0);
const SMALL_KNOB: f32 = 27.0;
const BIG_KNOB: f32 = 48.0;
const SEMITONES_SCALE: Option<[&str; 3]> = Some(["-12", "0", "+12"]);

const STYLE: PedalStyle = PedalStyle {
    name: "Pitch Shifter",
    model: "PS-10",
    paint: ui::theme::PAINT_ORANGE,
    ornament: Ornament::Arrows,
    jacks: ["Out", "In"],
    band_y: 407.0,
    art: Some(ui::artwork::PITCH_SHIFTER),
};

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(WINDOW_WIDTH, WINDOW_HEIGHT)
}

pub fn create(
    params: Arc<GuitarPitchShifterParams>,
    editor_state: Arc<EguiState>,
) -> Option<Box<dyn Editor>> {
    // Where the window is made, never in `process`: at most one request a day, on its own thread
    let update = UpdateCheck::start(params.update.clone());
    create_egui_editor(
        editor_state,
        (),
        {
            let update = update.clone();
            move |egui_ctx, _| {
                ui::install(egui_ctx);
                // A result arrives on the check's own thread: the window is painted only when
                // asked, so it asks
                let ctx = egui_ctx.clone();
                update.on_result(move || ctx.request_repaint());
            }
        },
        move |egui_ctx, setter, _state| {
            draw(egui_ctx, setter, &params, &update, ui::AudioNotice::current().as_ref())
        },
    )
}

/// The whole window, once the fonts are in: what the plugin shows and its snapshot test draws
fn draw(
    ctx: &Context,
    setter: &ParamSetter,
    params: &GuitarPitchShifterParams,
    update: &UpdateCheck,
    audio: Option<&ui::AudioNotice>,
) {
    ui::pedal(ctx, &STYLE, |ui, origin| {
        // The update line, between the jack captions, when a newer version is out
        ui::update_line(ui, origin + UPDATE_LINE_AT, update);
        // And the no-audio lamp, when the standalone has no audio
        ui::audio_lamp_left(ui, origin + NO_AUDIO_AT, origin.x + ui::MODEL_LEFT, audio);
        // Three small knobs in a row over the big one
        param_knob(
            ui,
            setter,
            &params.latency_ms,
            origin + LATENCY_AT,
            SMALL_KNOB,
            "Max Latency",
            None,
        );
        param_knob(ui, setter, &params.blend, origin + BLEND_AT, SMALL_KNOB, "Blend", None);
        param_knob(
            ui,
            setter,
            &params.smoothness_ms,
            origin + SMOOTHNESS_AT,
            SMALL_KNOB,
            "Smoothness",
            None,
        );
        param_knob(
            ui,
            setter,
            &params.semitones,
            origin + SEMITONES_AT,
            BIG_KNOB,
            "Semitones",
            SEMITONES_SCALE,
        );

        // Bypass switch, lit while the effect is on
        let bypassed = params.bypass.value();
        led(ui.painter(), origin + LED_AT, !bypassed);
        if footswitch(ui, origin + FOOTSWITCH_AT, "bypass").clicked() {
            setter.begin_set_parameter(&params.bypass);
            setter.set_parameter(&params.bypass, !bypassed);
            setter.end_set_parameter(&params.bypass);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    use suite_common::ui::snapshot::pedal_harness;
    use suite_common::ui::snapshot::NoHost;

    // The picture on the face is compiled in: a build without it is a build of other sources
    #[test]
    fn test_the_pedal_is_drawn_with_its_art() {
        assert!(STYLE.art == Some(ui::artwork::PITCH_SHIFTER), "the pedal has lost its art");
    }

    // The update line is drawn when a newer version is known, and only then: with the check off,
    // or the line closed for that version, the window is the one it was (pixel for pixel)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_update_line_is_drawn_when_a_newer_version_is_known() {
        use suite_common::ui::snapshot::{newer_version, pedal_harness, NoHost};
        let params = crate::GuitarPitchShifterParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let picture = |check: &UpdateCheck| {
            pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, |ctx| {
                draw(ctx, &setter, &params, check, None)
            })
            .render()
            .unwrap()
            .into_raw()
        };
        let none = picture(&UpdateCheck::resting(params.update.clone()));
        assert_ne!(picture(&newer_version()), none, "no line with a newer version known");
        let off = newer_version();
        off.turn_off();
        assert!(picture(&off) == none, "a line with the check off");
        let closed = newer_version();
        closed.dismiss();
        assert!(picture(&closed) == none, "a line after it was closed");
    }

    // The window with the line, against tests/snapshots/editor_update.png
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_editor_with_the_update_line_looks_as_it_did() {
        use suite_common::ui::snapshot::{newer_version, pedal_harness, NoHost};
        let params = crate::GuitarPitchShifterParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let check = newer_version();
        let mut harness = pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, |ctx| {
            draw(ctx, &setter, &params, &check, None)
        });
        harness.snapshot("editor_update");
    }

    // What the editor draws in its window, every parameter at its default, against the
    // picture in tests/snapshots/editor.png. On a change it leaves editor.new.png and
    // editor.diff.png there to look at (see suite_common::ui::snapshot)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_editor_looks_as_it_did() {
        let params = crate::GuitarPitchShifterParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let update = UpdateCheck::resting(params.update.clone());
        let mut harness = pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, |ctx| {
            draw(ctx, &setter, &params, &update, None)
        });
        harness.snapshot("editor");
    }

    use nih_plug_egui::egui::{epaint::Shape, Pos2, RawInput, Rect, Ui};

    fn collect(shape: &Shape, out: &mut Vec<Shape>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, out)),
            shape => out.push(shape.clone()),
        }
    }

    type Draw<'a> = &'a dyn Fn(&mut Ui, &ParamSetter, Pos2);

    /// The shapes `draw` paints in a window of the pedal's size, on the pedal or on the bare
    /// bench, once the fonts are in
    fn painted(on_pedal: bool, draw: Draw) -> Vec<Shape> {
        let ctx = Context::default();
        ui::install(&ctx);
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let window =
            Rect::from_min_size(Pos2::ZERO, vec2(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32));
        let mut shapes = Vec::new();
        for _ in 0..2 {
            let input = RawInput { screen_rect: Some(window), ..RawInput::default() };
            let output = ctx.run(input, |ctx| {
                if on_pedal {
                    ui::pedal(ctx, &STYLE, |ui, origin| draw(ui, &setter, origin));
                } else {
                    ui::rig(ctx, |ui, origin| draw(ui, &setter, origin));
                }
            });
            shapes.clear();
            output.shapes.iter().for_each(|clipped| collect(&clipped.shape, &mut shapes));
        }
        shapes
    }

    /// What a control paints, as boxes, without the bench under it
    fn boxes(draw: Draw) -> Vec<Rect> {
        painted(false, draw)
            .iter()
            .map(|shape| shape.visual_bounding_rect())
            .filter(|rect| rect.is_positive() && rect.width() < WINDOW_WIDTH as f32 - 10.0)
            .collect()
    }

    // Nobody can look at the pedal in CI: every control, with its caption and value tape,
    // stays clear of the others and of the name band, by the boxes of what egui paints
    #[test]
    fn test_the_controls_do_not_touch() {
        let params = crate::GuitarPitchShifterParams::default();
        let p = &params;
        let parts: [(&str, Vec<Rect>); 6] = [
            (
                "Max Latency",
                boxes(&|ui, setter, o| {
                    param_knob(
                        ui,
                        setter,
                        &p.latency_ms,
                        o + LATENCY_AT,
                        SMALL_KNOB,
                        "Max Latency",
                        None,
                    );
                }),
            ),
            (
                "Blend",
                boxes(&|ui, setter, o| {
                    param_knob(ui, setter, &p.blend, o + BLEND_AT, SMALL_KNOB, "Blend", None);
                }),
            ),
            (
                "Smoothness",
                boxes(&|ui, setter, o| {
                    param_knob(
                        ui,
                        setter,
                        &p.smoothness_ms,
                        o + SMOOTHNESS_AT,
                        SMALL_KNOB,
                        "Smoothness",
                        None,
                    );
                }),
            ),
            (
                "Semitones",
                boxes(&|ui, setter, o| {
                    let scale = SEMITONES_SCALE;
                    param_knob(
                        ui,
                        setter,
                        &p.semitones,
                        o + SEMITONES_AT,
                        BIG_KNOB,
                        "Semitones",
                        scale,
                    );
                }),
            ),
            ("LED", boxes(&|ui, _, o| led(ui.painter(), o + LED_AT, true))),
            (
                "Footswitch",
                boxes(&|ui, _, o| {
                    footswitch(ui, o + FOOTSWITCH_AT, "bypass");
                }),
            ),
        ];

        for (i, (a_name, a_boxes)) in parts.iter().enumerate() {
            assert!(!a_boxes.is_empty(), "{a_name} paints nothing");
            for (b_name, b_boxes) in &parts[i + 1..] {
                // The LED sits in the footswitch's shadow, as it did before the knob came
                if (*a_name, *b_name) == ("LED", "Footswitch") {
                    continue;
                }
                for a in a_boxes {
                    for b in b_boxes {
                        assert!(!a.intersects(*b), "{a_name} {a:?} touches {b_name} {b:?}");
                    }
                }
            }
        }

        // The name band, between the long lines along its edges (ink, and cream outside it)
        let edges: Vec<[Pos2; 2]> = painted(true, &|_, _, _| {})
            .into_iter()
            .filter_map(|shape| match shape {
                Shape::LineSegment { points, .. } if points[1].x - points[0].x > 300.0 => {
                    Some(points)
                }
                _ => None,
            })
            .collect();
        assert!(edges.len() >= 4, "the band's edges were not found");
        let height = |edge: &[Pos2; 2], x: f32| {
            edge[0].y + (edge[1].y - edge[0].y) * (x - edge[0].x) / (edge[1].x - edge[0].x)
        };
        // Half the width of the cream line
        let margin = 2.0;
        for (name, part) in &parts {
            for b in part {
                for x in [b.left(), b.right()] {
                    let top = edges.iter().map(|edge| height(edge, x)).fold(f32::MAX, f32::min);
                    let bottom = edges.iter().map(|edge| height(edge, x)).fold(f32::MIN, f32::max);
                    let clear = b.bottom() < top - margin || b.top() > bottom + margin;
                    assert!(clear, "{name} {b:?} touches the name band at x {x}");
                }
            }
        }
    }

    // The no-audio lamp is drawn when the audio is the dummy backend, and only then: with the audio
    // running (or in a DAW, which has no status) the window is the one it was (pixel for pixel)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_no_audio_lamp_is_drawn_when_the_audio_is_the_dummy() {
        let params = crate::GuitarPitchShifterParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let update = UpdateCheck::resting(params.update.clone());
        let picture = |audio: Option<&ui::AudioNotice>| {
            pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, |ctx| {
                draw(ctx, &setter, &params, &update, audio)
            })
            .render()
            .unwrap()
            .into_raw()
        };
        let none = picture(None);
        let dummy = ui::AudioNotice { reason: Some("48000 Hz not offered".into()) };
        assert_ne!(picture(Some(&dummy)), none, "no lamp on the dummy backend");
        assert_ne!(
            picture(Some(&ui::AudioNotice { reason: None })),
            none,
            "no lamp for a dummy backend that was asked for"
        );
        // A host that says its audio runs asks for no lamp
        assert!(
            picture(ui::notice_for(Some(ui::HostAudio::Running)).as_ref()) == none,
            "a lamp with audio"
        );
    }

    // The window with the lamp, against tests/snapshots/editor_no_audio.png
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_editor_with_the_no_audio_lamp_looks_as_it_did() {
        let params = crate::GuitarPitchShifterParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let update = UpdateCheck::resting(params.update.clone());
        let notice = ui::AudioNotice { reason: Some("48000 Hz not offered".into()) };
        let mut harness = pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, |ctx| {
            draw(ctx, &setter, &params, &update, Some(&notice))
        });
        harness.snapshot("editor_no_audio");
    }
}
