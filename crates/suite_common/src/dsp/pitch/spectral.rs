//! The shifter's engine: one channel, at a rate of 44.1 to 48 kHz.
//!
//! Every `hop` samples the last `FRAME` samples are taken as a spectrum, the spectrum is moved,
//! and the newest `chunk` samples of the result are overlap-added to the output. The window on
//! the input is long and lopsided: it rises over most of the frame and falls over the last half
//! chunk, so the spectrum has the resolution of a long frame while the output only waits for
//! the chunk. That is the latency: one chunk.
//!
//! Moving the spectrum has two parts, and their sum is the whole frame:
//!
//! 1. The steady partials. A second, symmetric window over the same samples gives a spectrum
//!    in which the partials of a chord stand apart; its peaks, with the frequency read from
//!    their phase, are the tracks. Each track is fitted to the lopsided frame as a sinusoid
//!    (its lobe is known: the transform of the window), taken out, and put back at its shifted
//!    frequency. The lopsided window has wide skirts, and this is what keeps a partial's skirt
//!    from being moved with its neighbour
//! 2. What is left (attacks, noise, partials the tracker has not caught yet or that bend). It
//!    is cut into regions around its peaks and each region is moved as it is, by what its own
//!    frequency or its track says
//!
//! Each track and each region carries a rotation that runs on from frame to frame, so a
//! partial leaves one chunk in the phase the next one takes it up in. A track that does not
//! fit the frame is left to the second part, which moves it by the same amount and rotation,
//! so the two parts agree where they meet.
//!
//! Nothing here allocates after `Engine::new`.

use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::f32::consts::{PI, TAU};
use std::sync::{Arc, OnceLock};

/// Samples in a frame. At 48 kHz its bins are 11.7 Hz apart, which tells the partials of a
/// chord on the low strings apart (41 Hz between a low E and the B above it)
pub const FRAME: usize = 4096;
const BINS: usize = FRAME / 2 + 1;

/// The chunk lengths there are windows for: the latency of the engine, in samples
pub const CHUNKS: [usize; 4] = [256, 512, 1024, 2048];

/// Shortest hop between frames, which bounds the work: 375 frames a second at 48 kHz
const LEAST_HOP: usize = 128;

/// The tracker looks every this many samples
const TRACK_HOP: usize = 512;

/// A sinusoid's lobe is taken out and put back this many bins to either side of it
const LOBE: usize = 32;

/// Points of the lobe table per bin
const LOBE_STEPS: usize = 32;
const LOBE_TABLE: usize = 2 * LOBE * LOBE_STEPS + 1;

/// Most partials followed at once. A six-string chord has about 150 below 6 kHz
const MAX_TRACKS: usize = 256;

/// A tracker peak counts when its frequency lies this close to its bin, in bins
const TRACK_NEAR_BIN: f32 = 0.6;

/// A tracker peak has to be this much of the largest one (-70 dB), and this large (-88 dBFS)
const TRACK_FLOOR: f32 = 0.000_316;
const TRACK_LEAST: f32 = 0.000_01 * FRAME as f32;

/// A track is the same partial as before when it has moved less than this, in bins
const TRACK_SAME: f32 = 0.5;

/// A region goes with a track when its own frequency lies this close to the track's, in bins
const REGION_NEAR_TRACK: f32 = 1.5;

/// A region's own frequency is believed up to this far from its peak, in bins
const REGION_REACH: f32 = 8.0;

/// Rounds of the fit: each takes every track once, with the others as they stand
const FIT_ROUNDS: usize = 2;

/// A track is left to the residual when this share of it or more is left around its peak
const FIT_WORST: f32 = 0.5;

/// A track's lobe is taken out down to this share of the largest bin of the frame (-70 dB)
const LOBE_FLOOR: f32 = 0.000_316;

/// Input below this for a whole frame is silence: the frame is not worked out
const SILENCE: f32 = 1.0e-7;

const INPUT_LEN: usize = 2 * FRAME;
const INPUT_MASK: usize = INPUT_LEN - 1;
const OUTPUT_LEN: usize = FRAME;
const OUTPUT_MASK: usize = OUTPUT_LEN - 1;

/// The windows of one chunk length
struct Window {
    chunk: usize,
    hop: usize,
    /// On the input: rises over the frame, falls over the last half chunk
    analysis: Vec<f32>,
    /// On the chunk of the output. With `analysis` it adds up to 1 over the hops, and it
    /// carries the scale of the inverse transform
    synthesis: Vec<f32>,
    /// Transform of `analysis` around the middle of the chunk, from -LOBE to LOBE bins
    lobe: Vec<Complex32>,
    /// Largest value of the lobe from each whole number of bins outwards, as a share of its top
    skirt: Vec<f32>,
}

/// What every engine shares: made once, read only
struct Tables {
    windows: Vec<Window>,
    /// The tracker's window, a Hann over the frame
    tracker: Vec<f32>,
    forward: Arc<dyn RealToComplex<f32>>,
    inverse: Arc<dyn ComplexToReal<f32>>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut planner = RealFftPlanner::<f32>::new();
        let forward = planner.plan_fft_forward(FRAME);
        let inverse = planner.plan_fft_inverse(FRAME);

        // The lobe is read off a transform of the window padded to LOBE_STEPS frames
        let padded = FRAME * LOBE_STEPS;
        let fine = planner.plan_fft_forward(padded);
        let mut time = vec![0.0_f32; padded];
        let mut spectrum = fine.make_output_vec();

        let windows = CHUNKS
            .iter()
            .map(|&chunk| {
                let half = chunk / 2;
                let rise = FRAME - half;
                // What the two windows multiply to on the chunk: a Hann, which adds up to a
                // constant over the hops
                let product = |i: usize| 0.5 - 0.5 * (TAU * (i as f32 + 0.5) / chunk as f32).cos();
                let analysis: Vec<f32> = (0..FRAME)
                    .map(|m| {
                        if m < rise {
                            0.5 - 0.5 * (PI * (m as f32 + 0.5) / rise as f32).cos()
                        } else {
                            product(m - (FRAME - chunk)).sqrt()
                        }
                    })
                    .collect();
                // Four hops to a chunk, two for the shortest: a hop is never under LEAST_HOP
                let hop = (chunk / 4).max(LEAST_HOP);
                let scale = 2.0 * hop as f32 / (chunk as f32 * FRAME as f32);
                let synthesis: Vec<f32> =
                    (0..chunk).map(|i| scale * product(i) / analysis[FRAME - chunk + i]).collect();

                let centre = FRAME - half;
                time.fill(0.0);
                for (m, &value) in analysis.iter().enumerate() {
                    time[(m + padded - centre) % padded] = value;
                }
                fine.process(&mut time, &mut spectrum).unwrap();
                let lobe: Vec<Complex32> = (0..LOBE_TABLE)
                    .map(|i| {
                        let steps = i as isize - (LOBE * LOBE_STEPS) as isize;
                        let value = spectrum[steps.unsigned_abs()];
                        if steps < 0 {
                            value.conj()
                        } else {
                            value
                        }
                    })
                    .collect();

                let top = lobe[LOBE * LOBE_STEPS].norm();
                let mut skirt = vec![0.0_f32; LOBE + 1];
                for bins in (0..LOBE).rev() {
                    let from = (LOBE + bins) * LOBE_STEPS;
                    let here = lobe[from..from + LOBE_STEPS]
                        .iter()
                        .map(|value| value.norm() / top)
                        .fold(0.0, f32::max);
                    skirt[bins] = here.max(skirt[bins + 1]);
                }

                Window { chunk, hop, analysis, synthesis, lobe, skirt }
            })
            .collect();

        let tracker =
            (0..FRAME).map(|m| 0.5 - 0.5 * (TAU * (m as f32 + 0.5) / FRAME as f32).cos()).collect();

        Tables { windows, tracker, forward, inverse }
    })
}

/// The lobe of a sinusoid `distance` bins away (less than LOBE)
#[inline]
fn lobe_at(table: &[Complex32], distance: f32) -> Complex32 {
    let position = (distance + LOBE as f32) * LOBE_STEPS as f32;
    let index = (position as usize).min(LOBE_TABLE - 2);
    let frac = position - index as f32;
    table[index] * (1.0 - frac) + table[index + 1] * frac
}

/// Adds a sinusoid of the complex amplitude `amp` at `freq` bins to a spectrum: its lobe, and
/// where it is low enough the lobe of its mirror image below zero. The lobe is followed out
/// to where it has fallen below `floor`
fn add_sinusoid(
    spectrum: &mut [Complex32],
    window: &Window,
    amp: Complex32,
    freq: f32,
    floor: f32,
) {
    let table = &window.lobe[..];
    let size = amp.norm() * table[LOBE * LOBE_STEPS].norm();
    let mut reach = LOBE - 1;
    while reach > 2 && size * window.skirt[reach] < floor {
        reach -= 1;
    }
    let base = freq.round() as isize;
    let reach = reach as isize;
    let first = (base - reach).max(0);
    let last = (base + reach).min(BINS as isize - 1);
    if first <= last {
        // One bin on is LOBE_STEPS table entries on, at the same fraction
        let position = (first as f32 - freq + LOBE as f32) * LOBE_STEPS as f32;
        let mut index = (position as usize).min(LOBE_TABLE - 2);
        let frac = position - index as f32;
        for bin in first..=last {
            if index + 1 >= LOBE_TABLE {
                break;
            }
            let value = table[index] * (1.0 - frac) + table[index + 1] * frac;
            spectrum[bin as usize] += amp * value;
            index += LOBE_STEPS;
        }
    }
    if freq < reach as f32 {
        let image = amp.conj();
        let mut bin = 0;
        while bin as f32 + freq < reach as f32 && bin < BINS {
            spectrum[bin] += image * lobe_at(table, bin as f32 + freq);
            bin += 1;
        }
    }
}

/// An angle brought back to -PI..PI
#[inline]
fn wrap(angle: f32) -> f32 {
    angle - TAU * (angle / TAU).round()
}

#[inline]
fn turned(angle: f32) -> Complex32 {
    let (sin, cos) = angle.sin_cos();
    Complex32::new(cos, sin)
}

/// Index of the value of a rising list nearest to `value`
fn nearest(list: &[f32], value: f32) -> usize {
    let above = list.partition_point(|&entry| entry < value);
    if above == 0 {
        0
    } else if above == list.len() || value - list[above - 1] <= list[above] - value {
        above - 1
    } else {
        above
    }
}

/// The steady partials: frequency in bins (rising), the rotation each has run up, and its
/// amplitude in the last frame
struct Tracks {
    len: usize,
    freq: Vec<f32>,
    turn: Vec<f32>,
    amp: Vec<Complex32>,
}

impl Tracks {
    fn new() -> Self {
        Self {
            len: 0,
            freq: vec![0.0; MAX_TRACKS],
            turn: vec![0.0; MAX_TRACKS],
            amp: vec![Complex32::ZERO; MAX_TRACKS],
        }
    }
}

pub struct Engine {
    window: usize,
    /// Output frequency over input frequency
    ratio: f32,

    input: Vec<f32>,
    input_pos: usize,
    output: Vec<f32>,
    output_pos: usize,
    since_frame: usize,
    since_track: usize,
    // The tracker has a frame from one look before to read frequencies against
    primed: bool,
    quiet: usize,

    time: Vec<f32>,
    scratch: Vec<Complex32>,
    /// The frame, then what is left of it without the tracks
    spectrum: Vec<Complex32>,
    /// The frame before, whole
    previous: Vec<Complex32>,
    tracked: Vec<Complex32>,
    tracked_previous: Vec<Complex32>,
    /// The tracks as the fit has them
    model: Vec<Complex32>,
    shifted: Vec<Complex32>,
    magnitude: Vec<f32>,

    // What each bin was moved by (in bins) and rotated by in the last frame, and in this one
    bin_shift: Vec<f32>,
    bin_turn: Vec<f32>,
    next_shift: Vec<f32>,
    next_turn: Vec<f32>,

    tracks: Tracks,
    found: Tracks,
}

impl Engine {
    pub fn new() -> Self {
        let tables = tables();
        let scratch = tables.forward.get_scratch_len().max(tables.inverse.get_scratch_len()).max(1);
        Self {
            window: 1,
            ratio: 1.0,
            input: vec![0.0; INPUT_LEN],
            input_pos: 0,
            output: vec![0.0; OUTPUT_LEN],
            output_pos: 0,
            since_frame: 0,
            since_track: 0,
            primed: false,
            quiet: 0,
            time: vec![0.0; FRAME],
            scratch: vec![Complex32::ZERO; scratch],
            spectrum: vec![Complex32::ZERO; BINS],
            previous: vec![Complex32::ZERO; BINS],
            tracked: vec![Complex32::ZERO; BINS],
            tracked_previous: vec![Complex32::ZERO; BINS],
            model: vec![Complex32::ZERO; BINS],
            shifted: vec![Complex32::ZERO; BINS],
            magnitude: vec![0.0; BINS],
            bin_shift: vec![0.0; BINS],
            bin_turn: vec![0.0; BINS],
            next_shift: vec![0.0; BINS],
            next_turn: vec![0.0; BINS],
            tracks: Tracks::new(),
            found: Tracks::new(),
        }
    }

    /// Index of the window in use, into `CHUNKS`
    pub fn window(&self) -> usize {
        self.window
    }

    /// Takes another chunk length. The output starts over: see `restart`
    pub fn set_window(&mut self, window: usize) {
        self.window = window.min(CHUNKS.len() - 1);
        self.restart();
    }

    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio;
    }

    /// Forgets the input as well as the output
    pub fn reset(&mut self) {
        self.input.fill(0.0);
        self.quiet = FRAME;
        self.restart();
    }

    /// Starts the output over: nothing of an earlier frame is heard, and the first chunk
    /// fades in. The input is kept, so the first frame already has a past
    pub fn restart(&mut self) {
        self.output.fill(0.0);
        self.since_frame = 0;
        self.since_track = TRACK_HOP;
        self.primed = false;
        self.previous.fill(Complex32::ZERO);
        self.tracked_previous.fill(Complex32::ZERO);
        self.bin_shift.fill(0.0);
        self.bin_turn.fill(0.0);
        self.tracks.len = 0;
    }

    /// Takes a sample in without working anything out, for while the engine is not heard
    pub fn feed(&mut self, sample: f32) {
        self.push(sample);
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        self.push(sample);
        self.since_frame += 1;
        self.since_track += 1;
        if self.since_frame >= tables().windows[self.window].hop {
            self.since_frame = 0;
            if self.quiet < FRAME {
                self.frame();
            }
        }
        let output = self.output[self.output_pos];
        self.output[self.output_pos] = 0.0;
        self.output_pos = (self.output_pos + 1) & OUTPUT_MASK;
        output
    }

    /// Address and capacity of every buffer
    pub fn buffers(&self) -> Vec<(usize, usize)> {
        let reals = [
            &self.input,
            &self.output,
            &self.time,
            &self.magnitude,
            &self.bin_shift,
            &self.bin_turn,
            &self.next_shift,
            &self.next_turn,
            &self.tracks.freq,
            &self.tracks.turn,
            &self.found.freq,
            &self.found.turn,
        ];
        let complexes = [
            &self.scratch,
            &self.spectrum,
            &self.previous,
            &self.tracked,
            &self.tracked_previous,
            &self.model,
            &self.shifted,
            &self.tracks.amp,
            &self.found.amp,
        ];
        // In the order of their addresses: frames swap some of these among themselves
        let mut all: Vec<(usize, usize)> = reals
            .iter()
            .map(|buffer| (buffer.as_ptr() as usize, buffer.capacity()))
            .chain(complexes.iter().map(|buffer| (buffer.as_ptr() as usize, buffer.capacity())))
            .collect();
        all.sort_unstable();
        all
    }

    #[inline]
    fn push(&mut self, sample: f32) {
        self.input[self.input_pos] = sample;
        self.input_pos = (self.input_pos + 1) & INPUT_MASK;
        self.quiet = if sample.abs() < SILENCE { (self.quiet + 1).min(FRAME) } else { 0 };
    }

    /// Works out one frame and adds its chunk to the output
    fn frame(&mut self) {
        let tables = tables();
        let window = &tables.windows[self.window];
        let (chunk, hop) = (window.chunk, window.hop);
        let centre = FRAME - chunk / 2;
        let oldest = self.input_pos.wrapping_sub(FRAME);

        // The frame, turned so that the middle of the chunk is its time zero: what is done to
        // the spectrum then holds best where the chunk is
        for (j, slot) in self.time.iter_mut().enumerate() {
            let m = (j + centre) & (FRAME - 1);
            *slot = self.input[oldest.wrapping_add(m) & INPUT_MASK] * window.analysis[m];
        }
        tables
            .forward
            .process_with_scratch(&mut self.time, &mut self.spectrum, &mut self.scratch)
            .unwrap();
        for (magnitude, value) in self.magnitude.iter_mut().zip(&self.spectrum) {
            *magnitude = value.norm();
        }

        if self.since_track >= TRACK_HOP {
            self.track(oldest, self.since_track);
            self.since_track = 0;
        }

        // Every track runs on by a hop: its rotation, and the phase its amplitude is taken up at
        let per_bin = TAU * hop as f32 / FRAME as f32;
        for j in 0..self.tracks.len {
            let freq = self.tracks.freq[j];
            self.tracks.turn[j] = wrap(self.tracks.turn[j] + per_bin * (self.ratio - 1.0) * freq);
            self.tracks.amp[j] *= turned(per_bin * freq);
        }

        self.regions(hop);
        self.previous.copy_from_slice(&self.spectrum);
        let floor = LOBE_FLOOR * self.magnitude.iter().copied().fold(0.0, f32::max);
        self.fit(window, floor);

        // The shifted frame: the tracks at their new frequencies, and the rest moved by region
        self.shifted.fill(Complex32::ZERO);
        for j in 0..self.tracks.len {
            let amp = self.tracks.amp[j];
            let target = self.ratio * self.tracks.freq[j];
            if amp != Complex32::ZERO && target > 0.5 && target < BINS as f32 - 1.5 {
                let amp = amp * turned(self.tracks.turn[j]);
                add_sinusoid(&mut self.shifted, window, amp, target, floor);
            }
        }
        let mut turn = f32::NAN;
        let mut rotation = Complex32::ZERO;
        for bin in 0..BINS {
            let shift = self.next_shift[bin];
            let whole = shift.floor();
            let frac = shift - whole;
            // A region has one rotation for all its bins
            if self.next_turn[bin] != turn {
                turn = self.next_turn[bin];
                rotation = turned(turn);
            }
            let value = self.spectrum[bin] * rotation;
            let target = bin as isize + whole as isize;
            for (target, weight) in [(target, 1.0 - frac), (target + 1, frac)] {
                if target >= BINS as isize {
                    continue;
                }
                // What is moved below zero comes back up as its mirror image
                if target >= 0 {
                    self.shifted[target as usize] += value * weight;
                } else if (-target as usize) < BINS {
                    self.shifted[-target as usize] += value.conj() * weight;
                }
            }
        }
        std::mem::swap(&mut self.bin_shift, &mut self.next_shift);
        std::mem::swap(&mut self.bin_turn, &mut self.next_turn);

        self.shifted[0].im = 0.0;
        self.shifted[BINS - 1].im = 0.0;
        tables
            .inverse
            .process_with_scratch(&mut self.shifted, &mut self.time, &mut self.scratch)
            .unwrap();
        for i in 0..chunk {
            let j = (i + FRAME - chunk / 2) & (FRAME - 1);
            self.output[(self.output_pos + i) & OUTPUT_MASK] += self.time[j] * window.synthesis[i];
        }
    }

    /// Frequency of what is in a bin, in bins, from how far its phase has come since the
    /// frame `hop` samples before
    #[inline]
    fn frequency(now: Complex32, before: Complex32, bin: usize, hop: usize) -> f32 {
        let advance = (now * before.conj()).arg();
        let expected = TAU * ((bin * hop) % FRAME) as f32 / FRAME as f32;
        bin as f32 + wrap(advance - expected) * FRAME as f32 / (TAU * hop as f32)
    }

    /// Finds the steady partials in a symmetric window over the frame. A partial that was
    /// there before keeps its rotation and amplitude, a new one takes up the rotation of the
    /// bin it appears in
    fn track(&mut self, oldest: usize, hop: usize) {
        let tables = tables();
        for (m, slot) in self.time.iter_mut().enumerate() {
            *slot = self.input[oldest.wrapping_add(m) & INPUT_MASK] * tables.tracker[m];
        }
        tables
            .forward
            .process_with_scratch(&mut self.time, &mut self.tracked, &mut self.scratch)
            .unwrap();

        let largest = self.tracked.iter().map(|value| value.norm_sqr()).fold(0.0, f32::max).sqrt();
        let least = (largest * TRACK_FLOOR).max(TRACK_LEAST);
        let size = |bin: usize| self.tracked[bin].norm_sqr();

        // Frequencies are read against the look one hop before. Without one (a start, or
        // silence in between) there are no tracks until the next look
        let peaks = if self.primed && hop == TRACK_HOP { 2..BINS - 2 } else { 0..0 };
        self.primed = true;

        self.found.len = 0;
        let mut old = 0;
        for bin in peaks {
            let here = size(bin);
            let is_peak = here > least * least
                && here > size(bin - 1)
                && here > size(bin - 2)
                && here >= size(bin + 1)
                && here >= size(bin + 2);
            if !is_peak || self.found.len == MAX_TRACKS {
                continue;
            }
            let freq = Self::frequency(self.tracked[bin], self.tracked_previous[bin], bin, hop);
            if (freq - bin as f32).abs() >= TRACK_NEAR_BIN {
                continue;
            }
            // Both lists rise, so the nearest old track is found by walking on
            while old + 1 < self.tracks.len
                && (self.tracks.freq[old + 1] - freq).abs() <= (self.tracks.freq[old] - freq).abs()
            {
                old += 1;
            }
            let j = self.found.len;
            self.found.freq[j] = freq;
            if old < self.tracks.len && (self.tracks.freq[old] - freq).abs() < TRACK_SAME {
                self.found.turn[j] = self.tracks.turn[old];
                self.found.amp[j] = self.tracks.amp[old];
            } else {
                self.found.turn[j] = self.bin_turn[bin];
                self.found.amp[j] = Complex32::ZERO;
            }
            self.found.len += 1;
        }
        std::mem::swap(&mut self.tracks, &mut self.found);
        std::mem::swap(&mut self.tracked, &mut self.tracked_previous);
    }

    /// Cuts the frame into regions around its peaks and says for every bin how far it moves
    /// and by what it is rotated (`next_shift`, `next_turn`)
    fn regions(&mut self, hop: usize) {
        let per_bin = TAU * hop as f32 / FRAME as f32;
        let tracks = &self.tracks.freq[..self.tracks.len];
        let is_peak = |bin: usize| {
            let here = self.magnitude[bin];
            bin >= 2
                && bin + 2 < BINS
                && here > self.magnitude[bin - 1]
                && here > self.magnitude[bin - 2]
                && here >= self.magnitude[bin + 1]
                && here >= self.magnitude[bin + 2]
        };

        let mut start = 0;
        let mut peak = (0..BINS).find(|&bin| is_peak(bin));
        if peak.is_none() {
            // Nothing stands out: the frame moves as one, by what its bins would
            for bin in 0..BINS {
                self.next_shift[bin] = (self.ratio - 1.0) * bin as f32;
                self.next_turn[bin] = wrap(
                    self.bin_turn[bin]
                        + 0.5 * per_bin * (self.bin_shift[bin] + self.next_shift[bin]),
                );
            }
            return;
        }
        while let Some(bin) = peak {
            // The region ends at the lowest bin before the next peak, or at the top
            let next = (bin + 1..BINS).find(|&other| is_peak(other));
            let end = match next {
                Some(other) => {
                    let mut lowest = bin;
                    for between in bin..=other {
                        if self.magnitude[between] < self.magnitude[lowest] {
                            lowest = between;
                        }
                    }
                    lowest
                }
                None => BINS,
            };

            let mut freq = Self::frequency(self.spectrum[bin], self.previous[bin], bin, hop);
            if (freq - bin as f32).abs() >= REGION_REACH || freq <= 0.5 {
                freq = bin as f32;
            }
            let from = (freq.round() as usize).min(BINS - 1);
            let mut shift = (self.ratio - 1.0) * freq;
            let mut turn =
                wrap(self.bin_turn[from] + 0.5 * per_bin * (self.bin_shift[from] + shift));
            if !tracks.is_empty() {
                let j = nearest(tracks, freq);
                if (tracks[j] - freq).abs() < REGION_NEAR_TRACK {
                    shift = (self.ratio - 1.0) * tracks[j];
                    turn = self.tracks.turn[j];
                }
            }
            self.next_shift[start..end].fill(shift);
            self.next_turn[start..end].fill(turn);
            start = end;
            peak = next;
        }
    }

    /// Fits every track to the frame as a sinusoid and takes it out: `model` is the tracks,
    /// `spectrum` what is left
    fn fit(&mut self, window: &Window, floor: f32) {
        let lobe = &window.lobe[..];
        self.model.fill(Complex32::ZERO);
        let usable = |freq: f32| freq > 1.5 && freq < BINS as f32 - 3.0;
        for j in 0..self.tracks.len {
            if !usable(self.tracks.freq[j]) {
                self.tracks.amp[j] = Complex32::ZERO;
            } else if self.tracks.amp[j] != Complex32::ZERO {
                add_sinusoid(
                    &mut self.model,
                    window,
                    self.tracks.amp[j],
                    self.tracks.freq[j],
                    floor,
                );
            }
        }

        // The three bins at a track's peak, with everything else the model has taken out,
        // say what the track is
        let around = |freq: f32| {
            let peak = freq.round() as usize;
            let weights = [
                lobe_at(lobe, (peak - 1) as f32 - freq),
                lobe_at(lobe, peak as f32 - freq),
                lobe_at(lobe, (peak + 1) as f32 - freq),
            ];
            let energy: f32 = weights.iter().map(|weight| weight.norm_sqr()).sum();
            (peak, weights, energy)
        };
        for _ in 0..FIT_ROUNDS {
            for j in 0..self.tracks.len {
                let freq = self.tracks.freq[j];
                if !usable(freq) {
                    continue;
                }
                let (peak, weights, energy) = around(freq);
                let amp = self.tracks.amp[j];
                let mut sum = Complex32::ZERO;
                for (i, weight) in weights.iter().enumerate() {
                    let bin = peak - 1 + i;
                    let alone = self.spectrum[bin] - self.model[bin] + amp * weight;
                    sum += weight.conj() * alone;
                }
                let fitted = sum / energy;
                add_sinusoid(&mut self.model, window, fitted - amp, freq, floor);
                self.tracks.amp[j] = fitted;
            }
        }

        // A track the frame does not agree with (it has ended, it bends, it was never one)
        // goes back into what is left
        for j in 0..self.tracks.len {
            let freq = self.tracks.freq[j];
            let amp = self.tracks.amp[j];
            if !usable(freq) || amp == Complex32::ZERO {
                continue;
            }
            let (peak, _, energy) = around(freq);
            let left: f32 = (peak - 1..=peak + 1)
                .map(|bin| (self.spectrum[bin] - self.model[bin]).norm_sqr())
                .sum();
            if left > FIT_WORST * amp.norm_sqr() * energy {
                add_sinusoid(&mut self.model, window, -amp, freq, floor);
                self.tracks.amp[j] = Complex32::ZERO;
            }
        }

        for (value, model) in self.spectrum.iter_mut().zip(&self.model) {
            *value -= model;
        }
    }
}
