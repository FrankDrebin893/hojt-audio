use nih_plug::prelude::*;
use nih_plug_egui::egui::{pos2, vec2, Context, Rect, Vec2};
use nih_plug_egui::{create_egui_editor, EguiState};
use std::sync::Arc;

use suite_common::ui::{
    self, footswitch, led, param_knob, silk_frame, silk_label, Ornament, PedalStyle,
};
use suite_common::update::UpdateCheck;

use crate::kit::Kit;
use crate::DrumSynthParams;

const UPDATE_LINE_AT: Vec2 = vec2(WINDOW_WIDTH as f32 / 2.0, 33.0);
const WINDOW_WIDTH: u32 = 660;
const WINDOW_HEIGHT: u32 = 600;

const STYLE: PedalStyle = PedalStyle {
    name: "Drum Synth",
    model: "DS-2",
    paint: ui::theme::PAINT_TEAL,
    ornament: Ornament::Rings,
    jacks: ["Stereo / Multi Out", "MIDI In"],
    band_y: 394.0,
    art: Some(ui::artwork::DRUMS),
};

// One footswitch per kit, with its horizontal position
const KIT_SWITCHES: [(Kit, &str, f32); 4] = [
    (Kit::Rock, "Rock", 115.0),
    (Kit::Jazz, "Jazz", 215.0),
    (Kit::Metal, "Metal", 315.0),
    (Kit::Groove, "Groove", 415.0),
];

// The Choke switch, apart from the kit switches and clear of the logo badge bottom right
const CHOKE_SWITCH_X: f32 = 535.0;

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(WINDOW_WIDTH, WINDOW_HEIGHT)
}

pub fn create(
    params: Arc<DrumSynthParams>,
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
fn draw(ctx: &Context, setter: &ParamSetter, params: &DrumSynthParams, update: &UpdateCheck) {
    ui::pedal(ctx, &STYLE, |ui, origin| {
        // The update line, between the jack captions, when a newer version is out
        ui::update_line(ui, origin + UPDATE_LINE_AT, update);
        // Overall sound
        let sound_y = 98.0;
        // Five knobs, evenly across the frame, the middle one over the Mixer's middle one
        let knobs: [(&FloatParam, &str); 5] = [
            (&params.gain, "Gain"),
            (&params.room, "Room"),
            (&params.tune, "Tune"),
            (&params.damping, "Damping"),
            (&params.human, "Human"),
        ];
        for (i, (param, label)) in knobs.into_iter().enumerate() {
            let x = 115.0 + 107.5 * i as f32;
            param_knob(ui, setter, param, origin + vec2(x, sound_y), 31.0, label, None);
        }

        // Mixer
        let frame =
            Rect::from_min_max(pos2(40.0, 206.0), pos2(620.0, 328.0)).translate(origin.to_vec2());
        silk_frame(ui.painter(), frame, "Mixer", STYLE.paint);
        let mixer_y = 246.0;
        param_knob(ui, setter, &params.kick, origin + vec2(100.0, mixer_y), 19.0, "Kick", None);
        param_knob(ui, setter, &params.snare, origin + vec2(215.0, mixer_y), 19.0, "Snare", None);
        param_knob(ui, setter, &params.toms, origin + vec2(330.0, mixer_y), 19.0, "Toms", None);
        param_knob(ui, setter, &params.hihat, origin + vec2(445.0, mixer_y), 19.0, "Hi-Hat", None);
        param_knob(
            ui,
            setter,
            &params.cymbals,
            origin + vec2(560.0, mixer_y),
            19.0,
            "Cymbals",
            None,
        );

        // Kit selector
        for (kit, label, x) in KIT_SWITCHES {
            led(ui.painter(), origin + vec2(x, 452.0), params.kit.value() == kit);
            if footswitch(ui, origin + vec2(x, 506.0), label).clicked() {
                setter.begin_set_parameter(&params.kit);
                setter.set_parameter(&params.kit, kit);
                setter.end_set_parameter(&params.kit);
            }
            silk_label(ui.painter(), origin + vec2(x, 558.0), label, 17.0);
        }

        // Choke: a note-off on a cymbal damps it
        led(ui.painter(), origin + vec2(CHOKE_SWITCH_X, 452.0), params.choke.value());
        if footswitch(ui, origin + vec2(CHOKE_SWITCH_X, 506.0), "Choke").clicked() {
            setter.begin_set_parameter(&params.choke);
            setter.set_parameter(&params.choke, !params.choke.value());
            setter.end_set_parameter(&params.choke);
        }
        silk_label(ui.painter(), origin + vec2(CHOKE_SWITCH_X, 558.0), "Choke", 17.0);
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
        assert!(STYLE.art == Some(ui::artwork::DRUMS), "the pedal has lost its art");
    }

    // The update line is drawn when a newer version is known, and only then: with the check off,
    // or the line closed for that version, the window is the one it was (pixel for pixel)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_update_line_is_drawn_when_a_newer_version_is_known() {
        use suite_common::ui::snapshot::{newer_version, pedal_harness, NoHost};
        let params = crate::DrumSynthParams::default();
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
        let params = crate::DrumSynthParams::default();
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
        let params = crate::DrumSynthParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let update = UpdateCheck::resting(params.update.clone());
        let mut harness =
            pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, |ctx| draw(ctx, &setter, &params, &update));
        harness.snapshot("editor");
    }
}
