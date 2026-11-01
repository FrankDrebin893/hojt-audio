//! Pitch shifter pedal: moves the guitar up or down by whole semitones in front of the amp,
//! for drop tunings, a baritone or a harmony. Three knobs, Semitones, Blend and Tracking, and
//! a switch. It is the algorithm of the Pitch Shifter plugin (`suite_common::dsp::pitch`)
//! as a stage of the chain: one channel, at the amp's oversampled rate, in front of it only.
//!
//! The pedal costs nothing while it is off, and passes the signal bit for bit. Switched on
//! it adds what the Tracking knob says at most when it shifts (the output lags the input by
//! at most that much, so that a low note has a whole period to splice on), and 2 samples of
//! the oversampled rate at a shift of 0.

use suite_common::dsp::pitch::PitchShifter;

/// The Semitones knob, from down an octave to up one
pub const SEMITONES: [i32; 2] = [-12, 12];

/// The Tracking knob: how far the output may fall behind the input, in ms. A low E has a
/// period of 12 ms, so under that a shift down splices in the middle of a period
pub const LATENCY_MS: [f32; 2] = [2.0, 50.0];

// Crossfade to and from the untouched signal when the pedal is switched
const FADE_MS: f32 = 20.0;

/// What the knobs say, read once per call
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PitchSettings {
    pub on: bool,
    /// Whole semitones, -12 to 12
    pub semitones: i32,
    /// 0.0 is the guitar as it comes in, 1.0 is the shifted guitar alone
    pub blend: f32,
    /// Tracking, in ms: the most the shifted guitar may lag behind the input
    pub latency_ms: f32,
}

impl Default for PitchSettings {
    fn default() -> Self {
        Self { on: false, semitones: 0, blend: 1.0, latency_ms: 15.0 }
    }
}

/// The pedal. Its buffers are made with it and never resized
pub struct Pitch {
    shifter: PitchShifter,
    // Share of the pedal in its output: 0.0 switched off, 1.0 on
    mix: f32,
    fade_step: f32,
    // Off and faded out: the signal is left alone
    idle: bool,
}

impl Pitch {
    pub fn new() -> Self {
        let mut pitch = Self { shifter: PitchShifter::new(), mix: 0.0, fade_step: 1.0, idle: true };
        pitch.set_sample_rate(44100.0);
        pitch
    }

    /// The rate the pedal runs at. Not for the audio thread: it clears the buffers
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.shifter.set_sample_rate(sample_rate);
        self.fade_step = 1.0 / (FADE_MS * 0.001 * sample_rate);
        self.reset();
    }

    /// As after a long time switched off. A pedal that is on fades in again, from an empty
    /// buffer (the buffer is cleared when it wakes, not here, so that this costs nothing)
    pub fn reset(&mut self) {
        self.mix = 0.0;
        self.idle = true;
    }

    /// True while the pedal is off and has faded out: the next block comes out as it goes in
    pub fn is_idle(&self) -> bool {
        self.idle
    }

    /// In front of the amp: one channel, at the amp's oversampled rate
    pub fn process_mono(&mut self, settings: &PitchSettings, block: &mut [f32]) {
        if self.idle && !settings.on {
            return;
        }
        if self.idle {
            // Nothing is heard yet: start over, so no earlier note is spliced in
            self.shifter.reset();
            self.idle = false;
        }
        self.shifter.set_semitones(settings.semitones.clamp(SEMITONES[0], SEMITONES[1]));
        self.shifter.set_blend(settings.blend);
        self.shifter.set_latency_ms(settings.latency_ms.clamp(LATENCY_MS[0], LATENCY_MS[1]));
        for sample in block.iter_mut() {
            let dry = *sample;
            let wet = self.shifter.process_sample(dry);
            let mix = self.next_mix(settings.on);
            *sample = if mix >= 1.0 { wet } else { dry + mix * (wet - dry) };
            if self.idle {
                // Faded out in the middle of the block: the rest of it is the dry input
                return;
            }
        }
    }

    /// Address and capacity of every buffer
    #[cfg(test)]
    pub fn buffers(&self) -> Vec<(usize, usize)> {
        self.shifter.buffers()
    }

    /// The share of the pedal in the next sample
    #[inline]
    fn next_mix(&mut self, on: bool) -> f32 {
        if on {
            if self.mix < 1.0 {
                self.mix = (self.mix + self.fade_step).min(1.0);
            }
        } else {
            self.mix = (self.mix - self.fade_step).max(0.0);
            self.idle = self.mix == 0.0;
        }
        self.mix
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::test_util::*;

    pub const RATE: f32 = 192_000.0;
    const BLOCK: usize = 128;

    pub fn on(semitones: i32) -> PitchSettings {
        PitchSettings { on: true, semitones, ..PitchSettings::default() }
    }

    /// The signal through a pedal in blocks of `BLOCK`
    pub fn run(settings: &PitchSettings, input: &[f32]) -> Vec<f32> {
        let mut pitch = Pitch::new();
        pitch.set_sample_rate(RATE);
        let mut output = input.to_vec();
        for block in output.chunks_mut(BLOCK) {
            pitch.process_mono(settings, block);
        }
        output
    }

    /// The frequency of a steady tone, from the zero crossings of its second half
    fn frequency(signal: &[f32]) -> f32 {
        let tail = &signal[signal.len() / 2..];
        let crossings = tail.windows(2).filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0).count();
        crossings as f32 / (tail.len() as f32 / RATE)
    }

    #[test]
    fn test_off_leaves_the_signal_to_the_bit() {
        let input = sine(110.0, 0.5, RATE, 20_000);
        let settings = PitchSettings { semitones: -5, ..PitchSettings::default() };
        assert!(run(&settings, &input) == input);
    }

    #[test]
    fn test_a_note_comes_out_the_semitones_away() {
        let input = sine(110.0, 0.5, RATE, 96_000);
        for semitones in [-12, -7, -2, 2, 5, 12] {
            let output = run(&on(semitones), &input);
            let wanted = 110.0 * 2.0f32.powf(semitones as f32 / 12.0);
            let heard = frequency(&output);
            assert!(
                (heard / wanted - 1.0).abs() < 0.03,
                "{semitones} semitones: {heard} Hz, wanted {wanted} Hz"
            );
        }
    }

    #[test]
    fn test_a_shift_of_nothing_passes_the_signal_on_a_few_samples_late() {
        let input = sine(220.0, 0.5, RATE, 20_000);
        let output = run(&on(0), &input);
        // Once it has faded in, the shifter reads its buffer back bit-exactly 2 samples behind
        // the input
        let settled = 4000;
        assert!(output[settled..] == input[settled - 2..input.len() - 2]);
    }

    #[test]
    fn test_switching_on_and_off_does_not_click() {
        let input = sine(110.0, 0.5, RATE, 60_000);
        let mut pitch = Pitch::new();
        pitch.set_sample_rate(RATE);
        let mut output = input.clone();
        for (index, block) in output.chunks_mut(BLOCK).enumerate() {
            let settings = PitchSettings { on: (index / 100) % 2 == 1, ..on(-5) };
            pitch.process_mono(&settings, block);
        }
        // A sine of 110 Hz moves 0.0018 per sample at most at this rate and level
        assert!(largest_step(&output) < 0.05, "step {}", largest_step(&output));
    }

    #[test]
    fn test_switched_off_the_pedal_is_idle_once_it_has_faded() {
        let input = sine(110.0, 0.5, RATE, 20_000);
        let mut pitch = Pitch::new();
        pitch.set_sample_rate(RATE);
        let mut block = input[..BLOCK].to_vec();
        pitch.process_mono(&on(5), &mut block);
        assert!(!pitch.is_idle());
        let off = PitchSettings::default();
        for _ in 0..40 {
            pitch.process_mono(&off, &mut block);
        }
        assert!(pitch.is_idle());
    }

    // What the report plays: a clean amp, so that the pedal's own doing is what is read
    fn report_amp(pitch: PitchSettings) -> crate::chain::AmpSettings {
        crate::chain::AmpSettings {
            gate_on: false,
            amp: crate::Amp::Klar,
            gain: 0.2,
            pitch,
            ..crate::chain::AmpSettings::default()
        }
    }

    fn through_amp(settings: &crate::chain::AmpSettings, input: &[f32], rate: f32) -> Vec<f32> {
        let mut chain = crate::chain::AmpChain::new();
        chain.set_sample_rate(rate);
        let mut output = input.to_vec();
        for block in output.chunks_mut(64) {
            chain.process(settings, block, None);
        }
        output
    }

    /// First sample over a tenth of the signal's own peak, in ms
    fn onset_ms(signal: &[f32], rate: f32) -> f32 {
        let top = peak(signal);
        signal.iter().position(|sample| sample.abs() > 0.1 * top).unwrap_or(0) as f32 / rate
            * 1000.0
    }

    /// Prints what the pedal does to the amp, switched off and on at -12, 0 and +12 semitones.
    /// Run before and after a change:
    ///   cargo test -p amp --release pitch_report -- --ignored --nocapture
    #[test]
    #[ignore]
    fn pitch_report() {
        use crate::test_util::{pluck, Pluck};
        pin_thread();
        const FS: f32 = 48_000.0;
        println!("Pitch shifter pedal in front of Klar (Gain 2, gate off) at {FS} Hz.");
        println!();
        println!("Response: level of the note that comes out, in dB against the amp playing that");
        println!("note itself with the pedal off, for a sine going in at the left. Blend 100 %.");
        println!();
        print!("{:>10}", "in Hz");
        let shifts = [-12, 0, 12];
        for semitones in shifts {
            print!("{:>10}", format!("{semitones:+} st"));
        }
        println!();
        for freq in [82.41, 110.0, 164.81, 220.0, 329.63, 440.0, 659.26] {
            print!("{freq:>10.1}");
            for semitones in shifts {
                let wanted = freq * 2.0f32.powf(semitones as f32 / 12.0);
                if wanted < 60.0 || wanted > 1500.0 {
                    print!("{:>10}", "-");
                    continue;
                }
                let input = sine(freq, 0.05, FS, 96_000);
                let shifted = through_amp(&report_amp(on(semitones)), &input, FS);
                let itself = through_amp(
                    &report_amp(PitchSettings::default()),
                    &sine(wanted, 0.05, FS, 96_000),
                    FS,
                );
                let level = |signal: &[f32]| level_at(&signal[48_000..], FS, wanted);
                print!("{:>10.1}", to_db(level(&shifted) / level(&itself)));
            }
            println!();
        }

        println!();
        println!(
            "Latency: how much later a plucked 110 Hz string is heard to start (a tenth of its"
        );
        println!("peak) than with the pedal off, in ms, per Tracking. Blend 100 %.");
        println!();
        print!("{:>10}", "Tracking");
        for semitones in shifts {
            print!("{:>10}", format!("{semitones:+} st"));
        }
        println!();
        let string = pluck(110.0, Pluck::OPEN, 3, FS, 48_000);
        let mut note = vec![0.0; 4_800];
        note.extend(string.iter().map(|sample| 0.3 * sample));
        let plain = onset_ms(&through_amp(&report_amp(PitchSettings::default()), &note, FS), FS);
        for latency_ms in [2.0, 15.0, 50.0] {
            print!("{latency_ms:>8.0} ms");
            for semitones in shifts {
                let settings = report_amp(PitchSettings { latency_ms, ..on(semitones) });
                let later = onset_ms(&through_amp(&settings, &note, FS), FS) - plain;
                print!("{later:>10.2}");
            }
            println!();
        }

        println!();
        println!(
            "Cost: Torden playing power chords in blocks of 64, median and 99th-percentile block as a share"
        );
        println!(
            "of the time the block lasts, per sample rate (the pedal runs at 4x and 2x those)"
        );
        println!();
        print!("{:>10}", "rate Hz");
        print!("{:>14}", "off");
        for semitones in shifts {
            print!("{:>14}", format!("{semitones:+} st"));
        }
        println!("   (median % of a core / 99th percentile %)");
        for rate in [48_000.0, 96_000.0, 192_000.0] {
            let chords = power_chords(rate, 2.0);
            print!("{rate:>10.0}");
            for settings in [PitchSettings::default(), on(-12), on(0), on(12)] {
                let amp = crate::chain::AmpSettings {
                    amp: crate::Amp::Torden,
                    pitch: settings,
                    ..crate::chain::AmpSettings::default()
                };
                let mut chain = crate::chain::AmpChain::new();
                chain.set_sample_rate(rate);
                let mut scratch = chords.clone();
                // Two passes: the first warms the caches, the second is timed
                for block in scratch.chunks_mut(64) {
                    chain.process(&amp, block, None);
                }
                let mut times: Vec<f64> = chords
                    .chunks(64)
                    .map(|block| {
                        let mut block = block.to_vec();
                        let start = std::time::Instant::now();
                        chain.process(&amp, &mut block, None);
                        start.elapsed().as_secs_f64()
                    })
                    .collect();
                times.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let lasts = 64.0 / rate as f64;
                print!(
                    "{:>14}",
                    format!(
                        "{:.1} / {:.1}",
                        100.0 * times[times.len() / 2] / lasts,
                        100.0 * times[times.len() * 99 / 100] / lasts
                    )
                );
            }
            println!();
        }
    }
}
