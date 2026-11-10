//! The amps beside the first: a copy of the signal after the front pedals goes through its
//! own preamp, tone stack, power amp and built-in cabinet, and the chain mixes it with the
//! first amp behind the cabinets (`chain.rs`). The second amp (#177) and the third and fourth
//! (#269) are the same `Second`, with the same stages the first amp uses, set up with the same
//! `AmpModel` constants. With one off none of this runs for it, its output is not added, and
//! the chain is as it was without it.
//!
//! The mix is `Mix`: a weight for each amp (its level, the Blend between the first two) and
//! the law that turns the weights into gains.

use crate::amp::model::{speaker_index, Amp, CabModel, SPEAKERS};
use crate::amp::poweramp::PowerAmp;
use crate::amp::preamp::Preamp;
use crate::amp::tonestack::{ToneCurve, ToneStack};
use crate::cab::{CabIr, CabVoicing, Cabinet, MicType};
use crate::dsp::oversample::Oversampler;
use crate::dsp::{curve, Ramp};
use nih_plug::params::persist::PersistentField;
use std::sync::atomic::{AtomicU32, Ordering};

/// The Blend law: the first amp's share is `(1 - blend)` and the second's `blend`, each raised
/// to this power. 1.0 would be a linear blend, which dips 2.8 LU in the middle for a heavy amp
/// and a clean one (they are largely not the same signal: their powers add, and each is
/// half); 0.5 is equal power, which is flat for them but adds 3 dB at the middle for two
/// amps that are largely the same, and lets the chords of Brøl and Glød over the safety
/// clip's knee. This is between the two, and `blend_report` holds it to both. Exactly 1 and
/// 0 at the ends of the dial
pub const BLEND_LAW: f32 = 0.6;

/// How much the law's power grows with the share of the weight the third and fourth amp hold
/// (`Mix::law`): each amp added to a pair is a little more of the same guitar, which sums
/// louder than the pair's 2.4 dB, so the power goes from `BLEND_LAW` at none of it to
/// `BLEND_LAW + EXTRA_LAW / 2` when the two hold half the mix. Exactly `BLEND_LAW` with the
/// third and fourth amp out, so the pair is as it was. `group_report` holds it
pub const EXTRA_LAW: f32 = 0.3;

/// The amps beside the first one: the second, third and fourth
pub const EXTRA_AMPS: usize = 3;

/// What the Blend dial leaves the third and fourth amp's weight at: the Blend is a crossfade of
/// the first two, 0.5 in the middle where each has half of it, and the amps it does not touch
/// have the same half, so equal levels are equal shares whatever the number of amps
pub const NEUTRAL_BLEND: f32 = 0.5;

/// What the second amp's switch, choice and dials say, 0.0 to 1.0 like the first amp's
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecondSettings {
    pub on: bool,
    pub amp: Amp,
    /// The built-in cabinet it plays through, by its place in `SPEAKERS`: None for its own
    /// amp's
    pub speaker: Option<usize>,
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
            speaker: None,
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
    // The built-in cabinet that plays (None: the amp's own) and the one that is wanted. A
    // change of either is an amp switch: faded out, made at silence, faded back in
    pub speaker: Option<usize>,
    pub wanted_speaker: Option<usize>,
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
    // The weight of this amp in the mix, before the others are counted in: its level times
    // its Blend (`Mix`)
    pub weight: Ramp,
    // Level of the second amp in steps, `fade_len` when switched on, 0 when off: it fades in
    // and out in the time of an amp switch
    pub on_fade: u32,
    // The same for the amp switch: down to 0 on the way to another amp and back up after
    pub amp_fade: u32,
    // Samples until the cabinet has finished its crossfade
    pub cabinet_busy: usize,
    // The stages are at rest and not being run: they start from there
    pub at_rest: bool,
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
            speaker: None,
            wanted_speaker: None,
            wanted_on: false,
            preamp: Preamp::new(),
            tone: ToneStack::new(),
            power: PowerAmp::new(),
            down: Oversampler::new(),
            cabinet: Cabinet::new(),
            voicing: CabVoicing::new(),
            dials: defaults.dials(),
            target: defaults.dials(),
            weight: Ramp::new(NEUTRAL_BLEND),
            on_fade: 0,
            amp_fade: 1,
            cabinet_busy: 0,
            at_rest: true,
            tone_applied: [f32::NAN; 3],
            presence_applied: f32::NAN,
            cab_applied: [f32::NAN; 2],
            mic_type_applied: None,
        }
    }

    /// Whether another amp or another cabinet is waited for
    pub fn switching(&self) -> bool {
        self.wanted_amp != self.amp || self.wanted_speaker != self.speaker
    }

    /// Where the cabinet that plays is in the chain's designed cabinets: its own amp's, then
    /// the built-in ones
    pub fn ir_index(&self) -> usize {
        match self.speaker {
            Some(speaker) => Amp::ALL.len() + speaker,
            None => self.amp.index(),
        }
    }

    /// The cabinet whose frequencies the Mic and Resonance dials work on
    pub fn voiced_cab(&self) -> &'static CabModel {
        match self.speaker {
            Some(speaker) => &SPEAKERS[speaker].cab,
            None => &self.amp.model().cab,
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
            self.voicing.set(self.voiced_cab(), cab_dials[0], cab_dials[1], sample_rate, steps);
            self.cab_applied = cab_dials;
        }
    }

    /// Fades the amp's output down while another amp is waiting, and back up after
    pub fn fade_amp(&mut self, block: &mut [f32], fade_len: u32) {
        let down = self.switching();
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

    /// Where this amp is in being heard, 0.0 to 1.0 in a straight line: 1.0 once it is faded
    /// in. Steps one sample on towards where its switch is
    fn presence(&mut self, fade_len: u32) -> f32 {
        if self.wanted_on {
            if self.on_fade < fade_len {
                self.on_fade += 1;
            }
        } else {
            self.on_fade = self.on_fade.saturating_sub(1);
        }
        self.on_fade as f32 / fade_len as f32
    }
}

/// The mix of the amps: the gain each has, from the weights they stand at
///
/// The weight of an amp is its level, times the Blend for the first two (`1 - blend` and `blend`)
/// and times `NEUTRAL_BLEND` for the others. What an amp adds is its share of what is heard:
/// its weight over the total of the amps that are in (`gains`), to the power of `BLEND_LAW`.
/// The first amp has what is left, so it is exactly 1 with no other amp in and the mix is
/// the single amp it was; at Blend 0 the second amp's weight is 0 and the first amp's share
/// 1, and the other way round at Blend 1, to the bit. An amp fades in over a switch's time
/// (`Second::on_fade`): its weight counts in the total as it does, a straight line, while its
/// share goes with the eased fade
pub struct Mix {
    // The first amp's weight, as the others have theirs in `Second::weight`
    pub first: Ramp,
    inputs: [f32; 7],
    gains: [f32; 4],
}

impl Mix {
    pub fn new() -> Self {
        Self { first: Ramp::new(NEUTRAL_BLEND), inputs: [f32::NAN; 7], gains: [1.0, 0.0, 0.0, 0.0] }
    }

    /// The gains of the four amps for the next sample. The amps' weights and fades step on
    pub fn gains(&mut self, extra: &mut [Second; EXTRA_AMPS], fade_len: u32) -> [f32; 4] {
        let first = self.first.next();
        let mut weights = [0.0f32; EXTRA_AMPS];
        let mut presences = [0.0f32; EXTRA_AMPS];
        for ((amp, weight), presence) in extra.iter_mut().zip(&mut weights).zip(&mut presences) {
            *weight = amp.weight.next();
            *presence = amp.presence(fade_len);
        }
        let inputs =
            [first, weights[0], weights[1], weights[2], presences[0], presences[1], presences[2]];
        if inputs != self.inputs {
            self.inputs = inputs;
            self.gains = Self::law(first, weights, presences);
        }
        self.gains
    }

    /// The law on its own: the gains of the first amp and of the others for their weights and
    /// how far each is faded in, 0.0 to 1.0
    pub fn law(first: f32, weights: [f32; EXTRA_AMPS], presences: [f32; EXTRA_AMPS]) -> [f32; 4] {
        let total = first
            + weights[0] * presences[0]
            + weights[1] * presences[1]
            + weights[2] * presences[2];
        if total <= f32::EPSILON {
            return [1.0, 0.0, 0.0, 0.0];
        }
        let mut shares = [0.0f32; EXTRA_AMPS];
        for ((share, &weight), &presence) in shares.iter_mut().zip(&weights).zip(&presences) {
            if presence > 0.0 {
                *share = weight * eased_share(presence) / total;
            }
        }
        let rest = (((1.0 - shares[0]) - shares[1]) - shares[2]).max(0.0);
        // The part of the weight the third and fourth amp hold
        let crowd = ((weights[1] * presences[1] + weights[2] * presences[2]) / total).min(1.0);
        let law = BLEND_LAW + EXTRA_LAW * crowd;
        [rest.powf(law), shares[0].powf(law), shares[1].powf(law), shares[2].powf(law)]
    }
}

impl Default for Mix {
    fn default() -> Self {
        Self::new()
    }
}

/// A fade of `presence` (0.0 to 1.0, a straight line) as the share it gets: eased at both
/// ends, as the fade of an amp switch is
fn eased_share(presence: f32) -> f32 {
    presence * presence * (3.0 - 2.0 * presence)
}

/// The built-in cabinet the second amp plays through, kept by the editor and saved with the
/// project (`cabinet2`) as the first amp's choice is (`cabinet`), by its frozen id:
/// `speaker:<id>`, or empty for the amp's own. Not a parameter. It is an atomic because a host
/// may restore a saved state on the audio thread, and the chain reads it with the dials: 0 for
/// the amp's own, the place in `SPEAKERS` plus 1 for a built-in one
pub struct SecondCabinet(AtomicU32);

impl SecondCabinet {
    pub const fn new() -> Self {
        Self(AtomicU32::new(0))
    }

    /// The built-in cabinet, None for the amp's own
    pub fn get(&self) -> Option<usize> {
        let stored = self.0.load(Ordering::Relaxed) as usize;
        stored.checked_sub(1).filter(|&speaker| speaker < SPEAKERS.len())
    }

    pub fn put(&self, speaker: Option<usize>) {
        self.0.store(speaker.map_or(0, |speaker| speaker as u32 + 1), Ordering::Relaxed);
    }

    /// One on in the round of the amp's own and the ten built-in ones, or one back
    pub fn step(&self, forward: bool) {
        let round = SPEAKERS.len() + 1;
        let here = self.get().map_or(0, |speaker| speaker + 1);
        let next = (here + if forward { 1 } else { round - 1 }) % round;
        self.put(next.checked_sub(1));
    }

    /// What the preset file and the saved state hold
    pub fn choice(&self) -> String {
        self.get().map_or_else(String::new, crate::user_cab::speaker_choice)
    }

    /// Takes a choice as `choice` writes it. One this build does not have is the amp's own
    pub fn set_choice(&self, choice: &str) {
        self.put(choice.strip_prefix("speaker:").and_then(speaker_index));
    }
}

impl Default for SecondCabinet {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistentField<'_, String> for SecondCabinet {
    fn set(&self, new_value: String) {
        self.set_choice(&new_value);
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&String) -> R,
    {
        f(&self.choice())
    }
}

/// `position` of `len` steps, eased at both ends
fn eased(position: u32, len: u32) -> f32 {
    let position = position as f32 / len as f32;
    position * position * (3.0 - 2.0 * position)
}

/// The amps beside the first with only the second one as given, for tests
#[cfg(test)]
pub fn second_only(second: SecondSettings) -> [SecondSettings; EXTRA_AMPS] {
    [second, SecondSettings::default(), SecondSettings::default()]
}
