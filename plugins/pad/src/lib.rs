use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

mod editor;
#[cfg(test)]
mod no_alloc;
mod pad;
use pad::{PadSettings, ShimmerPad};

pub struct ShimmerPadPlugin {
    params: Arc<ShimmerPadParams>,
    pad: ShimmerPad,
    /// The sustain pedal as the last CC 64 left it; the Hold switch is a parameter
    pedal_cc: bool,
}

#[derive(Params)]
pub struct ShimmerPadParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    /// The update check's settings (its off switch, when it last asked, the newest version it
    /// found, the version whose line was closed): saved with the project, never a parameter
    #[persist = "update"]
    pub update: Arc<std::sync::Mutex<suite_common::update::UpdateSettings>>,

    #[id = "volume"]
    pub volume: FloatParam,

    #[id = "attack"]
    pub attack: FloatParam,

    #[id = "release"]
    pub release: FloatParam,

    #[id = "cutoff"]
    pub cutoff: FloatParam,

    #[id = "detune"]
    pub detune: FloatParam,

    #[id = "breath"]
    pub breath: FloatParam,

    #[id = "motion"]
    pub motion: FloatParam,

    #[id = "dry"]
    pub dry: FloatParam,

    #[id = "chorus_on"]
    pub chorus_on: BoolParam,

    #[id = "chorus"]
    pub chorus: FloatParam,

    #[id = "reverb"]
    pub reverb: FloatParam,

    #[id = "reverb_decay"]
    pub reverb_decay: FloatParam,

    #[id = "shimmer_on"]
    pub shimmer_on: BoolParam,

    #[id = "shimmer"]
    pub shimmer: FloatParam,

    /// Keeps every key that is let go sounding, as a sustain pedal does
    #[id = "hold"]
    pub hold: BoolParam,
}

impl Default for ShimmerPadPlugin {
    fn default() -> Self {
        Self {
            params: Arc::new(ShimmerPadParams::default()),
            pad: ShimmerPad::new(),
            pedal_cc: false,
        }
    }
}

impl Default for ShimmerPadParams {
    fn default() -> Self {
        // Every default is the one of the DSP
        let defaults = PadSettings::default();
        Self {
            editor_state: editor::default_state(),
            update: Arc::default(),

            volume: FloatParam::new(
                "Volume",
                util::db_to_gain(pad::VOLUME_DEFAULT_DB),
                FloatRange::Skewed {
                    min: util::db_to_gain(pad::VOLUME_MIN_DB),
                    max: util::db_to_gain(pad::VOLUME_MAX_DB),
                    factor: FloatRange::gain_skew_factor(pad::VOLUME_MIN_DB, pad::VOLUME_MAX_DB),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            attack: seconds_param(
                "Attack",
                defaults.attack_s,
                pad::ATTACK_MIN_S,
                pad::ATTACK_MAX_S,
            ),
            release: seconds_param(
                "Release",
                defaults.release_s,
                pad::RELEASE_MIN_S,
                pad::RELEASE_MAX_S,
            ),

            cutoff: FloatParam::new(
                "Cutoff",
                defaults.cutoff_hz,
                FloatRange::Skewed {
                    min: pad::CUTOFF_MIN_HZ,
                    max: pad::CUTOFF_MAX_HZ,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_step_size(1.0)
            .with_unit(" Hz"),

            detune: FloatParam::new(
                "Detune",
                defaults.detune_cents,
                FloatRange::Linear { min: 0.0, max: pad::DETUNE_MAX_CENTS },
            )
            .with_step_size(0.1)
            .with_unit(" ct")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            breath: percent_param("Breath", defaults.breath, 1.0),
            motion: percent_param("Motion", defaults.motion, 1.0),
            dry: percent_param("Dry", defaults.dry, 1.0),

            chorus_on: BoolParam::new("Chorus", defaults.chorus_on),
            chorus: percent_param("Chorus Depth", defaults.chorus, 1.0),

            reverb: percent_param("Reverb", defaults.reverb, 1.0),
            reverb_decay: seconds_param(
                "Reverb Decay",
                defaults.reverb_decay_s,
                pad::REVERB_DECAY_MIN_S,
                pad::REVERB_DECAY_MAX_S,
            ),

            shimmer_on: BoolParam::new("Shimmer", defaults.shimmer_on),
            shimmer: percent_param("Shimmer Level", defaults.shimmer, pad::SHIMMER_MAX),

            hold: BoolParam::new("Hold", false),
        }
    }
}

/// 0.0 to `max` parameter shown as a percentage
fn percent_param(name: &str, default: f32, max: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max })
        .with_unit(" %")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

/// A time in seconds, with more of the knob for the short ones
fn seconds_param(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed { min, max, factor: FloatRange::skew_factor(-1.0) },
    )
    .with_step_size(0.01)
    .with_unit(" s")
    .with_value_to_string(formatters::v2s_f32_rounded(2))
}

/// CC 64: 64 and up (0.5 and up, as nih-plug scales 0-127) is down
fn pedal_down(value: f32) -> bool {
    value >= 0.5
}

/// The pedal and the Hold switch are one state: down if either is
fn sustain(pedal_cc: bool, hold: bool) -> bool {
    pedal_cc || hold
}

impl ShimmerPadParams {
    fn settings(&self) -> PadSettings {
        PadSettings {
            attack_s: self.attack.value(),
            release_s: self.release.value(),
            cutoff_hz: self.cutoff.value(),
            detune_cents: self.detune.value(),
            breath: self.breath.value(),
            motion: self.motion.value(),
            dry: self.dry.value(),
            chorus_on: self.chorus_on.value(),
            chorus: self.chorus.value(),
            reverb: self.reverb.value(),
            reverb_decay_s: self.reverb_decay.value(),
            shimmer_on: self.shimmer_on.value(),
            shimmer: self.shimmer.value(),
        }
    }
}

impl Plugin for ShimmerPadPlugin {
    const NAME: &'static str = "Hojt Shimmer Pad";
    const VENDOR: &'static str = suite_common::VENDOR;
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    // Controllers too: all notes off lets go of every key, all sound off stops voices and tails
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.params.editor_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.pad.set(&self.params.settings());
        self.pad.set_sample_rate(buffer_config.sample_rate);
        true
    }

    fn reset(&mut self) {
        self.pad.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        self.pad.set(&self.params.settings());
        // The Hold switch is read once per block; the pedal's edges are kept to the sample
        let hold = self.params.hold.value();
        self.pad.set_sustain(sustain(self.pedal_cc, hold));

        let mut next_event = context.next_event();

        for (sample_id, mut frame) in buffer.iter_samples().enumerate() {
            // Handle all events that land on this sample
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }

                match event {
                    NoteEvent::NoteOn { note, velocity, .. } => self.pad.note_on(note, velocity),
                    NoteEvent::NoteOff { note, .. } => self.pad.note_off(note),
                    NoteEvent::MidiCC { cc, .. } if cc == control_change::ALL_NOTES_OFF => {
                        self.pad.release()
                    }
                    NoteEvent::MidiCC { cc, .. } if cc == control_change::ALL_SOUND_OFF => {
                        self.pad.reset()
                    }
                    NoteEvent::MidiCC { cc, value, .. } if cc == control_change::DAMPER_PEDAL => {
                        self.pedal_cc = pedal_down(value);
                        self.pad.set_sustain(sustain(self.pedal_cc, hold));
                    }
                    NoteEvent::MidiCC { cc, value, .. } if cc == control_change::MODULATION_MSB => {
                        self.pad.set_wheel(value)
                    }
                    _ => {}
                }

                next_event = context.next_event();
            }

            let gain = self.params.volume.smoothed.next();
            let (left, right) = self.pad.process();

            for (channel, sample) in frame.iter_mut().enumerate() {
                *sample = if channel == 0 { left } else { right } * gain;
            }
        }

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for ShimmerPadPlugin {
    const CLAP_ID: &'static str = "net.hojte.shimmer-pad";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("A slow ambient pad with a chorus and a shimmering hall");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for ShimmerPadPlugin {
    // Frozen: DAW projects find the plugin by this ID. Never change it
    const VST3_CLASS_ID: [u8; 16] = *b"HojtShimmerPad01";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
    // The release and the hall ring on after the last key, up to 20 s. A host that asks only once, before the first block, would hear 0
    const VST3_INFINITE_TAIL: bool = true;
}

nih_export_clap!(ShimmerPadPlugin);
// vst3-sys's `vtable` macro, which `nih_export_vst3!` expands, ends an expression with a
// semicolon, which Rust is phasing out (rust-lang/rust#79813). It is in vst3-sys, pinned by
// nih-plug, not in our code: a fixed vst3-sys, pinned through our nih-plug fork, removes this.
// An `allow` on the macro call itself is ignored, so it is on a module around it; the
// factory's `#[no_mangle]` functions are exported from inside a module all the same
#[allow(semicolon_in_expressions_from_non_local_macros)]
mod vst3_export {
    use super::*;
    nih_export_vst3!(ShimmerPadPlugin);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cc_64_is_down_from_half() {
        let down: Vec<bool> = [0.0, 0.49, 0.5, 1.0].into_iter().map(pedal_down).collect();
        assert_eq!(down, [false, false, true, true]);
    }

    #[test]
    fn test_pedal_and_hold_are_or_ed() {
        assert!(!sustain(false, false));
        assert!(sustain(true, false));
        assert!(sustain(false, true));
        assert!(sustain(true, true));
        assert!(!ShimmerPadParams::default().hold.value());
    }

    #[test]
    fn test_parameter_defaults_are_those_of_the_dsp() {
        assert_eq!(ShimmerPadParams::default().settings(), PadSettings::default());
    }
}

#[cfg(test)]
mod ids {
    /// Bundle name, plugin ids and every parameter id stay as saved projects know them
    #[test]
    fn ids() {
        suite_common::ids::check::<super::ShimmerPadPlugin>(
            env!("CARGO_MANIFEST_DIR"),
            include_str!("../../../bundler.toml"),
            "pad",
        );
    }

    #[test]
    fn vst3_tail_is_infinite() {
        use nih_plug::prelude::Vst3Plugin;
        let infinite = super::ShimmerPadPlugin::VST3_INFINITE_TAIL;
        assert!(infinite);
    }
}
