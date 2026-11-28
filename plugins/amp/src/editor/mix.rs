use super::*;

// On the plate, from its top, in four zones: the Amps frame (the icon that says one amp or two,
// in view in both modes), the Blend frame (only with two amps) (the fader with its end captions
// and its value beside the slot, to the left of the middle), the Cabinets frame (Mic and Res,
// Mic Type, and the Cab switch with its LED, all of them for both cabinets), and under them the
// output jack. Across, from the plate's middle
pub(super) const PLATE_FRAME_INSET: f32 = 10.0;
pub(super) const AMPS_FRAME_TOP: f32 = 14.0;
pub(super) const AMPS_FRAME_BOTTOM: f32 = 58.0;
pub(super) const BLEND_FRAME_TOP: f32 = 72.0;
pub(super) const BLEND_FRAME_BOTTOM: f32 = 192.0;
pub(super) const FADER_DX: f32 = -12.0;
pub(super) const MIX_AMP1_CAPTION_Y: f32 = 92.0;
pub(super) const FADER_TOP: f32 = 117.0;
pub(super) const FADER_LENGTH: f32 = 38.0;
pub(super) const MIX_AMP2_CAPTION_Y: f32 = 180.0;
pub(super) const MIX_END_CAPTION_SIZE: f32 = 15.0;
pub(super) const MIX_BLEND_TAPE_DX: f32 = 38.0;
pub(super) const MIX_BLEND_TAPE_Y: f32 = FADER_TOP + FADER_LENGTH / 2.0 - 8.0;
pub(super) const CABS_FRAME_TOP: f32 = 206.0;
pub(super) const CABS_FRAME_BOTTOM: f32 = 444.0;
pub(super) const MIX_KNOB_Y: f32 = 251.0;
pub(super) const MIX_KNOB_DX: f32 = 31.0;
pub(super) const MIX_KNOB_RADIUS: f32 = 15.0;
pub(super) const MIC_CAPTION_Y: f32 = 331.0;
pub(super) const MIC_STEPPER_Y: f32 = 355.0;
pub(super) const MIC_STEPPER_WIDTH: f32 = 44.0;
pub(super) const MIX_SWITCH_CAPTION_Y: f32 = 386.0;
pub(super) const MIX_LED_Y: f32 = 410.0;
pub(super) const MIX_LED_DX: f32 = -30.0;
pub(super) const MIX_SWITCH_Y: f32 = 407.0;
pub(super) const MIX_SWITCH_DX: f32 = 22.0;
pub(super) const MIX_JACK_Y: f32 = 462.0;
pub(super) const MIX_JACK_DX: f32 = 16.0;
pub(super) const MIX_JACK_CAPTION_DX: f32 = -24.0;

impl Pedals<'_> {
    /// The plate where the lanes meet: the Amps icon, with two amps the Blend fader between
    /// them, then what both share. Returns the output jack
    pub(super) fn mix_plate(&self, ui: &mut Ui, plate: Rect) -> Pos2 {
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

        // The mode: one amp or two, the second amp's switch
        let two = params.amp2_on.value();
        // With one amp the frame takes the Blend's place too, so the plate has no bare stretch
        let bottom = if two { AMPS_FRAME_BOTTOM } else { BLEND_FRAME_BOTTOM };
        let icon = ui::amps_icon(ui, frame(AMPS_FRAME_TOP, bottom), paint, two, "amps");
        if icon.clicked() {
            setter.begin_set_parameter(&params.amp2_on);
            setter.set_parameter(&params.amp2_on, !two);
            setter.end_set_parameter(&params.amp2_on);
        }
        icon.on_hover_text(if two {
            "Two amps: click for one"
        } else {
            "One amp: click to add a second"
        });

        // Blend, with two amps: its frame, the fader and the value on a tape beside it
        if two {
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
        }

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
