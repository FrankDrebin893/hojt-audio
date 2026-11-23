//! Looper: records the amp's finished sound, plays it back on top of what is played and adds
//! to it (#280). It sits after the whole chain and before the metronome's click, so the loop
//! is what the player heard and a click is never in it, and it is not run through the amp again.
//!
//! The buffer is allocated once, in `set_sample_rate` (never on the audio thread), and the
//! length of a loop is counted in samples, so it holds to the sample and never drifts. A 5 ms
//! fade at the ends of a recorded loop and of an overdub takes the click out of the seam.
//! The editor reaches it through [`LooperControl`], atomics only, so nothing locks.

use nih_plug::prelude::Enum;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

/// The longest loop, in seconds of stereo; at a sample rate above `HIGH_RATE` it is `HIGH_RATE_SECONDS`
pub const MAX_SECONDS: f32 = 60.0;
pub const HIGH_RATE_SECONDS: f32 = 40.0;
const HIGH_RATE: f32 = 96_000.0;

/// How many tracks play together (#452): track 1 sets the length, tracks 2 and 3 are one pass of it
pub const TRACKS: usize = 3;

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

/// How long a loop is, in bars of the click (#450). Frozen like the time signatures: new ones go
/// at the end
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopBars {
    /// Whatever is played, closed by a second press (rounded to whole bars where the click is on)
    #[id = "free"]
    #[name = "Free"]
    Free,
    #[id = "bars1"]
    #[name = "1 bar"]
    Bars1,
    #[id = "bars2"]
    #[name = "2 bars"]
    Bars2,
    #[id = "bars4"]
    #[name = "4 bars"]
    Bars4,
    #[id = "bars8"]
    #[name = "8 bars"]
    Bars8,
}

impl LoopBars {
    pub const ALL: [Self; 5] = [Self::Free, Self::Bars1, Self::Bars2, Self::Bars4, Self::Bars8];

    /// How many bars, or `None` for Free
    pub fn bars(self) -> Option<u32> {
        match self {
            Self::Free => None,
            Self::Bars1 => Some(1),
            Self::Bars2 => Some(2),
            Self::Bars4 => Some(4),
            Self::Bars8 => Some(8),
        }
    }

    /// What the plate's stepper shows
    pub fn name(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Bars1 => "1 bar",
            Self::Bars2 => "2 bars",
            Self::Bars4 => "4 bars",
            Self::Bars8 => "8 bars",
        }
    }
}

/// Whether the click goes on sounding while the loop plays (#450). Frozen like `LoopBars`
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopClick {
    /// As it always was: the click sounds as long as it is on
    #[id = "always"]
    #[name = "Always"]
    Always,
    /// Only while the loop is empty or recording; silent while it plays, overdubs or is muted
    #[id = "recording"]
    #[name = "Rec only"]
    Recording,
}

impl LoopClick {
    pub const ALL: [Self; 2] = [Self::Always, Self::Recording];

    pub fn name(self) -> &'static str {
        match self {
            Self::Always => "Always",
            Self::Recording => "Rec only",
        }
    }

    /// Whether the click is held back while the looper is in `state`
    pub fn hushes(self, state: State) -> bool {
        self == Self::Recording
            && matches!(state, State::Playing | State::Overdubbing | State::Stopped)
    }
}

/// How many bars of the click are played before a recording starts (#451). Frozen like
/// `LoopBars`: new ones go at the end
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountIn {
    /// Record starts at once (or on the next downbeat), as it always did
    #[id = "off"]
    #[name = "Off"]
    Off,
    #[id = "bar1"]
    #[name = "1 bar"]
    Bar1,
    #[id = "bars2"]
    #[name = "2 bars"]
    Bars2,
}

impl CountIn {
    pub const ALL: [Self; 3] = [Self::Off, Self::Bar1, Self::Bars2];

    /// How many bars, or `None` for Off
    pub fn bars(self) -> Option<u32> {
        match self {
            Self::Off => None,
            Self::Bar1 => Some(1),
            Self::Bars2 => Some(2),
        }
    }

    /// What the plate's stepper shows
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Bar1 => "1 bar",
            Self::Bars2 => "2 bars",
        }
    }
}

/// A count-in a Record press would start, worked out ahead of the block by `lib.rs` from the
/// click (#451)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CountInPlan {
    /// Samples from the start of the block until the count-in begins: zero where the click is
    /// restarted by the press, the wait for the host's next downbeat where it follows the host
    pub lead: u32,
    /// The count-in's own length in samples: its bars times the bar
    pub length: u32,
    /// How many bars it is, and how many beats a bar has: what the plate counts
    pub bars: u32,
    pub per_bar: u32,
    /// The click is to restart at the press, so the press is beat one (it runs by itself)
    pub restart: bool,
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
    /// Commands, per track: index 0 is track 1, which sets the loop's length for all
    presses: [AtomicU8; TRACKS],
    stop: [AtomicBool; TRACKS],
    clear: [AtomicBool; TRACKS],
    state: AtomicU8,
    /// The track the plate's buttons act on (0 to 2): the editor's choice, kept here so it
    /// survives the window being closed
    selected: AtomicU8,
    /// What tracks 2 and 3 are doing, and whether one waits for the loop to come round (#452)
    track_states: [AtomicU8; TRACKS - 1],
    track_waiting: [AtomicBool; TRACKS - 1],
    length: AtomicU32,
    position: AtomicU32,
    capacity: AtomicU32,
    /// The length a recording is fixed at, in samples, while one is (Bars set); zero otherwise
    goal: AtomicU32,
    /// While a count-in plays: the beat of the bar it is on (from 1; zero while it still waits
    /// for the host's downbeat), and how many beats a bar has; both zero otherwise
    count_beat: AtomicU8,
    count_beats: AtomicU8,
}

impl LooperControl {
    /// The big button of track 1: Record, then Play, then Overdub, then Play again
    pub fn press(&self) {
        self.press_track(0);
    }

    /// Mute track 1 and keep it, or bring it back
    #[cfg_attr(not(test), allow(dead_code))] // the plate stops by track; the tests use this
    pub fn stop(&self) {
        self.stop_track(0);
    }

    /// Empty track 1, which empties them all: their length is gone
    #[cfg_attr(not(test), allow(dead_code))] // the plate clears by track; the tests use this
    pub fn clear(&self) {
        self.clear_track(0);
    }

    /// The big button of `track` (0 to 2). Tracks 2 and 3 wait for the loop to come round, record
    /// one pass of it and play; they do nothing while track 1 holds no loop
    pub fn press_track(&self, track: usize) {
        self.presses[track.min(TRACKS - 1)].fetch_add(1, Ordering::Release);
    }

    /// Mute `track` and keep it, or bring it back
    pub fn stop_track(&self, track: usize) {
        self.stop[track.min(TRACKS - 1)].store(true, Ordering::Release);
    }

    /// Empty `track`; clearing track 1 clears every track
    pub fn clear_track(&self, track: usize) {
        self.clear[track.min(TRACKS - 1)].store(true, Ordering::Release);
    }

    /// Where `track` (0 to 2) is. A track that waits for the loop to come round is still Empty
    /// here: `track_waiting` says it waits
    pub fn track_state(&self, track: usize) -> State {
        match track {
            0 => self.state(),
            t => State::from_u8(self.track_states[(t - 1).min(TRACKS - 2)].load(Ordering::Acquire)),
        }
    }

    /// The track the plate's buttons and ring are on
    pub fn selected(&self) -> usize {
        usize::from(self.selected.load(Ordering::Relaxed)).min(TRACKS - 1)
    }

    pub fn select(&self, track: usize) {
        self.selected.store(track.min(TRACKS - 1) as u8, Ordering::Relaxed);
    }

    /// Track 2 or 3 is pressed to record and waits for the loop's start
    pub fn track_waiting(&self, track: usize) -> bool {
        track > 0 && self.track_waiting[(track - 1).min(TRACKS - 2)].load(Ordering::Relaxed)
    }

    /// Track 2 or 3 records or waits to: what the click's Rec only mode counts as recording
    pub fn extra_recording(&self) -> bool {
        (1..TRACKS).any(|t| self.track_waiting(t) || self.track_state(t) == State::Recording)
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

    /// What the ring fills against while recording: the length a set number of bars fixed at
    /// the press, so it goes a full circle exactly when the loop is complete, else what the
    /// buffer holds
    pub fn fills_against(&self) -> u32 {
        match self.goal.load(Ordering::Relaxed) {
            0 => self.capacity(),
            goal => goal,
        }
    }

    /// While a count-in plays: the beat it is on (zero while it waits to begin) and the beats of
    /// a bar; `None` when there is no count-in
    pub fn count_in(&self) -> Option<(u32, u32)> {
        match self.count_beats.load(Ordering::Relaxed) {
            0 => None,
            beats => Some((u32::from(self.count_beat.load(Ordering::Relaxed)), u32::from(beats))),
        }
    }

    /// Sets the count-in the editor reads, for the editor's own tests
    #[cfg(test)]
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))] // its tests draw, and run on Linux alone
    pub(crate) fn show_count(&self, beat: u8, beats: u8) {
        self.count_beat.store(beat, Ordering::Relaxed);
        self.count_beats.store(beats, Ordering::Relaxed);
    }

    /// Fixes the length a recording fills, for the editor's own tests
    #[cfg(test)]
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))] // its tests draw, and run on Linux alone
    pub(crate) fn show_goal(&self, goal: u32) {
        self.goal.store(goal, Ordering::Relaxed);
    }

    /// Sets what the editor reads, for the editor's own tests
    #[cfg(test)]
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))] // its tests draw, and run on Linux alone
    pub(crate) fn show(&self, state: State, length: u32, position: u32, capacity: u32) {
        self.length.store(length, Ordering::Relaxed);
        self.position.store(position, Ordering::Relaxed);
        self.capacity.store(capacity, Ordering::Relaxed);
        self.state.store(state as u8, Ordering::Release);
    }

    /// Sets what the editor reads of track 2 or 3, for the editor's own tests
    #[cfg(test)]
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))] // its tests draw, and run on Linux alone
    pub(crate) fn show_track(&self, track: usize, state: State, waiting: bool) {
        self.track_states[track - 1].store(state as u8, Ordering::Release);
        self.track_waiting[track - 1].store(waiting, Ordering::Relaxed);
    }

    /// The commands of `track` the audio has not taken yet: presses, stop, clear
    #[cfg(test)]
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))] // its tests draw, and run on Linux alone
    pub(crate) fn pending(&self, track: usize) -> (u8, bool, bool) {
        (
            self.presses[track].load(Ordering::Acquire),
            self.stop[track].load(Ordering::Acquire),
            self.clear[track].load(Ordering::Acquire),
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
    /// The length a recording is to have, in samples: the Bars setting times the bar, where the
    /// click is on. Taken when the recording starts; when it is reached the loop closes by itself
    pub length_samples: Option<u32>,
    /// The count-in a Record press starts, where one is set and the click can play it (#451)
    pub count_in: Option<CountInPlan>,
    /// Linear gain of tracks 2 and 3 against the live sound (`level` is track 1's)
    pub extra_levels: [f32; TRACKS - 1],
}

impl Default for LooperSettings {
    fn default() -> Self {
        Self {
            level: level_gain(LEVEL_DEFAULT_DB),
            muted: false,
            bar_samples: None,
            downbeat_samples: None,
            length_samples: None,
            count_in: None,
            extra_levels: [level_gain(LEVEL_DEFAULT_DB); TRACKS - 1],
        }
    }
}

/// What a track beside the first needs to know about the sample it plays
struct Pass {
    /// The loop's length, and the sample of it now
    len: usize,
    pos: usize,
    /// One step of the 5 ms ramp, and the muting of the tuner
    step: f32,
    level: f32,
    level_coef: f32,
    muted: bool,
}

/// Track 2 or 3 (#452): one pass of track 1's loop, recorded as it goes round and played with it.
/// It has the loop's length and counts on the loop's position, so it cannot drift from it
struct Extra {
    left: Vec<f32>,
    right: Vec<f32>,
    state: State,
    /// Pressed to record: begins when the loop next starts
    waiting: bool,
    play_gain: f32,
    dub_gain: f32,
    clearing: bool,
    level: f32,
}

impl Extra {
    fn new() -> Self {
        Self {
            left: Vec::new(),
            right: Vec::new(),
            state: State::Empty,
            waiting: false,
            play_gain: 0.0,
            dub_gain: 0.0,
            clearing: false,
            level: 1.0,
        }
    }

    /// Nothing recorded; the buffer is left as it is, a pass writes before it reads
    fn empty(&mut self) {
        self.state = State::Empty;
        self.waiting = false;
        self.play_gain = 0.0;
        self.dub_gain = 0.0;
        self.clearing = false;
    }

    /// The loop's gain, or the gain it fades to, is silence: Empty or clearing
    fn active(&self) -> bool {
        self.state != State::Empty || self.waiting
    }

    /// A press that closes a pass before the loop is round: what was played stays, faded out at the
    /// end, and the rest of the loop is silent; a pass of under 10 ms is thrown away
    fn close_early(&mut self, pos: usize, len: usize, fade: usize) {
        if pos < 2 * fade {
            return self.empty();
        }
        for k in 0..fade {
            let gain = (fade - k) as f32 / fade as f32;
            self.left[pos - fade + k] *= gain;
            self.right[pos - fade + k] *= gain;
        }
        self.left[pos..len].fill(0.0);
        self.right[pos..len].fill(0.0);
        self.state = State::Playing;
        self.play_gain = 1.0;
        self.dub_gain = 0.0;
    }

    /// One sample: records or overdubs the live sound, and gives what the track plays
    fn step(&mut self, pass: &Pass, live_l: f32, live_r: f32) -> (f32, f32) {
        let pos = pass.pos;
        if self.waiting && pos == 0 {
            self.waiting = false;
            self.state = State::Recording;
        }
        if self.state == State::Recording {
            // Faded in over the first 5 ms of the pass and out over the last
            let ramp = (pos.min(pass.len - pos) as f32 * pass.step).min(1.0);
            self.left[pos] = live_l * ramp;
            self.right[pos] = live_r * ramp;
            if pos + 1 == pass.len {
                self.state = State::Playing;
                self.play_gain = 1.0;
                self.dub_gain = 0.0;
            }
            return (0.0, 0.0);
        }
        if self.state == State::Empty {
            return (0.0, 0.0);
        }
        let silent = self.state == State::Stopped || pass.muted || self.clearing;
        let target = if silent { 0.0 } else { 1.0 };
        self.play_gain += (target - self.play_gain).clamp(-pass.step, pass.step);
        let dub = if self.state == State::Overdubbing && !self.clearing { 1.0 } else { 0.0 };
        self.dub_gain += (dub - self.dub_gain).clamp(-pass.step, pass.step);
        self.level += (pass.level - self.level) * pass.level_coef;
        let (old_l, old_r) = (self.left[pos], self.right[pos]);
        if self.dub_gain > 0.0 {
            self.left[pos] = crate::delay::soft_limit(old_l + live_l * self.dub_gain);
            self.right[pos] = crate::delay::soft_limit(old_r + live_r * self.dub_gain);
        }
        let heard = self.level * self.play_gain;
        if self.clearing && self.play_gain == 0.0 && self.dub_gain == 0.0 {
            self.empty();
        }
        (old_l * heard, old_r * heard)
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
    /// The count-in the armed wait is, when it is one (#451)
    counting: Option<CountInPlan>,
    /// A count-in began this block and the click is to restart: `lib.rs` takes it
    restart_click: bool,
    /// A recording of a set length: the length and the bar it is in, both taken when the
    /// recording started, so a tempo change does not move them
    fixed: Option<(usize, u32)>,
    level: f32,
    level_coef: f32,
    /// Tracks 2 and 3 (#452)
    extras: [Extra; TRACKS - 1],
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
            counting: None,
            restart_click: false,
            fixed: None,
            level: 1.0,
            level_coef: 1.0,
            extras: [Extra::new(), Extra::new()],
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
        // Tracks 2 and 3 are as long as track 1 can be: three full buffers (zeroed memory the
        // system hands out as it is used), here and nowhere else
        for extra in &mut self.extras {
            if extra.left.len() != capacity {
                extra.left = vec![0.0; capacity];
                extra.right = vec![0.0; capacity];
            }
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
        self.disarm();
        self.fixed = None;
        for extra in &mut self.extras {
            extra.empty();
        }
        self.publish();
    }

    /// Takes back a wait for a downbeat or a count-in
    fn disarm(&mut self) {
        self.armed = None;
        self.counting = None;
        self.restart_click = false;
    }

    /// True once after a count-in began that wants the click to restart at the press
    pub fn take_restart(&mut self) -> bool {
        std::mem::take(&mut self.restart_click)
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
        let goal = match (self.state, self.fixed) {
            (State::Recording, Some((length, _))) => length as u32,
            _ => 0,
        };
        self.control.goal.store(goal, Ordering::Relaxed);
        let (beat, beats) = match (self.armed, self.counting) {
            (Some(left), Some(plan)) => {
                let elapsed = (plan.lead as usize + plan.length as usize).saturating_sub(left);
                let beat = match elapsed.checked_sub(plan.lead as usize) {
                    None => 0,
                    Some(into) => {
                        let total = u64::from(plan.bars * plan.per_bar);
                        let k =
                            (into as u64 * total / u64::from(plan.length.max(1))).min(total - 1);
                        k % u64::from(plan.per_bar) + 1
                    }
                };
                (beat as u8, plan.per_bar as u8)
            }
            _ => (0, 0),
        };
        self.control.count_beat.store(beat, Ordering::Relaxed);
        self.control.count_beats.store(beats, Ordering::Relaxed);
        for (index, extra) in self.extras.iter().enumerate() {
            self.control.track_waiting[index].store(extra.waiting, Ordering::Relaxed);
            self.control.track_states[index].store(extra.state as u8, Ordering::Release);
        }
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
        self.fixed = None;
        self.state = State::Playing;
        self.play_gain = 1.0;
        self.dub_gain = 0.0;
    }

    fn start_recording(&mut self, settings: &LooperSettings) {
        self.disarm();
        self.state = State::Recording;
        self.len = 0;
        self.pos = 0;
        // A set number of bars: the length and the bar are fixed now, to the whole bars the
        // buffer holds. Too short for a loop, or a bar longer than the buffer: as Free
        let capacity = self.capacity();
        self.fixed = settings
            .length_samples
            .zip(settings.bar_samples)
            .filter(|(length, bar)| *length > 0 && *bar > 0)
            .map(|(length, bar)| {
                let (length, bar_len) = (length as usize, bar as usize);
                let length = if length > capacity { capacity / bar_len * bar_len } else { length };
                (length, bar)
            })
            .filter(|(length, _)| *length >= 2 * self.fade);
    }

    fn take_commands(&mut self, settings: &LooperSettings) {
        let bar = settings.bar_samples;
        if self.control.clear[0].swap(false, Ordering::Acquire) {
            match self.state {
                State::Empty => self.disarm(),
                State::Recording => self.empty(),
                _ => {
                    // Track 1 sets the length of all: they fade out with it
                    self.clearing = true;
                    for extra in &mut self.extras {
                        match extra.state {
                            State::Playing | State::Overdubbing | State::Stopped => {
                                extra.clearing = true
                            }
                            _ => extra.empty(),
                        }
                    }
                }
            }
        }
        for _ in 0..self.control.presses[0].swap(0, Ordering::Acquire) {
            if self.clearing {
                break;
            }
            match self.state {
                State::Empty if self.capacity() == 0 => {}
                // Waiting for a downbeat: a second press takes it back
                State::Empty if self.armed.is_some() => self.disarm(),
                // A count-in: the click's bars, then the recording starts on the downbeat after
                State::Empty => match settings.count_in.filter(|plan| plan.length > 0) {
                    Some(plan) => {
                        self.armed = Some(plan.lead as usize + plan.length as usize);
                        self.counting = Some(plan);
                        self.restart_click = plan.restart;
                    }
                    None => match settings.downbeat_samples {
                        Some(wait) if wait > 0 => self.armed = Some(wait as usize),
                        _ => self.start_recording(settings),
                    },
                },
                // A set length rounds to the bar it was started in
                State::Recording => self.close(self.fixed.map_or(bar, |(_, bar)| Some(bar))),
                State::Playing => self.state = State::Overdubbing,
                State::Overdubbing | State::Stopped => self.state = State::Playing,
            }
        }
        if self.control.stop[0].swap(false, Ordering::Acquire) && !self.clearing {
            match self.state {
                State::Playing | State::Overdubbing => self.state = State::Stopped,
                State::Stopped => self.state = State::Playing,
                State::Empty | State::Recording => {}
            }
        }
        self.take_extra_commands();
    }

    /// The commands of tracks 2 and 3. They need a loop to be in time with: while track 1 holds
    /// none, or fades out, they do nothing
    fn take_extra_commands(&mut self) {
        let looping = !self.clearing
            && matches!(self.state, State::Playing | State::Overdubbing | State::Stopped);
        let (len, pos, fade) = (self.len, self.pos, self.fade);
        for (index, extra) in self.extras.iter_mut().enumerate() {
            let track = index + 1;
            if self.control.clear[track].swap(false, Ordering::Acquire) {
                match extra.state {
                    State::Playing | State::Overdubbing | State::Stopped => extra.clearing = true,
                    _ => extra.empty(),
                }
            }
            for _ in 0..self.control.presses[track].swap(0, Ordering::Acquire) {
                if !looping || extra.clearing {
                    break;
                }
                match extra.state {
                    // Waiting for the loop to come round: a second press takes it back
                    State::Empty => extra.waiting = !extra.waiting,
                    State::Recording => extra.close_early(pos, len, fade),
                    State::Playing => extra.state = State::Overdubbing,
                    State::Overdubbing | State::Stopped => extra.state = State::Playing,
                }
            }
            if self.control.stop[track].swap(false, Ordering::Acquire) && !extra.clearing {
                match extra.state {
                    State::Playing | State::Overdubbing => extra.state = State::Stopped,
                    State::Stopped => extra.state = State::Playing,
                    State::Empty | State::Recording => {}
                }
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
        for (extra, level) in self.extras.iter_mut().zip(settings.extra_levels) {
            if !extra.active() {
                extra.level = level;
            }
        }
        for index in 0..left.len() {
            let live_l = left[index];
            let live_r = right.as_deref().map_or(0.0, |right| right[index]);

            // The first press waits for the downbeat: until then the sound passes untouched
            if let Some(wait) = self.armed {
                if wait == 0 {
                    self.start_recording(settings);
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
                // The set length is reached: the loop is exactly that many bars, and it plays
                // from its first sample on the very next one, with no press
                if let Some((_, bar)) = self.fixed.filter(|(length, _)| self.len == *length) {
                    self.close(Some(bar));
                } else if self.len == self.capacity() {
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
            let (mut sum_l, mut sum_r) = (old_l * heard, old_r * heard);
            // Tracks 2 and 3 count on this track's position: one loop, in time
            for (extra, level) in self.extras.iter_mut().zip(settings.extra_levels) {
                if extra.active() {
                    let pass = Pass {
                        len: self.len,
                        pos: self.pos,
                        step,
                        level,
                        level_coef: self.level_coef,
                        muted: settings.muted,
                    };
                    let (more_l, more_r) = extra.step(&pass, live_l, live_r);
                    sum_l += more_l;
                    sum_r += more_r;
                }
            }
            left[index] = (live_l + sum_l).clamp(-1.0, 1.0);
            if let Some(right) = right.as_deref_mut() {
                right[index] = (live_r + sum_r).clamp(-1.0, 1.0);
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
mod tracks_tests;

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

    /// What `blocks` of `size` through a looper set to two bars of 4000 give, from a press
    /// on the first sample: the output, and the looper
    fn two_bars(size: usize, total: usize) -> (Vec<f32>, Looper) {
        let mut looper = Looper::with_seconds(1.0);
        looper.control().press();
        let settings = LooperSettings {
            bar_samples: Some(4000),
            length_samples: Some(8000),
            ..LooperSettings::default()
        };
        let input = tone(0, total, 220.0, 0.4);
        let mut heard = Vec::new();
        for block in input.chunks(size) {
            let mut left = block.to_vec();
            looper.process(&settings, &mut left, None);
            heard.extend(left);
        }
        (heard, looper)
    }

    #[test]
    fn test_a_set_number_of_bars_closes_itself_on_the_sample_and_plays_on_the_next() {
        for size in [64usize, 333, 512] {
            let (heard, looper) = two_bars(size, 8000 + 3000);
            assert_eq!(looper.state(), State::Playing, "block {size}");
            assert_eq!(looper.control().length(), 8000, "block {size}");
            // Up to the close the sound passes untouched; the loop begins on the next sample
            let phrase = tone(0, 8000 + 3000, 220.0, 0.4);
            assert_eq!(heard[..8000], phrase[..8000], "block {size}");
            let (recorded, _) = looper.recorded();
            for k in 0..3000 {
                let want = (phrase[8000 + k] + recorded[k]).clamp(-1.0, 1.0);
                assert!((heard[8000 + k] - want).abs() < 1e-6, "block {size} sample {k}");
            }
            // And the recording is what was played, faded at its ends
            for k in FADE..8000 - FADE {
                assert_eq!(recorded[k], phrase[k], "block {size} sample {k}");
            }
        }
    }

    #[test]
    fn test_a_loop_of_a_set_length_is_the_same_however_the_blocks_fall() {
        // The loop of a set length is the same however the blocks fall
        let (_, a) = two_bars(64, 9000);
        let (_, b) = two_bars(1000, 9000);
        assert_eq!(a.recorded().0, b.recorded().0);
        assert_eq!(a.control().position(), b.control().position());
    }

    #[test]
    fn test_a_press_before_a_set_length_is_full_closes_at_whole_bars() {
        let mut looper = fresh();
        let control = looper.control();
        let settings = LooperSettings {
            bar_samples: Some(4000),
            length_samples: Some(16_000),
            ..LooperSettings::default()
        };
        control.press();
        looper.process(&settings, &mut vec![0.3; 5600], None);
        assert_eq!(looper.state(), State::Recording);
        assert_eq!(control.fills_against(), 16_000, "the ring fills against the set length");
        control.press();
        looper.process(&settings, &mut [0.0; 10], None);
        assert_eq!(looper.state(), State::Playing);
        assert_eq!(control.length(), 4000, "1.4 bars is one");
        assert_eq!(control.fills_against(), looper.capacity() as u32, "back to the buffer");
    }

    #[test]
    fn test_a_tempo_change_while_recording_does_not_move_the_set_length() {
        let mut looper = fresh();
        let settings = LooperSettings {
            bar_samples: Some(4000),
            length_samples: Some(8000),
            ..LooperSettings::default()
        };
        looper.control().press();
        looper.process(&settings, &mut vec![0.3; 3000], None);
        // The click is slowed to a bar of 6000: the length is as it was fixed at the press
        let slower = LooperSettings {
            bar_samples: Some(6000),
            length_samples: Some(12_000),
            ..LooperSettings::default()
        };
        looper.process(&slower, &mut vec![0.3; 5500], None);
        assert_eq!(looper.state(), State::Playing);
        assert_eq!(looper.control().length(), 8000);
    }

    #[test]
    fn test_a_set_length_longer_than_the_buffer_is_the_whole_bars_that_fit() {
        // The buffer holds 48000: three bars of 16000 fit, the fourth does not
        let mut looper = fresh();
        let settings = LooperSettings {
            bar_samples: Some(16_000),
            length_samples: Some(64_000),
            ..LooperSettings::default()
        };
        looper.control().press();
        looper.process(&settings, &mut vec![0.3; 50_000], None);
        assert_eq!(looper.state(), State::Playing);
        assert_eq!(looper.control().length(), 48_000);
        // A bar longer than the buffer: as Free, the buffer's end
        let mut looper = fresh();
        let settings = LooperSettings {
            bar_samples: Some(60_000),
            length_samples: Some(60_000),
            ..LooperSettings::default()
        };
        looper.control().press();
        looper.process(&settings, &mut vec![0.3; 50_000], None);
        assert_eq!(looper.control().length(), 48_000);
        assert_eq!(looper.control().fills_against(), looper.capacity() as u32);
    }

    #[test]
    fn test_no_set_length_is_free_as_it_was() {
        // No length in the settings: the buffer's end closes it, a press closes at the bar
        let mut looper = fresh();
        let settings = LooperSettings { bar_samples: Some(4000), ..LooperSettings::default() };
        looper.control().press();
        looper.process(&settings, &mut vec![0.3; 9000], None);
        assert_eq!(looper.state(), State::Recording);
        assert_eq!(looper.control().fills_against(), looper.capacity() as u32);
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

    // A count-in (#451): one bar of 96 000 samples (120 bpm, 4/4 at 48 kHz) of the click, then
    // the recording

    const BAR: u32 = 96_000;

    fn plan(lead: u32, bars: u32, restart: bool) -> CountInPlan {
        CountInPlan { lead, length: bars * BAR, bars, per_bar: 4, restart }
    }

    /// Plays `total` samples of a tone through `looper` in blocks of `block`, with `settings`;
    /// `after` sees the looper after each block with the samples played so far
    fn run(
        looper: &mut Looper,
        settings: &LooperSettings,
        total: usize,
        block: usize,
        mut after: impl FnMut(usize, &Looper),
    ) -> Vec<f32> {
        let mut heard = Vec::new();
        let mut played = 0;
        while played < total {
            let n = block.min(total - played);
            let mut left = tone(played, n, 220.0, 0.4);
            let mut right = left.clone();
            looper.process(settings, &mut left, Some(&mut right));
            heard.extend(left);
            played += n;
            after(played, looper);
        }
        heard
    }

    #[test]
    fn test_a_count_in_starts_the_recording_exactly_one_bar_after_the_press_and_touches_nothing() {
        for block in [64usize, 333, 512] {
            let mut looper = Looper::with_seconds(6.0);
            let settings =
                LooperSettings { count_in: Some(plan(0, 1, true)), ..Default::default() };
            looper.control().press();
            let mut first_recording = None;
            let total = BAR as usize + 1000;
            let heard = run(&mut looper, &settings, total, block, |played, looper| {
                if first_recording.is_none() && looper.state() == State::Recording {
                    first_recording = Some(played);
                }
                assert!(!looper.is_idle(), "the plugin stays awake while it counts");
            });
            // The block that begins the recording is the first at or past the bar
            let began = first_recording.expect("recording began");
            assert!(began > BAR as usize && began <= BAR as usize + block, "{block}: {began}");
            // Nothing recorded before the bar: from sample 96 000 on, what was played
            let (recorded, _) = looper.recorded();
            assert_eq!(recorded.len(), 1000, "{block}");
            let want = tone(BAR as usize + 300, 100, 220.0, 0.4);
            assert_eq!(&recorded[300..400], &want[..], "{block}: the first sample is the bar's");
            // The sound passed untouched during the count-in
            assert_eq!(&heard[..BAR as usize], &tone(0, BAR as usize, 220.0, 0.4)[..], "{block}");
        }
    }

    #[test]
    fn test_a_second_press_or_clear_takes_a_count_in_back() {
        for take_back in [0, 1] {
            let mut looper = Looper::with_seconds(6.0);
            let control = looper.control();
            let settings =
                LooperSettings { count_in: Some(plan(0, 2, true)), ..Default::default() };
            control.press();
            run(&mut looper, &settings, 10_000, 480, |_, _| {});
            assert!(control.count_in().is_some());
            if take_back == 0 {
                control.press();
            } else {
                control.clear();
            }
            run(&mut looper, &settings, 2 * BAR as usize + 5000, 480, |_, looper| {
                assert_eq!(looper.state(), State::Empty);
            });
            assert!(control.count_in().is_none());
            assert!(looper.is_idle());
            assert!(!looper.take_restart(), "a count-in taken back leaves the click alone");
        }
    }

    #[test]
    fn test_a_count_in_that_waits_for_the_hosts_downbeat_counts_after_it() {
        let mut looper = Looper::with_seconds(6.0);
        let settings = LooperSettings { count_in: Some(plan(700, 1, false)), ..Default::default() };
        looper.control().press();
        let mut began = None;
        run(&mut looper, &settings, BAR as usize + 2000, 100, |played, looper| {
            if began.is_none() && looper.state() == State::Recording {
                began = Some(played);
            }
        });
        // 700 to the downbeat, a bar of count-in, in blocks of 100: the block that holds it
        assert_eq!(
            began,
            Some(BAR as usize + 800),
            "sample 96 700 is the first of the recording, in the block ending at 96 800"
        );
        assert!(!looper.take_restart(), "a click that follows the host is not restarted");
    }

    #[test]
    fn test_the_beat_shown_follows_the_sample_count_and_the_restart_is_asked_once() {
        let mut looper = Looper::with_seconds(6.0);
        let control = looper.control();
        let settings = LooperSettings { count_in: Some(plan(0, 2, true)), ..Default::default() };
        control.press();
        let mut seen = Vec::new();
        let mut asked = 0;
        run(&mut looper, &settings, 2 * BAR as usize, 1000, |played, _| {
            seen.push((played, control.count_in()));
        });
        if looper.take_restart() {
            asked += 1;
        }
        assert_eq!(asked, 1);
        assert!(!looper.take_restart());
        for (played, shown) in seen {
            if played == 2 * BAR as usize {
                break;
            }
            // Four beats a bar, two bars: beat 1 to 4 and again, each 24 000 samples
            let beat = (played / 24_000) % 4 + 1;
            assert_eq!(shown, Some((beat as u32, 4)), "after {played}");
        }
        // The recording begins: no count any more
        run(&mut looper, &settings, 10, 10, |_, _| {});
        assert_eq!(control.count_in(), None);
        assert_eq!(looper.state(), State::Recording);
    }

    #[test]
    fn test_a_count_in_then_two_bars_line_up_to_the_sample() {
        let mut looper = Looper::with_seconds(6.0);
        let control = looper.control();
        let settings = LooperSettings {
            bar_samples: Some(BAR),
            length_samples: Some(2 * BAR),
            count_in: Some(plan(0, 1, true)),
            ..Default::default()
        };
        control.press();
        let mut closed = None;
        run(&mut looper, &settings, 3 * BAR as usize + 4000, 64, |played, looper| {
            if closed.is_none() && looper.state() == State::Playing {
                closed = Some(played);
            }
        });
        // Count in one bar, record two, close in the block holding sample 288 000
        let closed = closed.expect("it closed by itself");
        assert!((3 * BAR as usize..=3 * BAR as usize + 64).contains(&closed), "{closed}");
        assert_eq!(control.length(), 2 * BAR);
    }

    #[test]
    fn test_with_no_count_in_a_press_starts_as_it_always_did() {
        let mut looper = Looper::with_seconds(6.0);
        let settings = LooperSettings::default();
        looper.control().press();
        run(&mut looper, &settings, 100, 100, |_, _| {});
        assert_eq!(looper.state(), State::Recording);
        assert!(looper.control().count_in().is_none());
    }
}
