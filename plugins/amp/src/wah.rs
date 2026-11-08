//! Wah pedal: a resonant band-pass filter whose centre sweeps from 350 Hz (heel down, woofy) to
//! 2.2 kHz (toe down, thin and vocal). In Pedal mode one dial, the rocker, is turned by hand
//! or automated by the host (a controller or an expression pedal mapped to it). In Auto mode the
//! playing sweeps it: a harder pick opens it further and it falls back in a fraction of a
//! second. Bite is the filter's resonance. Runs in front of the amp only, on one channel at
//! the amp's oversampled rate, and adds no latency

use crate::dsp::filters::ANTI_DENORMAL;
use crate::dsp::{curve, db_to_gain, lerp, smoothing_coeff};
use crate::mods::{Dial, Fade};
use nih_plug::prelude::Enum;

/// Where the centre is with the rocker at the heel and at the toe, in Hz. The sweep is by
/// octaves
pub const CENTRE_HZ: [f32; 2] = [350.0, 2200.0];

/// The filter's Q with Bite at 0 and at 10. A higher Q is a narrower, more vocal peak
pub const Q: [f32; 2] = [3.0, 9.0];

// A band-pass has no low end at all, which on a guitar is most of the note. This share of the
// low-pass (the same filter's other output) is added to it
const LOW_SHARE: f64 = 0.12;

// What the pedal's output is raised by, in dB, so that the guitar through the whole sweep is
// about as loud as with the pedal off (a guitar at -24 dBFS RMS, `PEDAL_DI_DB`): across the
// rocker (evenly from heel to toe) and across Bite (0, 5, 10)
const MAKEUP_ROCKER_DB: [f32; 5] = [7.2, 7.7, 8.1, 8.8, 9.8];
const MAKEUP_BITE_DB: [f32; 3] = [-1.0, 0.2, 0.8];

// Auto mode: how fast the follower of the playing rises and falls, and the level of it (a
// plain average of the rectified signal) that opens the pedal as far as Sens allows
const ENV_ATTACK_MS: f32 = 5.0;
const ENV_RELEASE_MS: f32 = 150.0;
const ENV_FULL: f32 = 0.2;

// The pedal's position is worked out once per this many samples of the stream at the rate it is
// called at, whatever the blocks it comes in, and followed in this time from tick to tick (the
// filter is designed once a tick)
const PIECE: usize = 32;
const DIAL_SMOOTH_MS: f32 = 3.0;

/// What the rocker does
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WahMode {
    /// The rocker is where the dial (or the host) puts it
    #[id = "pedal"]
    #[name = "Pedal"]
    Pedal,
    /// The playing moves it up from where the dial rests
    #[id = "auto"]
    #[name = "Auto"]
    Auto,
}

/// What the knobs say, read once per call. The pedal follows the values itself
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WahSettings {
    pub on: bool,
    pub mode: WahMode,
    /// The rocker, 0.0 (heel) to 1.0 (toe). In Auto mode its resting position
    pub pedal: f32,
    /// Bite, 0.0 to 1.0: see `Q`
    pub bite: f32,
    /// Auto mode only, 0.0 to 1.0: how far a loud note opens the pedal, as a share of the sweep
    pub sens: f32,
}

impl Default for WahSettings {
    fn default() -> Self {
        Self { on: false, mode: WahMode::Pedal, pedal: 0.5, bite: 0.5, sens: 0.5 }
    }
}

/// Where the centre is with the rocker at `position` (0.0 to 1.0), in Hz
pub fn centre_hz(position: f32) -> f32 {
    CENTRE_HZ[0] * (CENTRE_HZ[1] / CENTRE_HZ[0]).powf(position.clamp(0.0, 1.0))
}

/// The filter's Q with Bite at `bite` (0.0 to 1.0): geometric, so each step of the knob
/// narrows the peak by as much
pub fn q_at(bite: f32) -> f32 {
    Q[0] * (Q[1] / Q[0]).powf(bite.clamp(0.0, 1.0))
}

/// The gain that makes up for what the band-pass takes away
fn makeup(position: f32, bite: f32) -> f64 {
    db_to_gain(curve(&MAKEUP_ROCKER_DB, position) + curve(&MAKEUP_BITE_DB, bite)) as f64
}

/// A state-variable filter in the topology-preserving form: stable at any centre and any
/// sweep speed, at any sample rate from 44.1 kHz up. f64, so that a centre of a few hundred Hz
/// at the amp's highest oversampled rate is still designed to the digit
#[derive(Clone, Copy, Default)]
struct Svf {
    ic1: f64,
    ic2: f64,
}

#[derive(Clone, Copy)]
struct Design {
    a1: f64,
    a2: f64,
    a3: f64,
    /// The damping, the inverse of Q
    k: f64,
}

impl Design {
    fn new(centre_hz: f32, q: f32, sample_rate: f32) -> Self {
        let g = (std::f64::consts::PI * centre_hz as f64 / sample_rate as f64).tan();
        let k = 1.0 / q as f64;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        Design { a1, a2, a3: g * a2, k }
    }
}

impl Svf {
    /// The band-pass (1.0 at its centre, whatever the Q) and the low-pass (Q at its centre)
    fn process(&mut self, input: f64, design: &Design) -> (f64, f64) {
        let v3 = input - self.ic2;
        let v1 = design.a1 * self.ic1 + design.a2 * v3;
        let v2 = self.ic2 + design.a2 * self.ic1 + design.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1 + ANTI_DENORMAL as f64;
        self.ic2 = 2.0 * v2 - self.ic2 + ANTI_DENORMAL as f64;
        (design.k * v1, v2)
    }
}

pub struct Wah {
    filter: Svf,
    // The rocker's position and Bite, on their way to what the settings say. Followed once per
    // tick
    position: Dial,
    bite: Dial,
    fade: Fade,
    // The follower of the playing, for Auto mode
    envelope: f32,
    attack: f32,
    release: f32,
    rate: f32,
    // What the filter is designed for, and the gain behind it, as of the last tick; and how
    // many samples are left before the next one
    design: Design,
    gain: f64,
    until_tick: usize,
    // Where the centre is, as of the last tick, 0.0 to 1.0
    reached: f32,
    // Off and faded out: the signal is left alone
    idle: bool,
}

impl Wah {
    /// Allocates nothing: the filter is two numbers
    pub fn new() -> Self {
        let mut wah = Self {
            filter: Svf::default(),
            position: Dial::new(),
            bite: Dial::new(),
            fade: Fade::new(),
            envelope: 0.0,
            attack: 1.0,
            release: 1.0,
            rate: 192_000.0,
            design: Design::new(centre_hz(0.5), q_at(0.5), 192_000.0),
            gain: 1.0,
            until_tick: 0,
            reached: 0.5,
            idle: true,
        };
        wah.set_sample_rate(192_000.0);
        wah
    }

    /// The rate the pedal is called at, which starts it over. Never allocates
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.rate = sample_rate;
        let tick_rate = sample_rate / PIECE as f32;
        self.position.set_time(DIAL_SMOOTH_MS, tick_rate);
        self.bite.set_time(DIAL_SMOOTH_MS, tick_rate);
        self.fade.set_sample_rate(sample_rate);
        self.attack = smoothing_coeff(ENV_ATTACK_MS, sample_rate);
        self.release = smoothing_coeff(ENV_RELEASE_MS, sample_rate);
        self.reset();
    }

    /// As after a long time switched off. A pedal that is on fades in again
    pub fn reset(&mut self) {
        self.fade.snap(false);
        self.idle = true;
    }

    /// True while the pedal is off and has faded out: the next block comes out as it goes in
    #[cfg(test)]
    pub fn is_idle(&self) -> bool {
        self.idle
    }

    /// Where the rocker was as of the last tick, 0.0 to 1.0
    #[cfg(test)]
    pub fn reached(&self) -> f32 {
        self.reached
    }

    /// Where the rocker is to be, for one tick: the dial, and in Auto mode the playing's
    /// share of the way up from it
    fn target(&self, settings: &WahSettings) -> f32 {
        let rest = settings.pedal.clamp(0.0, 1.0);
        match settings.mode {
            WahMode::Pedal => rest,
            WahMode::Auto => {
                let level = (self.envelope / ENV_FULL).min(1.0);
                (rest + settings.sens.clamp(0.0, 1.0) * level).min(1.0)
            }
        }
    }

    /// In front of the amp: one channel, at the amp's oversampled rate
    pub fn process_mono(&mut self, settings: &WahSettings, block: &mut [f32]) {
        if self.idle && !settings.on {
            return;
        }
        if self.idle {
            // Nothing is heard yet, so there is nothing to follow
            self.filter = Svf::default();
            self.envelope = 0.0;
            self.position.snap(self.target(settings));
            self.bite.snap(settings.bite.clamp(0.0, 1.0));
            self.until_tick = 0;
            self.idle = false;
        }
        for sample in block.iter_mut() {
            // The playing, for Auto mode: the follower rises fast and falls slowly
            let level = sample.abs();
            let coeff = if level > self.envelope { self.attack } else { self.release };
            self.envelope += coeff * (level - self.envelope);
            // Once per `PIECE` samples of the stream, whatever the blocks it comes in
            if self.until_tick == 0 {
                self.until_tick = PIECE;
                self.reached = self.position.next(self.target(settings));
                let bite = self.bite.next(settings.bite.clamp(0.0, 1.0));
                self.design = Design::new(centre_hz(self.reached), q_at(bite), self.rate);
                self.gain = makeup(self.reached, bite);
            }
            self.until_tick -= 1;
            let mix = self.fade.next(settings.on);
            if mix == 0.0 && !settings.on {
                // Faded out: the rest of the block is the input itself
                self.idle = true;
                return;
            }
            let dry = *sample;
            let (band, low) = self.filter.process(dry as f64, &self.design);
            let wet = (self.gain * (band + LOW_SHARE * low)) as f32;
            *sample = if mix >= 1.0 { wet } else { lerp(dry, wet, mix) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    const FS: f32 = 48_000.0;
    // The rate the pedal runs at in front of the amp, in a host at 48 kHz
    const FRONT: f32 = 192_000.0;

    /// The rocker by hand at `pedal`, with Bite at `bite`
    fn on(pedal: f32, bite: f32) -> WahSettings {
        WahSettings { on: true, pedal, bite, ..WahSettings::default() }
    }

    fn auto(pedal: f32, sens: f32) -> WahSettings {
        WahSettings { on: true, mode: WahMode::Auto, pedal, sens, ..WahSettings::default() }
    }

    fn at(rate: f32) -> Wah {
        let mut wah = Wah::new();
        wah.set_sample_rate(rate);
        wah
    }

    fn run(wah: &mut Wah, settings: &WahSettings, input: &[f32], block: usize) -> Vec<f32> {
        let mut output = input.to_vec();
        output.chunks_mut(block).for_each(|piece| wah.process_mono(settings, piece));
        output
    }

    /// The pedal's gain in dB at a frequency, measured on a sine once it has settled
    fn gain_db(settings: &WahSettings, freq_hz: f32, rate: f32) -> f32 {
        let input = sine(freq_hz, 0.1, rate, (0.12 * rate) as usize);
        let output = run(&mut at(rate), settings, &input, 256);
        let half = output.len() / 2;
        to_db(level_at(&output[half..], rate, freq_hz) / level_at(&input[half..], rate, freq_hz))
    }

    /// Where the response peaks, the width of it 3 dB down, and the peak's height, read from
    /// the pedal on a sine at a 48th of an octave apart, within an octave of `around_hz`
    pub(crate) fn response(settings: &WahSettings, around_hz: f32, rate: f32) -> (f32, f32, f32) {
        let step = 2.0f32.powf(1.0 / 48.0);
        let freqs: Vec<f32> = (-48..=48).map(|n| around_hz * step.powi(n)).collect();
        let db: Vec<f32> = freqs.iter().map(|&freq| gain_db(settings, freq, rate)).collect();
        let top = (1..db.len() - 1)
            .max_by(|&a, &b| db[a].total_cmp(&db[b]))
            .expect("a response has a peak");
        // The peak between the points: a parabola through the three around it, in octaves
        let (a, b, c) = (db[top - 1], db[top], db[top + 1]);
        let shift = 0.5 * (a - c) / (a - 2.0 * b + c);
        let peak_hz = freqs[top] * step.powf(shift);
        let peak_db = b - 0.25 * (a - c) * shift;
        // Where it has fallen by 3 dB on either side, between two points
        let crossing = |range: &mut dyn Iterator<Item = usize>| {
            let mut last = top;
            for index in range {
                if db[index] < peak_db - 3.0 {
                    let share = (db[last] - (peak_db - 3.0)) / (db[last] - db[index]);
                    return freqs[last] * (freqs[index] / freqs[last]).powf(share);
                }
                last = index;
            }
            f32::NAN
        };
        let below = crossing(&mut (0..top).rev());
        let above = crossing(&mut (top + 1..db.len()));
        (peak_hz, above - below, peak_db)
    }

    #[test]
    fn test_the_rocker_and_bite_map_to_hz_and_q() {
        assert!((centre_hz(0.0) - 350.0).abs() < 1e-3);
        assert!((centre_hz(1.0) - 2200.0).abs() < 1e-2);
        // On a log scale: the middle is the geometric mean
        assert!((centre_hz(0.5) - (350.0f32 * 2200.0).sqrt()).abs() < 0.1);
        assert!((centre_hz(-1.0) - 350.0).abs() < 1e-3 && (centre_hz(2.0) - 2200.0).abs() < 1e-2);
        assert!((q_at(0.0) - 3.0).abs() < 1e-5 && (q_at(1.0) - 9.0).abs() < 1e-4);
        let defaults = WahSettings::default();
        assert!(!defaults.on);
        assert_eq!(defaults.mode, WahMode::Pedal);
        assert_eq!((defaults.pedal, defaults.bite, defaults.sens), (0.5, 0.5, 0.5));
    }

    #[test]
    fn test_the_block_size_does_not_matter() {
        let input = guitar_di(FS)[..48_000].to_vec();
        for settings in [on(0.3, 0.7), auto(0.2, 0.8)] {
            let whole = run(&mut at(FRONT), &settings, &input, input.len());
            for block in [1, 7, 33, 64, 480] {
                assert!(run(&mut at(FRONT), &settings, &input, block) == whole, "{block}");
            }
        }
    }

    #[test]
    fn test_off_leaves_the_signal_to_the_bit() {
        let input = guitar_di(FS)[..48_000].to_vec();
        for mode in [WahMode::Pedal, WahMode::Auto] {
            let moved = WahSettings { on: false, mode, pedal: 1.0, bite: 1.0, sens: 1.0 };
            assert_eq!(run(&mut at(FS), &moved, &input, 64), input);
        }
        assert!(at(FS).is_idle());
    }

    #[test]
    fn test_the_response_peaks_on_the_mapped_centre_and_narrows_with_bite() {
        for rate in [44_100.0, FRONT] {
            for pedal in [0.0, 0.5, 1.0] {
                let mut widths = Vec::new();
                for bite in [0.0, 0.5, 1.0] {
                    let (peak, width, _) = response(&on(pedal, bite), centre_hz(pedal), rate);
                    let centre = centre_hz(pedal);
                    assert!(
                        (peak / centre - 1.0).abs() < 0.05,
                        "{rate} Hz, rocker {pedal}, Bite {bite}: peak at {peak:.0} Hz, not {centre:.0}"
                    );
                    widths.push(width);
                }
                assert!(
                    widths[0] > widths[1] && widths[1] > widths[2],
                    "{rate} Hz, rocker {pedal}: the peak is {widths:?} Hz wide at Bite 0, 5, 10"
                );
            }
        }
    }

    #[test]
    fn test_the_level_through_the_sweep_is_within_3_db_of_the_pedal_off() {
        for (name, recording) in [("riffs", guitar_di(FS)), ("clean", clean_di(FS))] {
            let input = at_rms_db(&recording, PEDAL_DI_DB);
            for bite in [0.0, 0.5, 1.0] {
                for pedal in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    let output = run(&mut at(FS), &on(pedal, bite), &input, 256);
                    let apart = to_db(rms(&output[24_000..])) - to_db(rms(&input[24_000..]));
                    assert!(
                        apart.abs() < 3.0,
                        "{name}, rocker {pedal}, Bite {bite}: {apart:.2} dB from off"
                    );
                }
            }
        }
    }

    /// The centre after every piece of 32 samples, and when each is, in ms
    fn centre_track(wah: &mut Wah, settings: &WahSettings, input: &[f32], rate: f32) -> Vec<f32> {
        let mut track = Vec::new();
        for piece in input.chunks(PIECE) {
            let mut piece = piece.to_vec();
            wah.process_mono(settings, &mut piece);
            track.push(wah.reached());
        }
        let _ = rate;
        track
    }

    /// A note at -12 dBFS after a stretch of silence, then silence again: when the rocker has
    /// opened and when it has come back, in ms from the note's first and last sample
    pub(crate) fn auto_timing(settings: &WahSettings, rate: f32) -> (f32, f32, f32) {
        let silence = (0.2 * rate) as usize;
        let note = (0.15 * rate) as usize;
        let after = (0.8 * rate) as usize;
        let mut input = vec![0.0f32; silence];
        let level = 10.0f32.powf(-12.0 / 20.0);
        // A low string and its octave, as a pluck has them, with the decay of one
        input.extend((0..note).map(|n| {
            let t = n as f32 / rate;
            let tones = 0.7 * (std::f32::consts::TAU * 110.0 * t).sin()
                + 0.3 * (std::f32::consts::TAU * 220.0 * t).sin();
            level * tones * (-t * 4.0).exp()
        }));
        input.extend(std::iter::repeat_n(0.0, after));
        let track = centre_track(&mut at(rate), settings, &input, rate);
        let ms = |piece: usize| piece as f32 * PIECE as f32 / rate * 1000.0;
        let (start, end) = (silence / PIECE, (silence + note) / PIECE);
        let rest = settings.pedal;
        let top = track.iter().copied().fold(rest, f32::max);
        let opened = (start..track.len())
            .find(|&piece| track[piece] >= rest + 0.75 * (top - rest))
            .map_or(f32::NAN, |piece| ms(piece - start));
        let back = (end..track.len())
            .find(|&piece| track[piece..].iter().all(|&at| at < rest + 0.05))
            .map_or(f32::NAN, |piece| ms(piece - end));
        (opened, back, top)
    }

    #[test]
    fn test_auto_mode_opens_on_a_loud_note_within_20_ms_and_is_back_within_400() {
        for rate in [44_100.0, FRONT] {
            let (opened, back, top) = auto_timing(&auto(0.0, 0.5), rate);
            assert!(top > 0.3, "{rate} Hz: a note at -12 dBFS only opens it to {top}");
            assert!(opened <= 20.0, "{rate} Hz: opened in {opened} ms");
            assert!(back <= 400.0, "{rate} Hz: back in {back} ms");
        }
    }

    #[test]
    fn test_auto_mode_opens_further_for_a_harder_pick_and_sens_sets_how_far() {
        let note = |level_db: f32| {
            let level = 10.0f32.powf(level_db / 20.0);
            sine(196.0, level, FS, 12_000)
        };
        let reached = |settings: &WahSettings, input: &[f32]| {
            let mut wah = at(FS);
            centre_track(&mut wah, settings, input, FS).into_iter().fold(0.0, f32::max)
        };
        let (soft, hard) =
            (reached(&auto(0.1, 0.5), &note(-30.0)), reached(&auto(0.1, 0.5), &note(-10.0)));
        assert!(hard > soft + 0.1, "a hard pick opens to {hard}, a soft one to {soft}");
        let (little, much) =
            (reached(&auto(0.1, 0.2), &note(-10.0)), reached(&auto(0.1, 0.9), &note(-10.0)));
        assert!(much > little + 0.3, "Sens 9 opens to {much}, Sens 2 to {little}");
        // Sens at 0: the rocker stays where the dial rests
        assert!((reached(&auto(0.4, 0.0), &note(-6.0)) - 0.4).abs() < 1e-4);
        // In Pedal mode the playing has no say
        let by_hand = WahSettings { mode: WahMode::Pedal, ..auto(0.4, 1.0) };
        assert!((reached(&by_hand, &note(-6.0)) - 0.4).abs() < 1e-4);
    }

    /// The largest step of the signal with the rocker standing still at `pedal`
    fn steady_step(settings: &WahSettings, input: &[f32], rate: f32) -> f32 {
        let output = run(&mut at(rate), settings, input, 64);
        largest_step(&output[(0.25 * rate) as usize..])
    }

    #[test]
    fn test_a_fast_sweep_of_the_rocker_does_not_click_at_any_rate() {
        for rate in [44_100.0, FS, FRONT] {
            let len = (0.6 * rate) as usize;
            let tones: Vec<f32> = sine(220.0, 0.2, rate, len)
                .iter()
                .zip(sine(1_700.0, 0.1, rate, len))
                .map(|(a, b)| a + b)
                .collect();
            for bite in [0.0, 1.0] {
                // What the signal does by itself, with the rocker standing anywhere
                let own = (0..=20)
                    .map(|n| steady_step(&on(n as f32 / 20.0, bite), &tones, rate))
                    .fold(0.0, f32::max);
                assert!(own > 0.0);
                // The rocker from heel to toe and back in 40 ms, a step in every block of 64
                // as a host automates it, and thrown from one end to the other between two
                // blocks
                let mut swept = at(rate);
                let mut thrown = at(rate);
                let (mut sweep, mut throw) = (tones.clone(), tones.clone());
                let sweep_blocks = (0.04 * rate) as usize / 64;
                for (index, (a, b)) in sweep.chunks_mut(64).zip(throw.chunks_mut(64)).enumerate() {
                    let phase = (index % (2 * sweep_blocks)) as f32 / sweep_blocks as f32;
                    let pedal = if phase < 1.0 { phase } else { 2.0 - phase };
                    swept.process_mono(&on(pedal, bite), a);
                    thrown.process_mono(&on(if index % 100 < 50 { 0.0 } else { 1.0 }, bite), b);
                }
                let from = (0.25 * rate) as usize;
                let (sweep_step, throw_step) =
                    (largest_step(&sweep[from..]), largest_step(&throw[from..]));
                assert!(
                    sweep_step < 1.05 * own,
                    "{rate} Hz, Bite {bite}: a sweep steps {sweep_step}, the signal's own is {own}"
                );
                assert!(
                    throw_step < 1.15 * own,
                    "{rate} Hz, Bite {bite}: a throw steps {throw_step}, the signal's own is {own}"
                );
            }
        }
    }

    #[test]
    fn test_switching_on_and_off_does_not_click() {
        for rate in [44_100.0, FRONT] {
            let input = sine(220.0, 0.3, rate, rate as usize);
            let mut wah = at(rate);
            let mut output = input.clone();
            for (index, block) in output.chunks_mut(64).enumerate() {
                let settings = WahSettings { on: index % 500 < 250, ..on(0.7, 0.8) };
                wah.process_mono(&settings, block);
            }
            let own = largest_step(&sine(220.0, 0.3, rate, rate as usize));
            // The band-pass at 220 Hz is below its centre: it is quieter than the input
            let step = largest_step(&output);
            assert!(step < 1.1 * own, "{rate} Hz: a step of {step}, the signal's own is {own}");
        }
    }

    #[test]
    fn test_the_filter_is_stable_and_bounded_at_every_setting_and_rate() {
        for rate in [44_100.0, FS, 96_000.0, FRONT, 384_000.0] {
            let mut noise = Noise::new(7);
            let input: Vec<f32> = (0..(0.5 * rate) as usize).map(|_| 0.5 * noise.next()).collect();
            for mode in [WahMode::Pedal, WahMode::Auto] {
                let mut wah = at(rate);
                let mut output = input.clone();
                for (index, block) in output.chunks_mut(37).enumerate() {
                    // Thrown about every block, as no player would
                    let pedal = if index % 2 == 0 { 0.0 } else { 1.0 };
                    let bite = if index % 3 == 0 { 0.0 } else { 1.0 };
                    wah.process_mono(&WahSettings { mode, sens: 1.0, ..on(pedal, bite) }, block);
                }
                let largest = peak(&output);
                assert!(output.iter().all(|sample| sample.is_finite()), "{rate} Hz");
                assert!(largest < 20.0, "{rate} Hz {mode:?}: peaks at {largest}");
            }
        }
    }

    #[test]
    fn test_switched_off_the_pedal_is_idle_once_it_has_faded_and_starts_over_when_woken() {
        let input = sine(220.0, 0.3, FS, 48_000);
        let mut wah = at(FS);
        assert!(wah.is_idle());
        run(&mut wah, &on(0.5, 0.5), &input[..4_800], 64);
        assert!(!wah.is_idle());
        let mut rest = input[..4_800].to_vec();
        wah.process_mono(&WahSettings { on: false, ..on(0.5, 0.5) }, &mut rest);
        assert!(wah.is_idle());
        // Woken again it plays as a new pedal does
        let again = run(&mut wah, &on(0.5, 0.5), &input, 64);
        let new = run(&mut at(FS), &on(0.5, 0.5), &input, 64);
        assert!(again == new);
        // And a pedal reset from outside is idle too
        wah.reset();
        assert!(wah.is_idle());
    }

    #[test]
    fn test_the_wah_is_a_band_pass_with_some_low_end_kept() {
        // Far below the centre it falls away, but not to nothing: a note's body is kept
        let settings = on(0.5, 0.5);
        let centre = centre_hz(0.5);
        let (_, _, top) = response(&settings, centre, FS);
        let low = gain_db(&settings, 80.0, FS);
        let high = gain_db(&settings, 8_000.0, FS);
        assert!(low < top - 6.0 && low > top - 30.0, "80 Hz at {low:.1}, the peak at {top:.1}");
        assert!(high < top - 12.0, "8 kHz at {high:.1}, the peak at {top:.1}");
    }

    /// Prints what the pedal does: the response at the rocker's heel, middle and toe and three
    /// Bite settings, the level through the sweep, Auto mode's timing and what it costs. Run
    /// before and after a change:
    ///   cargo test -p amp --release wah_report -- --ignored --nocapture
    #[test]
    #[ignore]
    fn wah_report() {
        pin_thread();
        println!("Wah pedal.");
        println!();
        for rate in [44_100.0, FRONT] {
            println!("Response at {rate} Hz, read from the pedal on sines, by Bite 0, 5, 10:");
            println!(
                "{:>8}{:>10}{:>8}{:>26}{:>26}{:>26}",
                "rocker", "mapped Hz", "", "Bite 0", "Bite 5", "Bite 10"
            );
            println!(
                "{:>26}{:>26}{:>26}",
                "peak Hz / width Hz / dB", "peak Hz / width Hz / dB", "peak Hz / width Hz / dB"
            );
            for pedal in [0.0, 0.5, 1.0] {
                let mapped = centre_hz(pedal);
                print!("{:>8.0}{mapped:>10.0}{:>8}", pedal * 10.0, "");
                for bite in [0.0, 0.5, 1.0] {
                    let (peak, width, db) = response(&on(pedal, bite), mapped, rate);
                    print!(
                        "{:>26}",
                        format!(
                            "{peak:.0} ({:+.1} %) / {width:.0} / {db:.1}",
                            100.0 * (peak / mapped - 1.0)
                        )
                    );
                }
                println!();
            }
            println!();
        }

        println!("Level through the sweep: the guitar at -24 dBFS RMS (`PEDAL_DI_DB`), 48 kHz,");
        println!(
            "the output against the input, in dB, over the last 10 s. Within +-3 dB is the target."
        );
        println!();
        let input = at_rms_db(&guitar_di(FS), PEDAL_DI_DB);
        println!("{:>8}{:>10}{:>10}{:>10}", "rocker", "Bite 0", "Bite 5", "Bite 10");
        let mut spread = (f32::MAX, f32::MIN);
        for pedal in [0.0, 0.1, 0.25, 0.4, 0.5, 0.6, 0.75, 0.9, 1.0] {
            print!("{:>8.1}", pedal * 10.0);
            for bite in [0.0, 0.5, 1.0] {
                let output = run(&mut at(FS), &on(pedal, bite), &input, 256);
                let apart = to_db(rms(&output[24_000..])) - to_db(rms(&input[24_000..]));
                spread = (spread.0.min(apart), spread.1.max(apart));
                print!("{apart:>10.2}");
            }
            println!();
        }
        println!("Spread: {:.2} to {:.2} dB", spread.0, spread.1);
        println!();

        println!(
            "Auto mode, a note at -12 dBFS (150 ms, a low string and its octave) after silence:"
        );
        println!(
            "{:>10}{:>6}{:>14}{:>14}{:>14}",
            "rate", "Sens", "opens in ms", "back in ms", "opens to"
        );
        for rate in [44_100.0, FRONT] {
            for sens in [0.2, 0.5, 0.9] {
                let (opened, back, top) = auto_timing(&auto(0.0, sens), rate);
                println!("{rate:>10.0}{:>6.1}{opened:>14.1}{back:>14.1}{top:>14.2}", sens * 10.0);
            }
        }
        println!();

        println!(
            "Cost: the pedal by itself on, in front of the amp at 192 kHz (4x 48 kHz), blocks"
        );
        println!("of 64 host samples; % of a core, median / 99th percentile. Rocker swept.");
        let chords = at_rms_db(&guitar_di(FRONT)[..4 * 192_000], PEDAL_DI_DB);
        for mode in [WahMode::Pedal, WahMode::Auto] {
            let mut wah = at(FRONT);
            let block_len = 256;
            let settings = |index: usize| WahSettings {
                mode,
                sens: 0.9,
                ..on((index % 200) as f32 / 200.0, 0.5)
            };
            for (index, block) in chords.chunks(block_len).enumerate().take(500) {
                wah.process_mono(&settings(index), &mut block.to_vec());
            }
            let mut times: Vec<f64> = chords
                .chunks(block_len)
                .enumerate()
                .map(|(index, block)| {
                    let mut block = block.to_vec();
                    let start = std::time::Instant::now();
                    wah.process_mono(&settings(index), &mut block);
                    start.elapsed().as_secs_f64()
                })
                .collect();
            times.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let lasts = 64.0 / FS as f64;
            println!(
                "{mode:>8?}: {:.3} / {:.3}",
                100.0 * times[times.len() / 2] / lasts,
                100.0 * times[times.len() * 99 / 100] / lasts
            );
        }
    }
}
