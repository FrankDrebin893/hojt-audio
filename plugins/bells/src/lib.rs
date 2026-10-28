use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;
use suite_common::dsp::NoteValue;

mod bells;
mod editor;
#[cfg(test)]
mod no_alloc;
use bells::{BellSettings, GlassBells};

// The note the delay's Time knob picks when Sync is first switched on
const DELAY_NOTE: NoteValue = NoteValue::DottedEighth;

pub struct GlassBellsPlugin {
    params: Arc<GlassBellsParams>,
    bells: GlassBells,
}

#[derive(Params)]
pub struct GlassBellsParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "volume"]
    pub volume: FloatParam,

    #[id = "brightness"]
    pub brightness: FloatParam,

    #[id = "decay"]
    pub decay: FloatParam,

    #[id = "attack"]
    pub attack: FloatParam,

    #[id = "dry"]
    pub dry: FloatParam,

    #[id = "delay_on"]
    pub delay_on: BoolParam,

    #[id = "delay_time"]
    pub delay_time: FloatParam,

    #[id = "delay_feedback"]
    pub delay_feedback: FloatParam,

    #[id = "delay_mix"]
    pub delay_mix: FloatParam,

    /// The delay follows the song's tempo: the Time knob picks `delay_note`, and `delay_time`
    /// keeps its ms for when Sync is off
    #[id = "delay_sync"]
    pub delay_sync: BoolParam,

    #[id = "delay_note"]
    pub delay_note: EnumParam<NoteValue>,

    #[id = "reverb_on"]
    pub reverb_on: BoolParam,

    #[id = "reverb_decay"]
    pub reverb_decay: FloatParam,

    #[id = "reverb_mix"]
    pub reverb_mix: FloatParam,
}

impl Default for GlassBellsPlugin {
    fn default() -> Self {
        Self { params: Arc::new(GlassBellsParams::default()), bells: GlassBells::new() }
    }
}

impl Default for GlassBellsParams {
    fn default() -> Self {
        // Every default is the one of the DSP
        let defaults = BellSettings::default();
        Self {
            editor_state: editor::default_state(),

            volume: FloatParam::new(
                "Volume",
                util::db_to_gain(bells::VOLUME_DEFAULT_DB),
                FloatRange::Skewed {
                    min: util::db_to_gain(bells::VOLUME_MIN_DB),
                    max: util::db_to_gain(bells::VOLUME_MAX_DB),
                    factor: FloatRange::gain_skew_factor(
                        bells::VOLUME_MIN_DB,
                        bells::VOLUME_MAX_DB,
                    ),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(20.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            brightness: percent_param("Brightness", defaults.brightness, 1.0),

            decay: seconds_param("Decay", defaults.decay_s, bells::DECAY_MIN_S, bells::DECAY_MAX_S),

            attack: FloatParam::new(
                "Attack",
                defaults.attack_ms,
                FloatRange::Skewed {
                    min: bells::ATTACK_MIN_MS,
                    max: bells::ATTACK_MAX_MS,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_step_size(0.1)
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            dry: percent_param("Dry", defaults.dry, 1.0),

            delay_on: BoolParam::new("Delay", defaults.delay_on),

            delay_time: FloatParam::new(
                "Delay Time",
                defaults.delay_ms,
                FloatRange::Skewed {
                    min: bells::DELAY_MIN_MS,
                    max: bells::DELAY_MAX_MS,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_step_size(1.0)
            .with_unit(" ms"),

            delay_feedback: percent_param(
                "Delay Feedback",
                defaults.delay_feedback,
                bells::FEEDBACK_MAX,
            ),
            delay_mix: percent_param("Delay Mix", defaults.delay_mix, 1.0),
            // Off, so a project from before sounds as it did
            delay_sync: BoolParam::new("Delay Sync", false),
            delay_note: EnumParam::new("Delay Note", DELAY_NOTE),

            reverb_on: BoolParam::new("Reverb", defaults.reverb_on),

            reverb_decay: seconds_param(
                "Reverb Decay",
                defaults.reverb_decay_s,
                bells::REVERB_DECAY_MIN_S,
                bells::REVERB_DECAY_MAX_S,
            ),
            reverb_mix: percent_param("Reverb Mix", defaults.reverb_mix, 1.0),
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
    .with_step_size(0.1)
    .with_unit(" s")
    .with_value_to_string(formatters::v2s_f32_rounded(1))
}

impl GlassBellsParams {
    /// What the knobs say now, with the song at `tempo_bpm` (`None` where the host gives
    /// no tempo)
    fn settings(&self, tempo_bpm: Option<f64>) -> BellSettings {
        BellSettings {
            brightness: self.brightness.value(),
            decay_s: self.decay.value(),
            attack_ms: self.attack.value(),
            dry: self.dry.value(),
            delay_on: self.delay_on.value(),
            delay_ms: delay_time_ms(
                self.delay_time.value(),
                self.delay_sync.value(),
                self.delay_note.value(),
                tempo_bpm,
            ),
            delay_feedback: self.delay_feedback.value(),
            delay_mix: self.delay_mix.value(),
            reverb_on: self.reverb_on.value(),
            reverb_decay_s: self.reverb_decay.value(),
            reverb_mix: self.reverb_mix.value(),
        }
    }
}

/// The delay's time: the Time knob's ms, or with Sync on the note it picks at the song's tempo,
/// kept to the delay's range. A tempo that changes glides as a turn of the knob does
fn delay_time_ms(knob_ms: f32, sync: bool, note: NoteValue, tempo_bpm: Option<f64>) -> f32 {
    suite_common::dsp::delay_time_ms(
        knob_ms,
        sync.then_some(note),
        tempo_bpm,
        bells::DELAY_MIN_MS,
        bells::DELAY_MAX_MS,
    )
}

impl Plugin for GlassBellsPlugin {
    const NAME: &'static str = "Hojt Glass Bells";
    const VENDOR: &'static str = suite_common::VENDOR;
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    // Controllers too: all sound off stops bells and tails. A bell rings out by itself, so
    // all notes off has nothing to let go of
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
        self.bells.set(&self.params.settings(None));
        self.bells.set_sample_rate(buffer_config.sample_rate);
        true
    }

    fn reset(&mut self) {
        self.bells.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // The song's tempo is only asked for while the delay follows it
        let tempo_bpm =
            if self.params.delay_sync.value() { context.transport().tempo } else { None };
        self.bells.set(&self.params.settings(tempo_bpm));

        let mut next_event = context.next_event();

        for (sample_id, mut frame) in buffer.iter_samples().enumerate() {
            // Handle all events that land on this sample
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }

                match event {
                    NoteEvent::NoteOn { note, velocity, .. } => self.bells.note_on(note, velocity),
                    NoteEvent::MidiCC { cc, .. } if cc == control_change::ALL_SOUND_OFF => {
                        self.bells.reset()
                    }
                    _ => {}
                }

                next_event = context.next_event();
            }

            let gain = self.params.volume.smoothed.next();
            let (left, right) = self.bells.process();

            for (channel, sample) in frame.iter_mut().enumerate() {
                *sample = if channel == 0 { left } else { right } * gain;
            }
        }

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for GlassBellsPlugin {
    const CLAP_ID: &'static str = "net.hojte.glass-bells";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Soft glassy bells with a ping-pong delay and a hall");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] =
        &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for GlassBellsPlugin {
    // Frozen: DAW projects find the plugin by this ID. Never change it
    const VST3_CLASS_ID: [u8; 16] = *b"HojtGlassBells01";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
    // The bells, the delay and the hall ring on after the last note, up to 20 s. A host that asks only once, before the first block, would hear 0
    const VST3_INFINITE_TAIL: bool = true;
}

nih_export_clap!(GlassBellsPlugin);
// vst3-sys's `vtable` macro, which `nih_export_vst3!` expands, ends an expression with a
// semicolon, which Rust is phasing out (rust-lang/rust#79813). It is in vst3-sys, pinned by
// nih-plug, not in our code: a fixed vst3-sys, pinned through our nih-plug fork, removes this.
// An `allow` on the macro call itself is ignored, so it is on a module around it; the
// factory's `#[no_mangle]` functions are exported from inside a module all the same
#[allow(semicolon_in_expressions_from_non_local_macros)]
mod vst3_export {
    use super::*;
    nih_export_vst3!(GlassBellsPlugin);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_defaults_are_those_of_the_dsp() {
        assert_eq!(GlassBellsParams::default().settings(None), BellSettings::default());
    }

    #[test]
    fn test_a_synced_delay_repeats_on_the_note_at_the_songs_tempo() {
        let synced = |bpm| delay_time_ms(445.0, true, NoteValue::DottedEighth, Some(bpm));
        let at_120 = bells::tests::first_echo_ms(synced(120.0));
        assert!((at_120 - 375.0).abs() <= 1.0, "a dotted 1/8 at 120 bpm repeats at {at_120} ms");
        let at_90 = bells::tests::first_echo_ms(synced(90.0));
        assert!((at_90 - 500.0).abs() <= 1.0, "a dotted 1/8 at 90 bpm repeats at {at_90} ms");
        // Slower than the delay reaches: its longest time. With no tempo, the knob
        assert_eq!(delay_time_ms(445.0, true, NoteValue::Quarter, Some(30.0)), bells::DELAY_MAX_MS);
        assert_eq!(delay_time_ms(445.0, true, NoteValue::Quarter, None), 445.0);
    }

    #[test]
    fn test_without_sync_the_delay_is_as_it_was_at_any_tempo() {
        for knob_ms in [bells::DELAY_MIN_MS, 445.0, bells::DELAY_MAX_MS] {
            let before = bells::tests::echoes(knob_ms);
            for tempo in [None, Some(60.0), Some(120.0), Some(187.5)] {
                for note in [NoteValue::Quarter, NoteValue::Sixteenth] {
                    let now = bells::tests::echoes(delay_time_ms(knob_ms, false, note, tempo));
                    assert!(now == before, "{knob_ms} ms at {tempo:?} bpm");
                }
            }
        }
        // The parameters as they start, and as a project from before has them: Sync off
        let params = GlassBellsParams::default();
        assert!(!params.delay_sync.value());
        assert_eq!(params.delay_note.value(), NoteValue::DottedEighth);
        assert_eq!(params.settings(Some(120.0)), params.settings(None));
    }

    #[test]
    fn test_a_project_from_before_opens_with_sync_off() {
        // A state saved before there was Sync names neither new id. Nothing adds them, so
        // they stay at what a new plugin starts with: Sync off
        let mut state = PluginState {
            version: "0.2.0".to_owned(),
            params: [
                ("delay_on", nih_plug::wrapper::state::ParamValue::Bool(true)),
                ("delay_time", nih_plug::wrapper::state::ParamValue::F32(512.0)),
            ]
            .into_iter()
            .map(|(id, value)| (id.to_owned(), value))
            .collect(),
            fields: Default::default(),
        };
        GlassBellsPlugin::filter_state(&mut state);
        assert!(!state.params.contains_key("delay_sync"));
        assert!(!state.params.contains_key("delay_note"));
        assert!(!GlassBellsParams::default().delay_sync.default_plain_value());
    }
}

#[cfg(test)]
mod ids {
    /// Bundle name, plugin ids and every parameter id stay as saved projects know them
    #[test]
    fn ids() {
        suite_common::ids::check::<super::GlassBellsPlugin>(
            env!("CARGO_MANIFEST_DIR"),
            include_str!("../../../bundler.toml"),
            "bells",
        );
    }

    #[test]
    fn vst3_tail_is_infinite() {
        use nih_plug::prelude::Vst3Plugin;
        let infinite = super::GlassBellsPlugin::VST3_INFINITE_TAIL;
        assert!(infinite);
    }
}
