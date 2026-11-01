use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

mod amp;
mod board;
mod cab;
mod cab_stage;
mod chain;
mod comp;
mod delay;
mod dial_memory;
mod drive;
mod dsp;
mod editor;
mod fuzz;
mod gate;
mod lamps;
mod looks;
mod metronome;
mod mods;
#[cfg(test)]
mod no_alloc;
mod order;
mod pitch;
mod presets;
mod reverb;
mod second;
pub mod standalone;
#[cfg(test)]
mod test_util;
mod tuner;
mod user_cab;
mod user_presets;
pub use amp::model::Amp;
use cab::MicType;
use cab_stage::CabLoader;
use chain::{AmpChain, AmpSettings, GATE_RELEASE_MS, GATE_THRESHOLD_DB, IN_GAIN_DB};
use comp::CompSettings;
use delay::{DelaySettings, FEEDBACK_MAX, TIME_MAX_MS, TIME_MIN_MS};
use dial_memory::DialMemory;
use fuzz::FuzzSettings;
use metronome::{Metronome, MetronomeSettings, Sound, TimeSignature};
use mods::chorus::{self, ChorusSettings};
use mods::flanger::{self, FlangerSettings};
use mods::phaser::{self, PhaserSettings};
use mods::tremolo::{self, TremoloSettings};
use order::StoredOrder;
use pitch::{PitchSettings, LATENCY_MS, SEMITONES};
use reverb::{ReverbSettings, DECAY_MAX_S, DECAY_MIN_S};
use second::SecondSettings;
use suite_common::dsp::NoteValue;

// The id of the tuner switch, as in `GuitarAmpParams`
const TUNER_ID: &str = "tuner";

// The note the delay's Time knob picks when Sync is first switched on
const DELAY_NOTE: NoteValue = NoteValue::DottedEighth;

const LEVEL_MIN_DB: f32 = -30.0;
const LEVEL_MAX_DB: f32 = 6.0;

pub struct GuitarAmpPlugin {
    params: Arc<GuitarAmpParams>,
    chain: AmpChain,
    // Reads the player's own cabinet from its file and leaves it for the chain to take
    cab_loader: Arc<CabLoader>,
    /// A cabinet read is owed: `initialize` only asks, the first block hands it to the host's
    /// background thread, so a slow disk never holds up activation
    cab_load_due: bool,
    /// The practice click, added to the output after the whole chain
    metronome: Metronome,
}

/// What is done away from the audio thread
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// Read the chosen cabinet, for the sample rate the host runs at now
    LoadCabinet,
}

// Frozen: the ids below are what DAW projects store. Add parameters, never rename an id.
// The full table of ids is in `ids.txt`, held by the `ids` test
#[derive(Params)]
pub struct GuitarAmpParams {
    #[persist = "editor-state-rig6"]
    pub editor_state: Arc<EguiState>,

    /// Index into `presets::PRESETS` of the preset loaded last. Not a parameter: the editor
    /// sets the parameters themselves when a preset is loaded
    #[persist = "preset"]
    pub preset: std::sync::atomic::AtomicU32,

    /// File name, in the folder of the bank showing, of the player's own preset loaded
    /// last. Empty while the one loaded last is a factory preset (`preset`), and while a
    /// bank is showing that nothing has been loaded out of. Not a parameter, like `preset`
    #[persist = "user-preset"]
    pub user_preset: std::sync::Mutex<String>,

    /// The bank of presets showing on the head: empty for the factory presets, `/` for the
    /// presets folder itself and `/<folder>` for a folder in it (`user_presets`). A place,
    /// not a sound, so not a parameter
    #[persist = "preset-bank"]
    pub preset_bank: std::sync::Mutex<String>,

    /// File name, in the cabinets folder, of the player's own cabinet. Empty for the amp's
    /// own. Not a parameter: a file name cannot be one. The audio thread never reads it
    #[persist = "cabinet"]
    pub cabinet: std::sync::Mutex<String>,

    /// Where each amp's six dials stood when the player last switched away from it. Not
    /// parameters: the editor sets the dials themselves when an amp is picked on the head.
    /// The audio thread never reads it
    #[persist = "amp-dials"]
    pub dial_memory: DialMemory,

    /// The same for the second amp: where each amp's six dials stood when the player last
    /// switched the second amp away from it. Not parameters either
    #[persist = "amp2-dials"]
    pub dial_memory_second: DialMemory,

    /// The order the pedals stand in. Not a parameter: an arrangement cannot be automated.
    /// The editor changes it, and the audio thread reads it with the dials
    #[persist = "chain-order"]
    pub order: StoredOrder,

    #[id = "bypass"]
    pub bypass: BoolParam,

    // Input and gate
    #[id = "in_gain"]
    pub in_gain: FloatParam,

    #[id = "gate_on"]
    pub gate_on: BoolParam,

    #[id = "gate_thresh"]
    pub gate_thresh: FloatParam,

    #[id = "gate_release"]
    pub gate_release: FloatParam,

    // Fuzz pedal
    #[id = "fuzz_on"]
    pub fuzz_on: BoolParam,

    #[id = "fuzz_fuzz"]
    pub fuzz_fuzz: FloatParam,

    #[id = "fuzz_tone"]
    pub fuzz_tone: FloatParam,

    // Compressor pedal
    #[id = "comp_on"]
    pub comp_on: BoolParam,

    #[id = "comp_sustain"]
    pub comp_sustain: FloatParam,

    #[id = "comp_level"]
    pub comp_level: FloatParam,

    // Drive pedal
    #[id = "drive_on"]
    pub drive_on: BoolParam,

    #[id = "drive_gain"]
    pub drive_gain: FloatParam,

    #[id = "drive_tone"]
    pub drive_tone: FloatParam,

    #[id = "drive_level"]
    pub drive_level: FloatParam,

    // Amp
    #[id = "amp"]
    pub amp: EnumParam<Amp>,

    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "bass"]
    pub bass: FloatParam,

    #[id = "mid"]
    pub mid: FloatParam,

    #[id = "treble"]
    pub treble: FloatParam,

    #[id = "presence"]
    pub presence: FloatParam,

    #[id = "master"]
    pub master: FloatParam,

    // The second amp, beside the first behind the same front pedals
    #[id = "amp2_on"]
    pub amp2_on: BoolParam,

    #[id = "amp2"]
    pub amp2: EnumParam<Amp>,

    #[id = "amp2_gain"]
    pub amp2_gain: FloatParam,

    #[id = "amp2_bass"]
    pub amp2_bass: FloatParam,

    #[id = "amp2_mid"]
    pub amp2_mid: FloatParam,

    #[id = "amp2_treble"]
    pub amp2_treble: FloatParam,

    #[id = "amp2_presence"]
    pub amp2_presence: FloatParam,

    #[id = "amp2_master"]
    pub amp2_master: FloatParam,

    /// 0.0 is the first amp alone, 1.0 the second alone
    #[id = "amp_blend"]
    pub amp_blend: FloatParam,

    // Modulation pedals
    #[id = "chorus_on"]
    pub chorus_on: BoolParam,

    #[id = "chorus_rate"]
    pub chorus_rate: FloatParam,

    #[id = "chorus_depth"]
    pub chorus_depth: FloatParam,

    #[id = "phaser_on"]
    pub phaser_on: BoolParam,

    #[id = "phaser_rate"]
    pub phaser_rate: FloatParam,

    #[id = "phaser_depth"]
    pub phaser_depth: FloatParam,

    #[id = "flanger_on"]
    pub flanger_on: BoolParam,

    #[id = "flanger_rate"]
    pub flanger_rate: FloatParam,

    #[id = "flanger_depth"]
    pub flanger_depth: FloatParam,

    #[id = "trem_on"]
    pub trem_on: BoolParam,

    #[id = "trem_rate"]
    pub trem_rate: FloatParam,

    #[id = "trem_depth"]
    pub trem_depth: FloatParam,

    // Cabinet
    #[id = "cab_on"]
    pub cab_on: BoolParam,

    #[id = "cab_mic"]
    pub cab_mic: FloatParam,

    #[id = "cab_res"]
    pub cab_res: FloatParam,

    #[id = "cab_mic_type"]
    pub cab_mic_type: EnumParam<MicType>,

    // Delay
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

    // Reverb
    #[id = "reverb_on"]
    pub reverb_on: BoolParam,

    #[id = "reverb_decay"]
    pub reverb_decay: FloatParam,

    #[id = "reverb_mix"]
    pub reverb_mix: FloatParam,

    // Output
    #[id = "out_level"]
    pub out_level: FloatParam,

    // Tuner: a switch on the head, not part of the sound. Presets and saved projects leave it alone
    #[id = "tuner"]
    pub tuner_on: BoolParam,

    // Metronome: a click for practice, added to the output after the chain. Not part of a
    // sound, so presets leave all six alone
    #[id = "metronome_on"]
    pub metronome_on: BoolParam,

    #[id = "metronome_bpm"]
    pub metronome_bpm: FloatParam,

    /// Follow the host's tempo and transport, where it gives them
    #[id = "metronome_sync"]
    pub metronome_sync: BoolParam,

    #[id = "metronome_time_sig"]
    pub metronome_time_sig: EnumParam<TimeSignature>,

    #[id = "metronome_volume"]
    pub metronome_volume: FloatParam,

    #[id = "metronome_sound"]
    pub metronome_sound: EnumParam<Sound>,

    // Pitch shifter pedal, in front of the amp. Last in the list so the parameters before it
    // keep their places for a host that shows them in order
    #[id = "pitch_on"]
    pub pitch_on: BoolParam,

    #[id = "pitch_semitones"]
    pub pitch_semitones: IntParam,

    #[id = "pitch_blend"]
    pub pitch_blend: FloatParam,

    #[id = "pitch_latency"]
    pub pitch_latency: FloatParam,
}

impl Default for GuitarAmpPlugin {
    fn default() -> Self {
        let chain = AmpChain::new();
        let cab_loader = Arc::new(CabLoader::new(chain.cab_stage(), chain.sample_rate()));
        Self {
            params: Arc::new(GuitarAmpParams::default()),
            chain,
            cab_loader,
            cab_load_due: false,
            metronome: Metronome::new(),
        }
    }
}

impl Default for GuitarAmpParams {
    fn default() -> Self {
        // Every default comes from the settings the chain itself starts with, and the
        // ranges from the limits the chain and the effects keep to
        let defaults = AmpSettings::default();
        Self {
            editor_state: editor::default_state(),
            preset: std::sync::atomic::AtomicU32::new(0),
            user_preset: std::sync::Mutex::new(String::new()),
            preset_bank: std::sync::Mutex::new(String::new()),
            cabinet: std::sync::Mutex::new(String::new()),
            order: StoredOrder::new(),
            dial_memory: DialMemory::new([
                defaults.gain,
                defaults.bass,
                defaults.mid,
                defaults.treble,
                defaults.presence,
                defaults.master,
            ]),
            dial_memory_second: DialMemory::new(defaults.second.dials()),

            bypass: BoolParam::new("Bypass", defaults.bypass).make_bypass(),

            in_gain: FloatParam::new(
                "Input",
                defaults.in_gain_db,
                FloatRange::Linear { min: IN_GAIN_DB[0], max: IN_GAIN_DB[1] },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            gate_on: BoolParam::new("Gate", defaults.gate_on),
            gate_thresh: FloatParam::new(
                "Gate Threshold",
                defaults.gate_thresh_db,
                FloatRange::Linear { min: GATE_THRESHOLD_DB[0], max: GATE_THRESHOLD_DB[1] },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            gate_release: FloatParam::new(
                "Gate Release",
                defaults.gate_release_ms,
                FloatRange::Skewed {
                    min: GATE_RELEASE_MS[0],
                    max: GATE_RELEASE_MS[1],
                    factor: FloatRange::skew_factor(-1.0), // More resolution at low end
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),

            fuzz_on: BoolParam::new("Fuzz", defaults.fuzz.on),
            fuzz_fuzz: dial_param("Fuzz Amount", defaults.fuzz.fuzz),
            fuzz_tone: dial_param("Fuzz Tone", defaults.fuzz.tone),

            comp_on: BoolParam::new("Comp", defaults.comp.on),
            comp_sustain: dial_param("Comp Sustain", defaults.comp.sustain),
            comp_level: dial_param("Comp Level", defaults.comp.level),

            drive_on: BoolParam::new("Drive", defaults.drive_on),
            drive_gain: dial_param("Drive Gain", defaults.drive_gain),
            drive_tone: dial_param("Drive Tone", defaults.drive_tone),
            drive_level: dial_param("Drive Level", defaults.drive_level),

            amp: EnumParam::new("Amp", defaults.amp),

            gain: dial_param("Gain", defaults.gain),
            bass: dial_param("Bass", defaults.bass),
            mid: dial_param("Mid", defaults.mid),
            treble: dial_param("Treble", defaults.treble),
            presence: dial_param("Presence", defaults.presence),
            master: dial_param("Master", defaults.master),

            amp2_on: BoolParam::new("Amp 2", defaults.second.on),
            amp2: EnumParam::new("Amp 2 Model", defaults.second.amp),
            amp2_gain: dial_param("Amp 2 Gain", defaults.second.gain),
            amp2_bass: dial_param("Amp 2 Bass", defaults.second.bass),
            amp2_mid: dial_param("Amp 2 Mid", defaults.second.mid),
            amp2_treble: dial_param("Amp 2 Treble", defaults.second.treble),
            amp2_presence: dial_param("Amp 2 Presence", defaults.second.presence),
            amp2_master: dial_param("Amp 2 Master", defaults.second.master),
            amp_blend: dial_param("Amp Blend", defaults.blend),

            chorus_on: BoolParam::new("Chorus", defaults.chorus.on),
            chorus_rate: rate_param(
                "Chorus Rate",
                defaults.chorus.rate_hz,
                chorus::RATE_MIN_HZ,
                chorus::RATE_MAX_HZ,
            ),
            chorus_depth: percent_param("Chorus Depth", defaults.chorus.depth, 1.0),
            phaser_on: BoolParam::new("Phaser", defaults.phaser.on),
            phaser_rate: rate_param(
                "Phaser Rate",
                defaults.phaser.rate_hz,
                phaser::RATE_MIN_HZ,
                phaser::RATE_MAX_HZ,
            ),
            phaser_depth: percent_param("Phaser Depth", defaults.phaser.depth, 1.0),
            flanger_on: BoolParam::new("Flanger", defaults.flanger.on),
            flanger_rate: rate_param(
                "Flanger Rate",
                defaults.flanger.rate_hz,
                flanger::RATE_MIN_HZ,
                flanger::RATE_MAX_HZ,
            ),
            flanger_depth: percent_param("Flanger Depth", defaults.flanger.depth, 1.0),
            trem_on: BoolParam::new("Tremolo", defaults.tremolo.on),
            trem_rate: rate_param(
                "Tremolo Rate",
                defaults.tremolo.rate_hz,
                tremolo::RATE_MIN_HZ,
                tremolo::RATE_MAX_HZ,
            ),
            trem_depth: percent_param("Tremolo Depth", defaults.tremolo.depth, 1.0),

            cab_on: BoolParam::new("Cabinet", defaults.cab_on),
            cab_mic: dial_param("Cab Mic", defaults.cab_mic),
            cab_res: dial_param("Cab Resonance", defaults.cab_res),
            cab_mic_type: EnumParam::new("Cab Mic Type", defaults.mic_type),

            delay_on: BoolParam::new("Delay", defaults.delay.on),
            delay_time: FloatParam::new(
                "Delay Time",
                defaults.delay.time_ms,
                FloatRange::Skewed {
                    min: TIME_MIN_MS,
                    max: TIME_MAX_MS,
                    factor: FloatRange::skew_factor(-1.0), // More resolution at low end
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            delay_feedback: percent_param("Delay Feedback", defaults.delay.feedback, FEEDBACK_MAX),
            delay_mix: percent_param("Delay Mix", defaults.delay.mix, 1.0),
            // Off, so a project or a preset from before sounds as it did
            delay_sync: BoolParam::new("Delay Sync", false),
            delay_note: EnumParam::new("Delay Note", DELAY_NOTE),

            reverb_on: BoolParam::new("Reverb", defaults.reverb.on),
            reverb_decay: FloatParam::new(
                "Reverb Decay",
                defaults.reverb.decay_s,
                FloatRange::Skewed {
                    min: DECAY_MIN_S,
                    max: DECAY_MAX_S,
                    factor: FloatRange::skew_factor(-1.0), // More resolution at low end
                },
            )
            .with_unit(" s")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            reverb_mix: percent_param("Reverb Mix", defaults.reverb.mix, 1.0),

            out_level: FloatParam::new(
                "Output",
                defaults.out_level,
                FloatRange::Skewed {
                    min: util::db_to_gain(LEVEL_MIN_DB),
                    max: util::db_to_gain(LEVEL_MAX_DB),
                    factor: FloatRange::gain_skew_factor(LEVEL_MIN_DB, LEVEL_MAX_DB),
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            tuner_on: BoolParam::new("Tuner", defaults.tuner_on).non_automatable(),

            metronome_on: BoolParam::new("Metronome", false),
            metronome_bpm: FloatParam::new(
                "Metronome Tempo",
                metronome::BPM_DEFAULT,
                FloatRange::Linear { min: metronome::BPM_MIN, max: metronome::BPM_MAX },
            )
            .with_step_size(1.0)
            .with_unit(" bpm")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
            metronome_sync: BoolParam::new("Metronome Sync", false),
            metronome_time_sig: EnumParam::new("Metronome Time Signature", TimeSignature::FourFour),
            metronome_volume: FloatParam::new(
                "Metronome Volume",
                metronome::VOLUME_DEFAULT_DB,
                FloatRange::Linear { min: metronome::VOLUME_MIN_DB, max: metronome::VOLUME_MAX_DB },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
            metronome_sound: EnumParam::new("Metronome Sound", Sound::Woodblock),

            pitch_on: BoolParam::new("Pitch", defaults.pitch.on),
            pitch_semitones: IntParam::new(
                "Pitch Semitones",
                defaults.pitch.semitones,
                IntRange::Linear { min: SEMITONES[0], max: SEMITONES[1] },
            )
            .with_value_to_string(Arc::new(|semitones| format!("{semitones:+}").replace("+0", "0")))
            .with_string_to_value(Arc::new(|text| text.trim().parse::<i32>().ok())),
            pitch_blend: percent_param("Pitch Blend", defaults.pitch.blend, 1.0),
            pitch_latency: FloatParam::new(
                "Pitch Tracking",
                defaults.pitch.latency_ms,
                FloatRange::Skewed {
                    min: LATENCY_MS[0],
                    max: LATENCY_MS[1],
                    factor: FloatRange::skew_factor(-1.0), // More resolution at low end
                },
            )
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(0)),
        }
    }
}

impl GuitarAmpParams {
    /// The amp's own six dials, in the order they stand on the head
    pub fn amp_dials(&self) -> [&FloatParam; 6] {
        [&self.gain, &self.bass, &self.mid, &self.treble, &self.presence, &self.master]
    }

    /// The second amp's six dials, in the order they stand on the head
    pub fn amp2_dials(&self) -> [&FloatParam; 6] {
        [
            &self.amp2_gain,
            &self.amp2_bass,
            &self.amp2_mid,
            &self.amp2_treble,
            &self.amp2_presence,
            &self.amp2_master,
        ]
    }

    /// What the metronome is told this block: the knobs, and the host's tempo and transport
    /// where Sync is on. The tuner silences it
    fn metronome_settings(
        &self,
        tempo: Option<f64>,
        playing: bool,
        beats: Option<f64>,
    ) -> MetronomeSettings {
        let sync = self.metronome_sync.value();
        MetronomeSettings {
            on: self.metronome_on.value() && !self.tuner_on.value(),
            bpm: self.metronome_bpm.value(),
            sync,
            time_signature: self.metronome_time_sig.value(),
            gain: metronome::volume_gain(self.metronome_volume.value()),
            sound: self.metronome_sound.value(),
            host_tempo: if sync { tempo } else { None },
            host_playing: playing,
            host_beats: if sync { beats } else { None },
        }
    }

    /// What the knobs say now, with the song at `tempo_bpm` (`None` where the host gives
    /// no tempo)
    fn settings(&self, tempo_bpm: Option<f64>) -> AmpSettings {
        AmpSettings {
            bypass: self.bypass.value(),
            in_gain_db: self.in_gain.value(),
            gate_on: self.gate_on.value(),
            gate_thresh_db: self.gate_thresh.value(),
            gate_release_ms: self.gate_release.value(),
            fuzz: FuzzSettings {
                on: self.fuzz_on.value(),
                fuzz: self.fuzz_fuzz.value(),
                tone: self.fuzz_tone.value(),
            },
            comp: CompSettings {
                on: self.comp_on.value(),
                sustain: self.comp_sustain.value(),
                level: self.comp_level.value(),
            },
            pitch: PitchSettings {
                on: self.pitch_on.value(),
                semitones: self.pitch_semitones.value(),
                blend: self.pitch_blend.value(),
                latency_ms: self.pitch_latency.value(),
            },
            drive_on: self.drive_on.value(),
            drive_gain: self.drive_gain.value(),
            drive_tone: self.drive_tone.value(),
            drive_level: self.drive_level.value(),
            amp: self.amp.value(),
            gain: self.gain.value(),
            bass: self.bass.value(),
            mid: self.mid.value(),
            treble: self.treble.value(),
            presence: self.presence.value(),
            master: self.master.value(),
            second: SecondSettings {
                on: self.amp2_on.value(),
                amp: self.amp2.value(),
                gain: self.amp2_gain.value(),
                bass: self.amp2_bass.value(),
                mid: self.amp2_mid.value(),
                treble: self.amp2_treble.value(),
                presence: self.amp2_presence.value(),
                master: self.amp2_master.value(),
            },
            blend: self.amp_blend.value(),
            chorus: ChorusSettings {
                on: self.chorus_on.value(),
                rate_hz: self.chorus_rate.value(),
                depth: self.chorus_depth.value(),
            },
            phaser: PhaserSettings {
                on: self.phaser_on.value(),
                rate_hz: self.phaser_rate.value(),
                depth: self.phaser_depth.value(),
            },
            flanger: FlangerSettings {
                on: self.flanger_on.value(),
                rate_hz: self.flanger_rate.value(),
                depth: self.flanger_depth.value(),
            },
            tremolo: TremoloSettings {
                on: self.trem_on.value(),
                rate_hz: self.trem_rate.value(),
                depth: self.trem_depth.value(),
            },
            cab_on: self.cab_on.value(),
            cab_mic: self.cab_mic.value(),
            cab_res: self.cab_res.value(),
            mic_type: self.cab_mic_type.value(),
            delay: DelaySettings {
                on: self.delay_on.value(),
                time_ms: delay_time_ms(
                    self.delay_time.value(),
                    self.delay_sync.value(),
                    self.delay_note.value(),
                    tempo_bpm,
                ),
                feedback: self.delay_feedback.value(),
                mix: self.delay_mix.value(),
            },
            reverb: ReverbSettings {
                on: self.reverb_on.value(),
                decay_s: self.reverb_decay.value(),
                mix: self.reverb_mix.value(),
            },
            out_level: self.out_level.value(),
            tuner_on: self.tuner_on.value(),
            order: self.order.get(),
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
        TIME_MIN_MS,
        TIME_MAX_MS,
    )
}

/// 0.0-1.0 parameter shown as an amp dial, 0.0 to 10.0
fn dial_param(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_value_to_string(Arc::new(|value| format!("{:.1}", value * 10.0)))
        .with_string_to_value(Arc::new(|text| {
            text.trim().parse::<f32>().ok().map(|dial| dial / 10.0)
        }))
}

/// The rate of a modulation pedal, in Hz
fn rate_param(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min,
            max,
            factor: FloatRange::skew_factor(-1.0), // More resolution at low end
        },
    )
    .with_unit(" Hz")
    .with_value_to_string(formatters::v2s_f32_rounded(2))
}

/// 0.0 to `max` parameter shown as a percentage
fn percent_param(name: &str, default: f32, max: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max })
        .with_unit(" %")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

impl Plugin for GuitarAmpPlugin {
    const NAME: &'static str = "Hojt Guitar Amp";
    const VENDOR: &'static str = suite_common::VENDOR;
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    // The chain smooths its settings itself, once per chunk
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = Task;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        let params = self.params.clone();
        let cab_loader = self.cab_loader.clone();
        Box::new(move |task| match task {
            Task::LoadCabinet => cab_loader.load(&params.cabinet),
        })
    }

    fn editor(&mut self, async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
            self.chain.tuner_reading(),
            self.chain.lamp_reading(),
            self.metronome.reading(),
            self.cab_loader.clone(),
            async_executor,
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.chain.set_sample_rate(buffer_config.sample_rate);
        self.metronome.set_sample_rate(buffer_config.sample_rate);
        // The player's own cabinet is made for one sample rate, and a project that was
        // just loaded may name another file: read it again. The read is not waited for:
        // the first block asks the host to do it in the background, and the amp plays
        // through its own cabinet until the file has been read. With no file chosen no
        // disk is read
        self.cab_loader.set_sample_rate(buffer_config.sample_rate);
        self.cab_load_due = true;
        true
    }

    fn reset(&mut self) {
        self.chain.reset();
        self.metronome.reset();
    }

    /// The tuner is a parameter, so hosts store it, but a project does not open muted
    /// because it was saved while tuning: the switch stays as it is when a state is loaded,
    /// which is off in a plugin that was just created
    fn filter_state(state: &mut PluginState) {
        state.params.remove(TUNER_ID);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if std::mem::take(&mut self.cab_load_due) {
            context.execute_background(Task::LoadCabinet);
        }
        // The song's tempo is only asked for while the delay follows it
        let tempo_bpm =
            if self.params.delay_sync.value() { context.transport().tempo } else { None };
        let settings = self.params.settings(tempo_bpm);

        match buffer.as_slice() {
            [left, right] => self.chain.process(&settings, left, Some(&mut **right)),
            [mono] => self.chain.process(&settings, mono, None),
            _ => {}
        }

        // The click goes on top of the finished sound: the amp never distorts it, and
        // neither the Output knob nor Bypass changes it. The tuner silences it
        // The host's tempo and place are only asked for while the click follows them
        let following = self.params.metronome_on.value() && self.params.metronome_sync.value();
        let (tempo, playing, beats) = if following {
            let transport = context.transport();
            (transport.tempo, transport.playing, transport.pos_beats())
        } else {
            (None, false, None)
        };
        let metronome = self.params.metronome_settings(tempo, playing, beats);
        match buffer.as_slice() {
            [left, right] => self.metronome.process(&metronome, left, Some(&mut **right)),
            [mono] => self.metronome.process(&metronome, mono, None),
            _ => {}
        }

        // The repeats and the reverb tail go on after the input has stopped. The chain
        // knows when the last of them has died away, to the sample; the host may stop
        // calling from there on. `Tail` would need their length ahead of time, which with
        // feedback is a guess, and gains nothing: CLAP hosts are told to go on either way
        if self.chain.is_idle() && self.metronome.is_idle() {
            ProcessStatus::Normal
        } else {
            ProcessStatus::KeepAlive
        }
    }
}

impl ClapPlugin for GuitarAmpPlugin {
    // Frozen: DAW projects find the plugin by this ID. Never change it
    const CLAP_ID: &'static str = "com.guitarpitchshifter.guitar-amp";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("A low-latency guitar amp with pedals, cabinet, delay and reverb");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Distortion,
        ClapFeature::Mono,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for GuitarAmpPlugin {
    // Frozen: DAW projects find the plugin by this ID. Never change it
    const VST3_CLASS_ID: [u8; 16] = *b"HojtGuitarAmp003";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
    // The delay and reverb ring on after the input stops, for as long as their settings say. A host that asks only once, before the first block, would hear 0
    const VST3_INFINITE_TAIL: bool = true;
}

nih_export_clap!(GuitarAmpPlugin);
// vst3-sys's `vtable` macro, which `nih_export_vst3!` expands, ends an expression with a
// semicolon, which Rust is phasing out (rust-lang/rust#79813). It is in vst3-sys, pinned by
// nih-plug, not in our code: a fixed vst3-sys, pinned through our nih-plug fork, removes this.
// An `allow` on the macro call itself is ignored, so it is on a module around it; the
// factory's `#[no_mangle]` functions are exported from inside a module all the same
#[allow(semicolon_in_expressions_from_non_local_macros)]
mod vst3_export {
    use super::*;
    nih_export_vst3!(GuitarAmpPlugin);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An init context that counts the tasks run on the calling thread
    struct CountingContext(std::cell::Cell<u32>);

    impl InitContext<GuitarAmpPlugin> for CountingContext {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Standalone
        }
        fn execute(&self, _task: Task) {
            self.0.set(self.0.get() + 1);
        }
        fn set_latency_samples(&self, _samples: u32) {}
        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }

    #[test]
    fn test_activation_does_not_read_the_cabinet_file() {
        let mut amp = GuitarAmpPlugin::default();
        let mut context = CountingContext(std::cell::Cell::new(0));
        let config = BufferConfig {
            sample_rate: 44_100.0,
            min_buffer_size: None,
            max_buffer_size: 512,
            process_mode: ProcessMode::Realtime,
        };
        assert!(amp.initialize(&GuitarAmpPlugin::AUDIO_IO_LAYOUTS[0], &config, &mut context));
        // Nothing was read before `initialize` returned: the first block asks for it
        assert_eq!(context.0.get(), 0);
        assert!(amp.cab_load_due);
    }

    #[test]
    fn test_parameters_start_at_the_settings_the_chain_starts_with() {
        let params = GuitarAmpParams::default();
        assert_eq!(params.settings(None), AmpSettings::default());

        // The values DAW projects were saved against. Changing a default changes how a
        // project sounds that never touched that knob
        let settings = params.settings(None);
        assert!(!settings.bypass && settings.gate_on && !settings.drive_on && settings.cab_on);
        assert_eq!(
            (settings.comp.on, settings.comp.sustain, settings.comp.level),
            (false, 0.5, 0.5)
        );
        assert_eq!((settings.fuzz.on, settings.fuzz.fuzz, settings.fuzz.tone), (false, 0.5, 0.5));
        assert_eq!(
            (
                settings.pitch.on,
                settings.pitch.semitones,
                settings.pitch.blend,
                settings.pitch.latency_ms
            ),
            (false, 0, 1.0, 15.0)
        );
        assert_eq!(
            (settings.chorus.on, settings.chorus.rate_hz, settings.chorus.depth),
            (false, 0.8, 0.5)
        );
        assert_eq!(
            (settings.phaser.on, settings.phaser.rate_hz, settings.phaser.depth),
            (false, 0.5, 0.5)
        );
        assert_eq!(
            (settings.flanger.on, settings.flanger.rate_hz, settings.flanger.depth),
            (false, 0.3, 0.5)
        );
        assert_eq!(
            (settings.tremolo.on, settings.tremolo.rate_hz, settings.tremolo.depth),
            (false, 5.0, 0.5)
        );
        assert!(!settings.delay.on && !settings.reverb.on && !settings.tuner_on);
        assert_eq!(settings.amp, Amp::Brol);
        assert_eq!(
            (settings.in_gain_db, settings.gate_thresh_db, settings.gate_release_ms),
            (0.0, -60.0, 100.0)
        );
        assert_eq!(
            (settings.drive_gain, settings.drive_tone, settings.drive_level),
            (0.3, 0.5, 0.5)
        );
        assert_eq!(
            [
                settings.gain,
                settings.bass,
                settings.mid,
                settings.treble,
                settings.presence,
                settings.master
            ],
            [0.5; 6]
        );
        assert_eq!((settings.cab_mic, settings.cab_res), (0.5, 0.5));
        // The microphone that was the only one before there were three
        assert_eq!(settings.mic_type, MicType::Dynamic);
        assert_eq!(
            (settings.delay.time_ms, settings.delay.feedback, settings.delay.mix),
            (350.0, 0.35, 0.25)
        );
        assert_eq!((settings.reverb.decay_s, settings.reverb.mix), (1.5, 0.2));
        assert_eq!(settings.out_level, 1.0);
    }

    #[test]
    fn test_the_metronome_starts_off_at_120_in_four_four_and_is_not_a_sound() {
        let params = GuitarAmpParams::default();
        let settings = params.metronome_settings(Some(90.0), false, Some(4.0));
        assert_eq!(settings, MetronomeSettings::default());
        assert!(!settings.on && !settings.sync);
        assert_eq!(
            (settings.bpm, settings.time_signature, settings.sound),
            (120.0, TimeSignature::FourFour, Sound::Woodblock)
        );
        assert!((settings.gain - 0.2512).abs() < 1e-3, "-12 dB");
        assert_eq!(
            (
                metronome::BPM_MIN,
                metronome::BPM_MAX,
                metronome::VOLUME_MIN_DB,
                metronome::VOLUME_MAX_DB
            ),
            (40.0, 300.0, -60.0, 0.0)
        );
        // The host's tempo and place only matter while Sync is on
        assert_eq!((settings.host_tempo, settings.host_beats), (None, None));
        assert!(!params.metronome_sync.default_plain_value());
    }

    #[test]
    fn test_the_metronome_follows_the_knobs_and_the_tuner_silences_it() {
        let params = GuitarAmpParams {
            metronome_on: BoolParam::new("Metronome", true),
            metronome_sync: BoolParam::new("Metronome Sync", true),
            ..GuitarAmpParams::default()
        };
        let settings = params.metronome_settings(Some(90.0), false, Some(4.0));
        assert!(settings.on && settings.sync && !settings.host_playing);
        assert_eq!((settings.host_tempo, settings.host_beats), (Some(90.0), Some(4.0)));
        let tuning = GuitarAmpParams {
            metronome_on: BoolParam::new("Metronome", true),
            tuner_on: BoolParam::new("Tuner", true),
            ..GuitarAmpParams::default()
        };
        assert!(!tuning.metronome_settings(None, true, None).on);
    }

    #[test]
    fn test_the_metronome_is_not_part_of_a_projects_tuner_filter_and_has_its_ids() {
        let params = GuitarAmpParams::default();
        let ids: Vec<String> = params.param_map().into_iter().map(|(id, _, _)| id).collect();
        for id in [
            "metronome_on",
            "metronome_bpm",
            "metronome_sync",
            "metronome_time_sig",
            "metronome_volume",
            "metronome_sound",
        ] {
            assert!(ids.iter().any(|each| each == id), "{id}");
        }
    }

    /// Where the first repeat of an impulse lands, in ms, through the delay at `time_ms`:
    /// half way between its left side and its right one, which the spread puts 1.2 % early
    /// and late
    fn first_repeat_ms(time_ms: f32) -> f32 {
        const RATE: f32 = 48_000.0;
        let mut impulse = vec![0.0; (1.5 * RATE) as usize];
        impulse[0] = 1.0;
        let (left, right) =
            crate::delay::tests::run(&DelaySettings { time_ms, ..settings(0.0) }, &impulse, RATE);
        let peak = |channel: &[f32]| {
            (1..channel.len()).max_by(|&a, &b| channel[a].abs().total_cmp(&channel[b].abs()))
        };
        let (left, right) = (peak(&left).unwrap(), peak(&right).unwrap());
        (left + right) as f32 * 0.5 / RATE * 1000.0
    }

    /// The delay on, one repeat, all wet
    fn settings(feedback: f32) -> DelaySettings {
        DelaySettings { on: true, time_ms: 350.0, feedback, mix: 1.0 }
    }

    #[test]
    fn test_a_synced_delay_repeats_on_the_note_at_the_songs_tempo() {
        let synced = |bpm| delay_time_ms(350.0, true, NoteValue::DottedEighth, Some(bpm));
        let at_120 = first_repeat_ms(synced(120.0));
        assert!((at_120 - 375.0).abs() <= 1.0, "a dotted 1/8 at 120 bpm repeats at {at_120} ms");
        let at_90 = first_repeat_ms(synced(90.0));
        assert!((at_90 - 500.0).abs() <= 1.0, "a dotted 1/8 at 90 bpm repeats at {at_90} ms");
        // Slower than the delay reaches: its longest time. With no tempo, the knob
        assert_eq!(delay_time_ms(350.0, true, NoteValue::Quarter, Some(50.0)), TIME_MAX_MS);
        assert_eq!(delay_time_ms(350.0, true, NoteValue::Quarter, None), 350.0);
    }

    #[test]
    fn test_without_sync_the_delay_is_as_it_was_at_any_tempo() {
        const RATE: f32 = 48_000.0;
        let mut noise = suite_common::dsp::Noise::new(7);
        let input: Vec<f32> = (0..(RATE as usize)).map(|_| 0.3 * noise.next()).collect();
        for knob_ms in [20.0, 350.0, 1000.0] {
            let knob = DelaySettings { time_ms: knob_ms, ..settings(0.6) };
            let before = crate::delay::tests::run(&knob, &input, RATE);
            for tempo in [None, Some(60.0), Some(120.0), Some(187.5)] {
                for note in [NoteValue::Quarter, NoteValue::Sixteenth] {
                    let time_ms = delay_time_ms(knob_ms, false, note, tempo);
                    let now =
                        crate::delay::tests::run(&DelaySettings { time_ms, ..knob }, &input, RATE);
                    assert!(now == before, "{knob_ms} ms at {tempo:?} bpm");
                }
            }
        }
        // The parameters as they start, and as a project from before has them: Sync off
        let params = GuitarAmpParams::default();
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
        GuitarAmpPlugin::filter_state(&mut state);
        assert!(!state.params.contains_key("delay_sync"));
        assert!(!state.params.contains_key("delay_note"));
        assert!(!GuitarAmpParams::default().delay_sync.default_plain_value());
    }

    #[test]
    fn test_parameter_ranges_are_the_limits_of_the_chain() {
        let params = GuitarAmpParams::default();
        let range = |param: &FloatParam| (param.preview_plain(0.0), param.preview_plain(1.0));
        assert_eq!(range(&params.in_gain), (-24.0, 24.0));
        assert_eq!(range(&params.gate_thresh), (-80.0, -20.0));
        assert_eq!(range(&params.gate_release), (20.0, 500.0));
        assert_eq!(range(&params.delay_time), (20.0, 1000.0));
        assert_eq!(range(&params.delay_feedback), (0.0, 0.9));
        assert_eq!(range(&params.delay_mix), (0.0, 1.0));
        assert_eq!(range(&params.reverb_decay), (0.3, 6.0));
        assert_eq!(range(&params.reverb_mix), (0.0, 1.0));
        assert_eq!(range(&params.chorus_rate), (0.1, 6.0));
        assert_eq!(range(&params.phaser_rate), (0.1, 8.0));
        assert_eq!(range(&params.flanger_rate), (0.05, 5.0));
        assert_eq!(range(&params.trem_rate), (0.5, 12.0));
        assert_eq!(range(&params.pitch_latency), (2.0, 50.0));
        assert_eq!(range(&params.pitch_blend), (0.0, 1.0));
        assert_eq!(
            (params.pitch_semitones.preview_plain(0.0), params.pitch_semitones.preview_plain(1.0)),
            (-12, 12)
        );
        for depth in
            [&params.chorus_depth, &params.phaser_depth, &params.flanger_depth, &params.trem_depth]
        {
            assert_eq!(range(depth), (0.0, 1.0));
        }
        for dial in [
            &params.gain,
            &params.drive_gain,
            &params.cab_mic,
            &params.comp_sustain,
            &params.comp_level,
            &params.fuzz_fuzz,
            &params.fuzz_tone,
        ] {
            assert_eq!(range(dial), (0.0, 1.0));
        }
    }
}

#[cfg(test)]
mod ids {
    /// Bundle name, plugin ids and every parameter id stay as saved projects know them
    #[test]
    fn ids() {
        suite_common::ids::check::<super::GuitarAmpPlugin>(
            env!("CARGO_MANIFEST_DIR"),
            include_str!("../../../bundler.toml"),
            "amp",
        );
    }

    #[test]
    fn vst3_tail_is_infinite() {
        use nih_plug::prelude::Vst3Plugin;
        let infinite = super::GuitarAmpPlugin::VST3_INFINITE_TAIL;
        assert!(infinite);
    }
}
