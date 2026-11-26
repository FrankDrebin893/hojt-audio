//! `AmpSettings`: what the plugin tells the chain, read once per block. The parameters in
//! `lib.rs` are read into it, and the presets and the tests make it by hand.

use crate::amp::model::Amp;
use crate::cab::MicType;
use crate::comp::CompSettings;
use crate::delay::DelaySettings;
use crate::effects::BlockSettings;
use crate::eq::EqSettings;
use crate::fuzz::FuzzSettings;
use crate::layout::Layout;
use crate::mods::chorus::ChorusSettings;
use crate::mods::flanger::FlangerSettings;
use crate::mods::phaser::PhaserSettings;
use crate::mods::tremolo::TremoloSettings;
use crate::pitch::PitchSettings;
use crate::reverb::ReverbSettings;
use crate::second::SecondSettings;
use crate::slots::MAX_SLOTS;
use crate::wah::WahSettings;

/// What the knobs say, read once per block. The chain smooths the values itself
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmpSettings {
    pub bypass: bool,
    /// Gain in dB in front of everything but the gate's detector
    pub in_gain_db: f32,

    pub gate_on: bool,
    /// Level in dBFS at the plugin's input that opens the gate
    pub gate_thresh_db: f32,
    /// Time the gate takes to close, in milliseconds
    pub gate_release_ms: f32,

    /// The fuzz and the compressor pedals. They smooth their own settings
    pub fuzz: FuzzSettings,
    pub comp: CompSettings,
    /// The pitch shifter pedal. It smooths its own settings
    pub pitch: PitchSettings,
    /// The wah pedal. It smooths its own settings
    pub wah: WahSettings,
    /// The EQ pedal, on either side of the amp. It smooths its own settings
    pub eq: EqSettings,

    pub drive_on: bool,
    /// The drive pedal's dials, 0.0 to 1.0
    pub drive_gain: f32,
    pub drive_tone: f32,
    pub drive_level: f32,

    pub amp: Amp,
    /// The amp dials, 0.0 to 1.0
    pub gain: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub presence: f32,
    pub master: f32,

    /// The second amp, which plays beside the first one behind the same front pedals and
    /// through its own cabinet, and the share of it in the mix: 0.0 is the first amp alone,
    /// 1.0 the second alone. With the second amp off the blend is not heard
    pub second: SecondSettings,
    pub blend: f32,

    /// The modulation pedals. They smooth their own settings
    pub chorus: ChorusSettings,
    pub phaser: PhaserSettings,
    pub flanger: FlangerSettings,
    pub tremolo: TremoloSettings,

    /// Off leaves the amp's signal as it is, for a cabinet somewhere else
    pub cab_on: bool,
    /// The cabinet's dials, 0.0 to 1.0. Both leave the cabinet as designed at 0.5
    pub cab_mic: f32,
    pub cab_res: f32,
    /// Which microphone hears the cabinet
    pub mic_type: MicType,

    /// The two effects behind the cabinet. They smooth their own settings
    pub delay: DelaySettings,
    pub reverb: ReverbSettings,

    /// Linear gain
    pub out_level: f32,

    /// The tuner listens to the input and the amp is silent meanwhile. Bypass still passes
    /// the input on
    pub tuner_on: bool,

    /// What stands in front of the amp and what behind the cabinet, and in what order. A
    /// pedal that is not in it is not played
    pub layout: Layout,
    /// What the effects that play from a slot's parameters are told, by slot (counted from 0);
    /// `None` for a slot no such effect stands on
    pub slots: [BlockSettings; MAX_SLOTS],
}

/// What the plugin starts with. The parameters in `lib.rs` take their defaults from here
impl Default for AmpSettings {
    fn default() -> Self {
        Self {
            bypass: false,
            in_gain_db: 0.0,
            gate_on: true,
            gate_thresh_db: -60.0,
            gate_release_ms: 100.0,
            fuzz: FuzzSettings::default(),
            comp: CompSettings::default(),
            pitch: PitchSettings::default(),
            wah: WahSettings::default(),
            eq: EqSettings::default(),
            drive_on: false,
            drive_gain: 0.3,
            drive_tone: 0.5,
            drive_level: 0.5,
            amp: Amp::Roar,
            gain: 0.5,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
            presence: 0.5,
            master: 0.5,
            second: SecondSettings::default(),
            blend: 0.5,
            chorus: ChorusSettings::default(),
            phaser: PhaserSettings::default(),
            flanger: FlangerSettings::default(),
            tremolo: TremoloSettings::default(),
            cab_on: true,
            cab_mic: 0.5,
            cab_res: 0.5,
            mic_type: MicType::Dynamic,
            delay: DelaySettings::default(),
            reverb: ReverbSettings::default(),
            out_level: 1.0,
            tuner_on: false,
            layout: Layout::default(),
            slots: [BlockSettings::None; MAX_SLOTS],
        }
    }
}
