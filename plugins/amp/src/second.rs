//! The second amp: a copy of the signal after the front pedals goes through its own preamp,
//! tone stack, power amp and built-in cabinet, and the chain mixes it with the first amp
//! behind the cabinets (`chain.rs`). The stages are the same ones the first amp uses, set up
//! with the same `AmpModel` constants. With the second amp off none of this runs and the
//! chain is the single amp it was.

use crate::amp::model::Amp;
use crate::amp::poweramp::PowerAmp;
use crate::amp::preamp::Preamp;
use crate::amp::tonestack::{ToneCurve, ToneStack};
use crate::cab::{CabIr, CabVoicing, Cabinet, MicType};
use crate::dsp::oversample::Oversampler;
use crate::dsp::{curve, Ramp};

/// The Blend law: the first amp's share is `(1 - blend)` and the second's `blend`, each raised
/// to this power. 1.0 would be a linear blend, which dips 2.8 LU in the middle for a heavy amp
/// and a clean one (they are largely not the same signal: their powers add, and each is
/// half); 0.5 is equal power, which is flat for them but adds 3 dB at the middle for two
/// amps that are largely the same, and lets the chords of Brøl and Glød over the safety
/// clip's knee. This is between the two, and `blend_report` holds it to both. Exactly 1 and
/// 0 at the ends of the dial
pub const BLEND_LAW: f32 = 0.6;

/// What the second amp's switch, choice and dials say, 0.0 to 1.0 like the first amp's
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecondSettings {
    pub on: bool,
    pub amp: Amp,
    pub gain: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub presence: f32,
    pub master: f32,
}

impl SecondSettings {
    /// Gain, Bass, Mid, Treble, Presence, Master, kept on their range
    pub fn dials(&self) -> [f32; 6] {
        [self.gain, self.bass, self.mid, self.treble, self.presence, self.master]
            .map(|dial| dial.clamp(0.0, 1.0))
    }
}

/// What the parameters start with. Klar is the clean one, which is what the heavy amp is
/// paired with
impl Default for SecondSettings {
    fn default() -> Self {
        Self {
            on: false,
            amp: Amp::Klar,
            gain: 0.5,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
            presence: 0.5,
            master: 0.5,
        }
    }
}

/// The rates and piece lengths the dials are passed on at
#[derive(Clone, Copy)]
pub struct Timing {
    /// The rate the amp stages run at, and the host's
    pub oversampled_rate: f32,
    pub sample_rate: f32,
    /// Samples in a piece of the chain's, at the oversampled rate and at the host's
    pub oversampled_chunk: u32,
    pub chunk: u32,
}

/// The second amp's stages and the state of its switches
pub struct Second {
    pub amp: Amp,
    pub wanted_amp: Amp,
    pub wanted_on: bool,
    pub preamp: Preamp,
    pub tone: ToneStack,
    pub power: PowerAmp,
    // Only its half-bands for coming back down are used: the way up is shared with the
    // first amp, which brings the same signal up
    pub down: Oversampler,
    pub cabinet: Cabinet,
    pub voicing: CabVoicing,
    // The dials the stages are at, and where they are going
    pub dials: [f32; 6],
    pub target: [f32; 6],
    // The Blend dial: the share of the second amp in the mix, 0.0 to 1.0
    pub blend: Ramp,
    // Level of the second amp in steps, `fade_len` when switched on, 0 when off: it fades in
    // and out in the time of an amp switch
    pub on_fade: u32,
    // The same for the amp switch: down to 0 on the way to another amp and back up after
    pub amp_fade: u32,
    // Samples until the cabinet has finished its crossfade
    pub cabinet_busy: usize,
    // The stages are at rest and not being run: they start from there
    pub at_rest: bool,
    // The blend the shares were last worked out for, and the shares of the first and the
    // second amp
    shares_for: f32,
    shares: [f32; 2],
    tone_applied: [f32; 3],
    presence_applied: f32,
    cab_applied: [f32; 2],
    mic_type_applied: Option<MicType>,
}

impl Second {
    pub fn new() -> Self {
        let defaults = SecondSettings::default();
        Self {
            amp: defaults.amp,
            wanted_amp: defaults.amp,
            wanted_on: false,
            preamp: Preamp::new(),
            tone: ToneStack::new(),
            power: PowerAmp::new(),
            down: Oversampler::new(),
            cabinet: Cabinet::new(),
            voicing: CabVoicing::new(),
            dials: defaults.dials(),
            target: defaults.dials(),
            blend: Ramp::new(0.5),
            on_fade: 0,
            amp_fade: 1,
            cabinet_busy: 0,
            at_rest: true,
            shares_for: 0.0,
            shares: [1.0, 0.0],
            tone_applied: [f32::NAN; 3],
            presence_applied: f32::NAN,
            cab_applied: [f32::NAN; 2],
            mic_type_applied: None,
        }
    }

    /// Whether the stages have to be run: switched on, or still fading out
    pub fn runs(&self) -> bool {
        self.wanted_on || self.on_fade > 0
    }

    /// Everything that depends on the sample rate. Not for the audio thread
    pub fn set_sample_rate(&mut self, sample_rate: f32, factor: usize, ir: &CabIr) {
        self.down.set_factor(factor);
        self.cabinet.set_sample_rate(sample_rate);
        self.cabinet.set_ir(ir);
    }

    /// Sets the stages up as the amp they are for
    pub fn configure(&mut self, oversampled_rate: f32) {
        let model = self.amp.model();
        self.preamp.configure(model, oversampled_rate);
        self.power.configure(&model.power, oversampled_rate);
        self.tone_applied = [f32::NAN; 3];
        self.presence_applied = f32::NAN;
        self.cab_applied = [f32::NAN; 2];
        self.mic_type_applied = None;
    }

    /// The amp's stages start from rest, as when another amp is switched to. The cabinet is
    /// left alone, so a crossfade that is running goes on
    pub fn reset_amp_stages(&mut self) {
        self.down.reset();
        self.preamp.reset();
        self.tone.reset();
        self.power.reset();
    }

    /// Everything starts from rest, and is not run until the amp is switched on
    pub fn reset_stages(&mut self) {
        self.reset_amp_stages();
        self.cabinet.reset();
        self.voicing.reset();
        self.at_rest = true;
    }

    /// Moves the dials a step towards where they are going
    pub fn approach(&mut self, coeff: f32, settled: f32) {
        for (value, &target) in self.dials.iter_mut().zip(&self.target) {
            *value += coeff * (target - *value);
            if (target - *value).abs() < settled {
                *value = target;
            }
        }
    }

    /// Passes the dials on to the stages and the cabinet's filters, as the amp they are set
    /// up as reads them. The Mic and Resonance dials and the microphone are the first amp's,
    /// worked on the frequencies of this amp's own cabinet
    pub fn apply_dials(
        &mut self,
        snap: bool,
        timing: Timing,
        cab_dials: [f32; 2],
        mic_type: MicType,
    ) {
        let Timing { oversampled_rate, sample_rate, oversampled_chunk, chunk } = timing;
        let model = self.amp.model();
        let [gain, bass, mid, treble, presence, master] = self.dials;
        self.preamp.set_gain(model, gain, oversampled_chunk);
        self.power.set_master(
            &model.power,
            master,
            curve(&model.makeup_db, gain),
            oversampled_chunk,
        );
        if snap {
            self.preamp.snap();
            self.power.snap();
        }

        let steps = if snap { 0 } else { oversampled_chunk };
        let tone = [bass, mid, treble];
        if tone != self.tone_applied {
            self.tone.set(
                &ToneCurve::new(&model.tone, tone[0], tone[1], tone[2], oversampled_rate),
                steps,
            );
            self.tone_applied = tone;
        }
        if presence != self.presence_applied {
            self.power.set_presence(&model.power, presence, oversampled_rate, steps);
            self.presence_applied = presence;
        }
        let steps = if snap { 0 } else { chunk };
        if self.mic_type_applied != Some(mic_type) {
            self.voicing.set_mic_type(mic_type, sample_rate, steps);
            self.mic_type_applied = Some(mic_type);
        }
        if cab_dials != self.cab_applied {
            self.voicing.set(&model.cab, cab_dials[0], cab_dials[1], sample_rate, steps);
            self.cab_applied = cab_dials;
        }
    }

    /// Fades the amp's output down while another amp is waiting, and back up after
    pub fn fade_amp(&mut self, block: &mut [f32], fade_len: u32) {
        let down = self.wanted_amp != self.amp;
        if !down && self.amp_fade == fade_len {
            return;
        }
        for sample in block.iter_mut() {
            if down {
                self.amp_fade = self.amp_fade.saturating_sub(1);
            } else if self.amp_fade < fade_len {
                self.amp_fade += 1;
            }
            *sample *= eased(self.amp_fade, fade_len);
        }
    }

    /// The gains of the first and the second amp for a Blend of `weight`
    pub fn gains(&mut self, weight: f32) -> [f32; 2] {
        if weight != self.shares_for {
            self.shares_for = weight;
            self.shares = [(1.0 - weight).powf(BLEND_LAW), weight.powf(BLEND_LAW)];
        }
        self.shares
    }

    /// The share of this amp in the mix for the next sample: the Blend dial, times the
    /// level it is faded in to
    pub fn weight(&mut self, fade_len: u32) -> f32 {
        if self.wanted_on {
            if self.on_fade < fade_len {
                self.on_fade += 1;
            }
        } else {
            self.on_fade = self.on_fade.saturating_sub(1);
        }
        self.blend.next() * eased(self.on_fade, fade_len)
    }
}

/// `position` of `len` steps, eased at both ends
fn eased(position: u32, len: u32) -> f32 {
    let position = position as f32 / len as f32;
    position * position * (3.0 - 2.0 * position)
}
