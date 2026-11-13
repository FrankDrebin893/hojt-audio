use nih_plug::prelude::*;
use nih_plug_egui::egui::{pos2, vec2, Context, Rect, Vec2};
use nih_plug_egui::{create_egui_editor, EguiState};
use std::sync::Arc;

use suite_common::ui::{
    self, footswitch, led, param_knob, silk_frame, silk_label, Ornament, PedalStyle,
};
use suite_common::update::UpdateCheck;

use crate::bass::Voice;
use crate::BassSynthParams;

const UPDATE_LINE_AT: Vec2 = vec2(WINDOW_WIDTH as f32 / 2.0, 33.0);
const WINDOW_WIDTH: u32 = 660;
const WINDOW_HEIGHT: u32 = 600;

const STYLE: PedalStyle = PedalStyle {
    name: "Bass Synth",
    model: "BA-1",
    paint: ui::theme::PAINT_COBALT,
    ornament: Ornament::Strings,
    jacks: ["Out", "MIDI In"],
    band_y: 394.0,
    art: Some(ui::artwork::BASS),
};

// One footswitch per voice, with its horizontal position
const VOICE_SWITCHES: [(Voice, &str, f32); 2] =
    [(Voice::Metal, "Metal", 265.0), (Voice::Groove, "Groove", 395.0)];

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(WINDOW_WIDTH, WINDOW_HEIGHT)
}

fn set<P: Param>(setter: &ParamSetter, param: &P, value: P::Plain) {
    setter.begin_set_parameter(param);
    setter.set_parameter(param, value);
    setter.end_set_parameter(param);
}

/// Picks a voice and puts the knobs where that bass has them, like a preset. Bend stays where
/// it is: it belongs to the player's keyboard, as the wheel does
fn pick_voice(setter: &ParamSetter, params: &BassSynthParams, voice: Voice) {
    let settings = voice.settings();
    set(setter, &params.voice, voice);
    set(setter, &params.volume, util::db_to_gain(settings.volume_db));
    set(setter, &params.drive, settings.drive);
    set(setter, &params.brightness, settings.brightness);
    set(setter, &params.sub, settings.sub);
    set(setter, &params.tone, settings.tone_hz);
    set(setter, &params.sustain, settings.sustain_s);
    set(setter, &params.slide, settings.slide_ms);
    set(setter, &params.transpose, settings.transpose);
}

pub fn create(
    params: Arc<BassSynthParams>,
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
        move |egui_ctx, setter, _state| draw(egui_ctx, setter, &params, &update),
    )
}

/// The whole window, once the fonts are in: what the plugin shows and its snapshot test draws
fn draw(ctx: &Context, setter: &ParamSetter, params: &BassSynthParams, update: &UpdateCheck) {
    ui::pedal(ctx, &STYLE, |ui, origin| {
        // The update line, between the jack captions, when a newer version is out
        ui::update_line(ui, origin + UPDATE_LINE_AT, update);
        // Overall sound
        let sound_y = 98.0;
        param_knob(ui, setter, &params.volume, origin + vec2(130.0, sound_y), 31.0, "Volume", None);
        param_knob(ui, setter, &params.drive, origin + vec2(263.0, sound_y), 31.0, "Drive", None);
        param_knob(
            ui,
            setter,
            &params.brightness,
            origin + vec2(397.0, sound_y),
            31.0,
            "Brightness",
            None,
        );
        param_knob(ui, setter, &params.sub, origin + vec2(530.0, sound_y), 31.0, "Sub", None);

        // The string
        let frame =
            Rect::from_min_max(pos2(40.0, 206.0), pos2(620.0, 328.0)).translate(origin.to_vec2());
        silk_frame(ui.painter(), frame, "String", STYLE.paint);
        // Five knobs evenly over the frame, Tone at 115 and Bend at 545
        let string_y = 246.0;
        let string_x = |index: f32| 115.0 + index * (545.0 - 115.0) / 4.0;
        param_knob(
            ui,
            setter,
            &params.tone,
            origin + vec2(string_x(0.0), string_y),
            19.0,
            "Tone",
            None,
        );
        param_knob(
            ui,
            setter,
            &params.sustain,
            origin + vec2(string_x(1.0), string_y),
            19.0,
            "Sustain",
            None,
        );
        param_knob(
            ui,
            setter,
            &params.slide,
            origin + vec2(string_x(2.0), string_y),
            19.0,
            "Slide",
            None,
        );
        param_knob(
            ui,
            setter,
            &params.transpose,
            origin + vec2(string_x(3.0), string_y),
            19.0,
            "Transpose",
            None,
        );
        param_knob(
            ui,
            setter,
            &params.bend,
            origin + vec2(string_x(4.0), string_y),
            19.0,
            "Bend",
            None,
        );

        // Voice selector
        for (voice, label, x) in VOICE_SWITCHES {
            led(ui.painter(), origin + vec2(x, 452.0), params.voice.value() == voice);
            if footswitch(ui, origin + vec2(x, 506.0), label).clicked() {
                pick_voice(setter, params, voice);
            }
            silk_label(ui.painter(), origin + vec2(x, 558.0), label, 17.0);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    use suite_common::ui::snapshot::{pedal_harness, NoHost};

    // The picture on the face is compiled in: a build without it is a build of other sources
    #[test]
    fn test_the_pedal_is_drawn_with_its_art() {
        assert!(STYLE.art == Some(ui::artwork::BASS), "the pedal has lost its art");
    }

    // The update line is drawn when a newer version is known, and only then: with the check off,
    // or the line closed for that version, the window is the one it was (pixel for pixel)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_update_line_is_drawn_when_a_newer_version_is_known() {
        use suite_common::ui::snapshot::{newer_version, pedal_harness, NoHost};
        let params = crate::BassSynthParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let picture = |check: &UpdateCheck| {
            pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, |ctx| draw(ctx, &setter, &params, check))
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
        let params = crate::BassSynthParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let check = newer_version();
        let mut harness =
            pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, |ctx| draw(ctx, &setter, &params, &check));
        harness.snapshot("editor_update");
    }

    // What the editor draws in its window, every parameter at its default, against the
    // picture in tests/snapshots/editor.png. On a change it leaves editor.new.png and
    // editor.diff.png there to look at (see suite_common::ui::snapshot)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_editor_looks_as_it_did() {
        let params = crate::BassSynthParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let update = UpdateCheck::resting(params.update.clone());
        let mut harness =
            pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, |ctx| draw(ctx, &setter, &params, &update));
        harness.snapshot("editor");
    }
}
