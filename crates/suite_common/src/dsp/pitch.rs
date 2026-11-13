//! The pitch shifter of the Pitch Shifter plugin and of the Guitar Amp's pitch pedal.
//!
//! `PitchShifter` is what the plugins hold: the dry signal, the blend, the switch to and from a
//! shift of nothing, and the step down to the rate the engine works at. The shifting itself is
//! in `spectral`.

mod spectral;

use spectral::{Engine, CHUNKS};

/// How far the dry signal, and with it a shift of nothing, lags the input, in samples
const MIN_DELAY: usize = 2;

/// The engine runs at the host's rate divided by a whole number, as near to this as that gets
const ENGINE_RATE: f32 = 48000.0;

/// Largest such number (768 kHz)
const MAX_FACTOR: usize = 16;

/// Length of the filter that steps the rate down and up again: this many taps per factor,
/// plus one. Its delay is half of that, each way
const TAPS_PER_FACTOR: usize = 12;
const MAX_TAPS: usize = TAPS_PER_FACTOR * MAX_FACTOR + 1;

/// Where the filter is 6 dB down, as a share of the engine's rate (21 kHz at 48 kHz)
const FILTER_CUTOFF: f32 = 0.44;

const HIGH_LEN: usize = 256;
const HIGH_MASK: usize = HIGH_LEN - 1;
const LOW_LEN: usize = 32;
const LOW_MASK: usize = LOW_LEN - 1;

/// One channel: its engine, the last few samples for the dry signal, and the history of the
/// filters around the engine
struct Channel {
    engine: Engine,
    dry: [f32; 4],
    // Input at the host's rate, for stepping down
    high: Vec<f32>,
    // The engine's output, for stepping up
    low: Vec<f32>,
    low_pos: usize,
}

impl Channel {
    fn new() -> Self {
        Self {
            engine: Engine::new(),
            dry: [0.0; 4],
            high: vec![0.0; HIGH_LEN],
            low: vec![0.0; LOW_LEN],
            low_pos: 0,
        }
    }

    fn reset(&mut self) {
        self.engine.reset();
        self.dry = [0.0; 4];
        self.high.fill(0.0);
        self.low.fill(0.0);
        self.low_pos = 0;
    }
}

/// A pitch shifter for chords as well as single notes, with a fixed latency.
///
/// The shifted signal lags the input by a fixed time, the largest of 5, 11, 21 and 43 ms that
/// `set_latency_ms` allows (the longer, the cleaner). A shift of nothing passes the input
/// through bit for bit, `MIN_DELAY` samples late, and so does the dry part of the blend.
pub struct PitchShifter {
    channels: [Channel; 2],

    // Samples taken in so far
    pos: usize,

    semitones: i32,
    // How much of the output is the shifted signal (1.0) rather than the dry input (0.0)
    blend: f32,

    sample_rate: f32,
    latency_ms: f32,
    smoothness_ms: f32,

    // The engine's rate is the host's divided by this
    factor: usize,
    taps: Vec<f32>,
    num_taps: usize,

    // The engines are working: there is a shift, or one is fading out
    running: bool,
    // Samples until the engines' first output after a start has come through
    warm_up: usize,
    // Share of the engines in the shifted signal, the rest being the dry input: 0.0 at a
    // shift of nothing, 1.0 once a shift has faded in
    heard: f32,
    fade_step: f32,
    // The chunk length the latency setting asks for, as an index into `CHUNKS`
    window: usize,
}

impl Default for PitchShifter {
    fn default() -> Self {
        Self::new()
    }
}

impl PitchShifter {
    pub fn new() -> Self {
        let mut shifter = Self {
            channels: [Channel::new(), Channel::new()],
            pos: 0,
            semitones: 0,
            blend: 1.0,
            sample_rate: 44100.0,
            latency_ms: 15.0,
            smoothness_ms: 2.0,
            factor: 1,
            taps: vec![0.0; MAX_TAPS],
            num_taps: 1,
            running: false,
            warm_up: 0,
            heard: 0.0,
            fade_step: 1.0,
            window: 0,
        };
        shifter.update_timing();
        shifter.reset();
        shifter
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.factor = ((sample_rate / ENGINE_RATE).round() as usize).clamp(1, MAX_FACTOR);

        // A windowed sinc (Blackman) that cuts at the engine's band, with a gain of 1
        self.num_taps = TAPS_PER_FACTOR * self.factor + 1;
        let middle = (self.num_taps - 1) as f32 / 2.0;
        let cutoff = FILTER_CUTOFF / self.factor as f32;
        let mut sum = 0.0;
        for (i, tap) in self.taps[..self.num_taps].iter_mut().enumerate() {
            let t = i as f32 - middle;
            let sinc = if t == 0.0 {
                2.0 * cutoff
            } else {
                (std::f32::consts::TAU * cutoff * t).sin() / (std::f32::consts::PI * t)
            };
            let phase = std::f32::consts::TAU * i as f32 / (self.num_taps - 1) as f32;
            *tap = sinc * (0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos());
            sum += *tap;
        }
        for tap in &mut self.taps[..self.num_taps] {
            *tap /= sum;
        }

        self.update_timing();
        self.reset();
    }

    /// Most the shifted signal may lag the input. The lag is fixed: the largest of the
    /// engine's chunk lengths that fits (the smallest, about 5 ms, where none does). A longer
    /// one is cleaner, most of all on low notes and chords
    pub fn set_latency_ms(&mut self, latency_ms: f32) {
        if latency_ms != self.latency_ms {
            self.latency_ms = latency_ms;
            self.update_timing();
        }
    }

    /// Length of the crossfade between the input and the shifted signal when a shift starts,
    /// ends, or takes another latency
    pub fn set_smoothness_ms(&mut self, smoothness_ms: f32) {
        if smoothness_ms != self.smoothness_ms {
            self.smoothness_ms = smoothness_ms;
            self.update_timing();
        }
    }

    pub fn set_semitones(&mut self, semitones: i32) {
        if semitones != self.semitones {
            self.semitones = semitones;
            let ratio = 2.0_f32.powf(semitones as f32 / 12.0);
            for channel in &mut self.channels {
                channel.engine.set_ratio(ratio);
            }
        }
    }

    /// Mix of the dry input (0.0) and the shifted one (1.0). The dry part is the input
    /// `MIN_DELAY` samples late, which is also what a shift of nothing is, so at a shift of
    /// nothing the two are the same signal and the mix leaves it as it is.
    pub fn set_blend(&mut self, blend: f32) {
        self.blend = blend.clamp(0.0, 1.0);
    }

    /// Samples the shifted signal lags the input by, at the host's rate
    pub fn latency_samples(&self) -> usize {
        let engine = CHUNKS[self.window] - 1;
        if self.factor == 1 {
            engine
        } else {
            engine * self.factor + self.num_taps - 1
        }
    }

    /// Address and capacity of every buffer, for tests that hold a host's audio thread to
    /// never allocating or resizing one
    pub fn buffers(&self) -> Vec<(usize, usize)> {
        let mut all = vec![(self.taps.as_ptr() as usize, self.taps.capacity())];
        for channel in &self.channels {
            all.push((channel.high.as_ptr() as usize, channel.high.capacity()));
            all.push((channel.low.as_ptr() as usize, channel.low.capacity()));
            all.extend(channel.engine.buffers());
        }
        all
    }

    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
        self.pos = 0;
        self.running = false;
        self.warm_up = 0;
        self.heard = 0.0;
    }

    /// Converts the parameters to sample counts for the current sample rate
    fn update_timing(&mut self) {
        let engine_rate = self.sample_rate / self.factor as f32;
        let filters = if self.factor == 1 { 0.0 } else { (self.num_taps - 1) as f32 };
        let lag_ms =
            |chunk: usize| ((chunk - 1) as f32 / engine_rate + filters / self.sample_rate) * 1000.0;
        self.window =
            (0..CHUNKS.len()).rev().find(|&i| lag_ms(CHUNKS[i]) <= self.latency_ms).unwrap_or(0);
        self.fade_step = 1.0 / (self.smoothness_ms * 0.001 * self.sample_rate).max(1.0);
    }

    /// Processes one mono sample
    pub fn process_sample(&mut self, input: f32) -> f32 {
        self.process_frame([input, 0.0], 1)[0]
    }

    /// Processes one stereo frame. Each channel is shifted on its own, by the same amount
    /// and with the same latency.
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let output = self.process_frame([left, right], 2);
        (output[0], output[1])
    }

    fn process_frame(&mut self, input: [f32; 2], channels: usize) -> [f32; 2] {
        // A shift is wanted, and at the latency the engines have
        let shifting = self.semitones != 0;
        let settled = self.channels[0].engine.window() == self.window;
        if !self.running && shifting {
            // Start from nothing heard: the engines begin a new output at the latency asked
            // for, and are faded in once their first chunk has come through
            for channel in &mut self.channels {
                channel.engine.set_window(self.window);
            }
            self.running = true;
            self.warm_up = self.latency_samples() + 1;
        }

        let phase = self.pos % self.factor;
        let mut output = [0.0; 2];
        for (index, channel) in self.channels[..channels].iter_mut().enumerate() {
            let dry = channel.dry[self.pos.wrapping_sub(MIN_DELAY) & 3];
            channel.dry[self.pos & 3] = input[index];

            let wet = if self.factor == 1 {
                if self.running {
                    channel.engine.process(input[index])
                } else {
                    channel.engine.feed(input[index]);
                    0.0
                }
            } else {
                channel.high[self.pos & HIGH_MASK] = input[index];
                if phase == 0 {
                    let mut low = 0.0;
                    for (i, tap) in self.taps[..self.num_taps].iter().enumerate() {
                        low += tap * channel.high[self.pos.wrapping_sub(i) & HIGH_MASK];
                    }
                    channel.low_pos = (channel.low_pos + 1) & LOW_MASK;
                    channel.low[channel.low_pos] = if self.running {
                        channel.engine.process(low)
                    } else {
                        channel.engine.feed(low);
                        0.0
                    };
                }
                // Every sample of the engine's is `factor` of the host's: the taps a whole
                // number of them apart meet the samples there are
                let mut high = 0.0;
                let mut tap = phase;
                let mut back = 0;
                while tap < self.num_taps {
                    high +=
                        self.taps[tap] * channel.low[channel.low_pos.wrapping_sub(back) & LOW_MASK];
                    tap += self.factor;
                    back += 1;
                }
                high * self.factor as f32
            };

            // At a shift of nothing, faded out, the output is the dry input, bit for bit
            let shifted = if self.heard <= 0.0 {
                dry
            } else if self.heard >= 1.0 {
                wet
            } else {
                dry + self.heard * (wet - dry)
            };
            output[index] = if self.blend <= 0.0 {
                dry
            } else if self.blend < 1.0 && self.heard > 0.0 {
                (1.0 - self.blend) * dry + self.blend * shifted
            } else {
                shifted
            };
        }
        self.pos = self.pos.wrapping_add(1);

        if self.running {
            if self.warm_up > 0 {
                self.warm_up -= 1;
            } else if shifting && settled {
                self.heard = (self.heard + self.fade_step).min(1.0);
            } else {
                self.heard = (self.heard - self.fade_step).max(0.0);
                // Faded out: the engines rest, and start over when there is a shift again
                self.running = self.heard > 0.0;
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI as PI64;

    const SAMPLE_RATE: f32 = 44100.0;

    // Test signals are one second long. Measurements skip the first third, so the shifter
    // has settled on the note.
    const NUM_SAMPLES: usize = 44100;
    const SETTLE: usize = 14700;

    // What a measurement has to stay under, in dB of artifacts
    const SINE_CLEAN: f64 = -45.0;
    const TONE_CLEAN: f64 = -40.0;
    const STRING_CLEAN: f64 = -40.0;
    const CHORD_CLEAN: f64 = -18.0;

    fn new_shifter(semitones: i32) -> PitchShifter {
        let mut shifter = PitchShifter::new();
        shifter.set_sample_rate(SAMPLE_RATE);
        shifter.set_semitones(semitones);
        shifter
    }

    fn shift_ratio(semitones: i32) -> f64 {
        2.0_f64.powf(semitones as f64 / 12.0)
    }

    fn generate_sine_wave(frequency: f64, sample_rate: f32, num_samples: usize) -> Vec<f32> {
        (0..num_samples)
            .map(|i| (2.0 * PI64 * frequency * i as f64 / sample_rate as f64).sin() as f32)
            .collect()
    }

    /// Steady tone with harmonics falling off as 1/n
    fn generate_harmonic_tone(frequency: f64, harmonics: usize, num_samples: usize) -> Vec<f32> {
        let scale: f64 = (1..=harmonics).map(|h| 1.0 / h as f64).sum();
        (0..num_samples)
            .map(|i| {
                let phase = 2.0 * PI64 * frequency * i as f64 / SAMPLE_RATE as f64;
                let sum: f64 = (1..=harmonics).map(|h| (phase * h as f64).sin() / h as f64).sum();
                (sum / scale) as f32
            })
            .collect()
    }

    fn generate_chord(notes: &[f64], harmonics: usize, num_samples: usize) -> Vec<f32> {
        let mut chord = vec![0.0; num_samples];
        for &note in notes {
            for (sum, sample) in
                chord.iter_mut().zip(generate_harmonic_tone(note, harmonics, num_samples))
            {
                *sum += sample / notes.len() as f32;
            }
        }
        chord
    }

    fn generate_noise(num_samples: usize) -> Vec<f32> {
        let mut seed = 987654321_u32;
        (0..num_samples)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 9) as f32 / (1 << 22) as f32 - 1.0
            })
            .collect()
    }

    fn process_buffer(shifter: &mut PitchShifter, input: &[f32]) -> Vec<f32> {
        input.iter().map(|&s| shifter.process_sample(s)).collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// The partials a shifted tone should consist of
    fn shifted_partials(
        notes: &[f64],
        harmonics: usize,
        semitones: i32,
        sample_rate: f32,
    ) -> Vec<f64> {
        let mut partials: Vec<f64> = notes
            .iter()
            .flat_map(|&note| (1..=harmonics).map(move |h| note * h as f64))
            .map(|frequency| frequency * shift_ratio(semitones))
            .filter(|&frequency| frequency < sample_rate as f64 * 0.45)
            .collect();
        partials.sort_by(|a, b| a.partial_cmp(b).unwrap());
        partials.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        partials
    }

    /// Energy that is not at any of the expected frequencies, relative to the total, in dB.
    /// Lower is cleaner. Each expected partial is fitted by least squares and removed; what
    /// remains is the sidebands, clicks and noise the shifter added.
    fn artifact_ratio_db(signal: &[f32], sample_rate: f32, partials: &[f64]) -> f64 {
        let len = signal.len();
        let window: Vec<f64> =
            (0..len).map(|i| 0.5 - 0.5 * (2.0 * PI64 * i as f64 / len as f64).cos()).collect();
        let mut residual: Vec<f64> = signal.iter().map(|&s| s as f64).collect();
        let energy =
            |samples: &[f64]| -> f64 { samples.iter().zip(&window).map(|(s, w)| w * s * s).sum() };
        let total = energy(&residual);

        for _ in 0..2 {
            for &frequency in partials {
                let omega = 2.0 * PI64 * frequency / sample_rate as f64;
                let (mut cc, mut ss, mut cs, mut yc, mut ys) = (0.0, 0.0, 0.0, 0.0, 0.0);
                for i in 0..len {
                    let (sin, cos) = (omega * i as f64).sin_cos();
                    cc += window[i] * cos * cos;
                    ss += window[i] * sin * sin;
                    cs += window[i] * cos * sin;
                    yc += window[i] * residual[i] * cos;
                    ys += window[i] * residual[i] * sin;
                }
                let det = cc * ss - cs * cs;
                let a = (yc * ss - ys * cs) / det;
                let b = (ys * cc - yc * cs) / det;
                for i in 0..len {
                    let (sin, cos) = (omega * i as f64).sin_cos();
                    residual[i] -= a * cos + b * sin;
                }
            }
        }

        10.0 * (energy(&residual) / total).max(1e-20).log10()
    }

    /// Shifts a steady tone and measures how much of the output is not the shifted tone
    fn measure_artifacts(notes: &[f64], harmonics: usize, semitones: i32, latency_ms: f32) -> f64 {
        let mut shifter = new_shifter(semitones);
        shifter.set_latency_ms(latency_ms);
        let output = process_buffer(&mut shifter, &generate_chord(notes, harmonics, NUM_SAMPLES));
        let partials = shifted_partials(notes, harmonics, semitones, SAMPLE_RATE);
        artifact_ratio_db(&output[SETTLE..], SAMPLE_RATE, &partials)
    }

    /// The notes of chords as a guitar plays them: equal temperament, so no two notes share
    /// a period
    const E_POWER: [f64; 3] = [82.41, 123.47, 164.81];
    const E_MAJOR: [f64; 6] = [82.41, 123.47, 164.81, 207.65, 246.94, 329.63];
    const A_MINOR_HIGH: [f64; 3] = [220.0, 261.63, 329.63];

    /// Stiffness of a string: its partials lie a little above whole multiples
    const STIFFNESS: f64 = 1.2e-4;

    /// The partials of a plucked string up to 6 kHz: (frequency, level, decay time in s)
    fn string_partials(note: f64) -> Vec<(f64, f64, f64)> {
        (1..)
            .map(|n| n as f64)
            .map(|n| {
                let frequency = n * note * (1.0 + STIFFNESS * n * n).sqrt();
                let level = (n * PI64 * 0.18).sin().abs() / n;
                (frequency, level, 2.2 / (1.0 + 0.012 * n * n))
            })
            .take_while(|&(frequency, _, _)| frequency < 6000.0)
            .collect()
    }

    /// Plucked strings ringing together, each partial with its own decay, played `ratio`
    /// times higher: at a ratio other than 1 this is what a perfect shifter would put out
    fn generate_strings(
        notes: &[f64],
        ratio: f64,
        sample_rate: f32,
        num_samples: usize,
    ) -> Vec<f32> {
        let mut signal = vec![0.0_f64; num_samples];
        let mut seed = 2463534242_u32;
        for &note in notes {
            for (frequency, level, decay) in string_partials(note) {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let phase = 2.0 * PI64 * (seed >> 8) as f64 / (1 << 24) as f64;
                let frequency = frequency * ratio;
                if frequency > 0.45 * sample_rate as f64 {
                    continue;
                }
                for (i, sample) in signal.iter_mut().enumerate() {
                    let t = i as f64 / sample_rate as f64;
                    *sample +=
                        level * (-t / decay).exp() * (2.0 * PI64 * frequency * t + phase).sin();
                }
            }
        }
        let scale = 0.25 / (notes.len() as f64).sqrt();
        signal
            .iter()
            .enumerate()
            .map(|(i, sample)| {
                // A pick's attack: 3 ms
                let attack = (i as f64 / (0.003 * sample_rate as f64)).min(1.0);
                (scale * attack * sample) as f32
            })
            .collect()
    }

    /// The frequencies strings shifted by `semitones` should consist of
    fn shifted_string_partials(notes: &[f64], semitones: i32, sample_rate: f32) -> Vec<f64> {
        let mut partials: Vec<f64> = notes
            .iter()
            .flat_map(|&note| string_partials(note))
            .map(|(frequency, _, _)| frequency * shift_ratio(semitones))
            .filter(|&frequency| frequency < sample_rate as f64 * 0.45)
            .collect();
        partials.sort_by(|a, b| a.partial_cmp(b).unwrap());
        partials.dedup_by(|a, b| (*a - *b).abs() < 2.0);
        partials
    }

    /// Like `artifact_ratio_db` for partials that fade: each is fitted with a level that may
    /// rise or fall in a straight line over the span, so a decay is not taken for an artifact
    fn fading_artifact_ratio_db(signal: &[f32], sample_rate: f32, partials: &[f64]) -> f64 {
        let len = signal.len();
        let window: Vec<f64> =
            (0..len).map(|i| 0.5 - 0.5 * (2.0 * PI64 * i as f64 / len as f64).cos()).collect();
        let mut residual: Vec<f64> = signal.iter().map(|&s| s as f64).collect();
        let energy =
            |samples: &[f64]| -> f64 { samples.iter().zip(&window).map(|(s, w)| w * s * s).sum() };
        let total = energy(&residual);
        let mut fitted = vec![[0.0_f64; 4]; partials.len()];

        for _ in 0..4 {
            for (partial, &frequency) in partials.iter().enumerate() {
                let omega = 2.0 * PI64 * frequency / sample_rate as f64;
                let basis = |i: usize| {
                    let (sin, cos) = (omega * i as f64).sin_cos();
                    let slope = 2.0 * i as f64 / len as f64 - 1.0;
                    [cos, sin, slope * cos, slope * sin]
                };
                // Put this partial back, then fit it again to what the others have left
                let mut normal = [[0.0_f64; 5]; 4];
                for i in 0..len {
                    let basis = basis(i);
                    let was: f64 = (0..4).map(|k| fitted[partial][k] * basis[k]).sum();
                    residual[i] += was;
                    for row in 0..4 {
                        for column in 0..4 {
                            normal[row][column] += window[i] * basis[row] * basis[column];
                        }
                        normal[row][4] += window[i] * basis[row] * residual[i];
                    }
                }
                // Four equations, by elimination
                for pivot in 0..4 {
                    let scale = normal[pivot][pivot];
                    for column in pivot..5 {
                        normal[pivot][column] /= scale;
                    }
                    for row in 0..4 {
                        if row != pivot {
                            let factor = normal[row][pivot];
                            for column in pivot..5 {
                                normal[row][column] -= factor * normal[pivot][column];
                            }
                        }
                    }
                }
                for k in 0..4 {
                    fitted[partial][k] = normal[k][4];
                }
                for (i, sample) in residual.iter_mut().enumerate() {
                    let basis = basis(i);
                    *sample -= (0..4).map(|k| fitted[partial][k] * basis[k]).sum::<f64>();
                }
            }
        }

        10.0 * (energy(&residual) / total).max(1e-20).log10()
    }

    /// Span the string measurements are taken over: 186 ms, from a quarter second in
    const STRING_SPAN: std::ops::Range<usize> = 11025..11025 + 8192;

    /// Shifts ringing strings and measures how much of the output is not the shifted strings
    fn measure_string_artifacts(notes: &[f64], semitones: i32, latency_ms: f32) -> f64 {
        let mut shifter = new_shifter(semitones);
        shifter.set_latency_ms(latency_ms);
        let input = generate_strings(notes, 1.0, SAMPLE_RATE, 22050);
        let output = process_buffer(&mut shifter, &input);
        let partials = shifted_string_partials(notes, semitones, SAMPLE_RATE);
        fading_artifact_ratio_db(&output[STRING_SPAN], SAMPLE_RATE, &partials)
    }

    /// Samples the shifter takes to be back at the dry input after a shift ends
    fn fade_len(shifter: &PitchShifter) -> usize {
        (1.0 / shifter.fade_step).ceil() as usize + 1
    }

    #[test]
    fn test_passthrough_is_exact_at_zero_semitones() {
        let mut shifter = new_shifter(0);

        let input = generate_harmonic_tone(110.0, 10, 4000);
        let output = process_buffer(&mut shifter, &input);

        for i in MIN_DELAY..input.len() {
            assert_eq!(output[i], input[i - MIN_DELAY], "Sample {} was altered", i);
        }
    }

    #[test]
    fn test_returns_to_passthrough_after_shifting() {
        let mut shifter = new_shifter(-3);

        let input = generate_harmonic_tone(110.0, 10, 30000);
        let mut output = process_buffer(&mut shifter, &input[..20000]);
        shifter.set_semitones(0);
        output.extend(process_buffer(&mut shifter, &input[20000..]));

        // One crossfade back to the dry input, then untouched again
        for i in 20000 + fade_len(&shifter)..input.len() {
            assert_eq!(output[i], input[i - MIN_DELAY], "Sample {} was altered", i);
        }
    }

    /// Level of one frequency in a signal, in dB (Hann-windowed single-bin transform)
    fn level_at_db(signal: &[f32], sample_rate: f32, frequency: f64) -> f64 {
        let len = signal.len();
        let omega = 2.0 * PI64 * frequency / sample_rate as f64;
        let (mut re, mut im, mut weight) = (0.0, 0.0, 0.0);
        for (i, &sample) in signal.iter().enumerate() {
            let w = 0.5 - 0.5 * (2.0 * PI64 * i as f64 / len as f64).cos();
            let (sin, cos) = (omega * i as f64).sin_cos();
            re += w * sample as f64 * cos;
            im += w * sample as f64 * sin;
            weight += w;
        }
        20.0 * (2.0 * (re * re + im * im).sqrt() / weight).max(1e-12).log10()
    }

    /// Runs a shifter with the blend set every sample, as the plugin does
    fn process_blended(shifter: &mut PitchShifter, input: &[f32], blend: f32) -> Vec<f32> {
        input
            .iter()
            .map(|&s| {
                shifter.set_blend(blend);
                shifter.process_sample(s)
            })
            .collect()
    }

    #[test]
    fn test_full_blend_is_the_shifted_output_unchanged() {
        // 100 % is the default and the sound without the knob, bit for bit
        let input = generate_harmonic_tone(110.0, 10, NUM_SAMPLES);
        for semitones in [-12, -5, 7, 12] {
            let without_knob = process_buffer(&mut new_shifter(semitones), &input);
            let at_full = process_blended(&mut new_shifter(semitones), &input, 1.0);
            for (i, (a, b)) in without_knob.iter().zip(&at_full).enumerate() {
                assert_eq!(
                    a.to_bits(),
                    b.to_bits(),
                    "Semitones={}: sample {} changed",
                    semitones,
                    i
                );
            }
        }
    }

    #[test]
    fn test_no_blend_is_the_dry_input_at_every_shift() {
        let input = generate_harmonic_tone(110.0, 10, 20000);
        for semitones in -12..=12 {
            let output = process_blended(&mut new_shifter(semitones), &input, 0.0);
            assert!(output[..MIN_DELAY].iter().all(|&s| s == 0.0));
            for i in MIN_DELAY..input.len() {
                assert_eq!(
                    output[i].to_bits(),
                    input[i - MIN_DELAY].to_bits(),
                    "Semitones={}: sample {} is not the dry input",
                    semitones,
                    i
                );
            }
        }
    }

    #[test]
    fn test_no_shift_passes_through_at_any_blend() {
        let input = generate_harmonic_tone(110.0, 10, 4000);
        for blend in [0.0, 0.3, 0.5, 1.0] {
            let output = process_blended(&mut new_shifter(0), &input, blend);
            for i in MIN_DELAY..input.len() {
                assert_eq!(
                    output[i],
                    input[i - MIN_DELAY],
                    "Blend={}: sample {} was altered",
                    blend,
                    i
                );
            }
        }
    }

    #[test]
    fn test_bypass_returns_to_passthrough_at_any_blend() {
        // Bypass is a shift of 0: after one crossfade back it is bit-exact whatever the blend
        let input = generate_harmonic_tone(110.0, 10, 30000);
        for blend in [0.0, 0.3, 0.5, 1.0] {
            let mut shifter = new_shifter(-12);
            let mut output = process_blended(&mut shifter, &input[..20000], blend);
            shifter.set_semitones(0);
            output.extend(process_blended(&mut shifter, &input[20000..], blend));
            for i in 20000 + fade_len(&shifter)..input.len() {
                assert_eq!(
                    output[i],
                    input[i - MIN_DELAY],
                    "Blend={}: sample {} was altered",
                    blend,
                    i
                );
            }
        }
    }

    #[test]
    fn test_half_blend_is_a_two_note_harmony() {
        // A fifth up on a 110 Hz sine: both notes at the same level, and nothing else
        let input = generate_sine_wave(110.0, SAMPLE_RATE, NUM_SAMPLES);
        let shifted = 110.0 * shift_ratio(7);
        let mixed = process_blended(&mut new_shifter(7), &input, 0.5);

        let dry_db = level_at_db(&mixed[SETTLE..], SAMPLE_RATE, 110.0);
        let wet_db = level_at_db(&mixed[SETTLE..], SAMPLE_RATE, shifted);
        assert!(
            (dry_db - wet_db).abs() < 1.0,
            "110 Hz at {:.2} dB, {:.1} Hz at {:.2} dB",
            dry_db,
            shifted,
            wet_db
        );

        let artifacts = artifact_ratio_db(&mixed[SETTLE..], SAMPLE_RATE, &[110.0, shifted]);
        assert!(artifacts < -40.0, "Artifacts at {:.1} dB", artifacts);
    }

    #[test]
    fn test_no_clicks_when_turning_blend() {
        // Blend swept 0 to 100 % and back over 50 ms while shifting an octave down
        let mut shifter = new_shifter(-12);
        let input = generate_sine_wave(220.0, SAMPLE_RATE, 60000);
        let sweep = (SAMPLE_RATE * 0.05) as usize;
        let mut previous = 0.0;
        for (i, &sample) in input.iter().enumerate() {
            let phase = (i % (2 * sweep)) as f32 / sweep as f32;
            shifter.set_blend(if phase < 1.0 { phase } else { 2.0 - phase });

            let output = shifter.process_sample(sample);
            assert!(output.is_finite() && output.abs() < 1.5, "Sample {}: output {}", i, output);

            // The same measure as test_no_clicks_on_parameter_changes
            let step = (output - previous).abs();
            assert!(step < 0.25, "Sample {}: jumped by {}", i, step);
            previous = output;
        }
    }

    #[test]
    fn test_sine_stays_clean_at_all_semitones() {
        for semitones in -12..=12 {
            for note in [82.41, 196.0, 659.26] {
                let artifacts = measure_artifacts(&[note], 1, semitones, 15.0);
                assert!(
                    artifacts < SINE_CLEAN,
                    "Semitones={}, {} Hz: artifacts at {:.1} dB",
                    semitones,
                    note,
                    artifacts
                );
            }
        }
    }

    #[test]
    fn test_harmonic_tone_stays_clean() {
        for semitones in [-12, -7, -5, -2, -1, 1, 2, 5, 7, 12] {
            let artifacts = measure_artifacts(&[110.0], 10, semitones, 15.0);
            assert!(
                artifacts < TONE_CLEAN,
                "Semitones={}: artifacts at {:.1} dB",
                semitones,
                artifacts
            );
        }
    }

    #[test]
    fn test_a_ringing_string_stays_clean() {
        // What the splicing shifter before this one buzzed on: a string's partials are not
        // whole multiples, so no period repeats exactly
        for note in [82.41, 196.0, 659.26] {
            for semitones in [-12, -5, 7, 12] {
                let artifacts = measure_string_artifacts(&[note], semitones, 15.0);
                assert!(
                    artifacts < STRING_CLEAN,
                    "{} Hz, semitones={}: artifacts at {:.1} dB",
                    note,
                    semitones,
                    artifacts
                );
            }
        }
    }

    #[test]
    fn test_chords_stay_clean_at_the_default_latency() {
        // Equal-tempered chords, whose notes share no period short enough to splice on
        for (name, chord) in [("E5", &E_POWER[..]), ("E", &E_MAJOR[..]), ("Am", &A_MINOR_HIGH[..])]
        {
            for semitones in [-12, -5, -2, 2, 7, 12] {
                let artifacts = measure_string_artifacts(chord, semitones, 15.0);
                assert!(
                    artifacts < CHORD_CLEAN,
                    "{}, semitones={}: artifacts at {:.1} dB",
                    name,
                    semitones,
                    artifacts
                );
            }
        }
    }

    #[test]
    fn test_stays_clean_at_other_sample_rates() {
        // 96 and 192 kHz go through the step down to the engine's rate and back
        for sample_rate in [48000.0_f32, 96000.0, 192000.0] {
            for semitones in [-2, 7] {
                let mut shifter = PitchShifter::new();
                shifter.set_sample_rate(sample_rate);
                shifter.set_semitones(semitones);

                let num_samples = sample_rate as usize;
                let input = generate_sine_wave(110.0, sample_rate, num_samples);
                let output = process_buffer(&mut shifter, &input);

                let partials = shifted_partials(&[110.0], 1, semitones, sample_rate);
                let artifacts =
                    artifact_ratio_db(&output[num_samples / 3..], sample_rate, &partials);
                assert!(
                    artifacts < SINE_CLEAN,
                    "{} Hz, semitones={}: artifacts at {:.1} dB",
                    sample_rate,
                    semitones,
                    artifacts
                );
                let ratio = rms(&output[num_samples / 3..]) / rms(&input[num_samples / 3..]);
                assert!((ratio - 1.0).abs() < 0.02, "{} Hz: level times {}", sample_rate, ratio);
            }
        }
    }

    #[test]
    fn test_level_is_preserved() {
        for semitones in [-12, -2, 2, 12] {
            let mut shifter = new_shifter(semitones);

            let input = generate_sine_wave(196.0, SAMPLE_RATE, NUM_SAMPLES);
            let output = process_buffer(&mut shifter, &input);

            let ratio = rms(&output[SETTLE..]) / rms(&input[SETTLE..]);
            assert!(
                (ratio - 1.0).abs() < 0.02,
                "Semitones={}: level changed by a factor of {}",
                semitones,
                ratio
            );
        }
    }

    #[test]
    fn test_a_chord_keeps_its_level() {
        // Against the same chord played that much higher or lower. Partials of two strings
        // that nearly meet beat at another rate once shifted, which moves the level a little
        for semitones in [-12, -5, 7, 12] {
            let input = generate_strings(&E_MAJOR, 1.0, SAMPLE_RATE, NUM_SAMPLES);
            let wanted =
                generate_strings(&E_MAJOR, shift_ratio(semitones), SAMPLE_RATE, NUM_SAMPLES);
            let output = process_buffer(&mut new_shifter(semitones), &input);
            let ratio = rms(&output[SETTLE..]) / rms(&wanted[SETTLE..]);
            assert!(
                (20.0 * ratio.log10()).abs() < 1.0,
                "Semitones={}: level changed by {:.2} dB",
                semitones,
                20.0 * ratio.log10()
            );
        }
    }

    /// First sample over a tenth of the signal's peak
    fn onset(signal: &[f32]) -> usize {
        let top = signal.iter().fold(0.0_f32, |top, sample| top.max(sample.abs()));
        signal.iter().position(|sample| sample.abs() > 0.1 * top).unwrap()
    }

    #[test]
    fn test_a_note_starts_one_latency_late() {
        // The lag does not depend on the note, as it did when it was a period of it
        for (latency_ms, note) in [(6.0, 82.41), (15.0, 82.41), (15.0, 659.26), (30.0, 196.0)] {
            for semitones in [-12, 7] {
                let mut shifter = new_shifter(semitones);
                shifter.set_latency_ms(latency_ms);
                // The shifter is running before the note comes
                let mut input = vec![0.0; 8000];
                input.extend(generate_strings(&[note], 1.0, SAMPLE_RATE, 12000));
                let output = process_buffer(&mut shifter, &input);

                let lag = onset(&output) as f32 - onset(&input) as f32;
                let stated = shifter.latency_samples() as f32;
                assert!(stated <= latency_ms * 0.001 * SAMPLE_RATE);
                // The start of a note is spread over the chunk it falls in: it is heard
                // between half a chunk early and a quarter late
                assert!(
                    lag > 0.4 * stated && lag < 1.3 * stated,
                    "{} ms, {} Hz, semitones={}: starts {} samples late, latency {}",
                    latency_ms,
                    note,
                    semitones,
                    lag,
                    stated
                );
            }
        }
    }

    #[test]
    fn test_latency_stays_within_the_setting() {
        for sample_rate in [44100.0_f32, 48000.0, 96000.0, 192000.0] {
            let mut shifter = PitchShifter::new();
            shifter.set_sample_rate(sample_rate);
            let mut last = 0;
            for latency_ms in [6.0, 10.0, 12.0, 15.0, 22.0, 30.0, 50.0] {
                shifter.set_latency_ms(latency_ms);
                let lag = shifter.latency_samples();
                assert!(
                    lag as f32 <= latency_ms * 0.001 * sample_rate,
                    "{} Hz, {} ms: lags {} samples",
                    sample_rate,
                    latency_ms,
                    lag
                );
                assert!(lag >= last);
                last = lag;
            }
            // Under the shortest chunk there is, the shortest is what it gets
            shifter.set_latency_ms(2.0);
            assert!(shifter.latency_samples() as f32 <= 0.0062 * sample_rate);
        }
    }

    #[test]
    fn test_noise_comes_through_at_its_level() {
        let input = generate_noise(40000);
        for semitones in [-12, -5, 7, 12] {
            let output = process_buffer(&mut new_shifter(semitones), &input);
            assert!(output.iter().all(|sample| sample.is_finite()));
            // Shifted up, the top of the noise leaves the band
            let ratio = rms(&output[SETTLE..]) / rms(&input[SETTLE..]);
            let least = if semitones > 0 { 0.5 } else { 0.8 };
            assert!(ratio > least && ratio < 1.2, "Semitones={}: level times {}", semitones, ratio);
        }
    }

    #[test]
    fn test_silence_stays_silent() {
        let mut shifter = new_shifter(-5);
        for _ in 0..20000 {
            assert_eq!(shifter.process_sample(0.0), 0.0);
        }
    }

    #[test]
    fn test_stereo_channels_are_shifted_alike() {
        let mut shifter = new_shifter(5);

        let input = generate_harmonic_tone(110.0, 10, 20000);
        for &sample in &input {
            let (left, right) = shifter.process_stereo(sample, 0.5 * sample);
            assert!(
                (right - 0.5 * left).abs() < 1e-4,
                "Channels drifted apart: left={}, right={}",
                left,
                right
            );
        }
    }

    #[test]
    fn test_no_clicks_on_parameter_changes() {
        let mut shifter = new_shifter(0);

        let input = generate_sine_wave(220.0, SAMPLE_RATE, 60000);
        let mut previous = 0.0;

        for (i, &sample) in input.iter().enumerate() {
            if i % 1500 == 0 {
                shifter.set_semitones(((i / 1500 * 7) % 25) as i32 - 12);
            }
            if i % 2300 == 0 {
                shifter.set_latency_ms(((i / 2300 * 11) % 48) as f32 + 2.0);
            }
            if i % 3100 == 0 {
                shifter.set_smoothness_ms(((i / 3100 * 3) % 9) as f32 + 1.0);
            }

            let output = shifter.process_sample(sample);
            assert!(output.is_finite() && output.abs() < 1.5, "Sample {}: output {}", i, output);

            // A clean 440 Hz sine moves at most 0.063 per sample; a click is several times that
            let step = (output - previous).abs();
            assert!(step < 0.25, "Sample {}: jumped by {}", i, step);
            previous = output;
        }
    }

    #[test]
    fn test_reset_clears_state() {
        let mut shifter = new_shifter(5);

        let input = generate_sine_wave(440.0, SAMPLE_RATE, 20000);
        let first = process_buffer(&mut shifter, &input);

        shifter.reset();
        for _ in 0..2000 {
            assert_eq!(shifter.process_sample(0.0), 0.0);
        }

        // And it plays again as it did the first time
        shifter.reset();
        assert!(process_buffer(&mut shifter, &input) == first);
    }

    #[test]
    fn test_the_rate_is_stepped_down_to_the_engines() {
        let mut shifter = PitchShifter::new();
        for (sample_rate, factor) in
            [(44100.0, 1), (48000.0, 1), (88200.0, 2), (96000.0, 2), (176400.0, 4), (192000.0, 4)]
        {
            shifter.set_sample_rate(sample_rate);
            assert_eq!(shifter.factor, factor, "{} Hz", sample_rate);
        }
    }

    /// Prints artifact levels, latency and cost for a range of notes and shifts. Use it to
    /// compare before and after changing the engine or its constants:
    ///   cargo test -p suite_common --release quality_report -- --ignored --nocapture
    #[test]
    #[ignore]
    fn quality_report() {
        let shifts = [-12, -7, -5, -2, -1, 0, 1, 2, 5, 7, 12];
        let notes = [82.41, 110.0, 196.0, 329.63, 659.26];

        println!("Artifacts in dB (lower is cleaner), sines and a 110 Hz harmonic tone, at 15 ms");
        println!("{:>4}{:>8}{:>8}{:>8}{:>8}{:>8}{:>10}", "st", 82, 110, 196, 330, 659, "harmonic");
        for semitones in shifts {
            print!("{:>4}", semitones);
            for note in notes {
                print!("{:>8.1}", measure_artifacts(&[note], 1, semitones, 15.0));
            }
            println!("{:>10.1}", measure_artifacts(&[110.0], 10, semitones, 15.0));
        }

        println!();
        println!("Artifacts in dB, ringing strings (stiff, each partial with its own decay) and");
        println!("equal-tempered chords of them, at 15 ms");
        println!(
            "{:>4}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}{:>8}",
            "st", 82, 110, 196, 330, 659, "E5", "E", "Am"
        );
        for semitones in shifts {
            print!("{:>4}", semitones);
            for note in notes {
                print!("{:>8.1}", measure_string_artifacts(&[note], semitones, 15.0));
            }
            for chord in [&E_POWER[..], &E_MAJOR[..], &A_MINOR_HIGH[..]] {
                print!("{:>8.1}", measure_string_artifacts(chord, semitones, 15.0));
            }
            println!();
        }

        println!();
        println!("The same per latency setting: a low E string, and an E major chord");
        println!("{:>4}{:>16}{:>16}{:>16}{:>16}", "st", "5.5 ms", "15 ms", "30 ms", "50 ms");
        for semitones in [-12, -5, -2, 2, 7, 12] {
            print!("{:>4}", semitones);
            for latency_ms in [5.5, 15.0, 30.0, 50.0] {
                print!(
                    "{:>8.1}{:>8.1}",
                    measure_string_artifacts(&[82.41], semitones, latency_ms),
                    measure_string_artifacts(&E_MAJOR, semitones, latency_ms)
                );
            }
            println!();
        }

        println!();
        println!("Level of a shifted E major chord against the chord played there, in dB");
        for semitones in [-12, -5, -2, 2, 7, 12] {
            let input = generate_strings(&E_MAJOR, 1.0, SAMPLE_RATE, NUM_SAMPLES);
            let wanted =
                generate_strings(&E_MAJOR, shift_ratio(semitones), SAMPLE_RATE, NUM_SAMPLES);
            let output = process_buffer(&mut new_shifter(semitones), &input);
            let ratio = rms(&output[SETTLE..]) / rms(&wanted[SETTLE..]);
            println!("{:>4}{:>8.2}", semitones, 20.0 * ratio.log10());
        }

        println!();
        println!("Latency in ms per setting and sample rate");
        println!("{:>10}{:>8}{:>8}{:>8}{:>8}", "rate", "5.5 ms", "15 ms", "30 ms", "50 ms");
        for sample_rate in [44100.0_f32, 48000.0, 96000.0, 192000.0] {
            let mut shifter = PitchShifter::new();
            shifter.set_sample_rate(sample_rate);
            print!("{:>10}", sample_rate);
            for latency_ms in [5.5, 15.0, 30.0, 50.0] {
                shifter.set_latency_ms(latency_ms);
                print!("{:>8.2}", shifter.latency_samples() as f32 / sample_rate * 1000.0);
            }
            println!();
        }

        println!();
        println!("Cost of an E major chord in blocks of 64 at 48 kHz, as a share of the time a");
        println!("block lasts: over all blocks, and the 99th percentile block");
        for sample_rate in [48000.0_f32, 192000.0] {
            for latency_ms in [5.5, 15.0, 30.0, 50.0] {
                let block = 64 * (sample_rate / 48000.0) as usize;
                let input = generate_strings(&E_MAJOR, 1.0, sample_rate, 4 * sample_rate as usize);
                let mut shifter = PitchShifter::new();
                shifter.set_sample_rate(sample_rate);
                shifter.set_latency_ms(latency_ms);
                shifter.set_semitones(-5);
                let mut times: Vec<f64> = input
                    .chunks(block)
                    .map(|block| {
                        let start = std::time::Instant::now();
                        for &sample in block {
                            std::hint::black_box(shifter.process_sample(sample));
                        }
                        start.elapsed().as_secs_f64()
                    })
                    .collect();
                let total: f64 = times.iter().sum();
                times.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let lasts = block as f64 / sample_rate as f64;
                println!(
                    "{:>8} Hz {:>5} ms: {:>5.1} % / {:>5.1} %",
                    sample_rate,
                    latency_ms,
                    100.0 * total / 4.0,
                    100.0 * times[times.len() * 99 / 100] / lasts
                );
            }
        }
    }

    /// Writes before/after WAV files to target/renders for listening:
    ///   cargo test -p suite_common --release render_wavs -- --ignored
    ///
    /// Set PITCH_SHIFTER_INPUT_WAV to the path of a recording to render that as well, and
    /// PITCH_SHIFTER_LATENCY_MS to try another max latency.
    #[test]
    #[ignore]
    fn render_wavs() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/renders");
        std::fs::create_dir_all(&dir).unwrap();

        let latency_ms: Option<f32> = std::env::var("PITCH_SHIFTER_LATENCY_MS")
            .ok()
            .map(|value| value.parse().expect("PITCH_SHIFTER_LATENCY_MS must be a number"));

        // A phrase of single notes across the strings, a strummed E major chord left to
        // ring, and power chords
        let rate = SAMPLE_RATE;
        let note_len = (rate * 0.5) as usize;
        let mut phrase = vec![0.0; note_len * 7];
        for (i, note) in [82.41, 110.0, 146.83, 196.0, 246.94, 329.63].iter().enumerate() {
            for (j, sample) in generate_strings(&[*note], 1.0, rate, note_len).iter().enumerate() {
                phrase[i * note_len + j] += sample;
            }
        }
        let strum_gap = (rate * 0.015) as usize;
        let mut chord = vec![0.0; note_len * 6];
        for (i, note) in E_MAJOR.iter().enumerate() {
            let start = i * strum_gap;
            let string = generate_strings(&[*note], 1.0, rate, chord.len() - start);
            for (j, sample) in string.iter().enumerate() {
                chord[start + j] += 0.5 * sample;
            }
        }
        let mut riff = vec![0.0; note_len * 6];
        for (i, root) in [82.41, 82.41, 98.0, 110.0, 82.41, 73.42].iter().enumerate() {
            let notes = [*root, root * 1.4983, root * 2.0];
            for (j, sample) in generate_strings(&notes, 1.0, rate, note_len).iter().enumerate() {
                riff[i * note_len + j] += sample;
            }
        }
        let sustain = generate_strings(&[196.0], 1.0, rate, note_len * 6);

        let mut sources = vec![
            ("notes".to_string(), phrase, rate as u32),
            ("chord".to_string(), chord, rate as u32),
            ("power_chords".to_string(), riff, rate as u32),
            ("long_note".to_string(), sustain, rate as u32),
        ];
        if let Ok(path) = std::env::var("PITCH_SHIFTER_INPUT_WAV") {
            let (samples, sample_rate) = read_wav_mono(&path);
            sources.push(("input".to_string(), samples, sample_rate));
        }

        for (name, input, sample_rate) in &sources {
            write_wav(&dir.join(format!("{}_original.wav", name)), input, *sample_rate);
            for semitones in [-12, -5, -2, -1, 0, 2, 7, 12] {
                let mut shifter = PitchShifter::new();
                shifter.set_sample_rate(*sample_rate as f32);
                shifter.set_semitones(semitones);
                if let Some(latency_ms) = latency_ms {
                    shifter.set_latency_ms(latency_ms);
                }
                let output = process_buffer(&mut shifter, input);
                write_wav(
                    &dir.join(format!("{}_{:+}.wav", name, semitones)),
                    &output,
                    *sample_rate,
                );
            }
        }
        println!("Wrote renders to {}", dir.display());
    }

    fn write_wav(path: &std::path::Path, samples: &[f32], sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for &sample in samples {
            writer.write_sample((sample.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap();
        }
        writer.finalize().unwrap();
    }

    /// Reads a WAV file and mixes it down to mono
    fn read_wav_mono(path: &str) -> (Vec<f32>, u32) {
        let mut reader =
            hound::WavReader::open(path).expect("Could not open PITCH_SHIFTER_INPUT_WAV");
        let spec = reader.spec();
        let interleaved: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
            hound::SampleFormat::Int => {
                let scale = (1_i64 << (spec.bits_per_sample - 1)) as f32;
                reader.samples::<i32>().map(|s| s.unwrap() as f32 / scale).collect()
            }
        };
        let channels = spec.channels as usize;
        let mono = interleaved
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect();
        (mono, spec.sample_rate)
    }
}
