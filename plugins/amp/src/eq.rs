//! EQ pedal: three bands to shape the tone, Low and High as shelves and Mid as a bell whose
//! centre Freq sweeps. Each dial is 0.0 to 1.0 (shown 0 to 10) around a flat middle, a band
//! at its middle is skipped, and with all three flat the signal passes untouched. Runs in
//! front of the amp on one channel or behind the cabinet on two, and sounds the same in both

use crate::dsp::filters::{BiquadCoeffs, GlidingBiquad};
use crate::dsp::lerp;

/// What a band does with its dial at either end, in dB: the dial's middle is flat
pub const GAIN_DB: f32 = 12.0;

/// The corner of the Low shelf and of the High shelf, in Hz
pub const LOW_HZ: f32 = 100.0;
pub const HIGH_HZ: f32 = 4000.0;

/// Where Mid is centred with Freq at either end, in Hz. The sweep is by octaves
pub const MID_HZ: [f32; 2] = [200.0, 5000.0];

// Shelves with the slope of a Butterworth filter: no bump before the corner. The bell is
// wide enough to be a tone control and not a notch
const SHELF_Q: f32 = 0.707;
const MID_Q: f32 = 0.9;

// A dial this close to its middle is flat: 0.01 dB is a thousandth of a turn
const FLAT_DB: f32 = 0.01;

// A switch fades over this long, and a dial's change is followed over this long
const FADE_MS: f32 = 10.0;
const GLIDE_MS: f32 = 20.0;

// A band that has become flat runs on for this many samples after its coefficients have
// arrived at the identity: its state is then zero and it can be left out
const FLUSH: u32 = 3;

/// What a Low or High dial (0.0 to 1.0) is, in dB
pub fn band_db(dial: f32) -> f32 {
    let db = lerp(-GAIN_DB, GAIN_DB, dial.clamp(0.0, 1.0));
    if db.abs() < FLAT_DB {
        0.0
    } else {
        db
    }
}

/// Where Freq (0.0 to 1.0) puts the centre of Mid, in Hz
pub fn mid_hz(dial: f32) -> f32 {
    MID_HZ[0] * (MID_HZ[1] / MID_HZ[0]).powf(dial.clamp(0.0, 1.0))
}

/// What the knobs say, read once per call. The EQ follows the values itself
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqSettings {
    pub on: bool,
    /// The dials, 0.0 to 1.0, flat at 0.5
    pub low: f32,
    pub mid: f32,
    /// The centre of Mid, 0.0 to 1.0: see `mid_hz`
    pub freq: f32,
    pub high: f32,
}

impl Default for EqSettings {
    fn default() -> Self {
        Self { on: false, low: 0.5, mid: 0.5, freq: 0.5, high: 0.5 }
    }
}

pub struct Eq {
    // Low, Mid and High, each with a filter for either channel
    bands: [[GlidingBiquad; 2]; 3],
    // What the filters were designed for: the gain of each band in dB, and the centre of Mid.
    // NaN until designed, so that the first design is made
    designed: [f32; 4],
    // A band whose filter is the identity, and how many samples it still has to run before it
    // can be left out
    flat: [bool; 3],
    keep: [u32; 3],
    // Share of the pedal in its output: 0.0 switched off, 1.0 on
    mix: f32,
    fade_step: f32,
    glide_steps: u32,
    rate: f32,
    // Off and faded out: the signal is left alone
    idle: bool,
}

impl Eq {
    /// Allocates nothing: the filters are a few numbers each
    pub fn new() -> Self {
        let mut eq = Self {
            bands: [[GlidingBiquad::new(); 2]; 3],
            designed: [f32::NAN; 4],
            flat: [true; 3],
            keep: [0; 3],
            mix: 0.0,
            fade_step: 1.0,
            glide_steps: 1,
            rate: 48_000.0,
            idle: true,
        };
        eq.set_sample_rate(48_000.0);
        eq
    }

    /// The rate the pedal runs at, which starts it over. Never allocates: it may be called on
    /// the audio thread when the pedal is moved between its two places
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.rate = sample_rate;
        self.fade_step = 1.0 / (FADE_MS * 0.001 * sample_rate);
        self.glide_steps = ((GLIDE_MS * 0.001 * sample_rate).round() as u32).max(1);
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
    pub fn process_mono(&mut self, settings: &EqSettings, block: &mut [f32]) {
        self.process(settings, [block]);
    }

    /// Behind the cabinet: two channels at the host's rate
    pub fn process_stereo(&mut self, settings: &EqSettings, left: &mut [f32], right: &mut [f32]) {
        debug_assert_eq!(left.len(), right.len());
        let len = left.len().min(right.len());
        self.process(settings, [&mut left[..len], &mut right[..len]]);
    }

    /// Designs the filters of the bands whose dials have moved, to be arrived at in `steps`
    /// samples (at once with none)
    fn design(&mut self, settings: &EqSettings, steps: u32) {
        let (low, mid, high) =
            (band_db(settings.low), band_db(settings.mid), band_db(settings.high));
        let hz = mid_hz(settings.freq);
        let rate = self.rate;
        // A flat Mid has no centre to follow
        let mid_moved = mid != self.designed[1] || (mid != 0.0 && hz != self.designed[2]);
        let designs: [(bool, f32, BiquadCoeffs); 3] = [
            (low != self.designed[0], low, BiquadCoeffs::low_shelf(LOW_HZ, SHELF_Q, low, rate)),
            (mid_moved, mid, BiquadCoeffs::peak(hz, MID_Q, mid, rate)),
            (
                high != self.designed[3],
                high,
                BiquadCoeffs::high_shelf(HIGH_HZ, SHELF_Q, high, rate),
            ),
        ];
        for (band, (moved, db, coeffs)) in designs.into_iter().enumerate() {
            if !moved {
                continue;
            }
            let coeffs = if db == 0.0 { BiquadCoeffs::IDENTITY } else { coeffs };
            for filter in &mut self.bands[band] {
                filter.set(coeffs, steps);
            }
            self.flat[band] = db == 0.0;
            if self.flat[band] {
                self.keep[band] = steps + FLUSH;
            }
        }
        self.designed = [low, mid, if mid != 0.0 { hz } else { self.designed[2] }, high];
    }

    /// False if the block is to be left alone. Wakes a pedal that was idle and is switched on
    fn awake(&mut self, settings: &EqSettings) -> bool {
        if self.idle && !settings.on {
            return false;
        }
        if self.idle {
            // Nothing is heard yet, so there is nothing to glide
            for filter in self.bands.iter_mut().flatten() {
                filter.reset();
            }
            self.designed = [f32::NAN; 4];
            self.design(settings, 0);
            self.keep = [0; 3];
            self.mix = 0.0;
            self.idle = false;
        } else {
            self.design(settings, self.glide_steps);
        }
        true
    }

    fn process<const N: usize>(&mut self, settings: &EqSettings, mut channels: [&mut [f32]; N]) {
        if !self.awake(settings) {
            return;
        }
        let len = channels[0].len();
        // The bands that do something now, or still have something of it in them
        let run: [bool; 3] = std::array::from_fn(|band| !self.flat[band] || self.keep[band] > 0);
        for keep in &mut self.keep {
            *keep = keep.saturating_sub(len as u32);
        }
        let target = if settings.on { 1.0 } else { 0.0 };
        for index in 0..len {
            if self.mix != target {
                self.mix = (self.mix + self.fade_step.copysign(target - self.mix)).clamp(0.0, 1.0);
            }
            if self.mix == 0.0 && !settings.on {
                // Faded out: the rest of the block is the input itself
                self.idle = true;
                return;
            }
            if !run.contains(&true) {
                continue;
            }
            for (channel, samples) in channels.iter_mut().enumerate() {
                let dry = samples[index];
                let mut wet = dry as f64;
                for (band, filters) in self.bands.iter_mut().enumerate() {
                    if run[band] {
                        wet = filters[channel].process(wet);
                    }
                }
                let wet = wet as f32;
                samples[index] = if self.mix >= 1.0 { wet } else { dry + self.mix * (wet - dry) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    const FS: f32 = 48_000.0;

    fn on(low: f32, mid: f32, freq: f32, high: f32) -> EqSettings {
        EqSettings { on: true, low, mid, freq, high }
    }

    fn run_mono(eq: &mut Eq, settings: &EqSettings, input: &[f32], block: usize) -> Vec<f32> {
        let mut output = input.to_vec();
        output.chunks_mut(block).for_each(|piece| eq.process_mono(settings, piece));
        output
    }

    fn run_stereo(
        eq: &mut Eq,
        settings: &EqSettings,
        input: &[f32],
        block: usize,
    ) -> (Vec<f32>, Vec<f32>) {
        let (mut left, mut right) = (input.to_vec(), input.to_vec());
        for (l, r) in left.chunks_mut(block).zip(right.chunks_mut(block)) {
            eq.process_stereo(settings, l, r);
        }
        (left, right)
    }

    fn at(rate: f32) -> Eq {
        let mut eq = Eq::new();
        eq.set_sample_rate(rate);
        eq
    }

    /// The pedal's gain in dB at a frequency, measured on a sine
    pub(crate) fn measured_db(settings: &EqSettings, freq_hz: f32, rate: f32) -> f32 {
        let input = sine(freq_hz, 0.1, rate, rate as usize);
        let output = run_mono(&mut at(rate), settings, &input, 64);
        let half = output.len() / 2;
        to_db(level_at(&output[half..], rate, freq_hz) / level_at(&input[half..], rate, freq_hz))
    }

    #[test]
    fn test_dials_map_to_db_and_hz() {
        assert_eq!(band_db(0.0), -12.0);
        assert_eq!(band_db(0.5), 0.0);
        assert_eq!(band_db(1.0), 12.0);
        assert_eq!(band_db(0.4999), 0.0, "A dial a hair from flat is flat");
        assert!((mid_hz(0.0) - 200.0).abs() < 1e-3);
        assert!((mid_hz(1.0) - 5000.0).abs() < 1e-2);
        assert!((mid_hz(0.5) - 1000.0).abs() < 1e-2, "{}", mid_hz(0.5));
        let defaults = EqSettings::default();
        assert!(!defaults.on);
        assert_eq!(
            (defaults.low, defaults.mid, defaults.freq, defaults.high),
            (0.5, 0.5, 0.5, 0.5)
        );
    }

    #[test]
    fn test_off_leaves_the_signal_to_the_bit() {
        let input = guitar_di(FS)[..48_000].to_vec();
        let moved = EqSettings { on: false, ..on(1.0, 0.0, 0.2, 1.0) };
        assert_eq!(run_mono(&mut at(FS), &moved, &input, 64), input);
        let (left, right) = run_stereo(&mut at(FS), &moved, &input, 64);
        assert!(left == input && right == input);
        assert!(at(FS).is_idle());
    }

    #[test]
    fn test_on_with_every_band_flat_leaves_the_signal_to_the_bit() {
        let input = guitar_di(FS)[..48_000].to_vec();
        for freq in [0.0, 0.5, 1.0] {
            let flat = on(0.5, 0.5, freq, 0.5);
            for block in [1, 64, 480] {
                assert_eq!(run_mono(&mut at(FS), &flat, &input, block), input);
            }
            let (left, right) = run_stereo(&mut at(FS), &flat, &input, 64);
            assert!(left == input && right == input);
        }
    }

    #[test]
    fn test_a_band_at_its_ends_is_as_far_as_the_dial_says_on_its_plateau() {
        // The shelves' plateaus are well away from their corners; at 48 kHz and at the rate
        // the pedal runs at in front of the amp at 192 kHz
        for rate in [48_000.0, 192_000.0] {
            for (dial, db) in [(0.0, -12.0), (1.0, 12.0)] {
                let low = measured_db(&on(dial, 0.5, 0.5, 0.5), 20.0, rate);
                assert!((low - db).abs() < 0.5, "Low {dial} at 20 Hz, {rate} Hz: {low} dB");
                let high = measured_db(&on(0.5, 0.5, 0.5, dial), 16_000.0, rate);
                assert!((high - db).abs() < 0.5, "High {dial} at 16 kHz, {rate} Hz: {high} dB");
            }
        }
    }

    #[test]
    fn test_a_band_at_its_middle_is_flat_and_the_others_stay_out_of_its_way() {
        // Everything flat but one band: the far side of each shelf and the rest of the
        // spectrum are within 0.1 dB at a decade from the corner, 0.5 dB at the least room
        for (dial, far) in [(0.0, 12.0), (1.0, 12.0)] {
            let (low, high) = (on(dial, 0.5, 0.5, 0.5), on(0.5, 0.5, 0.5, dial));
            // A shelf is flat a decade away on the other side of its corner
            let away = measured_db(&low, 1_000.0, FS).abs();
            assert!(away < 0.5, "Low {dial} at 1 kHz: {away} dB (of {far})");
            let clear = measured_db(&low, 5_000.0, FS).abs();
            assert!(clear < 0.1, "Low {dial} at 5 kHz: {clear} dB");
            let away = measured_db(&high, 400.0, FS).abs();
            assert!(away < 0.5, "High {dial} at 400 Hz: {away} dB");
            let clear = measured_db(&high, 80.0, FS).abs();
            assert!(clear < 0.1, "High {dial} at 80 Hz: {clear} dB");
        }
        // Mid out of the way of the shelves' plateaus
        for dial in [0.0, 1.0] {
            let mid = on(0.5, dial, 0.5, 0.5);
            for freq in [25.0, 16_000.0] {
                let db = measured_db(&mid, freq, FS).abs();
                assert!(db < 0.5, "Mid {dial} at {freq} Hz: {db} dB");
            }
        }
    }

    #[test]
    fn test_mid_peaks_on_its_centre_across_the_sweep() {
        for rate in [48_000.0, 192_000.0] {
            for freq_dial in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let centre = mid_hz(freq_dial);
                for (dial, db) in [(1.0, 12.0), (0.0, -12.0)] {
                    let settings = on(0.5, dial, freq_dial, 0.5);
                    let at_centre = measured_db(&settings, centre, rate);
                    assert!(
                        (at_centre - db).abs() < 0.3,
                        "Mid {dial} at Freq {freq_dial} ({centre:.0} Hz), {rate} Hz: {at_centre} dB"
                    );
                    // And it is the extreme of the response, within an eighth of an octave
                    let (mut extreme, mut extreme_hz) = (0.0f32, 0.0);
                    for step in -8..=8 {
                        let hz = centre * 2.0f32.powf(step as f32 / 8.0 * 2.0);
                        let level = measured_db(&settings, hz, rate);
                        if level.abs() > extreme.abs() {
                            (extreme, extreme_hz) = (level, hz);
                        }
                    }
                    assert!(
                        (extreme_hz / centre).log2().abs() < 0.13,
                        "Mid {dial} at Freq {freq_dial}: extreme at {extreme_hz:.0} Hz, not {centre:.0}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_the_response_is_the_same_on_either_side_of_the_amp() {
        // One channel at the amp's rate and two at the host's: the same curve
        let settings = on(0.9, 0.2, 0.6, 0.1);
        for freq in [50.0, 300.0, 1_500.0, 8_000.0] {
            let one = measured_db(&settings, freq, 192_000.0);
            let two = measured_db(&settings, freq, 48_000.0);
            assert!((one - two).abs() < 0.5, "{freq} Hz: {one} against {two} dB");
        }
        // Both channels get the same
        let input = clean_di(FS)[..48_000].to_vec();
        let (left, right) = run_stereo(&mut at(FS), &settings, &input, 32);
        assert_eq!(left, right);
        // And a side that is silent stays silent
        let (mut left, mut right) = (input.clone(), vec![0.0; input.len()]);
        at(FS).process_stereo(&settings, &mut left, &mut right);
        assert!(rms(&left) > 0.1 * rms(&input));
        // (a filter's anti-denormal bias, 1e-20, is all that comes out)
        assert!(right.iter().all(|&sample| sample.abs() < 1e-12));
    }

    #[test]
    fn test_the_block_size_does_not_matter() {
        let input = guitar_di(FS)[..48_000].to_vec();
        let settings = on(0.8, 0.3, 0.7, 0.2);
        let whole = run_mono(&mut at(FS), &settings, &input, input.len());
        for block in [1, 7, 64, 480] {
            assert!(run_mono(&mut at(FS), &settings, &input, block) == whole, "{block}");
        }
        let stereo = run_stereo(&mut at(FS), &settings, &input, input.len());
        for block in [1, 33, 480] {
            assert!(run_stereo(&mut at(FS), &settings, &input, block) == stereo, "{block}");
        }
    }

    /// The largest step between samples of the same signal with the dials where they are
    fn steady_step(settings: &EqSettings, input: &[f32]) -> f32 {
        let output = run_mono(&mut at(FS), settings, input, 64);
        largest_step(&output[24_000..])
    }

    #[test]
    fn test_dials_turned_while_playing_do_not_click() {
        // Two tones and a sweep of every dial from one end to the other, a step in each block
        // of 64 samples, the way a host automates them
        let tones: Vec<f32> = sine(220.0, 0.2, FS, 96_000)
            .iter()
            .zip(sine(1_700.0, 0.1, FS, 96_000))
            .map(|(a, b)| a + b)
            .collect();
        let ends = [on(0.0, 0.0, 0.0, 0.0), on(1.0, 1.0, 1.0, 1.0), on(1.0, 0.0, 0.5, 1.0)];
        // The most the signal steps with the dials standing at any of these
        // along the way too: a bell passing a tone is louder there than at either end
        let own = (0..=10)
            .flat_map(|share| {
                let share = share as f32 / 10.0;
                ends.into_iter().flat_map(move |from| {
                    ends.into_iter().map(move |to| {
                        let at = |a: f32, b: f32| lerp(a, b, share);
                        on(
                            at(from.low, to.low),
                            at(from.mid, to.mid),
                            at(from.freq, to.freq),
                            at(from.high, to.high),
                        )
                    })
                })
            })
            .map(|settings| steady_step(&settings, &tones))
            .fold(0.0, f32::max);
        assert!(own > 0.0);
        for (from, to) in [(0, 1), (1, 0), (0, 2), (2, 1)] {
            let mut eq = at(FS);
            let mut output = tones.clone();
            let blocks = output.len() / 64;
            for (index, block) in output.chunks_mut(64).enumerate() {
                let share = (index as f32 / (blocks as f32 * 0.5)).min(1.0);
                let mix = |a: f32, b: f32| lerp(a, b, share);
                let (a, b) = (ends[from], ends[to]);
                let settings = on(
                    mix(a.low, b.low),
                    mix(a.mid, b.mid),
                    mix(a.freq, b.freq),
                    mix(a.high, b.high),
                );
                eq.process_mono(&settings, block);
            }
            let step = largest_step(&output[2_000..]);
            assert!(
                step < 1.05 * own,
                "Dials {from} to {to}: a step of {step}, the signal's own is {own}"
            );
        }
        // A dial thrown from one end to the other between two blocks too
        // (all four dials jump at once between ends 12 dB apart: the filters' state, which
        // belongs to the old curve, rings a little into the new one)
        let mut eq = at(FS);
        let mut output = tones.clone();
        for (index, block) in output.chunks_mut(64).enumerate() {
            let settings = if index % 100 < 50 { ends[0] } else { ends[1] };
            eq.process_mono(&settings, block);
        }
        let step = largest_step(&output[2_000..]);
        assert!(step < 1.15 * own, "Thrown: a step of {step}, the signal's own is {own}");
    }

    #[test]
    fn test_switching_on_and_off_does_not_click() {
        let input = sine(220.0, 0.3, FS, 48_000);
        let mut eq = at(FS);
        let loud = on(1.0, 1.0, 0.5, 1.0);
        let mut output = input.clone();
        for (index, block) in output.chunks_mut(64).enumerate() {
            let settings = EqSettings { on: index % 150 < 75, ..loud };
            eq.process_mono(&settings, block);
        }
        let own = largest_step(&run_mono(&mut at(FS), &loud, &input, 64)[24_000..]);
        assert!(largest_step(&output[1_000..]) < 1.05 * own);
    }

    #[test]
    fn test_a_band_let_go_back_to_flat_ends_up_untouched() {
        let input = guitar_di(FS)[..48_000].to_vec();
        let mut eq = at(FS);
        let mut output = input.clone();
        let (first, rest) = output.split_at_mut(12_000);
        for block in first.chunks_mut(64) {
            eq.process_mono(&on(1.0, 0.0, 0.3, 0.0), block);
        }
        for block in rest.chunks_mut(64) {
            eq.process_mono(&on(0.5, 0.5, 0.3, 0.5), block);
        }
        // Well after the glide, nothing of it is left
        assert!(output[24_000..] == input[24_000..]);
        assert_ne!(output[..12_000], input[..12_000]);
    }

    #[test]
    fn test_switched_off_the_pedal_is_idle_once_it_has_faded() {
        let mut eq = at(FS);
        let mut block = [0.1f32; 64];
        assert!(eq.is_idle());
        eq.process_mono(&on(1.0, 0.5, 0.5, 0.5), &mut block);
        assert!(!eq.is_idle());
        let off = EqSettings::default();
        for _ in 0..10 {
            eq.process_mono(&off, &mut block);
        }
        assert!(eq.is_idle());
        // Off for good, it leaves a block as it is
        let mut block = [0.3f32; 64];
        eq.process_mono(&off, &mut block);
        assert_eq!(block, [0.3f32; 64]);
    }

    #[test]
    fn test_moved_to_the_other_side_it_starts_from_rest() {
        let mut eq = at(FS);
        let mut block = [0.2f32; 64];
        eq.process_mono(&on(1.0, 1.0, 0.5, 1.0), &mut block);
        assert!(!eq.is_idle());
        eq.set_sample_rate(192_000.0);
        assert!(eq.is_idle());
    }

    /// Prints what the pedal does at the dials' ends and middle, and what it costs. Run before
    /// and after a change:
    ///   cargo test -p amp --release eq_report -- --ignored --nocapture
    #[test]
    #[ignore]
    fn eq_report() {
        pin_thread();
        println!("EQ pedal. Gain in dB at a sine, measured through the pedal.");
        println!();
        let freqs = [20.0, 50.0, 100.0, 200.0, 400.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0];
        for rate in [48_000.0, 192_000.0] {
            println!(
                "Bands one at a time at {rate} Hz ({}):",
                if rate > 100_000.0 {
                    "in front of the amp, at 4x 48 kHz"
                } else {
                    "behind the cabinet"
                }
            );
            print!("{:>26}", "dial");
            for freq in freqs {
                print!("{freq:>8.0}");
            }
            println!("   (Hz)");
            for (name, make) in [
                ("Low", (|dial| on(dial, 0.5, 0.5, 0.5)) as fn(f32) -> EqSettings),
                ("Mid at Freq 5 (1 kHz)", |dial| on(0.5, dial, 0.5, 0.5)),
                ("High", |dial| on(0.5, 0.5, 0.5, dial)),
            ] {
                for dial in [0.0, 0.5, 1.0] {
                    print!("{:>26}", format!("{name} {:.0}", dial * 10.0));
                    for freq in freqs {
                        print!("{:>8.2}", measured_db(&make(dial), freq, rate));
                    }
                    println!();
                }
            }
            println!();
        }

        println!("Mid across Freq: gain in dB on the centre Freq says, and where the response is");
        println!("furthest from flat (Mid 10 and Mid 0), at 48 kHz.");
        println!();
        println!(
            "{:>8}{:>10}{:>12}{:>12}{:>14}{:>14}",
            "Freq", "centre Hz", "Mid 10 dB", "Mid 0 dB", "peak Hz +", "peak Hz -"
        );
        for freq_dial in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let centre = mid_hz(freq_dial);
            let mut row = Vec::new();
            for dial in [1.0, 0.0] {
                let settings = on(0.5, dial, freq_dial, 0.5);
                row.push(measured_db(&settings, centre, FS));
                let mut extreme = (0.0f32, centre);
                for step in -16..=16 {
                    let hz = centre * 2.0f32.powf(step as f32 / 16.0 * 2.0);
                    let level = measured_db(&settings, hz, FS);
                    if level.abs() > extreme.0.abs() {
                        extreme = (level, hz);
                    }
                }
                row.push(extreme.1);
            }
            println!(
                "{:>8.1}{centre:>10.0}{:>12.2}{:>12.2}{:>14.0}{:>14.0}",
                freq_dial * 10.0,
                row[0],
                row[2],
                row[1],
                row[3]
            );
        }

        println!();
        println!("Flat: every band at 5 passes the signal untouched (to the bit): checked by");
        println!("test_on_with_every_band_flat_leaves_the_signal_to_the_bit.");

        println!();
        println!("Dials turned while playing: largest step between samples against the signal's");
        println!(
            "own with the dials where they end (a sweep of all four over 0.5 s, 220 Hz + 1.7 kHz)."
        );
        let tones: Vec<f32> = sine(220.0, 0.2, FS, 96_000)
            .iter()
            .zip(sine(1_700.0, 0.1, FS, 96_000))
            .map(|(a, b)| a + b)
            .collect();
        for (name, from, to) in [
            ("all down to all up", on(0.0, 0.0, 0.0, 0.0), on(1.0, 1.0, 1.0, 1.0)),
            ("all up to all down", on(1.0, 1.0, 1.0, 1.0), on(0.0, 0.0, 0.0, 0.0)),
        ] {
            let own = steady_step(&to, &tones).max(steady_step(&from, &tones));
            let mut eq = at(FS);
            let mut output = tones.clone();
            let blocks = output.len() / 64;
            for (index, block) in output.chunks_mut(64).enumerate() {
                let share = (index as f32 / (blocks as f32 * 0.5)).min(1.0);
                let mix = |a: f32, b: f32| lerp(a, b, share);
                eq.process_mono(
                    &on(
                        mix(from.low, to.low),
                        mix(from.mid, to.mid),
                        mix(from.freq, to.freq),
                        mix(from.high, to.high),
                    ),
                    block,
                );
            }
            println!(
                "  {name:<22} step {:.4}, signal's own {own:.4} ({:.2}x)",
                largest_step(&output[2_000..]),
                largest_step(&output[2_000..]) / own
            );
        }

        println!();
        println!("Cost: the pedal by itself on a guitar playing power chords, median and");
        println!("99th-percentile block as a share of the time the block lasts. In front of the");
        println!("amp it runs on one channel at 4x the host's rate (2x from 88.2 kHz), behind the");
        println!("cabinet on two at the host's rate. Blocks of 64 samples of the host.");
        println!();
        println!(
            "{:>10}{:>12}{:>18}{:>18}",
            "rate Hz", "side", "all flat (on)", "every band moved"
        );
        for rate in [48_000.0f32, 96_000.0, 192_000.0] {
            let factor = if rate < 88_200.0 { 4 } else { 2 };
            for (side, front) in [("in front", true), ("behind", false)] {
                let run_rate = if front { rate * factor as f32 } else { rate };
                let chords = power_chords(run_rate, 2.0);
                let block_len = if front { 64 * factor } else { 64 };
                let mut cells = Vec::new();
                for settings in [on(0.5, 0.5, 0.5, 0.5), on(0.9, 0.2, 0.7, 0.1)] {
                    let mut eq = at(run_rate);
                    let (mut left, mut right) = (chords.clone(), chords.clone());
                    for (l, r) in left.chunks_mut(block_len).zip(right.chunks_mut(block_len)) {
                        if front {
                            eq.process_mono(&settings, l);
                        } else {
                            eq.process_stereo(&settings, l, r);
                        }
                    }
                    let mut times: Vec<f64> = chords
                        .chunks(block_len)
                        .map(|block| {
                            let (mut l, mut r) = (block.to_vec(), block.to_vec());
                            let start = std::time::Instant::now();
                            if front {
                                eq.process_mono(&settings, &mut l);
                            } else {
                                eq.process_stereo(&settings, &mut l, &mut r);
                            }
                            start.elapsed().as_secs_f64()
                        })
                        .collect();
                    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let lasts = 64.0 / rate as f64;
                    cells.push(format!(
                        "{:.2} / {:.2}",
                        100.0 * times[times.len() / 2] / lasts,
                        100.0 * times[times.len() * 99 / 100] / lasts
                    ));
                }
                println!("{rate:>10.0}{side:>12}{:>18}{:>18}", cells[0], cells[1]);
            }
        }
    }
}
