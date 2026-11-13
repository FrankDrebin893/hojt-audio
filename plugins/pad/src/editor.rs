use nih_plug::prelude::*;
use nih_plug_egui::egui::{pos2, vec2, Context, Rect, Vec2};
use nih_plug_egui::{create_egui_editor, EguiState};
use std::sync::Arc;

use suite_common::ui::{
    self, footswitch, led, param_knob, silk_frame, silk_label, Ornament, PedalStyle,
};
use suite_common::update::UpdateCheck;

use crate::ShimmerPadParams;

const UPDATE_LINE_AT: Vec2 = vec2(WINDOW_WIDTH as f32 / 2.0, 33.0);
const WINDOW_WIDTH: u32 = 660;
const WINDOW_HEIGHT: u32 = 600;

const STYLE: PedalStyle = PedalStyle {
    name: "Shimmer Pad",
    model: "SP-1",
    paint: ui::theme::PAINT_PLUM,
    ornament: Ornament::Waves,
    jacks: ["Stereo Out", "MIDI In"],
    band_y: 394.0,
    art: Some(ui::artwork::PAD),
};

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(WINDOW_WIDTH, WINDOW_HEIGHT)
}

pub fn create(
    params: Arc<ShimmerPadParams>,
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
fn draw(ctx: &Context, setter: &ParamSetter, params: &ShimmerPadParams, update: &UpdateCheck) {
    ui::pedal(ctx, &STYLE, |ui, origin| {
        // The update line, between the jack captions, when a newer version is out
        ui::update_line(ui, origin + UPDATE_LINE_AT, update);
        // The pad
        let pad_y = 98.0;
        param_knob(ui, setter, &params.volume, origin + vec2(110.0, pad_y), 28.0, "Volume", None);
        param_knob(ui, setter, &params.attack, origin + vec2(220.0, pad_y), 28.0, "Attack", None);
        param_knob(ui, setter, &params.release, origin + vec2(330.0, pad_y), 28.0, "Release", None);
        param_knob(ui, setter, &params.cutoff, origin + vec2(440.0, pad_y), 28.0, "Cutoff", None);
        param_knob(ui, setter, &params.dry, origin + vec2(550.0, pad_y), 28.0, "Dry", None);

        let frames_y = 246.0;
        let voice =
            Rect::from_min_max(pos2(40.0, 206.0), pos2(296.0, 328.0)).translate(origin.to_vec2());
        silk_frame(ui.painter(), voice, "Voice", STYLE.paint);
        param_knob(ui, setter, &params.detune, origin + vec2(88.0, frames_y), 19.0, "Detune", None);
        param_knob(
            ui,
            setter,
            &params.breath,
            origin + vec2(168.0, frames_y),
            19.0,
            "Breath",
            None,
        );
        param_knob(
            ui,
            setter,
            &params.motion,
            origin + vec2(248.0, frames_y),
            19.0,
            "Motion",
            None,
        );

        let space =
            Rect::from_min_max(pos2(312.0, 206.0), pos2(620.0, 328.0)).translate(origin.to_vec2());
        silk_frame(ui.painter(), space, "Space", STYLE.paint);
        param_knob(
            ui,
            setter,
            &params.chorus,
            origin + vec2(352.0, frames_y),
            19.0,
            "Chorus",
            None,
        );
        param_knob(
            ui,
            setter,
            &params.reverb,
            origin + vec2(428.0, frames_y),
            19.0,
            "Reverb",
            None,
        );
        param_knob(
            ui,
            setter,
            &params.reverb_decay,
            origin + vec2(504.0, frames_y),
            19.0,
            "Decay",
            None,
        );
        param_knob(
            ui,
            setter,
            &params.shimmer,
            origin + vec2(580.0, frames_y),
            19.0,
            "Shimmer",
            None,
        );

        for (param, label, x) in [
            (&params.chorus_on, "Chorus", 200.0),
            (&params.shimmer_on, "Shimmer", 330.0),
            (&params.hold, "Hold", 460.0),
        ] {
            let on = param.value();
            led(ui.painter(), origin + vec2(x, 452.0), on);
            if footswitch(ui, origin + vec2(x, 506.0), label).clicked() {
                setter.begin_set_parameter(param);
                setter.set_parameter(param, !on);
                setter.end_set_parameter(param);
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
        assert!(STYLE.art == Some(ui::artwork::PAD), "the pedal has lost its art");
    }

    // The update line is drawn when a newer version is known, and only then: with the check off,
    // or the line closed for that version, the window is the one it was (pixel for pixel)
    #[test]
    #[cfg(target_os = "linux")]
    fn test_the_update_line_is_drawn_when_a_newer_version_is_known() {
        use suite_common::ui::snapshot::{newer_version, pedal_harness, NoHost};
        let params = crate::ShimmerPadParams::default();
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
        let params = crate::ShimmerPadParams::default();
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
        let params = crate::ShimmerPadParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let update = UpdateCheck::resting(params.update.clone());
        let mut harness =
            pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, |ctx| draw(ctx, &setter, &params, &update));
        harness.snapshot("editor");
    }
}
