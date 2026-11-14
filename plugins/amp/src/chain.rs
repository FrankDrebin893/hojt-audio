use crate::amp::model::{Amp, CabModel, NEUTRAL_CAB, SPEAKERS};
use crate::amp::poweramp::PowerAmp;
use crate::amp::preamp::Preamp;
use crate::amp::tonestack::{ToneCurve, ToneStack};
use crate::cab::{design_ir, CabIr, CabVoicing, Cabinet, MicType};
use crate::cab_stage::CabStage;
use crate::comp::{Comp, CompSettings};
use crate::delay::{Delay, DelaySettings};
use crate::drive::{BackDrive, Drive};
use crate::dsp::filters::DcBlocker;
use crate::dsp::oversample::{factor_for, Oversampler, MAX_FACTOR};
use crate::dsp::shaper::{output_clip, OUTPUT_CLIP_KNEE};
use crate::dsp::{curve, db_to_gain, smoothing_coeff, Ramp};
use crate::eq::{Eq, EqSettings};
use crate::extras::{BlockSettings, Pool};
use crate::fuzz::{Fuzz, FuzzSettings};
use crate::gate::Gate;
use crate::lamps::LampReading;
use crate::layout::{Layout, Voice};
use crate::mods::chorus::{Chorus, ChorusSettings};
use crate::mods::flanger::{Flanger, FlangerSettings};
use crate::mods::phaser::{Phaser, PhaserSettings};
use crate::mods::tremolo::{Tremolo, TremoloSettings};
use crate::order::{Pedal, Zone};
use crate::pitch::{Pitch, PitchSettings};
use crate::reverb::{Reverb, ReverbSettings};
#[cfg(test)]
use crate::second::BLEND_LAW;
use crate::second::{Second, SecondSettings, Timing};
use crate::slots::{EffectKind, MAX_SLOTS};
use crate::tuner::{Tuner, TunerReading};
use crate::wah::{Wah, WahSettings};
use std::sync::Arc;

// The chain works in pieces of at most this many samples, and reads the dials once per piece
const CHUNK: usize = 32;
// Room for a piece at the highest oversampled rate
pub(crate) const OVERSAMPLED_CHUNK: usize = CHUNK * MAX_FACTOR;

// How fast the chain follows a dial
const DIAL_SMOOTH_MS: f32 = 20.0;

// A dial this close to where it is going is taken as there, so the filters stop being redesigned
const DIAL_SETTLED: f32 = 1e-5;

// Crossfade between the amp and the untouched input when Bypass is switched. The repeats and
// the reverb tail fade out with the amp, and are dropped once nothing is heard of them: what
// comes back is the amp from rest, not what was left in the effects when it was switched off
const BYPASS_FADE_MS: f32 = 10.0;

// When another amp is picked, the amp's output fades to silence in this time, the stages are
// set up as the new amp, and it fades back in as fast
const AMP_FADE_MS: f32 = 4.5;

// When the pedals are put in another order, the output fades to silence in this time, the
// pedals change places there, and it fades back in as fast
const ORDER_FADE_MS: f32 = 5.0;

// Crossfade between the cabinet and the amp's own signal when the cabinet is switched
const CAB_FADE_MS: f32 = 20.0;

// While the tuner is on the amp is given silence and its output is turned down, both in this
// time. The same when it is switched off again
const TUNER_FADE_MS: f32 = 10.0;

// Level of the amp's signal with the cabinet off, in dB: about as loud as with it on
const CAB_OFF_DB: f32 = -1.4;

// The limits of the settings that are not dials. The parameters in `lib.rs` take their
// ranges from these, and their defaults from `AmpSettings::default`
pub const IN_GAIN_DB: [f32; 2] = [-24.0, 24.0];
pub const GATE_THRESHOLD_DB: [f32; 2] = [-80.0, -20.0];
pub const GATE_RELEASE_MS: [f32; 2] = [20.0, 500.0];

/// Which cabinet plays
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Playing {
    /// The amp's own: it changes with the amp
    Own,
    /// One of the built-in cabinets, by its place in `SPEAKERS`, whichever amp plays
    Speaker(usize),
    /// A file of the player's own
    File,
}

/// Where the cabinet that plays is in the chain's designed cabinets: the amp's own, then
/// the built-in ones. None for a file
fn cab_ir_index(playing: Playing, amp: Amp) -> Option<usize> {
    match playing {
        Playing::Own => Some(amp.index()),
        Playing::Speaker(speaker) => Some(Amp::ALL.len() + speaker),
        Playing::File => None,
    }
}

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
            amp: Amp::Brol,
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

/// The dial values the chain is at, on their way to the settings
#[derive(Clone, Copy, PartialEq)]
struct Dials {
    in_gain_db: f32,
    drive_gain: f32,
    drive_tone: f32,
    drive_level: f32,
    gain: f32,
    bass: f32,
    mid: f32,
    treble: f32,
    presence: f32,
    master: f32,
    cab_mic: f32,
    cab_res: f32,
    out_level: f32,
}

impl Dials {
    fn from_settings(settings: &AmpSettings) -> Self {
        Self {
            in_gain_db: settings.in_gain_db.clamp(IN_GAIN_DB[0], IN_GAIN_DB[1]),
            drive_gain: settings.drive_gain.clamp(0.0, 1.0),
            drive_tone: settings.drive_tone.clamp(0.0, 1.0),
            drive_level: settings.drive_level.clamp(0.0, 1.0),
            gain: settings.gain.clamp(0.0, 1.0),
            bass: settings.bass.clamp(0.0, 1.0),
            mid: settings.mid.clamp(0.0, 1.0),
            treble: settings.treble.clamp(0.0, 1.0),
            presence: settings.presence.clamp(0.0, 1.0),
            master: settings.master.clamp(0.0, 1.0),
            cab_mic: settings.cab_mic.clamp(0.0, 1.0),
            cab_res: settings.cab_res.clamp(0.0, 1.0),
            out_level: settings.out_level.max(0.0),
        }
    }

    /// The six dials of the amp itself jump to where they are going. For the sample an amp
    /// starts from rest at
    fn arrive_at_amp_dials(&mut self, target: &Dials) {
        self.gain = target.gain;
        self.bass = target.bass;
        self.mid = target.mid;
        self.treble = target.treble;
        self.presence = target.presence;
        self.master = target.master;
    }

    fn approach(&mut self, target: &Dials, coeff: f32) {
        let step = |value: &mut f32, target: f32| {
            *value += coeff * (target - *value);
            if (target - *value).abs() < DIAL_SETTLED {
                *value = target;
            }
        };
        step(&mut self.in_gain_db, target.in_gain_db);
        step(&mut self.drive_gain, target.drive_gain);
        step(&mut self.drive_tone, target.drive_tone);
        step(&mut self.drive_level, target.drive_level);
        step(&mut self.gain, target.gain);
        step(&mut self.bass, target.bass);
        step(&mut self.mid, target.mid);
        step(&mut self.treble, target.treble);
        step(&mut self.presence, target.presence);
        step(&mut self.master, target.master);
        step(&mut self.cab_mic, target.cab_mic);
        step(&mut self.cab_res, target.cab_res);
        step(&mut self.out_level, target.out_level);
    }
}

/// The whole signal chain. Mono through the amp and cabinet, stereo from the delay on
pub struct AmpChain {
    sample_rate: f32,
    // How many times the drive and the amp run faster than the host: 4 at the usual sample
    // rates, 2 from 88.2 kHz on
    factor: usize,
    // The amp the stages are set up as, and the one the settings ask for
    amp: Amp,
    wanted_amp: Amp,

    in_gain: Ramp,
    gate: Gate,
    oversampler: Oversampler,
    // The effects stand in this order, and are set up for the side of the amp they are on.
    // And the layout the settings ask for
    layout: Layout,
    wanted_layout: Layout,
    // The effects of the slots that are not a pedal's own, and what they are told
    pool: Pool,
    slot_settings: [BlockSettings; MAX_SLOTS],
    // Level of the output in steps: `order_fade_len` playing, down to 0 on the way to
    // another order and back up after
    order_fade: u32,
    order_fade_len: u32,
    fuzz: Fuzz,
    fuzz_settings: FuzzSettings,
    comp: Comp,
    comp_settings: CompSettings,
    pitch: Pitch,
    pitch_settings: PitchSettings,
    wah: Wah,
    wah_settings: WahSettings,
    eq: Eq,
    eq_settings: EqSettings,
    chorus: Chorus,
    phaser: Phaser,
    flanger: Flanger,
    tremolo: Tremolo,
    // What the modulation pedals are told, read with the dials like the effects' settings
    chorus_settings: ChorusSettings,
    phaser_settings: PhaserSettings,
    flanger_settings: FlangerSettings,
    tremolo_settings: TremoloSettings,
    drive: Drive,
    preamp: Preamp,
    tone: ToneStack,
    power: PowerAmp,
    cabinet: Cabinet,
    // The second amp with its own cabinet, run only while it is switched on
    second: Second,
    // Every cabinet designed here, when the sample rate is set: the amps' own in the order
    // of `Amp::ALL`, then the built-in ones of `SPEAKERS` (`cab_ir_index`)
    cab_irs: Vec<CabIr>,
    // Where a cabinet the player chose is left for the chain to take, and which cabinet
    // plays: one the player chose stays whatever amp is picked
    cab_stage: Arc<CabStage>,
    playing: Playing,
    cab_voicing: CabVoicing,
    // Share of the cabinet in what follows it: 1.0 on, 0.0 the amp's own signal
    cab_mix: Ramp,
    cab_fade_steps: u32,
    dc: DcBlocker,
    delay: Delay,
    reverb: Reverb,
    // The same two effects for when they stand in front of the amp (#268): one of each, made
    // for the oversampled rate, so a pedal that changes sides allocates nothing. Only the
    // one of the side the pedal stands on is run; the other is left at rest
    front_delay: Delay,
    front_reverb: Reverb,
    // And the three that were only ever in front of the amp, for when they stand behind the
    // cabinet (#268): the wah and the pitch shifter one a channel at the host's rate, the drive
    // raising the rate itself as the fuzz does there
    back_drive: BackDrive,
    back_wah: [Wah; 2],
    back_pitch: [Pitch; 2],
    // Whether the drive pedal's switch is on, as of the last read of the dials
    drive_on: bool,
    // What the effects are told. Read with the dials, so a switch takes effect on the same
    // sample whatever the block size
    delay_settings: DelaySettings,
    reverb_settings: ReverbSettings,
    out_level: Ramp,

    dials: Dials,
    // Where the dials are going: the settings as they were read last
    dial_target: Dials,
    dial_coeff: f32,
    // The dial positions the tone filters were last designed for
    tone_applied: [f32; 3],
    presence_applied: f32,
    // Mic and Resonance, likewise
    cab_applied: [f32; 2],
    // The microphone, which is not a dial: it is taken as it is set
    mic_type: MicType,
    mic_type_applied: Option<MicType>,
    // False until the first block after a reset: the dials then start at the settings
    primed: bool,
    // Samples left until the dials are read again. Counted across blocks, so the output
    // does not depend on how the host cuts the stream into blocks
    until_tick: usize,

    // Share of the amp in the output: 1.0 playing, 0.0 bypassed
    wet: f32,
    wet_step: f32,

    // Level of the amp's output in front of the cabinet, in steps: `amp_fade_len` playing,
    // down to 0 on the way to another amp and back up after
    amp_fade: u32,
    amp_fade_len: u32,
    // Samples until the cabinet has finished its crossfade and can start another
    cabinet_busy: usize,

    // Hears the input as it arrives, while `tuning`
    tuner: Tuner,
    tuning: bool,
    // Share of the input the amp is given and of its output that is heard: 1.0 playing,
    // 0.0 tuning
    tuner_mute: Ramp,
    tuner_fade_steps: u32,

    // What the editor's lamps show: left there at the end of every block
    lamps: Arc<LampReading>,
    // Whether a sample of this block was over the knee of the safety clip
    clipped: bool,
}

impl AmpChain {
    pub fn new() -> Self {
        let mut chain = Self {
            sample_rate: 44100.0,
            factor: MAX_FACTOR,
            amp: Amp::Brol,
            wanted_amp: Amp::Brol,
            in_gain: Ramp::new(1.0),
            gate: Gate::new(),
            oversampler: Oversampler::new(),
            layout: Layout::default(),
            wanted_layout: Layout::default(),
            pool: Pool::new(),
            slot_settings: [BlockSettings::None; MAX_SLOTS],
            order_fade: 1,
            order_fade_len: 1,
            fuzz: Fuzz::new(),
            fuzz_settings: FuzzSettings::default(),
            comp: Comp::new(),
            comp_settings: CompSettings::default(),
            pitch: Pitch::new(),
            pitch_settings: PitchSettings::default(),
            wah: Wah::new(),
            wah_settings: WahSettings::default(),
            eq: Eq::new(),
            eq_settings: EqSettings::default(),
            chorus: Chorus::new(),
            phaser: Phaser::new(),
            flanger: Flanger::new(),
            tremolo: Tremolo::new(),
            chorus_settings: ChorusSettings::default(),
            phaser_settings: PhaserSettings::default(),
            flanger_settings: FlangerSettings::default(),
            tremolo_settings: TremoloSettings::default(),
            drive: Drive::new(),
            preamp: Preamp::new(),
            tone: ToneStack::new(),
            power: PowerAmp::new(),
            cabinet: Cabinet::new(),
            second: Second::new(),
            cab_irs: Vec::new(),
            cab_stage: Arc::new(CabStage::new()),
            playing: Playing::Own,
            cab_voicing: CabVoicing::new(),
            cab_mix: Ramp::new(1.0),
            cab_fade_steps: 1,
            dc: DcBlocker::new(),
            delay: Delay::new(),
            reverb: Reverb::new(),
            front_delay: Delay::new(),
            front_reverb: Reverb::new(),
            back_drive: BackDrive::new(),
            back_wah: [Wah::new(), Wah::new()],
            back_pitch: [Pitch::new(), Pitch::new()],
            drive_on: false,
            delay_settings: DelaySettings::default(),
            reverb_settings: ReverbSettings::default(),
            out_level: Ramp::new(1.0),
            dials: Dials::from_settings(&AmpSettings::default()),
            dial_target: Dials::from_settings(&AmpSettings::default()),
            dial_coeff: 1.0,
            tone_applied: [f32::NAN; 3],
            presence_applied: f32::NAN,
            cab_applied: [f32::NAN; 2],
            mic_type: MicType::Dynamic,
            mic_type_applied: None,
            primed: false,
            until_tick: 0,
            wet: 1.0,
            wet_step: 1.0,
            amp_fade: 1,
            amp_fade_len: 1,
            cabinet_busy: 0,
            tuner: Tuner::new(),
            tuning: false,
            tuner_mute: Ramp::new(1.0),
            tuner_fade_steps: 1,
            lamps: Arc::new(LampReading::new()),
            clipped: false,
        };
        chain.set_sample_rate(44100.0);
        chain
    }

    /// Allocates and designs everything that depends on the sample rate. Not for the audio thread
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.dc.set_sample_rate(sample_rate);
        self.dial_coeff = smoothing_coeff(DIAL_SMOOTH_MS, sample_rate / CHUNK as f32);
        self.wet_step = 1.0 / (BYPASS_FADE_MS * 0.001 * sample_rate);
        self.amp_fade_len = ((AMP_FADE_MS * 0.001 * sample_rate).round() as u32).max(1);
        self.cab_fade_steps = ((CAB_FADE_MS * 0.001 * sample_rate).round() as u32).max(1);
        self.order_fade_len = ((ORDER_FADE_MS * 0.001 * sample_rate).round() as u32).max(1);
        self.tuner_fade_steps = ((TUNER_FADE_MS * 0.001 * sample_rate).round() as u32).max(1);
        self.tuner.set_sample_rate(sample_rate);

        self.factor = factor_for(sample_rate);
        self.oversampler.set_factor(self.factor);
        self.gate.set_sample_rate(sample_rate);
        self.drive.configure(self.oversampled_rate());
        self.back_drive.configure(sample_rate);
        // The pedals that stand on either side are made for both: only the one of the side
        // the pedal is on is run, so a pedal that changes sides allocates nothing
        self.pitch.set_sample_rate(self.oversampled_rate());
        self.wah.set_sample_rate(self.oversampled_rate());
        for side in 0..2 {
            self.back_pitch[side].set_sample_rate(sample_rate);
            self.back_wah[side].set_sample_rate(sample_rate);
        }
        for pedal in Pedal::ALL {
            self.set_up_pedal(pedal);
        }
        self.delay.set_sample_rate(sample_rate);
        self.reverb.set_sample_rate(sample_rate);
        let oversampled = self.oversampled_rate();
        self.front_delay.set_sample_rate(oversampled);
        self.front_reverb.set_sample_rate(oversampled);
        self.pool.set_sample_rate(sample_rate, oversampled);
        self.follow_pool(true);

        self.cab_irs = Amp::ALL
            .iter()
            .map(|amp| &amp.model().cab)
            .chain(SPEAKERS.iter().map(|speaker| &speaker.cab))
            .map(|cab| design_ir(cab, sample_rate))
            .collect();
        self.cabinet.set_sample_rate(sample_rate);
        // A cabinet the player chose was made for the rate before: it has to be left on
        // the stage again for this one. Until then the amp plays through its own
        self.cabinet.set_ir(&self.cab_irs[self.amp.index()]);
        self.playing = Playing::Own;
        self.second.set_sample_rate(
            sample_rate,
            self.factor,
            &self.cab_irs[self.second.ir_index()],
        );

        self.configure_amp();
        self.second.configure(self.oversampled_rate());
        self.reset();
    }

    /// The rate the drive and the amp run at
    fn oversampled_rate(&self) -> f32 {
        self.sample_rate * self.factor as f32
    }

    /// The rate a pedal runs at on that side of the amp
    fn rate_in(&self, zone: Zone) -> f32 {
        match zone {
            Zone::Front => self.oversampled_rate(),
            Zone::Back => self.sample_rate,
        }
    }

    /// Sets a pedal that can stand on either side of the amp up for the side it is on, from
    /// rest. Does not allocate
    fn set_up_pedal(&mut self, pedal: Pedal) {
        // A pedal the chain does not hold is set up as if it stood in front of the amp, and
        // put to rest: it is not played, and it starts from rest where it comes in
        let held = self.layout.home_zone(pedal);
        let zone = held.unwrap_or(Zone::Front);
        let rate = self.rate_in(zone);
        match pedal {
            // Behind the cabinet it raises the rate itself for its clipping
            Pedal::Fuzz => self.fuzz.set_sample_rate(rate, zone == Zone::Back),
            Pedal::Comp => self.comp.set_sample_rate(rate),
            // One instance for either side (`back_pitch`, `back_wah`): the one it left is
            // dropped, so what it held is not heard if it comes back
            Pedal::Pitch => match zone {
                Zone::Front => self.back_pitch.iter_mut().for_each(Pitch::reset),
                Zone::Back => self.pitch.reset(),
            },
            Pedal::Wah => match zone {
                Zone::Front => self.back_wah.iter_mut().for_each(Wah::reset),
                Zone::Back => self.wah.reset(),
            },
            Pedal::Eq => self.eq.set_sample_rate(rate),
            Pedal::Chorus => self.chorus.set_sample_rate(rate),
            Pedal::Phaser => self.phaser.set_sample_rate(rate),
            Pedal::Flanger => self.flanger.set_sample_rate(rate),
            Pedal::Tremolo => self.tremolo.set_sample_rate(rate),
            // The pedal's switch follows the side it is on, and the amp is driven only by the
            // one in front of it. From rest, as it crosses
            Pedal::Drive => {
                match zone {
                    Zone::Front => self.back_drive.reset(),
                    Zone::Back => self.drive.reset(),
                }
                self.follow_drive_side(true);
            }
            // One of each for either side (`front_delay`, `front_reverb`): the one it left is
            // dropped, so its tail is not heard if it comes back
            Pedal::Delay => match zone {
                Zone::Front => self.delay.reset(),
                Zone::Back => self.front_delay.reset(),
            },
            Pedal::Reverb => match zone {
                Zone::Front => self.reverb.reset(),
                Zone::Back => self.front_reverb.reset(),
            },
        }
    }

    /// Puts a pedal the chain has taken out to rest, whatever side it was on
    fn put_away(&mut self, pedal: Pedal) {
        match pedal {
            Pedal::Fuzz => self.fuzz.reset(),
            Pedal::Comp => self.comp.reset(),
            Pedal::Pitch => {
                self.pitch.reset();
                self.back_pitch.iter_mut().for_each(Pitch::reset);
            }
            Pedal::Wah => {
                self.wah.reset();
                self.back_wah.iter_mut().for_each(Wah::reset);
            }
            Pedal::Eq => self.eq.reset(),
            Pedal::Chorus => self.chorus.reset(),
            Pedal::Phaser => self.phaser.reset(),
            Pedal::Flanger => self.flanger.reset(),
            Pedal::Tremolo => self.tremolo.reset(),
            Pedal::Drive => {
                self.drive.reset();
                self.back_drive.reset();
            }
            Pedal::Delay => {
                self.delay.reset();
                self.front_delay.reset();
            }
            Pedal::Reverb => {
                self.reverb.reset();
                self.front_reverb.reset();
            }
        }
    }

    /// Whether a drive that plays from a slot is switched on in front of the amp
    fn slot_drive_in_front(&self) -> bool {
        self.layout.front().iter().any(|voice| match *voice {
            Voice::Slot { kind: EffectKind::Drive, slot, .. } => {
                self.slot_settings[slot as usize].is_on()
            }
            _ => false,
        })
    }

    /// Whether the amp is pushed by a drive pedal: one that is on in front of it
    fn driven(&self) -> bool {
        (self.drive_on && self.layout.home_zone(Pedal::Drive) == Some(Zone::Front))
            || self.slot_drive_in_front()
    }

    /// Moves the effects of the pool to where the layout puts them. Does not allocate
    fn follow_pool(&mut self, snap: bool) {
        let layout = self.layout;
        self.pool.follow(|kind, rank| layout.zone_of_slot(kind, rank));
        self.tell_pool(snap);
    }

    /// Tells the effects of the pool what their slots say, over the next chunk
    fn tell_pool(&mut self, snap: bool) {
        let steps = if snap { 0 } else { self.oversampled_chunk() };
        for unit in self.pool.units_mut() {
            if let Some((zone, slot)) = unit.assigned() {
                unit.tell(&self.slot_settings[slot as usize], zone, steps, snap);
            }
        }
    }

    /// Tells the pedal that is on the side the drive stands on, and the amp, about the switch.
    /// With the pedal in front the amp is pushed harder than it ever is without; behind the
    /// cabinet it is not in front of it. `snap` for a pedal that has just changed sides, while
    /// nothing of it is heard
    fn follow_drive_side(&mut self, snap: bool) {
        let zone = self.layout.home_zone(Pedal::Drive);
        let steps = if snap { 0 } else { self.drive.fade_steps() };
        let driven = self.driven();
        self.drive.set_on(self.drive_on && zone == Some(Zone::Front));
        self.back_drive.set_on(self.drive_on && zone == Some(Zone::Back));
        self.preamp.set_driven(driven, steps);
        self.power.set_driven(driven, steps);
        self.second.preamp.set_driven(driven, steps);
        self.second.power.set_driven(driven, steps);
        if snap {
            self.drive.snap();
            self.back_drive.snap();
        }
    }

    /// Moves towards the order the settings ask for. Does not allocate. While the amp is
    /// heard the output first fades to silence; there the pedals change places, and one
    /// that changes sides starts from rest at the rate of its new side. The repeats and the
    /// tail of the effects ring on through it
    fn follow_order(&mut self) {
        if self.wanted_layout == self.layout {
            return;
        }
        let unheard = !self.primed || self.wet == 0.0;
        if !unheard && self.order_fade > 0 {
            return;
        }
        let before = std::mem::replace(&mut self.layout, self.wanted_layout);
        for pedal in Pedal::ALL {
            let now = self.layout.home_zone(pedal);
            if before.home_zone(pedal) != now {
                if now.is_none() {
                    self.put_away(pedal);
                }
                self.set_up_pedal(pedal);
            }
        }
        // The drive's switch follows the side it is on, and so may the amp
        self.follow_drive_side(true);
        self.follow_pool(true);
        if unheard {
            self.order_fade = self.order_fade_len;
        }
    }

    /// Samples at that rate in a piece of `CHUNK`: the time the stages take to follow a dial
    fn oversampled_chunk(&self) -> u32 {
        (CHUNK * self.factor) as u32
    }

    /// Moves towards the amp the settings ask for. Does not allocate. While the amp is heard
    /// its output first fades to silence; there the stages are set up as the new amp and
    /// start from rest, and the cabinet crossfades to the new amp's own, unless the player
    /// chose a cabinet: that one stays
    fn follow_amp(&mut self) {
        if self.wanted_amp == self.amp {
            return;
        }
        // Before the first block and while bypassed nothing is heard and every stage is at rest
        let unheard = !self.primed || self.wet == 0.0;
        let silent = self.amp_fade == 0 && self.cabinet_busy == 0;
        if !unheard && !silent {
            return;
        }

        self.amp = self.wanted_amp;
        let own = self.playing == Playing::Own;
        if unheard {
            if own {
                self.cabinet.set_ir(&self.cab_irs[self.amp.index()]);
            }
            self.amp_fade = self.amp_fade_len;
        } else if own {
            debug_assert!(!self.cabinet.is_swapping());
            self.cabinet.swap_ir(&self.cab_irs[self.amp.index()]);
            self.cabinet_busy = self.cabinet.swap_len();
        }
        self.configure_amp();
        self.reset_amp_stages();
        // The amp's own dials may have been turned along with the selector: every amp
        // remembers its own. The new amp starts where they are going, not on the way there
        // from where the old amp had them. Nothing of the amp is heard at this sample
        self.dials.arrive_at_amp_dials(&self.dial_target);
        self.apply_dials(true);
    }

    /// The second amp's counterpart of `follow_amp`, and puts its stages to rest once it is
    /// switched off and has faded out. Does not allocate. Its cabinet is its own amp's or a
    /// built-in one, never a file
    fn follow_second(&mut self) {
        if !self.second.runs() && !self.second.at_rest {
            self.second.reset_stages();
            self.second.amp_fade = self.amp_fade_len;
            self.second.cabinet_busy = 0;
        }
        if !self.second.switching() {
            return;
        }
        // Nothing of the second amp is heard before the first block, while bypassed and
        // while it is faded out
        let unheard = !self.primed || self.wet == 0.0 || self.second.on_fade == 0;
        let silent = self.second.amp_fade == 0 && self.second.cabinet_busy == 0;
        if !unheard && !silent {
            return;
        }

        self.second.amp = self.second.wanted_amp;
        self.second.speaker = self.second.wanted_speaker;
        let ir = &self.cab_irs[self.second.ir_index()];
        if unheard {
            self.second.cabinet.set_ir(ir);
            self.second.amp_fade = self.amp_fade_len;
        } else {
            debug_assert!(!self.second.cabinet.is_swapping());
            self.second.cabinet.swap_ir(ir);
            self.second.cabinet_busy = self.second.cabinet.swap_len();
        }
        self.second.configure(self.oversampled_rate());
        self.second.reset_amp_stages();
        if self.second.runs() {
            // The new amp starts where the dials are going, not on the way there from where
            // the old amp had them. Nothing of it is heard at this sample
            self.second.dials = self.second.target;
            self.second.at_rest = false;
            self.apply_second_dials(true);
        }
    }

    /// Takes the cabinet that was left on the stage, if there is one and the cabinet is
    /// free to change: no crossfade is running and no other amp is on its way. Until then
    /// it waits there, and a newer one may replace it. Crossfades to it while the amp is
    /// heard. Does not allocate, free or wait
    fn follow_cabinet(&mut self) {
        if !self.cab_stage.is_ready() || self.cabinet_busy > 0 || self.wanted_amp != self.amp {
            return;
        }
        let unheard = !self.primed || self.wet == 0.0;
        let was_voiced = self.voiced_cab();
        let Self { cab_stage, cabinet, cab_irs, playing, cabinet_busy, amp, sample_rate, .. } =
            self;
        cab_stage.take(|taps, spectra, speaker, made_for| {
            // One made for another sample rate is of no use: its successor is on its way
            if made_for != *sample_rate {
                return;
            }
            let chosen = match speaker {
                _ if !taps.is_empty() => Playing::File,
                Some(speaker) if speaker < SPEAKERS.len() => Playing::Speaker(speaker),
                _ => Playing::Own,
            };
            // Which cabinet designed here plays, and which is chosen
            let was_built_in = cab_ir_index(*playing, *amp);
            *playing = chosen;
            let Some(index) = cab_ir_index(chosen, *amp) else {
                if unheard {
                    cabinet.set_taps(taps, spectra);
                } else {
                    cabinet.swap_taps(taps, spectra);
                    *cabinet_busy = cabinet.swap_len();
                }
                return;
            };
            // A cabinet is not swapped for itself
            if was_built_in == Some(index) {
                return;
            }
            if unheard {
                cabinet.set_ir(&cab_irs[index]);
            } else {
                cabinet.swap_ir(&cab_irs[index]);
                *cabinet_busy = cabinet.swap_len();
            }
        });
        // The dials work on the frequencies of the cabinet that plays: set them again
        if self.voiced_cab() != was_voiced {
            self.cab_applied = [f32::NAN; 2];
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The cabinet whose frequencies the Mic and Resonance dials work on: the one that
    /// plays. A file has its own mic position and resonance: the dials work on fixed
    /// frequencies there
    fn voiced_cab(&self) -> &'static CabModel {
        match self.playing {
            Playing::Own => &self.amp.model().cab,
            Playing::Speaker(speaker) => &SPEAKERS[speaker].cab,
            Playing::File => &NEUTRAL_CAB,
        }
    }

    /// Where to leave a cabinet for the chain: see `CabStage`
    pub fn cab_stage(&self) -> Arc<CabStage> {
        self.cab_stage.clone()
    }

    fn configure_amp(&mut self) {
        let model = self.amp.model();
        let oversampled_rate = self.oversampled_rate();
        self.preamp.configure(model, oversampled_rate);
        self.power.configure(&model.power, oversampled_rate);
        self.tone_applied = [f32::NAN; 3];
        self.presence_applied = f32::NAN;
        self.cab_applied = [f32::NAN; 2];
        self.mic_type_applied = None;
    }

    pub fn reset(&mut self) {
        self.reset_stages();
        self.primed = false;
        self.until_tick = 0;
        // Starts again with the first block if it is still switched on
        self.tuning = false;
        self.tuner.stop();
        self.clipped = false;
        self.lamps.set_gate_gain(self.gate.gain());
    }

    /// Whether the gate is open and the output clips, for the editor
    pub fn lamp_reading(&self) -> Arc<LampReading> {
        self.lamps.clone()
    }

    /// What the tuner hears, for the editor
    pub fn tuner_reading(&self) -> Arc<TunerReading> {
        self.tuner.reading()
    }

    fn reset_stages(&mut self) {
        self.gate.reset();
        self.fuzz.reset();
        self.comp.reset();
        self.pitch.reset();
        self.wah.reset();
        self.back_drive.reset();
        self.back_pitch.iter_mut().for_each(Pitch::reset);
        self.back_wah.iter_mut().for_each(Wah::reset);
        self.eq.reset();
        // As for the delay and the reverb below: a pedal with nothing in it is left alone
        if !self.chorus.is_idle() {
            self.chorus.reset();
        }
        if !self.flanger.is_idle() {
            self.flanger.reset();
        }
        self.phaser.reset();
        self.tremolo.reset();
        self.pool.reset();
        self.reset_amp_stages();
        self.cabinet.reset();
        self.cab_voicing.reset();
        self.second.reset_stages();
        self.second.on_fade = 0;
        self.second.amp_fade = self.amp_fade_len;
        self.second.cabinet_busy = 0;
        self.dc.reset();
        // An idle effect has nothing left to be heard already. Clearing its lines would
        // only cost time, and this is also called on the audio thread, when Bypass has faded
        if !self.delay.is_idle() {
            self.delay.reset();
        }
        if !self.reverb.is_idle() {
            self.reverb.reset();
        }
        if !self.front_delay.is_idle() {
            self.front_delay.reset();
        }
        if !self.front_reverb.is_idle() {
            self.front_reverb.reset();
        }
        self.amp_fade = self.amp_fade_len;
        self.order_fade = self.order_fade_len;
        self.cabinet_busy = 0;
    }

    /// True when nothing is left to hear of earlier input: no repeats and no reverb tail are
    /// ringing. The amp and the cabinet themselves are silent within milliseconds
    pub fn is_idle(&self) -> bool {
        self.delay.is_idle()
            && self.reverb.is_idle()
            && self.front_delay.is_idle()
            && self.front_reverb.is_idle()
            && self.pitch.is_idle()
            && self.back_pitch.iter().all(Pitch::is_idle)
            && self.chorus.is_idle()
            && self.flanger.is_idle()
            && self.phaser.is_idle()
            && self.tremolo.is_idle()
            && self.pool.is_idle()
    }

    /// Everything between the gate and the cabinet. The effects are not touched: their
    /// repeats and tail ring on while another amp is set up
    fn reset_amp_stages(&mut self) {
        self.oversampler.reset();
        self.drive.reset();
        self.preamp.reset();
        self.tone.reset();
        self.power.reset();
    }

    /// Processes a block in place. With two channels the input is their average and the
    /// output is stereo: the amp in the middle, the repeats and the reverb tail around it.
    /// With one channel the output is the left one of those two. Their sum would keep the amp
    /// as it is as well, but the tail and the repeats come out 3 dB lower in it, and the
    /// repeats hollow: left and right repeat a few milliseconds apart, which is a comb
    pub fn process(
        &mut self,
        settings: &AmpSettings,
        left: &mut [f32],
        mut right: Option<&mut [f32]>,
    ) {
        self.wanted_amp = settings.amp;
        self.second.wanted_amp = settings.second.amp;
        self.second.wanted_speaker =
            settings.second.speaker.filter(|&speaker| speaker < SPEAKERS.len());
        self.mic_type = settings.mic_type;
        let wet_target = if settings.bypass { 0.0 } else { 1.0 };
        if !self.primed {
            self.wet = wet_target;
            self.wanted_layout = settings.layout;
        }

        let mut start = 0;
        while start < left.len() {
            self.follow_amp();
            self.follow_order();
            self.follow_cabinet();
            self.follow_second();
            if self.until_tick == 0 {
                self.read_dials(settings);
                self.until_tick = CHUNK;
            }
            let mut len = (left.len() - start).min(self.until_tick);
            if self.wanted_amp != self.amp {
                // The piece ends where the amp has faded to silence, or where the cabinet is
                // ready for the next amp, so the switch falls on that sample
                let wait =
                    if self.amp_fade > 0 { self.amp_fade as usize } else { self.cabinet_busy };
                if wait > 0 {
                    len = len.min(wait);
                }
            }
            if self.second.switching() && self.second.on_fade > 0 {
                // And likewise for the second amp, while it is heard
                let wait = if self.second.amp_fade > 0 {
                    self.second.amp_fade as usize
                } else {
                    self.second.cabinet_busy
                };
                if wait > 0 {
                    len = len.min(wait);
                }
            }
            if self.wanted_layout != self.layout && self.order_fade > 0 {
                // Likewise where the output has faded to silence for another order
                len = len.min(self.order_fade as usize);
            }
            let end = start + len;
            let right_chunk = right.as_deref_mut().map(|right| &mut right[start..end]);
            self.process_chunk(wet_target, &mut left[start..end], right_chunk);
            self.cabinet_busy = self.cabinet_busy.saturating_sub(len);
            self.second.cabinet_busy = self.second.cabinet_busy.saturating_sub(len);
            self.until_tick -= len;
            start = end;
        }

        // For the lamps. Two stores per block, and nothing is read back: the sound does
        // not depend on them. While bypassed the gate is at rest, which reads as closed
        self.lamps.set_gate_gain(self.gate.gain());
        if self.clipped {
            self.lamps.count_clip();
            self.clipped = false;
        }
    }

    /// Moves the dials a step towards the settings and passes them on to the stages
    fn read_dials(&mut self, settings: &AmpSettings) {
        let target = Dials::from_settings(settings);
        self.dial_target = target;
        // Nothing is heard of the amp while bypassed, so there is nothing to smooth
        let jump = !self.primed || self.wet == 0.0;
        if jump {
            self.dials = target;
        } else {
            self.dials.approach(&target, self.dial_coeff);
        }

        self.in_gain.set_target(db_to_gain(self.dials.in_gain_db), CHUNK as u32);
        self.out_level.set_target(self.dials.out_level, CHUNK as u32);

        // The switches and the gate's settings are read here as well, not once per block,
        // so that they too take effect on the same sample whatever the block size
        self.gate.set(
            settings.gate_on,
            settings.gate_thresh_db.clamp(GATE_THRESHOLD_DB[0], GATE_THRESHOLD_DB[1]),
            settings.gate_release_ms.clamp(GATE_RELEASE_MS[0], GATE_RELEASE_MS[1]),
        );
        self.drive_on = settings.drive_on;
        self.slot_settings = settings.slots;
        self.follow_drive_side(false);
        let steps = self.oversampled_chunk();
        let (gain, tone, level) =
            (self.dials.drive_gain, self.dials.drive_tone, self.dials.drive_level);
        self.drive.set(gain, tone, level, steps);
        self.back_drive.set(gain, tone, level, steps);
        let cab_target = if settings.cab_on { 1.0 } else { 0.0 };
        if cab_target != self.cab_mix.target() {
            self.cab_mix.set_target(cab_target, self.cab_fade_steps);
        }
        // The layout is read here as well, so a change falls on the same sample whatever
        // the block size, and what the slots say with it
        self.wanted_layout = settings.layout;
        self.fuzz_settings = settings.fuzz;
        self.comp_settings = settings.comp;
        self.pitch_settings = settings.pitch;
        self.wah_settings = settings.wah;
        self.eq_settings = settings.eq;
        self.chorus_settings = settings.chorus;
        self.phaser_settings = settings.phaser;
        self.flanger_settings = settings.flanger;
        self.tremolo_settings = settings.tremolo;
        self.delay_settings = settings.delay;
        self.reverb_settings = settings.reverb;

        if settings.tuner_on != self.tuning {
            self.tuning = settings.tuner_on;
            if self.tuning {
                self.tuner.start();
            } else {
                self.tuner.stop();
            }
        }
        let mute_target = if self.tuning { 0.0 } else { 1.0 };
        if mute_target != self.tuner_mute.target() {
            self.tuner_mute.set_target(mute_target, self.tuner_fade_steps);
        }

        if jump {
            self.tuner_mute.snap();
            self.in_gain.snap();
            self.out_level.snap();
            self.gate.snap();
            self.drive.snap();
            self.back_drive.snap();
            self.cab_mix.snap();
        }
        self.apply_dials(jump);
        self.tell_pool(jump);
        self.read_second(settings, jump);
        self.primed = true;
    }

    /// Reads the second amp's switch, dials and the Blend dial with the others. It is only
    /// worked on while it is heard: switched on, or fading out
    fn read_second(&mut self, settings: &AmpSettings, jump: bool) {
        self.second.wanted_on = settings.second.on;
        self.second.target = settings.second.dials();
        let blend = settings.blend.clamp(0.0, 1.0);
        self.second.blend.set_target(blend, CHUNK as u32);
        if jump {
            // Nothing is heard of it while bypassed, or before the first block
            self.second.on_fade = if self.second.wanted_on { self.amp_fade_len } else { 0 };
        }
        // The stages follow the first amp's drive pedal like the first amp's own
        let driven = self.driven();
        self.second.preamp.set_driven(driven, self.drive.fade_steps());
        self.second.power.set_driven(driven, self.drive.fade_steps());
        if !self.second.runs() {
            self.second.dials = self.second.target;
            self.second.blend.snap();
            return;
        }
        // Coming from rest the dials start where they are going
        let starting = self.second.at_rest;
        if jump || starting {
            self.second.dials = self.second.target;
            self.second.blend.snap();
        } else {
            self.second.approach(self.dial_coeff, DIAL_SETTLED);
        }
        self.second.at_rest = false;
        self.apply_second_dials(jump || starting);
    }

    fn apply_second_dials(&mut self, snap: bool) {
        let cab_dials = [self.dials.cab_mic, self.dials.cab_res];
        let timing = Timing {
            oversampled_rate: self.oversampled_rate(),
            sample_rate: self.sample_rate,
            oversampled_chunk: self.oversampled_chunk(),
            chunk: CHUNK as u32,
        };
        self.second.apply_dials(snap, timing, cab_dials, self.mic_type);
    }

    /// Passes the amp's own dials on to its stages and its cabinet, as the amp they are set
    /// up as reads them
    fn apply_dials(&mut self, snap: bool) {
        let model = self.amp.model();
        self.preamp.set_gain(model, self.dials.gain, self.oversampled_chunk());
        self.power.set_master(
            &model.power,
            self.dials.master,
            curve(&model.makeup_db, self.dials.gain),
            self.oversampled_chunk(),
        );
        if snap {
            self.preamp.snap();
            self.power.snap();
        }

        // The filters of the dials glide to where the dial is now over the piece that
        // follows, so that reading the dials once per piece is not heard as steps
        let oversampled_rate = self.oversampled_rate();
        let steps = if snap { 0 } else { self.oversampled_chunk() };
        let tone = [self.dials.bass, self.dials.mid, self.dials.treble];
        if tone != self.tone_applied {
            self.tone.set(
                &ToneCurve::new(&model.tone, tone[0], tone[1], tone[2], oversampled_rate),
                steps,
            );
            self.tone_applied = tone;
        }
        if self.dials.presence != self.presence_applied {
            self.power.set_presence(&model.power, self.dials.presence, oversampled_rate, steps);
            self.presence_applied = self.dials.presence;
        }
        if self.mic_type_applied != Some(self.mic_type) {
            let steps = if snap { 0 } else { CHUNK as u32 };
            self.cab_voicing.set_mic_type(self.mic_type, self.sample_rate, steps);
            self.mic_type_applied = Some(self.mic_type);
        }
        let cab = [self.dials.cab_mic, self.dials.cab_res];
        if cab != self.cab_applied {
            let steps = if snap { 0 } else { CHUNK as u32 };
            let voiced = self.voiced_cab();
            self.cab_voicing.set(voiced, cab[0], cab[1], self.sample_rate, steps);
            self.cab_applied = cab;
        }
    }

    /// Fades the amp's output down while another amp is waiting, and back up after
    fn fade_amp(&mut self, block: &mut [f32]) {
        let down = self.wanted_amp != self.amp;
        if !down && self.amp_fade == self.amp_fade_len {
            return;
        }
        for sample in block.iter_mut() {
            if down {
                self.amp_fade = self.amp_fade.saturating_sub(1);
            } else if self.amp_fade < self.amp_fade_len {
                self.amp_fade += 1;
            }
            // Eased at both ends
            let position = self.amp_fade as f32 / self.amp_fade_len as f32;
            *sample *= position * position * (3.0 - 2.0 * position);
        }
    }

    /// The share of the cabinet in what follows it for each sample of a piece of `len`:
    /// 1.0 on, 0.0 the amp's own signal. None while it is all cabinet, which is what runs
    /// when nothing is switched
    fn cabinet_shares(&mut self, len: usize) -> Option<[f32; CHUNK]> {
        let all_cabinet = self.cab_mix.value() == 1.0 && self.cab_mix.target() == 1.0;
        if all_cabinet {
            return None;
        }
        let mut shares = [1.0f32; CHUNK];
        for share in &mut shares[..len] {
            *share = self.cab_mix.next();
        }
        Some(shares)
    }

    /// A cabinet with its dials, or the amp's own signal, or on the way between the two.
    /// The cabinet always runs, so it has its history when it is switched back on
    fn run_cabinet(
        cabinet: &mut Cabinet,
        voicing: &mut CabVoicing,
        shares: Option<&[f32; CHUNK]>,
        block: &mut [f32],
    ) {
        let mut direct = [0.0f32; CHUNK];
        if shares.is_some() {
            direct[..block.len()].copy_from_slice(block);
        }

        cabinet.process(block);
        voicing.process(block);

        if let Some(shares) = shares {
            let direct_gain = db_to_gain(CAB_OFF_DB);
            for ((sample, &direct), &share) in block.iter_mut().zip(&direct).zip(shares) {
                let direct = direct * direct_gain;
                *sample = direct + share * (*sample - direct);
            }
        }
    }

    /// Runs a stereo effect on one channel, for an effect that stands in front of the amp:
    /// the signal goes in on both sides and what comes out is the mean of the two, so the
    /// level of the dry signal is as it was and the repeats and the tail are what a listener
    /// in the middle would hear of them. Does not allocate
    pub(crate) fn mono_through_stereo(
        high: &mut [f32],
        effect: impl FnOnce(&mut [f32], &mut [f32]),
    ) {
        let mut other = [0.0f32; OVERSAMPLED_CHUNK];
        let other = &mut other[..high.len()];
        other.copy_from_slice(high);
        effect(high, other);
        for (sample, other) in high.iter_mut().zip(other.iter()) {
            *sample = 0.5 * (*sample + *other);
        }
    }

    /// At most `CHUNK` samples
    fn process_chunk(&mut self, wet_target: f32, left: &mut [f32], mut right: Option<&mut [f32]>) {
        let bypassed = |chain: &Self| chain.wet == 0.0 && wet_target == 0.0;
        if bypassed(self) && !self.tuning {
            return;
        }
        let len = left.len();

        let mut signal = [0.0f32; CHUNK];
        match &right {
            Some(right) => {
                for ((mono, l), r) in signal.iter_mut().zip(left.iter()).zip(right.iter()) {
                    *mono = 0.5 * (l + r);
                }
            }
            None => signal[..len].copy_from_slice(left),
        }

        // The tuner hears the input as it arrives, whatever the amp does with it
        if self.tuning {
            self.tuner.process(&signal[..len]);
            if bypassed(self) {
                return;
            }
        }

        // While tuning the amp gets silence, so nothing of it is left in the repeats and
        // the tail afterwards, and what still rings in them is turned down behind them
        let muting = self.tuner_mute.value() != 1.0 || self.tuner_mute.target() != 1.0;
        let mut heard_share = [1.0f32; CHUNK];
        if muting {
            for share in &mut heard_share[..len] {
                *share = self.tuner_mute.next();
            }
        }

        // For the input meter: the level the guitar arrives at, before the input gain
        let input_peak = signal[..len].iter().fold(0.0f32, |peak, sample| peak.max(sample.abs()));

        // The gate listens to the input as it arrives and acts on it after the input gain
        let heard = signal;
        for sample in &mut signal[..len] {
            *sample *= self.in_gain.next();
        }
        if muting {
            for (sample, share) in signal[..len].iter_mut().zip(&heard_share) {
                *sample *= share;
            }
        }
        self.gate.process(&heard[..len], &mut signal[..len]);

        let mut oversampled = [0.0f32; OVERSAMPLED_CHUNK];
        let high = &mut oversampled[..len * self.factor];
        self.oversampler.upsample(&signal[..len], high);
        for index in 0..self.layout.front().len() {
            match self.layout.front()[index] {
                Voice::Home(Pedal::Fuzz) => self.fuzz.process_mono(&self.fuzz_settings, high),
                Voice::Home(Pedal::Comp) => self.comp.process_mono(&self.comp_settings, high),
                Voice::Home(Pedal::Pitch) => self.pitch.process_mono(&self.pitch_settings, high),
                Voice::Home(Pedal::Wah) => self.wah.process_mono(&self.wah_settings, high),
                Voice::Home(Pedal::Eq) => self.eq.process_mono(&self.eq_settings, high),
                Voice::Home(Pedal::Drive) => self.drive.process(high),
                Voice::Home(Pedal::Chorus) => self.chorus.process_mono(&self.chorus_settings, high),
                Voice::Home(Pedal::Phaser) => self.phaser.process_mono(&self.phaser_settings, high),
                Voice::Home(Pedal::Flanger) => {
                    self.flanger.process_mono(&self.flanger_settings, high)
                }
                Voice::Home(Pedal::Tremolo) => {
                    self.tremolo.process_mono(&self.tremolo_settings, high)
                }
                Voice::Home(Pedal::Delay) => {
                    if self.delay_settings.on || !self.front_delay.is_idle() {
                        let (delay, settings) = (&mut self.front_delay, &self.delay_settings);
                        Self::mono_through_stereo(high, |l, r| delay.process(settings, l, r));
                    }
                }
                Voice::Home(Pedal::Reverb) => {
                    if self.reverb_settings.on || !self.front_reverb.is_idle() {
                        let (reverb, settings) = (&mut self.front_reverb, &self.reverb_settings);
                        Self::mono_through_stereo(high, |l, r| reverb.process(settings, l, r));
                    }
                }
                Voice::Slot { kind, slot, rank } => {
                    let settings = &self.slot_settings[slot as usize];
                    self.pool.unit_mut(kind, rank).process_front(settings, high);
                }
            }
        }
        // The second amp hears what the first one hears
        let second_runs = self.second.runs();
        let mut second_high = [0.0f32; OVERSAMPLED_CHUNK];
        if second_runs {
            second_high[..high.len()].copy_from_slice(high);
        }
        self.preamp.process(high);
        self.tone.process(high);
        self.power.process(high);
        self.oversampler.downsample(high, &mut signal[..len]);

        self.fade_amp(&mut signal[..len]);
        let shares = self.cabinet_shares(len);
        Self::run_cabinet(
            &mut self.cabinet,
            &mut self.cab_voicing,
            shares.as_ref(),
            &mut signal[..len],
        );

        // The second amp plays beside the first one, from the same pedals, and is mixed in
        // behind the cabinets, so both reach what follows on time. The Blend law is between
        // linear and equal power (`second::BLEND_LAW`). At Blend 0 and 1 one share is exactly
        // 1 and the other 0
        if second_runs {
            let high_second = &mut second_high[..len * self.factor];
            self.second.preamp.process(high_second);
            self.second.tone.process(high_second);
            self.second.power.process(high_second);
            let mut second_signal = [0.0f32; CHUNK];
            self.second.down.downsample(high_second, &mut second_signal[..len]);
            self.second.fade_amp(&mut second_signal[..len], self.amp_fade_len);
            Self::run_cabinet(
                &mut self.second.cabinet,
                &mut self.second.voicing,
                shares.as_ref(),
                &mut second_signal[..len],
            );
            for (sample, &second) in signal[..len].iter_mut().zip(&second_signal) {
                let weight = self.second.weight(self.amp_fade_len);
                let [first_gain, second_gain] = self.second.gains(weight);
                *sample = *sample * first_gain + second * second_gain;
            }
        }

        // Two channels from here on. The effects add to what they are given and leave the
        // amp's own signal as it is, on time. Both are called whether they are on or not:
        // switched off they let their repeats and tail ring out, and idle they do nothing
        let mut wide = [[0.0f32; CHUNK]; 2];
        for index in 0..len {
            let sample = self.dc.process(signal[index]);
            wide[0][index] = sample;
            wide[1][index] = sample;
        }
        let [wide_left, wide_right] = &mut wide;
        let (wide_left, wide_right) = (&mut wide_left[..len], &mut wide_right[..len]);
        for index in 0..self.layout.back().len() {
            match self.layout.back()[index] {
                Voice::Home(Pedal::Fuzz) => {
                    self.fuzz.process_stereo(&self.fuzz_settings, wide_left, wide_right)
                }
                Voice::Home(Pedal::Comp) => {
                    self.comp.process_stereo(&self.comp_settings, wide_left, wide_right)
                }
                Voice::Home(Pedal::Eq) => {
                    self.eq.process_stereo(&self.eq_settings, wide_left, wide_right)
                }
                Voice::Home(Pedal::Delay) => {
                    self.delay.process(&self.delay_settings, wide_left, wide_right)
                }
                Voice::Home(Pedal::Reverb) => {
                    self.reverb.process(&self.reverb_settings, wide_left, wide_right)
                }
                Voice::Home(Pedal::Chorus) => {
                    self.chorus.process_stereo(&self.chorus_settings, wide_left, wide_right)
                }
                Voice::Home(Pedal::Phaser) => {
                    self.phaser.process_stereo(&self.phaser_settings, wide_left, wide_right)
                }
                Voice::Home(Pedal::Flanger) => {
                    self.flanger.process_stereo(&self.flanger_settings, wide_left, wide_right)
                }
                Voice::Home(Pedal::Tremolo) => {
                    self.tremolo.process_stereo(&self.tremolo_settings, wide_left, wide_right)
                }
                Voice::Home(Pedal::Drive) => self.back_drive.process_stereo(wide_left, wide_right),
                Voice::Home(Pedal::Pitch) => {
                    self.back_pitch[0].process_mono(&self.pitch_settings, wide_left);
                    self.back_pitch[1].process_mono(&self.pitch_settings, wide_right);
                }
                Voice::Home(Pedal::Wah) => {
                    self.back_wah[0].process_mono(&self.wah_settings, wide_left);
                    self.back_wah[1].process_mono(&self.wah_settings, wide_right);
                }
                Voice::Slot { kind, slot, rank } => {
                    let settings = &self.slot_settings[slot as usize];
                    self.pool.unit_mut(kind, rank).process_back(settings, wide_left, wide_right);
                }
            }
        }

        // Down to silence while another order is waiting, and back up after
        let reordering = self.wanted_layout != self.layout || self.order_fade < self.order_fade_len;
        let mut order_share = [1.0f32; CHUNK];
        if reordering {
            let down = self.wanted_layout != self.layout;
            for share in &mut order_share[..len] {
                if down {
                    self.order_fade = self.order_fade.saturating_sub(1);
                } else if self.order_fade < self.order_fade_len {
                    self.order_fade += 1;
                }
                // Eased at both ends
                let position = self.order_fade as f32 / self.order_fade_len as f32;
                *share = position * position * (3.0 - 2.0 * position);
            }
        }

        // The effects round off their own output under full scale; the safety clip comes last
        let mut output_peak = 0.0f32;
        for index in 0..len {
            let level = self.out_level.next();
            if self.wet != wet_target {
                self.wet =
                    (self.wet + self.wet_step.copysign(wet_target - self.wet)).clamp(0.0, 1.0);
            }

            let driven = wide_left[index] * level;
            output_peak = output_peak.max(driven.abs());
            self.clipped |= driven.abs() > OUTPUT_CLIP_KNEE;
            let mut output = output_clip(driven);
            if muting {
                output *= heard_share[index];
            }
            if reordering {
                output *= order_share[index];
            }
            if self.wet >= 1.0 {
                left[index] = output;
            } else {
                left[index] += self.wet * (output - left[index]);
            }
            if let Some(right) = right.as_deref_mut() {
                let driven = wide_right[index] * level;
                output_peak = output_peak.max(driven.abs());
                self.clipped |= driven.abs() > OUTPUT_CLIP_KNEE;
                let mut output = output_clip(driven);
                if muting {
                    output *= heard_share[index];
                }
                if reordering {
                    output *= order_share[index];
                }
                if self.wet >= 1.0 {
                    right[index] = output;
                } else {
                    right[index] += self.wet * (output - right[index]);
                }
            }
        }

        // For the meters, one more store per chunk: the sound does not depend on it
        self.lamps.note_peaks(input_peak, output_peak);

        // Start from silence when the amp comes back, not from where it was left, and
        // without the repeats and the tail of what was played before
        if bypassed(self) {
            self.reset_stages();
        }
    }
}

#[cfg(test)]
mod tests;
