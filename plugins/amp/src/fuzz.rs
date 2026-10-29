//! Fuzz pedal: thick, saturated fuzz that sustains and breaks up as a note dies. Two knobs,
//! Fuzz and Tone; its level is matched inside, so it is about as loud as the pedal switched
//! off. Runs in front of the amp on one channel, inside the amp's oversampling, or behind the
//! cabinet on two, each with an oversampler of its own: it clips far harder than anything
//! else in the chain, and does that nowhere at the host's rate

use crate::dsp::filters::{Biquad, BiquadCoeffs, OnePoleHp, OnePoleLp, ANTI_DENORMAL};
use crate::dsp::oversample::{factor_for, Oversampler, MAX_FACTOR};
use crate::dsp::pair::Pair;
use crate::dsp::shaper::AsymClipper;
use crate::dsp::{curve, db_to_gain, lerp, smoothing_coeff};

// In front of the clipping: the lowest rumble and the pick's fizz stay out of it
const INPUT_HIGHPASS_HZ: f32 = 60.0;
const INPUT_LOWPASS_HZ: f32 = 5000.0;

// Gain into the clipper in dB with Fuzz at 0 and at 10
const FUZZ_DB: [f32; 2] = [26.0, 56.0];

// Where the clipper tops out, upwards and downwards. The difference leaves the waveform
// lopsided, which is what the bias below feeds on
const CLIP_LEVEL: [f32; 2] = [0.5, 0.75];

// The bias shifts with what the pedal plays: the average of the clipped signal, followed this
// fast, is taken off the clipper's input this strongly. A hard note pushes the operating point
// away from the side it clips on less, and a note dying away rides an operating point that is
// still on its way back: it thins out and breaks up instead of fading cleanly
const BIAS_MS: f32 = 25.0;
const BIAS_PULL: f32 = 0.6;

// Tone: a low-pass and a high-pass of the clipped signal, blended. Tone 0 is the low-pass
// alone (dark, woolly), Tone 10 the high-pass alone (thin, biting), and the middle scoops the
// mids between the two corners
const TONE_LOWPASS_HZ: f32 = 650.0;
const TONE_HIGHPASS_HZ: f32 = 1100.0;

// The top of the fuzz is rounded off here, steeply (fourth order, two sections of these Qs):
// it does not fizz, and what is left above the host's half rate of a waveform this square
// is gone before the rate is brought down
const FIZZ_HZ: f32 = 6000.0;
const FIZZ_Q: [f32; 2] = [0.5412, 1.3066];

// What the asymmetric clipping leaves as a constant offset is taken out behind it
const DC_HZ: f32 = 20.0;

// Taken off the output in dB across Fuzz (0, 5, 10), so that the pedal is about as loud as
// switched off for a guitar recorded at -24 dBFS RMS (`PEDAL_DI_DB`), at Tone 5
const TRIM_DB: [f32; 3] = [-7.4, -9.7, -10.5];
// Added across Tone (0, 2.5, 5, 7.5, 10): the scoop in the middle takes away what is heard
// most, and the rest is made as loud as the middle again
const TONE_DB: [f32; 5] = [-3.3, -1.05, 0.0, -1.45, -3.9];

// Crossfade to and from the untouched signal when the pedal is switched
const FADE_MS: f32 = 10.0;
// How fast Fuzz and Tone follow their dials
const DIAL_SMOOTH_MS: f32 = 20.0;
const DIAL_SETTLED: f32 = 1e-5;

// The pedal works through a block in pieces of at most this many samples at the rate it
// is called at
const PIECE: usize = 32;

/// What the knobs say, read once per call. The pedal smooths the values itself
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuzzSettings {
    pub on: bool,
    /// 0.0 to 1.0: how hard the pedal clips
    pub fuzz: f32,
    /// 0.0 to 1.0: dark to bright, with the mids scooped in the middle
    pub tone: f32,
}

impl Default for FuzzSettings {
    fn default() -> Self {
        Self { on: false, fuzz: 0.5, tone: 0.5 }
    }
}

/// Where the dials stand for one sample at the rate the pedal is called at
#[derive(Clone, Copy, Default)]
struct Step {
    gain: f32,
    tone: f32,
    trim: f32,
    mix: f32,
}

/// One channel of the pedal, at the rate it clips at
struct Voice {
    input_highpass: OnePoleHp,
    input_lowpass: OnePoleLp,
    clipper: AsymClipper,
    // The average of the clipped signal, which shifts the bias
    bias: f32,
    tone_lowpass: OnePoleLp,
    tone_highpass: OnePoleHp,
    fizz: [Biquad; 2],
    dc: OnePoleHp,
}

impl Voice {
    fn new() -> Self {
        // Antialiased to the second order: this clips harder than anything else in the chain
        let mut clipper = AsymClipper::new();
        clipper.set_limits(CLIP_LEVEL[0], CLIP_LEVEL[1]);
        clipper.set_second_order(true);
        clipper.reset(0.0);
        Self {
            input_highpass: OnePoleHp::new(),
            input_lowpass: OnePoleLp::new(),
            clipper,
            bias: 0.0,
            tone_lowpass: OnePoleLp::new(),
            tone_highpass: OnePoleHp::new(),
            fizz: [Biquad::new(); 2],
            dc: OnePoleHp::new(),
        }
    }

    fn set_sample_rate(&mut self, rate: f32) {
        self.input_highpass.set(INPUT_HIGHPASS_HZ, rate);
        self.input_lowpass.set(INPUT_LOWPASS_HZ, rate);
        self.tone_lowpass.set(TONE_LOWPASS_HZ, rate);
        self.tone_highpass.set(TONE_HIGHPASS_HZ, rate);
        for (section, q) in self.fizz.iter_mut().zip(FIZZ_Q) {
            section.set(BiquadCoeffs::lowpass(FIZZ_HZ, q, rate));
        }
        self.dc.set(DC_HZ, rate);
        self.reset();
    }

    fn reset(&mut self) {
        self.input_highpass.reset();
        self.input_lowpass.reset();
        self.clipper.reset(0.0);
        self.bias = 0.0;
        self.tone_lowpass.reset();
        self.tone_highpass.reset();
        for section in &mut self.fizz {
            section.reset();
        }
        self.dc.reset();
    }

    /// The pedal's own signal for `block`, in place: `factor` samples to each step.
    /// `bias_coeff` is for a pair of samples
    fn process(&mut self, block: &mut [f32], steps: &[Step], factor: usize, bias_coeff: f32) {
        for (index, sample) in block.iter_mut().enumerate() {
            let conditioned = self.input_lowpass.process(self.input_highpass.process(*sample));
            *sample = conditioned * steps[index / factor].gain;
        }
        // Two samples at a time through the clipper (see `AsymClipper::process_pair`), and
        // the bias moves once per pair. A block is always of an even length here: the amp's
        // oversampling or the pedal's own make it so
        debug_assert!(block.len().is_multiple_of(2));
        for pair in block.as_chunks_mut::<2>().0 {
            let bias = -BIAS_PULL * self.bias;
            let driven = Pair::new((pair[0] + bias) as f64, (pair[1] + bias) as f64);
            let clipped = self.clip_pair(driven);
            (pair[0], pair[1]) = (clipped.first() as f32, clipped.second() as f32);
            self.bias += bias_coeff * (0.5 * (pair[0] + pair[1]) - self.bias) + ANTI_DENORMAL;
        }
        for (index, sample) in block.iter_mut().enumerate() {
            *sample = self.finish(*sample, &steps[index / factor]);
        }
    }

    /// Kept out of the loop in `process`, as the drive pedal's: see `Drive::clip_pair`
    #[inline(never)]
    fn clip_pair(&mut self, driven: Pair) -> Pair {
        self.clipper.process_pair(driven)
    }

    /// Everything behind the clipping, for one sample
    fn finish(&mut self, clipped: f32, step: &Step) -> f32 {
        let dark = self.tone_lowpass.process(clipped);
        let thin = self.tone_highpass.process(clipped);
        let toned = dark + step.tone * (thin - dark);
        let half = self.fizz[0].process(toned as f64);
        let smooth = self.fizz[1].process(half) as f32;
        self.dc.process(smooth) * step.trim
    }
}

pub struct Fuzz {
    voices: [Voice; 2],
    // Behind the cabinet only: one per channel
    oversamplers: [Oversampler; 2],
    // Samples the pedal clips at per sample it is called with: 1 in front of the amp, whose
    // oversampling it is inside
    factor: usize,
    bias_coeff: f32,

    // Where the dials are, on their way to the settings, and what they make of it
    fuzz: f32,
    tone: f32,
    gain: f32,
    trim: f32,
    // Share of the pedal in its output: 0.0 switched off, 1.0 on
    mix: f32,

    dial_coeff: f32,
    fade_step: f32,

    // Off and faded out: the signal is left alone
    idle: bool,
}

impl Fuzz {
    pub fn new() -> Self {
        let defaults = FuzzSettings::default();
        let mut fuzz = Self {
            voices: [Voice::new(), Voice::new()],
            oversamplers: [Oversampler::new(), Oversampler::new()],
            factor: 1,
            bias_coeff: 1.0,
            fuzz: defaults.fuzz,
            tone: defaults.tone,
            gain: gain_at(defaults.fuzz),
            trim: trim_at(defaults.fuzz, defaults.tone),
            mix: 0.0,
            dial_coeff: 1.0,
            fade_step: 1.0,
            idle: true,
        };
        fuzz.set_sample_rate(176_400.0, false);
        fuzz
    }

    /// The rate the pedal is called at, and whether it raises that rate itself for the
    /// clipping (behind the cabinet) or is called at a raised rate already (in front of the
    /// amp). Never allocates
    pub fn set_sample_rate(&mut self, sample_rate: f32, oversampled_here: bool) {
        self.factor = if oversampled_here { factor_for(sample_rate) } else { 1 };
        for oversampler in &mut self.oversamplers {
            oversampler.set_factor(factor_for(sample_rate));
        }
        let clip_rate = sample_rate * self.factor as f32;
        for voice in &mut self.voices {
            voice.set_sample_rate(clip_rate);
        }
        // Once per pair of samples
        self.bias_coeff = smoothing_coeff(BIAS_MS, 0.5 * clip_rate);
        self.dial_coeff = smoothing_coeff(DIAL_SMOOTH_MS, sample_rate);
        self.fade_step = 1.0 / (FADE_MS * 0.001 * sample_rate);
        self.reset();
    }

    /// As after a long time switched off. A pedal that is on fades in again
    pub fn reset(&mut self) {
        self.mix = 0.0;
        self.idle = true;
    }

    /// True while the pedal is off and has faded out: the next block comes out as it goes in
    #[cfg(test)]
    pub fn is_idle(&self) -> bool {
        self.idle
    }

    /// In front of the amp: one channel, at the amp's oversampled rate
    pub fn process_mono(&mut self, settings: &FuzzSettings, block: &mut [f32]) {
        if !self.awake(settings) {
            return;
        }
        for piece in block.chunks_mut(PIECE) {
            let len = piece.len();
            let mut steps = [Step::default(); PIECE];
            self.next_steps(settings, &mut steps[..len]);
            let mut wet = [0.0f32; PIECE];
            wet[..len].copy_from_slice(piece);
            self.voices[0].process(&mut wet[..len], &steps[..len], 1, self.bias_coeff);
            mix_into(piece, &wet[..len], &steps[..len]);
            if self.idle {
                return;
            }
        }
    }

    /// Behind the cabinet: two channels at the host's rate, each raised for the clipping
    pub fn process_stereo(&mut self, settings: &FuzzSettings, left: &mut [f32], right: &mut [f32]) {
        debug_assert_eq!(left.len(), right.len());
        if !self.awake(settings) {
            return;
        }
        let factor = self.factor;
        for (left, right) in left.chunks_mut(PIECE).zip(right.chunks_mut(PIECE)) {
            let len = left.len();
            let mut steps = [Step::default(); PIECE];
            self.next_steps(settings, &mut steps[..len]);
            for (channel, block) in [left, right].into_iter().enumerate() {
                let mut high = [0.0f32; PIECE * MAX_FACTOR];
                let high = &mut high[..len * factor];
                self.oversamplers[channel].upsample(block, high);
                self.voices[channel].process(high, &steps[..len], factor, self.bias_coeff);
                let mut wet = [0.0f32; PIECE];
                self.oversamplers[channel].downsample(high, &mut wet[..len]);
                mix_into(block, &wet[..len], &steps[..len]);
            }
            if self.idle {
                return;
            }
        }
    }

    /// False if the block is to be left alone. Wakes a pedal that was idle and is switched on
    fn awake(&mut self, settings: &FuzzSettings) -> bool {
        if self.idle && !settings.on {
            return false;
        }
        if self.idle {
            // Nothing is heard yet, so there is nothing to smooth
            for voice in &mut self.voices {
                voice.reset();
            }
            for oversampler in &mut self.oversamplers {
                oversampler.reset();
            }
            self.fuzz = settings.fuzz.clamp(0.0, 1.0);
            self.tone = settings.tone.clamp(0.0, 1.0);
            self.gain = gain_at(self.fuzz);
            self.trim = trim_at(self.fuzz, self.tone);
            self.mix = 0.0;
            self.idle = false;
        }
        true
    }

    /// Moves the dials and the switch a sample at a time over `steps`
    fn next_steps(&mut self, settings: &FuzzSettings, steps: &mut [Step]) {
        let (fuzz, tone) = (settings.fuzz.clamp(0.0, 1.0), settings.tone.clamp(0.0, 1.0));
        for step in steps.iter_mut() {
            let (fuzz_moves, tone_moves) = (self.fuzz != fuzz, self.tone != tone);
            if fuzz_moves {
                self.fuzz = follow(self.fuzz, fuzz, self.dial_coeff);
                self.gain = gain_at(self.fuzz);
            }
            if tone_moves {
                self.tone = follow(self.tone, tone, self.dial_coeff);
            }
            if fuzz_moves || tone_moves {
                self.trim = trim_at(self.fuzz, self.tone);
            }
            if settings.on {
                self.mix = (self.mix + self.fade_step).min(1.0);
            } else {
                self.mix = (self.mix - self.fade_step).max(0.0);
                self.idle = self.mix == 0.0;
            }
            *step = Step { gain: self.gain, tone: self.tone, trim: self.trim, mix: self.mix };
        }
    }
}

/// The gain into the clipper at a Fuzz position
fn gain_at(fuzz: f32) -> f32 {
    db_to_gain(lerp(FUZZ_DB[0], FUZZ_DB[1], fuzz))
}

/// The level the pedal's output is set to at a Fuzz and a Tone position
fn trim_at(fuzz: f32, tone: f32) -> f32 {
    db_to_gain(curve(&TRIM_DB, fuzz) + curve(&TONE_DB, tone))
}

/// The pedal's signal `wet` into `block`, as much of it as the switch says
fn mix_into(block: &mut [f32], wet: &[f32], steps: &[Step]) {
    for ((sample, &wet), step) in block.iter_mut().zip(wet).zip(steps) {
        if step.mix >= 1.0 {
            *sample = wet;
        } else if step.mix > 0.0 {
            *sample += step.mix * (wet - *sample);
        }
    }
}

/// A step of a one-pole follower that arrives
fn follow(value: f32, target: f32, coeff: f32) -> f32 {
    let next = value + coeff * (target - value);
    if (target - next).abs() < DIAL_SETTLED {
        target
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    const SAMPLE_RATE: f32 = 44_100.0;

    /// The pedal behind the cabinet, on, with its dials where they are going
    fn new_fuzz(sample_rate: f32) -> Fuzz {
        let mut pedal = Fuzz::new();
        pedal.set_sample_rate(sample_rate, true);
        pedal
    }

    fn on(fuzz: f32, tone: f32) -> FuzzSettings {
        FuzzSettings { on: true, fuzz, tone }
    }

    /// `input` through the pedal behind the cabinet, in blocks of `block`: the left channel
    fn run_in(settings: &FuzzSettings, input: &[f32], sample_rate: f32, block: usize) -> Vec<f32> {
        let mut pedal = new_fuzz(sample_rate);
        let mut left = input.to_vec();
        let mut right = input.to_vec();
        for (left, right) in left.chunks_mut(block).zip(right.chunks_mut(block)) {
            pedal.process_stereo(settings, left, right);
        }
        left
    }

    fn run(settings: &FuzzSettings, input: &[f32]) -> Vec<f32> {
        run_in(settings, input, SAMPLE_RATE, 64)
    }

    /// Non-harmonic energy against the whole of a sine through the pedal, in dB
    fn aliasing_db(settings: &FuzzSettings, freq_hz: f32, sample_rate: f32) -> f64 {
        let len = (0.75 * sample_rate) as usize;
        let output = run_in(settings, &sine(freq_hz, 0.178, sample_rate, len), sample_rate, 64);
        fit_partials(&output[len / 3..], sample_rate, &harmonics_of(freq_hz, sample_rate)).1
    }

    #[test]
    fn test_switched_off_it_leaves_the_signal_alone() {
        let input = sine(220.0, 0.3, SAMPLE_RATE, 4800);
        let off = FuzzSettings { on: false, ..on(1.0, 1.0) };
        assert_eq!(run(&off, &input), input);
        let mut pedal = Fuzz::new();
        pedal.set_sample_rate(176_400.0, false);
        let mut output = input.clone();
        pedal.process_mono(&off, &mut output);
        assert!(pedal.is_idle());
        assert_eq!(output, input);
    }

    #[test]
    fn test_clipping_is_antialiased() {
        // The pedal alone behind the cabinet, at the lowest rate and at one where the rate is
        // only doubled: everything that is not a harmonic of the note at least 80 dB under it
        for (fuzz, tone, sample_rate) in [
            (1.0, 1.0, SAMPLE_RATE),
            (1.0, 0.5, SAMPLE_RATE),
            (1.0, 0.0, SAMPLE_RATE),
            (0.5, 1.0, SAMPLE_RATE),
            (0.0, 1.0, SAMPLE_RATE),
            (1.0, 0.5, 48_000.0),
            (1.0, 1.0, 96_000.0),
        ] {
            for freq_hz in [1245.0, 4186.0] {
                let aliasing = aliasing_db(&on(fuzz, tone), freq_hz, sample_rate);
                assert!(
                    aliasing < -80.0,
                    "Fuzz {} Tone {} at {sample_rate} Hz: {aliasing:.1} dB at {freq_hz} Hz",
                    fuzz * 10.0,
                    tone * 10.0
                );
            }
        }
    }

    #[test]
    fn test_fuzz_saturates_and_sustains() {
        // A note dying away over 40 dB comes out falling far less: the fuzz holds it up
        let len = (2.0 * SAMPLE_RATE) as usize;
        let mut note = sine(196.0, 0.25, SAMPLE_RATE, len);
        let fall = (0.01f32).powf(1.0 / len as f32);
        let mut gain = 1.0;
        for sample in &mut note {
            *sample *= gain;
            gain *= fall;
        }
        let output = run(&on(0.7, 0.5), &note);
        let quarter = len / 4;
        let early = rms(&output[quarter / 2..quarter]);
        let late = rms(&output[3 * quarter..]);
        assert!(to_db(early / late) < 15.0, "{:.1} dB down", to_db(early / late));
        // And distorts heavily even at Fuzz 0
        let held = sine(196.0, 0.1, SAMPLE_RATE, len);
        let distortion = thd_db(&run(&on(0.0, 0.5), &held)[len / 2..], SAMPLE_RATE, 196.0);
        assert!(distortion > -15.0, "Fuzz 0: {distortion:.1} dB");
    }

    #[test]
    fn test_tone_goes_from_dark_to_bright() {
        let input = power_chords(SAMPLE_RATE, 0.5);
        let top = |tone: f32| {
            let output = run(&on(0.5, tone), &input);
            band_level_db(&output, SAMPLE_RATE, Some(2000.0), None)
        };
        let (dark, middle, bright) = (top(0.0), top(0.5), top(1.0));
        assert!(
            middle > dark + 3.0 && bright > middle + 3.0,
            "Above 2 kHz: {dark:.1}, {middle:.1}, {bright:.1} dB"
        );
    }

    #[test]
    fn test_it_is_about_as_loud_as_switched_off() {
        // For a guitar recorded at the level the pedal was matched at, whatever the Fuzz and the Tone
        let recording = at_rms_db(&guitar_di(SAMPLE_RATE), PEDAL_DI_DB);
        let off = loudness_lufs(&[&recording], SAMPLE_RATE);
        for fuzz in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for tone in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let output = run(&on(fuzz, tone), &recording);
                let heard = loudness_lufs(&[&output], SAMPLE_RATE) - off;
                assert!(
                    heard.abs() < 1.0,
                    "Fuzz {} Tone {}: {heard:+.1} LU",
                    fuzz * 10.0,
                    tone * 10.0
                );
            }
        }
    }

    #[test]
    fn test_switching_does_not_click() {
        let input = sine(220.0, 0.3, SAMPLE_RATE, 48_000);
        let mut pedal = new_fuzz(SAMPLE_RATE);
        let mut left = input.clone();
        let mut right = input.clone();
        for (index, (left, right)) in left.chunks_mut(64).zip(right.chunks_mut(64)).enumerate() {
            let settings = FuzzSettings { on: !(250..500).contains(&index), ..on(0.7, 0.5) };
            pedal.process_stereo(&settings, left, right);
        }
        let (off, back) = (250 * 64, 500 * 64);
        let fade = (FADE_MS * 0.001 * SAMPLE_RATE) as usize + 1;
        // Off it is the input again, to the bit, once faded
        assert_eq!(left[off + fade..back], input[off + fade..back]);
        assert!(pedal.is_idle() || rms(&left[back + fade..]) > 0.01);
        // Neither way a step larger than the signal takes by itself
        let own_step = largest_step(&left[fade..off]).max(largest_step(&input));
        for at in [off, back] {
            let step = largest_step(&left[at - 1..at + fade + 1]);
            assert!(step < own_step * 1.1, "{step} against {own_step} at {at}");
        }
    }

    #[test]
    fn test_dial_jumps_do_not_click() {
        let input = sine(220.0, 0.2, SAMPLE_RATE, 24_000);
        let mut pedal = new_fuzz(SAMPLE_RATE);
        let mut left = input.clone();
        let mut right = input.clone();
        for (index, (left, right)) in left.chunks_mut(64).zip(right.chunks_mut(64)).enumerate() {
            let settings = if index < 150 { on(0.0, 0.0) } else { on(1.0, 1.0) };
            pedal.process_stereo(&settings, left, right);
        }
        let at = 150 * 64;
        let own_step = largest_step(&left[at + 4000..]).max(largest_step(&left[..at]));
        assert!(largest_step(&left[at - 1..at + 2000]) < own_step * 1.1);
    }

    #[test]
    fn test_output_is_bounded_and_finite() {
        for sample_rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            let input = sine(300.0, 4.0, sample_rate, 9600);
            for (fuzz, tone) in [(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0)] {
                let output = run_in(&on(fuzz, tone), &input, sample_rate, 64);
                assert!(output.iter().all(|sample| sample.is_finite()));
                // Once faded in: during the fade the input is heard as it is
                let faded = (FADE_MS * 0.001 * sample_rate) as usize;
                let peak = peak(&output[faded..]);
                assert!(peak < 1.0, "Peak {peak} at {sample_rate} Hz");
            }
        }
    }

    #[test]
    fn test_output_does_not_depend_on_block_size() {
        let mut input = power_chords(SAMPLE_RATE, 0.25);
        input.extend([0.0; 4801]);
        let settings = on(0.8, 0.3);
        let whole = run_in(&settings, &input, SAMPLE_RATE, 32);
        assert!(whole.iter().all(|sample| sample.is_finite()));
        assert!(run_in(&settings, &input, SAMPLE_RATE, 7) == whole);
        assert!(run_in(&settings, &input, SAMPLE_RATE, 1) == whole);
    }

    #[test]
    fn test_in_front_of_the_amp_it_works_at_the_rate_it_is_given() {
        // At the amp's oversampled rate, without raising it again: the same tone and level
        // as behind the cabinet, within a little
        let input = power_chords(SAMPLE_RATE, 0.5);
        let behind = run(&on(0.6, 0.5), &input);

        let factor = factor_for(SAMPLE_RATE);
        let mut up = Oversampler::new();
        up.set_factor(factor);
        let mut high = vec![0.0; input.len() * factor];
        up.upsample(&input, &mut high);
        let mut pedal = Fuzz::new();
        pedal.set_sample_rate(SAMPLE_RATE * factor as f32, false);
        for block in high.chunks_mut(128) {
            pedal.process_mono(&on(0.6, 0.5), block);
        }
        let mut front = vec![0.0; input.len()];
        up.downsample(&high, &mut front);
        let difference = to_db(rms(&front) / rms(&behind));
        assert!(difference.abs() < 0.5, "{difference:.2} dB apart");
    }

    /// The pedal by itself: how loud it is against switched off for a guitar at the level
    /// the pedal was matched at, per Fuzz and Tone, and what it folds back, per Fuzz, Tone
    /// and rate
    #[test]
    #[ignore]
    fn fuzz_report() {
        let recording = at_rms_db(&guitar_di(SAMPLE_RATE), PEDAL_DI_DB);
        let off = loudness_lufs(&[&recording], SAMPLE_RATE);
        println!("Loudness against off, LU (rows Fuzz, columns Tone 0, 2.5, 5, 7.5, 10)");
        for fuzz in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let heard: Vec<String> = [0.0, 0.25, 0.5, 0.75, 1.0]
                .iter()
                .map(|&tone| {
                    let output = run(&on(fuzz, tone), &recording);
                    format!("{:+6.1}", loudness_lufs(&[&output], SAMPLE_RATE) - off)
                })
                .collect();
            println!("  Fuzz {:4.1}: {}", fuzz * 10.0, heard.join(" "));
        }
        // What it costs per sample of the host, at 48 kHz, on either side of the amp
        pin_thread();
        let input = power_chords(48_000.0, 4.0);
        let factor = factor_for(48_000.0);
        let mut high = vec![0.0; input.len() * factor];
        let mut up = Oversampler::new();
        up.set_factor(factor);
        up.upsample(&input, &mut high);
        let mut front = Fuzz::new();
        front.set_sample_rate(48_000.0 * factor as f32, false);
        let start = std::time::Instant::now();
        for block in high.chunks_mut(32 * factor) {
            front.process_mono(&on(1.0, 0.5), block);
        }
        let front_ns = start.elapsed().as_secs_f64() * 1e9 / input.len() as f64;
        let mut behind = new_fuzz(48_000.0);
        let (mut left, mut right) = (input.clone(), input.clone());
        let start = std::time::Instant::now();
        for (left, right) in left.chunks_mut(32).zip(right.chunks_mut(32)) {
            behind.process_stereo(&on(1.0, 0.5), left, right);
        }
        let behind_ns = start.elapsed().as_secs_f64() * 1e9 / input.len() as f64;
        std::hint::black_box((&high, &left, &right));
        println!(
            "Cost at 48 kHz, ns per sample: {front_ns:.0} in front of the amp, {behind_ns:.0} \
             behind the cabinet ({:.2} % and {:.2} % of a core)",
            front_ns * 48_000.0 * 1e-7,
            behind_ns * 48_000.0 * 1e-7
        );

        println!("Aliasing, dB (Fuzz, Tone, 1245 Hz and 4186 Hz)");
        for sample_rate in [44_100.0, 48_000.0, 96_000.0] {
            for (fuzz, tone) in [(0.0, 0.5), (0.5, 0.5), (1.0, 0.0), (1.0, 0.5), (1.0, 1.0)] {
                let low = aliasing_db(&on(fuzz, tone), 1245.0, sample_rate);
                let high = aliasing_db(&on(fuzz, tone), 4186.0, sample_rate);
                println!(
                    "  {sample_rate} Hz, Fuzz {:4.1}, Tone {:4.1}: {low:6.1} {high:6.1}",
                    fuzz * 10.0,
                    tone * 10.0
                );
            }
        }
    }
}
