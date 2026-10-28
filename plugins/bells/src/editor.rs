use nih_plug::prelude::*;
use nih_plug_egui::egui::{pos2, vec2, Rect};
use nih_plug_egui::{create_egui_editor, EguiState};
use std::sync::Arc;

use suite_common::ui::{
    self, footswitch, led, param_knob, silk_frame, silk_label, Ornament, PedalStyle,
};

use crate::GlassBellsParams;

const WINDOW_WIDTH: u32 = 660;
const WINDOW_HEIGHT: u32 = 600;

const STYLE: PedalStyle = PedalStyle {
    name: "Glass Bells",
    model: "GB-1",
    paint: ui::theme::PAINT_PINE,
    ornament: Ornament::Glints,
    jacks: ["Stereo Out", "MIDI In"],
    band_y: 394.0,
    art: Some(ui::artwork::BELLS),
};

// The footswitches, with their horizontal positions: Delay and Sync under the Delay frame,
// Reverb under its own
const DELAY_SWITCH_X: f32 = 156.0;
const SYNC_SWITCH_X: f32 = 256.0;
const REVERB_SWITCH_X: f32 = 504.0;

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(WINDOW_WIDTH, WINDOW_HEIGHT)
}

pub fn create(
    params: Arc<GlassBellsParams>,
    editor_state: Arc<EguiState>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        editor_state,
        (),
        |egui_ctx, _| ui::install(egui_ctx),
        move |egui_ctx, setter, _state| {
            ui::pedal(egui_ctx, &STYLE, |ui, origin| {
                // The bells
                let bells_y = 98.0;
                param_knob(
                    ui,
                    setter,
                    &params.volume,
                    origin + vec2(110.0, bells_y),
                    28.0,
                    "Volume",
                    None,
                );
                param_knob(
                    ui,
                    setter,
                    &params.brightness,
                    origin + vec2(220.0, bells_y),
                    28.0,
                    "Brightness",
                    None,
                );
                param_knob(
                    ui,
                    setter,
                    &params.decay,
                    origin + vec2(330.0, bells_y),
                    28.0,
                    "Decay",
                    None,
                );
                param_knob(
                    ui,
                    setter,
                    &params.attack,
                    origin + vec2(440.0, bells_y),
                    28.0,
                    "Attack",
                    None,
                );
                param_knob(
                    ui,
                    setter,
                    &params.dry,
                    origin + vec2(550.0, bells_y),
                    28.0,
                    "Dry",
                    None,
                );

                // Each effect in its frame, over its switch
                let effects_y = 246.0;
                let delay = Rect::from_min_max(pos2(40.0, 206.0), pos2(372.0, 328.0))
                    .translate(origin.to_vec2());
                silk_frame(ui.painter(), delay, "Delay", STYLE.paint);
                // While the delay follows the song's tempo (Sync) the Time knob, in the same
                // place, picks the note, and its tape shows it
                let time = origin + vec2(100.0, effects_y);
                if params.delay_sync.value() {
                    param_knob(ui, setter, &params.delay_note, time, 19.0, "Time", None);
                } else {
                    param_knob(ui, setter, &params.delay_time, time, 19.0, "Time", None);
                }
                param_knob(
                    ui,
                    setter,
                    &params.delay_feedback,
                    origin + vec2(206.0, effects_y),
                    19.0,
                    "Feedback",
                    None,
                );
                param_knob(
                    ui,
                    setter,
                    &params.delay_mix,
                    origin + vec2(312.0, effects_y),
                    19.0,
                    "Mix",
                    None,
                );

                let reverb = Rect::from_min_max(pos2(388.0, 206.0), pos2(620.0, 328.0))
                    .translate(origin.to_vec2());
                silk_frame(ui.painter(), reverb, "Reverb", STYLE.paint);
                param_knob(
                    ui,
                    setter,
                    &params.reverb_decay,
                    origin + vec2(450.0, effects_y),
                    19.0,
                    "Decay",
                    None,
                );
                param_knob(
                    ui,
                    setter,
                    &params.reverb_mix,
                    origin + vec2(558.0, effects_y),
                    19.0,
                    "Mix",
                    None,
                );

                for (param, label, x) in [
                    (&params.delay_on, "Delay", DELAY_SWITCH_X),
                    (&params.delay_sync, "Sync", SYNC_SWITCH_X),
                    (&params.reverb_on, "Reverb", REVERB_SWITCH_X),
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
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // The picture on the face is compiled in: a build without it is a build of other sources
    #[test]
    fn test_the_pedal_is_drawn_with_its_art() {
        assert!(STYLE.art == Some(ui::artwork::BELLS), "the pedal has lost its art");
    }

    #[test]
    fn test_the_footswitches_stand_apart_on_the_face() {
        // A footswitch is 74 px across; the kit switches of the drums stand 100 apart
        let switches = [DELAY_SWITCH_X, SYNC_SWITCH_X, REVERB_SWITCH_X];
        for pair in switches.windows(2) {
            assert!(pair[1] - pair[0] >= 100.0, "{pair:?}");
        }
        // Delay and Sync under the Delay frame (40 to 372), Reverb under its own (388 to 620)
        const {
            assert!(DELAY_SWITCH_X - 37.0 >= 40.0 && SYNC_SWITCH_X + 37.0 <= 372.0);
            assert!(REVERB_SWITCH_X - 37.0 >= 388.0 && REVERB_SWITCH_X + 37.0 <= 620.0);
            assert!(REVERB_SWITCH_X + 37.0 < WINDOW_WIDTH as f32);
        }
    }
}
