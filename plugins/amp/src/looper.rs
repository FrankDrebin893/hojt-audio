//! Looper: records the amp's finished sound, plays it back on top of what is played and adds
//! to it (#280). It sits after the whole chain and before the metronome's click, so the loop
//! is what the player heard and a click is never in it, and it is not run through the amp again.
//!
//! The buffer is allocated once, in `set_sample_rate` (never on the audio thread), and the
//! length of a loop is counted in samples, so it holds to the sample and never drifts. A 5 ms
//! fade at the ends of a recorded loop and of an overdub takes the click out of the seam.
//! The editor reaches it through [`LooperControl`], atomics only, so nothing locks.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

/// The longest loop, in seconds of stereo; at a sample rate above `HIGH_RATE` it is `HIGH_RATE_SECONDS`
pub const MAX_SECONDS: f32 = 60.0;
pub const HIGH_RATE_SECONDS: f32 = 40.0;
const HIGH_RATE: f32 = 96_000.0;

/// The fade at the loop point and at the ends of a record or an overdub, in seconds
const FADE_SECONDS: f32 = 0.005;
/// How fast the level knob is followed, in seconds
const LEVEL_SECONDS: f32 = 0.01;

pub const LEVEL_MIN_DB: f32 = -60.0;
pub const LEVEL_MAX_DB: f32 = 6.0;
pub const LEVEL_DEFAULT_DB: f32 = 0.0;

/// The loop's linear gain for a level in dB; the bottom of the range is silence
pub fn level_gain(db: f32) -> f32 {
    if db <= LEVEL_MIN_DB {
        0.0
    } else {
        10.0f32.powf(db.min(LEVEL_MAX_DB) / 20.0)
    }
}

/// Where the looper is, as the editor reads it
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    /// Nothing recorded: the looper costs nothing and touches nothing
    Empty,
    Recording,
    Playing,
    Overdubbing,
    /// Muted and kept; it goes on counting, so it comes back in time
    Stopped,
}

impl State {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Recording,
            2 => Self::Playing,
            3 => Self::Overdubbing,
            4 => Self::Stopped,
            _ => Self::Empty,
        }
    }
}

/// What the editor tells the looper, and what it reads back. Commands are counted or flagged
/// and taken by the audio thread at the start of the next block
#[derive(Default)]
pub struct LooperControl {
    presses: AtomicU8,
    stop: AtomicBool,
    clear: AtomicBool,
    state: AtomicU8,
    length: AtomicU32,
    position: AtomicU32,
    capacity: AtomicU32,
}

impl LooperControl {
    /// The big button: Record, then Play, then Overdub, then Play again
    pub fn press(&self) {
        self.presses.fetch_add(1, Ordering::Release);
    }

    /// Mute the loop and keep it, or bring it back
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
    }

    /// Empty the loop
    pub fn clear(&self) {
        self.clear.store(true, Ordering::Release);
    }

    pub fn state(&self) -> State {
        State::from_u8(self.state.load(Ordering::Acquire))
    }

    /// The loop's length in samples (what has been recorded so far, while recording)
    pub fn length(&self) -> u32 {
        self.length.load(Ordering::Relaxed)
    }

    /// The most samples a loop can hold at the plugin's sample rate: what the plate's ring
    /// fills against while recording
    pub fn capacity(&self) -> u32 {
        self.capacity.load(Ordering::Relaxed)
    }

    /// Sets what the editor reads, for the editor's own tests
    #[cfg(test)]
    pub(crate) fn show(&self, state: State, length: u32, position: u32, capacity: u32) {
        self.length.store(length, Ordering::Relaxed);
        self.position.store(position, Ordering::Relaxed);
        self.capacity.store(capacity, Ordering::Relaxed);
        self.state.store(state as u8, Ordering::Release);
    }

    /// The commands the audio has not taken yet: presses, stop, clear
    #[cfg(test)]
    pub(crate) fn pending(&self) -> (u8, bool, bool) {
        (
            self.presses.load(Ordering::Acquire),
            self.stop.load(Ordering::Acquire),
            self.clear.load(Ordering::Acquire),
        )
    }

    /// Where the loop is, in samples from its start
    pub fn position(&self) -> u32 {
        self.position.load(Ordering::Relaxed)
    }
}

/// What the looper is told with every block
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LooperSettings {
    /// Linear gain of the loop against the live sound
    pub level: f32,
    /// The tuner is on: the loop is silent, as the rest is
    pub muted: bool,
    /// The metronome's bar in samples, where it is on: a loop is closed on a whole number of them
    pub bar_samples: Option<u32>,
    /// Samples from the start of the block to the metronome's next downbeat, where it is on and
    /// one is coming: the first Record press waits for it
    pub downbeat_samples: Option<u32>,
}

impl Default for LooperSettings {
    fn default() -> Self {
        Self {
            level: level_gain(LEVEL_DEFAULT_DB),
            muted: false,
            bar_samples: None,
            downbeat_samples: None,
        }
    }
}

pub struct Looper {
    control: Arc<LooperControl>,
    sample_rate: f32,
    seconds: f32,
    left: Vec<f32>,
    right: Vec<f32>,
    state: State,
    /// Samples recorded while recording, then the length of the loop
    len: usize,
    pos: usize,
    fade: usize,
    /// The loop's own gain, which ramps for Stop, Resume and Clear
    play_gain: f32,
    /// How much of what is played goes into the loop, ramped at the ends of an overdub
    dub_gain: f32,
    clearing: bool,
    /// A first Record press that waits for the metronome's downbeat: samples still to go
    armed: Option<usize>,
    level: f32,
    level_coef: f32,
}

impl Default for Looper {
    fn default() -> Self {
        Self::new()
    }
}

impl Looper {
    /// A looper with no buffer yet: `set_sample_rate` (the plugin's `initialize`) allocates it,
    /// and until then a press does nothing
    pub fn new() -> Self {
        Self::unallocated(MAX_SECONDS)
    }

    /// A looper at 48 kHz that holds at most `seconds` (a shorter one for tests)
    #[cfg(test)]
    pub fn with_seconds(seconds: f32) -> Self {
        let mut looper = Self::unallocated(seconds);
        looper.set_sample_rate(48_000.0);
        looper
    }

    fn unallocated(seconds: f32) -> Self {
        Self {
            control: Arc::new(LooperControl::default()),
            sample_rate: 0.0,
            seconds,
            left: Vec::new(),
            right: Vec::new(),
            state: State::Empty,
            len: 0,
            pos: 0,
            fade: 1,
            play_gain: 0.0,
            dub_gain: 0.0,
            clearing: false,
            armed: None,
            level: 1.0,
            level_coef: 1.0,
        }
    }

    pub fn control(&self) -> Arc<LooperControl> {
        self.control.clone()
    }

    #[cfg(test)]
    pub fn state(&self) -> State {
        self.state
    }

    /// The samples a loop can hold
    pub fn capacity(&self) -> usize {
        self.left.len()
    }

    /// Allocates the buffer for `sample_rate` (this is the only place it is). A change of
    /// rate empties the loop, so no audio of another rate is played; the same rate leaves it
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if sample_rate == self.sample_rate {
            return;
        }
        self.sample_rate = sample_rate;
        let seconds = if sample_rate > HIGH_RATE {
            self.seconds.min(HIGH_RATE_SECONDS)
        } else {
            self.seconds
        };
        let capacity = (seconds * sample_rate) as usize;
        if self.left.len() != capacity {
            self.left = vec![0.0; capacity];
            self.right = vec![0.0; capacity];
        }
        self.control.capacity.store(capacity as u32, Ordering::Relaxed);
        self.fade = ((FADE_SECONDS * sample_rate) as usize).max(1);
        self.level_coef = 1.0 - (-1.0 / (LEVEL_SECONDS * sample_rate)).exp();
        self.empty();
    }

    /// Nothing recorded; the buffer is left as it is, a loop writes before it reads
    fn empty(&mut self) {
        self.state = State::Empty;
        self.len = 0;
        self.pos = 0;
        self.play_gain = 0.0;
        self.dub_gain = 0.0;
        self.clearing = false;
        self.armed = None;
        self.publish();
    }

    /// What has been recorded, for the tests that look inside the loop
    #[cfg(test)]
    pub(crate) fn recorded(&self) -> (&[f32], &[f32]) {
        (&self.left[..self.len], &self.right[..self.len])
    }

    /// True while there is nothing to hear or to count
    pub fn is_idle(&self) -> bool {
        self.state == State::Empty && self.armed.is_none()
    }

    fn publish(&self) {
        self.control.length.store(self.len as u32, Ordering::Relaxed);
        self.control.position.store(self.pos as u32, Ordering::Relaxed);
        self.control.state.store(self.state as u8, Ordering::Release);
    }

    /// Closes the recording: the loop is what was recorded, rounded to whole bars where
    /// there is a bar (at least one), cut to what the buffer holds, with its ends faded
    fn close(&mut self, bar: Option<u32>) {
        let recorded = self.len;
        // Too short to be a loop: nothing happened
        if recorded < 2 * self.fade {
            return self.empty();
        }
        let mut len = recorded;
        if let Some(bar) = bar.filter(|bar| *bar > 0).map(|bar| bar as usize) {
            let bars = ((recorded as f64 / bar as f64).round() as usize).max(1);
            len = bars * bar;
            if len > self.capacity() {
                len = self.capacity() / bar * bar;
                if len == 0 {
                    len = recorded;
                }
            }
        }
        let end = recorded.min(len);
        for k in 0..self.fade {
            let gain = (end - (end - self.fade + k)) as f32 / self.fade as f32;
            self.left[end - self.fade + k] *= gain;
            self.right[end - self.fade + k] *= gain;
        }
        // Rounded up: the rest of the last bar is silence
        self.left[end..len].fill(0.0);
        self.right[end..len].fill(0.0);
        self.len = len;
        self.pos = 0;
        self.state = State::Playing;
        self.play_gain = 1.0;
        self.dub_gain = 0.0;
    }

    fn start_recording(&mut self) {
        self.armed = None;
        self.state = State::Recording;
        self.len = 0;
        self.pos = 0;
    }

    fn take_commands(&mut self, settings: &LooperSettings) {
        let bar = settings.bar_samples;
        if self.control.clear.swap(false, Ordering::Acquire) {
            match self.state {
                State::Empty => self.armed = None,
                State::Recording => self.empty(),
                _ => self.clearing = true,
            }
        }
        for _ in 0..self.control.presses.swap(0, Ordering::Acquire) {
            if self.clearing {
                break;
            }
            match self.state {
                State::Empty if self.capacity() == 0 => {}
                // Waiting for a downbeat: a second press takes it back
                State::Empty if self.armed.is_some() => self.armed = None,
                State::Empty => match settings.downbeat_samples {
                    Some(wait) if wait > 0 => self.armed = Some(wait as usize),
                    _ => self.start_recording(),
                },
                State::Recording => self.close(bar),
                State::Playing => self.state = State::Overdubbing,
                State::Overdubbing | State::Stopped => self.state = State::Playing,
            }
        }
        if self.control.stop.swap(false, Ordering::Acquire) && !self.clearing {
            match self.state {
                State::Playing | State::Overdubbing => self.state = State::Stopped,
                State::Stopped => self.state = State::Playing,
                State::Empty | State::Recording => {}
            }
        }
    }

    /// Records, plays and overdubs over a block, which is the amp's finished sound. A block
    /// of one channel is the left one
    pub fn process(
        &mut self,
        settings: &LooperSettings,
        left: &mut [f32],
        mut right: Option<&mut [f32]>,
    ) {
        self.take_commands(settings);
        if self.state == State::Empty && self.armed.is_none() {
            self.level = settings.level;
            self.publish();
            return;
        }
        let step = 1.0 / self.fade as f32;
        for index in 0..left.len() {
            let live_l = left[index];
            let live_r = right.as_deref().map_or(0.0, |right| right[index]);

            // The first press waits for the downbeat: until then the sound passes untouched
            if let Some(wait) = self.armed {
                if wait == 0 {
                    self.start_recording();
                } else {
                    self.armed = Some(wait - 1);
                    continue;
                }
            }
            if self.state == State::Recording {
                let ramp = (self.len as f32 * step).min(1.0);
                self.left[self.len] = live_l * ramp;
                self.right[self.len] = live_r * ramp;
                self.len += 1;
                if self.len == self.capacity() {
                    self.close(None);
                }
                continue;
            }
            if self.state == State::Empty {
                break;
            }

            let silent = self.state == State::Stopped || settings.muted || self.clearing;
            let target = if silent { 0.0 } else { 1.0 };
            self.play_gain += (target - self.play_gain).clamp(-step, step);
            let dub = if self.state == State::Overdubbing && !self.clearing { 1.0 } else { 0.0 };
            self.dub_gain += (dub - self.dub_gain).clamp(-step, step);
            self.level += (settings.level - self.level) * self.level_coef;

            let (old_l, old_r) = (self.left[self.pos], self.right[self.pos]);
            if self.dub_gain > 0.0 {
                self.left[self.pos] = crate::delay::soft_limit(old_l + live_l * self.dub_gain);
                self.right[self.pos] = crate::delay::soft_limit(old_r + live_r * self.dub_gain);
            }
            let heard = self.level * self.play_gain;
            left[index] = (live_l + old_l * heard).clamp(-1.0, 1.0);
            if let Some(right) = right.as_deref_mut() {
                right[index] = (live_r + old_r * heard).clamp(-1.0, 1.0);
            }

            self.pos += 1;
            if self.pos == self.len {
                self.pos = 0;
            }
            if self.clearing && self.play_gain == 0.0 && self.dub_gain == 0.0 {
                self.empty();
            }
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;
    const FADE: usize = 240;

    fn tone(from: usize, len: usize, hz: f32, amp: f32) -> Vec<f32> {
        (0..len)
            .map(|i| amp * (std::f32::consts::TAU * hz * (from + i) as f32 / RATE).sin())
            .collect()
    }

    /// A looper holding one second, and a block through it
    fn fresh() -> Looper {
        Looper::with_seconds(1.0)
    }

    fn play(looper: &mut Looper, input: &[f32], bar: Option<u32>) -> Vec<f32> {
        let mut left = input.to_vec();
        let mut right = input.to_vec();
        let settings = LooperSettings { bar_samples: bar, ..LooperSettings::default() };
        looper.process(&settings, &mut left, Some(&mut right));
        assert_eq!(left, right);
        left
    }

    /// Records `phrase` and closes the loop; what plays in the block that closes it is returned
    fn record(looper: &mut Looper, phrase: &[f32], bar: Option<u32>) -> Vec<f32> {
        let control = looper.control();
        control.press();
        play(looper, phrase, bar);
        control.press();
        play(looper, &vec![0.0; 100], bar)
    }

    #[test]
    fn test_record_then_play_returns_the_recorded_samples_inside_the_fades() {
        let mut looper = fresh();
        let phrase = tone(0, 4800, 220.0, 0.5);
        let first = record(&mut looper, &phrase, None);
        assert_eq!(looper.state(), State::Playing);
        assert_eq!(looper.control().length(), 4800);
        // The block that closed it already plays the loop from its start
        for i in 0..100 {
            let faded = phrase[i] * (i as f32 / FADE as f32);
            assert!((first[i] - faded).abs() < 1e-6, "sample {i}");
        }
        // And on, to the length of what was recorded
        let rest = play(&mut looper, &vec![0.0; 4700], None);
        for i in 0..4700 - FADE {
            let at = 100 + i;
            if (FADE..4800 - FADE).contains(&at) {
                assert_eq!(rest[i], phrase[at], "sample {at}");
            }
        }
        // Faded at its end and back to the start: silence at the seam
        assert!(rest[4700 - 1].abs() < 0.01);
        assert!(first[0].abs() < 0.01);
    }

    #[test]
    fn test_the_loop_repeats_without_drift_over_a_thousand_passes() {
        let mut looper = fresh();
        let phrase = tone(0, 2400, 330.0, 0.4);
        record(&mut looper, &phrase, None);
        // Where the closing block left it: 100 samples in
        let mut heard = Vec::new();
        let sizes = [64usize, 333, 512, 480];
        let mut size = 0;
        while heard.len() < 2400 * 1000 {
            let n = sizes[size % sizes.len()];
            size += 1;
            heard.extend(play(&mut looper, &vec![0.0; n], None));
        }
        let first_pass: Vec<f32> = heard[2300..2300 + 2400].to_vec();
        for pass in 1..990 {
            let from = 2300 + pass * 2400;
            assert_eq!(&heard[from..from + 2400], &first_pass[..], "pass {pass}");
        }
    }

    #[test]
    fn test_an_overdub_adds_to_the_loop_and_the_next_pass_has_both() {
        let mut looper = fresh();
        let control = looper.control();
        let base = tone(0, 4800, 220.0, 0.2);
        record(&mut looper, &base, None);
        // Finish the pass in progress (100 samples in), then overdub from the start
        play(&mut looper, &vec![0.0; 4700], None);
        control.press();
        assert_eq!(looper.state(), State::Playing);
        let second = tone(0, 4800, 330.0, 0.2);
        let during = play(&mut looper, &second, None);
        assert_eq!(looper.state(), State::Overdubbing);
        // You hear the loop and what you play
        for i in 2 * FADE..4800 - FADE {
            assert!((during[i] - (base[i] + second[i])).abs() < 1e-6, "sample {i}");
        }
        control.press();
        let after = play(&mut looper, &vec![0.0; 4800], None);
        assert_eq!(looper.state(), State::Playing);
        for i in 2 * FADE..4800 - 2 * FADE {
            assert!((after[i] - (base[i] + second[i])).abs() < 1e-6, "sample {i}");
        }
    }

    #[test]
    fn test_an_overdub_cannot_run_away() {
        let mut looper = fresh();
        let control = looper.control();
        record(&mut looper, &vec![0.5; 4800], None);
        control.press();
        for _ in 0..50 {
            let out = play(&mut looper, &vec![0.5; 4800], None);
            assert!(out.iter().all(|s| s.abs() <= 1.0));
        }
        for sample in &looper.left[..4800] {
            assert!(sample.abs() < 1.0);
        }
    }

    #[test]
    fn test_a_loop_closes_on_the_nearest_whole_bar_and_never_on_none() {
        let bar = 4000u32;
        let length_for = |recorded: usize| {
            let mut looper = fresh();
            record(&mut looper, &tone(0, recorded, 220.0, 0.3), Some(bar));
            looper.control().length() as usize
        };
        assert_eq!(length_for(5600), 4000, "1.4 bars");
        assert_eq!(length_for(6400), 8000, "1.6 bars");
        assert_eq!(length_for(8000), 8000);
        assert_eq!(length_for(1000), 4000, "a quarter of a bar is a bar, not nothing");
        assert_eq!(length_for(5000), 4000, "no bar: it is what was played");
        // The rest of a bar rounded up is silence
        let mut looper = fresh();
        record(&mut looper, &tone(0, 6400, 220.0, 0.3), Some(bar));
        assert!(looper.left[6400..8000].iter().all(|s| *s == 0.0));
        // A bar longer than the buffer: what was recorded
        let mut looper = fresh();
        record(&mut looper, &tone(0, 6000, 220.0, 0.3), Some(100_000));
        assert_eq!(looper.control().length(), 6000);
    }

    #[test]
    fn test_the_first_press_waits_for_the_downbeat_and_the_loop_starts_on_it() {
        let mut looper = fresh();
        let control = looper.control();
        let phrase = tone(0, 6000, 220.0, 0.4);
        let settings = LooperSettings {
            bar_samples: Some(4000),
            downbeat_samples: Some(1500),
            ..LooperSettings::default()
        };
        control.press();
        // Before the downbeat the sound passes untouched and nothing is recorded
        let mut left = phrase[..1000].to_vec();
        looper.process(&settings, &mut left, None);
        assert_eq!(left, phrase[..1000]);
        assert_eq!(looper.state(), State::Empty);
        assert!(!looper.is_idle(), "it is counting down");
        // The block with the downbeat in it starts the record 500 samples in
        let settings = LooperSettings { downbeat_samples: Some(500), ..settings };
        let mut left = phrase[1000..2000].to_vec();
        looper.process(&settings, &mut left, None);
        assert_eq!(left, phrase[1000..2000]);
        assert_eq!(looper.state(), State::Recording);
        assert_eq!(looper.control().length(), 500);
        // 3500 recorded in all: 0.875 of a bar of 4000, so a bar
        let mut rest = phrase[2000..5000].to_vec();
        looper.process(&settings, &mut rest, None);
        control.press();
        looper.process(&settings, &mut [0.0; 10], None);
        assert_eq!(looper.state(), State::Playing);
        assert_eq!(looper.control().length(), 4000);
        // It begins with what was played on the downbeat
        let (left, _) = looper.recorded();
        for i in FADE..3500 - FADE {
            assert_eq!(left[i], phrase[1500 + i], "sample {i}");
        }
    }

    #[test]
    fn test_a_press_or_clear_while_waiting_for_the_downbeat_takes_it_back() {
        for command in [true, false] {
            let mut looper = fresh();
            let control = looper.control();
            let settings =
                LooperSettings { downbeat_samples: Some(900), ..LooperSettings::default() };
            control.press();
            looper.process(&settings, &mut [0.0; 100], None);
            assert!(!looper.is_idle());
            if command {
                control.press();
            } else {
                control.clear();
            }
            let input = tone(0, 2000, 220.0, 0.3);
            let mut block = input.clone();
            looper.process(&settings, &mut block, None);
            assert_eq!(block, input);
            assert_eq!(looper.state(), State::Empty);
            assert!(looper.is_idle());
        }
    }

    #[test]
    fn test_with_no_downbeat_ahead_the_first_press_starts_at_once() {
        let mut looper = fresh();
        looper.control().press();
        let settings = LooperSettings { downbeat_samples: Some(0), ..LooperSettings::default() };
        looper.process(&settings, &mut [0.1; 64], None);
        assert_eq!(looper.state(), State::Recording);
        assert_eq!(looper.control().length(), 64);
    }

    #[test]
    fn test_clear_returns_the_sound_untouched() {
        let mut looper = fresh();
        let control = looper.control();
        record(&mut looper, &tone(0, 4800, 220.0, 0.5), None);
        play(&mut looper, &vec![0.0; 1000], None);
        control.clear();
        // It fades out, and is empty
        play(&mut looper, &vec![0.0; 1000], None);
        assert_eq!(looper.state(), State::Empty);
        assert!(looper.is_idle());
        let input = tone(0, 960, 440.0, 0.3);
        assert_eq!(play(&mut looper, &input, None), input, "bit for bit");
        // And a new loop can be recorded
        let again = record(&mut looper, &tone(0, 4800, 110.0, 0.3), None);
        assert_eq!(looper.control().length(), 4800);
        assert!(again.iter().any(|s| *s != 0.0));
    }

    #[test]
    fn test_a_full_buffer_closes_itself_at_its_length() {
        let mut looper = Looper::with_seconds(0.5);
        let control = looper.control();
        control.press();
        play(&mut looper, &tone(0, 30_000, 220.0, 0.3), None);
        assert_eq!(looper.state(), State::Playing);
        assert_eq!(control.length(), 24_000);
        assert_eq!(looper.capacity(), 24_000);
    }

    #[test]
    fn test_a_change_of_sample_rate_keeps_no_stale_audio() {
        let mut looper = fresh();
        record(&mut looper, &tone(0, 4800, 220.0, 0.5), None);
        looper.set_sample_rate(RATE);
        assert_eq!(looper.state(), State::Playing, "the same rate leaves the loop");
        looper.set_sample_rate(44_100.0);
        assert_eq!(looper.state(), State::Empty);
        assert_eq!(looper.capacity(), 44_100);
        let input = tone(0, 480, 440.0, 0.3);
        assert_eq!(play(&mut looper, &input, None), input);
        // A press records, from nothing
        let control = looper.control();
        control.press();
        play(&mut looper, &vec![0.0; 3000], None);
        control.press();
        let silent = play(&mut looper, &vec![0.0; 3000], None);
        assert!(silent.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn test_high_sample_rates_get_a_shorter_buffer() {
        let mut looper = Looper::new();
        looper.set_sample_rate(48_000.0);
        assert_eq!(looper.capacity(), 60 * 48_000);
        looper.set_sample_rate(192_000.0);
        assert_eq!(looper.capacity(), 40 * 192_000);
    }

    #[test]
    fn test_stop_mutes_the_loop_and_keeps_it_in_time() {
        let mut looper = fresh();
        let control = looper.control();
        let phrase = tone(0, 4800, 220.0, 0.5);
        record(&mut looper, &phrase, None);
        control.stop();
        let muted = play(&mut looper, &vec![0.0; 2000], None);
        assert_eq!(looper.state(), State::Stopped);
        assert!(muted[FADE + 100..].iter().all(|s| *s == 0.0), "silent after the fade");
        control.stop();
        let back = play(&mut looper, &vec![0.0; 2000], None);
        assert_eq!(looper.state(), State::Playing);
        // 100 + 2000 samples in, then 2000 more: the loop is where it would have been
        let at = 2100 + FADE * 2;
        assert!((back[at - 2100] - phrase[at]).abs() < 1e-6);
        assert!(!looper.is_idle());
    }

    #[test]
    fn test_level_sets_the_loop_against_the_live_sound() {
        let mut quiet = fresh();
        let mut loud = fresh();
        let phrase = tone(0, 4800, 220.0, 0.4);
        record(&mut quiet, &phrase, None);
        record(&mut loud, &phrase, None);
        let block = |looper: &mut Looper, level: f32| {
            let mut l = vec![0.0f32; 2000];
            let settings = LooperSettings { level, ..LooperSettings::default() };
            // Let the level settle
            for _ in 0..4 {
                looper.process(&settings, &mut l, None);
            }
            l.iter().fold(0.0f32, |a, s| a.max(s.abs()))
        };
        let half = block(&mut quiet, level_gain(-6.0));
        let full = block(&mut loud, 1.0);
        assert!(half < full * 0.55 && half > full * 0.45, "{half} against {full}");
        assert_eq!(level_gain(LEVEL_MIN_DB), 0.0);
        assert!((level_gain(0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_the_tuner_silences_the_loop() {
        let mut looper = fresh();
        record(&mut looper, &tone(0, 4800, 220.0, 0.5), None);
        let settings = LooperSettings { muted: true, ..LooperSettings::default() };
        let mut left = vec![0.0f32; 2000];
        looper.process(&settings, &mut left, None);
        assert!(left[FADE..].iter().all(|s| *s == 0.0));
    }

    #[test]
    fn test_a_press_that_closes_a_loop_of_nothing_goes_back_to_empty() {
        let mut looper = fresh();
        let control = looper.control();
        control.press();
        play(&mut looper, &[0.1; 10], None);
        control.press();
        play(&mut looper, &[0.1; 10], None);
        assert_eq!(looper.state(), State::Empty);
    }

    #[test]
    fn test_a_mono_block_records_and_plays_its_channel() {
        let mut looper = fresh();
        let control = looper.control();
        let phrase = tone(0, 4800, 220.0, 0.5);
        control.press();
        let mut block = phrase.clone();
        looper.process(&LooperSettings::default(), &mut block, None);
        control.press();
        let mut block = vec![0.0f32; 4800];
        looper.process(&LooperSettings::default(), &mut block, None);
        assert!((block[FADE + 10] - phrase[FADE + 10]).abs() < 1e-6);
    }

    #[test]
    fn test_pressing_while_the_loop_fades_out_is_ignored() {
        let mut looper = fresh();
        let control = looper.control();
        record(&mut looper, &tone(0, 4800, 220.0, 0.5), None);
        control.clear();
        control.press();
        play(&mut looper, &vec![0.0; 2000], None);
        assert_eq!(looper.state(), State::Empty);
    }

    /// What the looper costs in a block of 64 samples, empty, playing and overdubbing, against
    /// the amp's own CPU in `amp_report`:
    /// `cargo test -p amp --release looper_report -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn looper_report() {
        let mut looper = Looper::new();
        looper.set_sample_rate(48_000.0);
        let control = looper.control();
        let settings = LooperSettings::default();
        let (mut left, mut right) = (vec![0.1f32; 64], vec![0.1f32; 64]);
        let time = |looper: &mut Looper, what: &str, left: &mut [f32], right: &mut [f32]| {
            let blocks = 200_000;
            let start = std::time::Instant::now();
            for _ in 0..blocks {
                looper.process(&settings, left, Some(&mut *right));
            }
            let each = start.elapsed().as_secs_f64() / blocks as f64;
            // A block of 64 at 48 kHz lasts 1333 us
            println!(
                "looper {what}: {:.3} us a block of 64 ({:.4} % of a core)",
                each * 1e6,
                each / 64.0 * 48_000.0 * 100.0
            );
        };
        time(&mut looper, "empty", &mut left, &mut right);
        control.press();
        looper.process(&settings, &mut left, Some(&mut right));
        for _ in 0..2000 {
            looper.process(&settings, &mut left, Some(&mut right));
        }
        control.press();
        time(&mut looper, "playing", &mut left, &mut right);
        control.press();
        time(&mut looper, "overdubbing", &mut left, &mut right);
    }
}
