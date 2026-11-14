use nih_plug::prelude::Enum;
use suite_common::dsp::{advance, decay_coeff, note_hz, sine, Noise, Svf};

/// The two basses. They share the string and differ in how it is plucked and in what
/// stands behind it.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    /// Played with a pick: clean lows with a distorted band of mids over them
    Metal,
    /// Played with a finger: a warm tone filter and a little tube drive
    // New voices go at the end: a project remembers its voice by its place here
    Groove,
}

impl Voice {
    pub const ALL: [Voice; 2] = [Voice::Metal, Voice::Groove];

    /// Where the knobs stand when the voice is picked on the pedal
    pub fn settings(self) -> BassSettings {
        match self {
            Voice::Metal => BassSettings {
                voice: self,
                volume_db: -2.0,
                drive: 0.4,
                brightness: 0.6,
                sub: 0.15,
                tone_hz: 2600.0,
                sustain_s: 3.0,
                slide_ms: 0.0,
                transpose: 0.0,
                bend: BEND_DEFAULT,
            },
            Voice::Groove => BassSettings {
                voice: self,
                volume_db: -3.0,
                drive: 0.15,
                brightness: 0.54,
                sub: 0.15,
                tone_hz: 1400.0,
                sustain_s: 3.0,
                slide_ms: 60.0,
                transpose: 0.0,
                bend: BEND_DEFAULT,
            },
        }
    }
}

pub const VOLUME_MIN_DB: f32 = -30.0;
pub const VOLUME_MAX_DB: f32 = 12.0;
pub const TONE_MIN_HZ: f32 = 300.0;
pub const TONE_MAX_HZ: f32 = 4000.0;
pub const SUSTAIN_MIN_S: f32 = 0.5;
pub const SUSTAIN_MAX_S: f32 = 8.0;
pub const SLIDE_MAX_MS: f32 = 300.0;
pub const TRANSPOSE_RANGE: f32 = 24.0;
// How far the pitch-bend wheel goes at its end, in semitones
pub const BEND_MIN: i32 = 1;
pub const BEND_MAX: i32 = 12;
pub const BEND_DEFAULT: f32 = 2.0;

/// Everything the pedal sets, as the plugin reads it once per block
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BassSettings {
    pub voice: Voice,
    pub volume_db: f32,
    /// 0.0-1.0
    pub drive: f32,
    /// 0.0-1.0: how hard the string is plucked and how long its top rings
    pub brightness: f32,
    /// 0.0-1.0: level of the sine under the string
    pub sub: f32,
    /// The low-pass behind the string (Groove) or behind the drive (Metal)
    pub tone_hz: f32,
    /// Time for a held note to fall by 60 dB
    pub sustain_s: f32,
    /// Time of a slide between overlapping notes. 0 plucks every note
    pub slide_ms: f32,
    /// Semitones
    pub transpose: f32,
    /// Semitones the pitch-bend wheel goes at its end, up and down. A setting of the player's
    /// keyboard, not of a voice: every voice has the default, and the plugin's footswitches
    /// leave it where it is
    pub bend: f32,
}

impl Default for BassSettings {
    fn default() -> Self {
        Voice::Metal.settings()
    }
}

// Time for the string to fall by 60 dB once the note is let go
const RELEASE_METAL_S: f32 = 0.07;
const RELEASE_GROOVE_S: f32 = 0.12;

// The string's delay line at 48 kHz and below; longer in proportion above
const STRING_SAMPLES: usize = 16384;

// Where the string's top is heard: Brightness sets how long the partial here rings
const TOP_HZ: f32 = 1500.0;
// How long the top rings at Brightness 0 and 100 %, at full velocity
const TOP_T60_MIN_S: f32 = 0.15;
const TOP_T60_MAX_S: f32 = 0.8;

// Where the pick, the finger and the pickup are, as parts of the open string from the bridge.
// They are fixed distances from the bridge, as on an instrument: a note is played on the
// highest open string at or below it (`OPEN_STRINGS`), and on a fret they are a larger part of
// the shorter length that rings, `2^(fret/12)` times as large. So the pickup's comb takes out
// another harmonic on each fret, not always the 5th
const PICK: f32 = 0.18;
const FINGER: f32 = 0.12;
const PICKUP: f32 = 0.2;
// The four open strings of a bass, E1 A1 D2 G2
const OPEN_STRINGS: [f32; 4] = [28.0, 33.0, 38.0, 43.0];
// Up to where the pick, the finger and the pickup move: past the middle of the string the comb
// and the pluck are as they are as far from the other end
const ALONG_MOST: f32 = 0.3;

// How loud the string is, before the output stage
const STRING_GAIN: f32 = 0.62;
// The sine under the string at Sub 100 %, against the string's own fundamental: it sits under
// the string rather than being half of it
const SUB_GAIN: f32 = 1.5;
// The sine's T60 against the fundamental's: it never outlasts the string
const SUB_DECAY: f32 = 0.85;
// Where Metal splits the clean lows from the driven mids
const CROSSOVER_HZ: f32 = 120.0;
// The cabinet after each voice's output stage, the same for both: a low-mid lift for the note's
// body, a presence rise for its definition and a gentle cut where a bass speaker stops, as
// (Hz, Q, dB) for the two lifts and (Hz, Q) for the cut; and its gain, which keeps the voices
// about as loud as without it
const CABINET_BODY: (f32, f32, f32) = (350.0, 0.7, 5.0);
const CABINET_PRESENCE: (f32, f32, f32) = (1600.0, 0.8, 7.0);
const CABINET_TOP: (f32, f32) = (4500.0, 0.6);
const CABINET_GAIN: f32 = 0.7;
// A stiff string: a chain of first-order all-passes in the loop makes the upper partials run
// sharp, as they do on a wound string (partial n at n f0 sqrt(1 + B n^2)). B is
// `STIFFNESS_E` on the open E and falls with the square of the pitch up the neck (a thinner string
// is less stiff). The chain is fitted so that partial `STIFF_FIT_PARTIAL` is where that puts
// it; the loop's length takes out what the chain adds at the fundamental
const STIFF_SECTIONS: usize = 4;
const STIFFNESS_E: f32 = 1.5e-4;
const STIFFNESS_E_HZ: f32 = 41.203;
const STIFF_FIT_PARTIAL: f32 = 10.0;
// Above this pitch a string is taken as limp: the loop is too short for the chain
const STIFF_MAX_HZ: f32 = 800.0;
// How far the all-passes' pole goes towards the unit circle
const STIFF_POLE_MAX: f64 = 0.995;
// The string's other plane: a second loop of the same string, plucked and read with this share
// of the first's level, `PLANE_BEAT_HZ` sharp of it (in Hz, not cents, so that the fundamental
// beats at every pitch: a few tenths of a cent beats at 0.01 Hz on the low E) and ringing
// `PLANE_T60` times as long. Held together they beat, and the decay is two lines, not one.
// The second plane pulls the note's pitch up, as the spectral peak of the sum is read; the first
// is tuned `PLANE_FLAT_HZ` flat to take that back. The pull is a fixed number of Hz at every
// pitch, measured with `PLANE_FLAT_HZ` at 0 as +0.03 to +0.04 Hz (1.5 cents on the low E, 0.4 on
// the G above it). It is not the amplitude-weighted 0.4 * 0.15 / 1.15 = 0.052 Hz, which tunes the
// low E 1.4 cents flat, nor the power-weighted 0.009 Hz; `bass_report` and the tuning test hold it
const PLANE_LEVEL: f32 = 0.15;
const PLANE_BEAT_HZ: f32 = 0.4;
const PLANE_FLAT_HZ: f32 = 0.03;
const PLANE_T60: f32 = 1.2;
// What the hand makes on the string besides the pluck, each a few ms long. A pick scrapes it
// (Metal): noise over `SCRAPE_S` that swells and dies, at this level against the click it
// replaces. A finger (Groove) lands with a thump, one period of a sine of `THUMP_S` that is as
// long, and a little noise (a resonance at `FINGER_NOISE_HZ`, where Tone lets it through) that
// dies in about `FINGER_NOISE_S`; both scale with the velocity
const SCRAPE_S: f32 = 0.0025;
const SCRAPE_LEVEL: f32 = 2.4;
const THUMP_S: f32 = 0.003;
const THUMP_LEVEL: f32 = 0.1;
const FINGER_NOISE_S: f32 = 0.004;
const FINGER_KNOCK_LEVEL: f32 = 9.0;
const FINGER_NOISE_LEVEL: f32 = 1.0;
const FINGER_NOISE_HZ: f32 = 1500.0;
const FINGER_NOISE_POLE: f32 = 0.9;
// Letting go of the string makes a quiet tick of the fretting finger: noise, low-passed at
// `RELEASE_TICK_HZ` and differenced, that dies with this time constant, at this level against a
// note of the same velocity
const RELEASE_TICK_LEVEL: f32 = 0.07;
const RELEASE_TICK_S: f32 = 0.004;
const RELEASE_TICK_HZ: f32 = 3300.0;
// Time constant of the fade that takes the step out of a new pluck over a ringing string
const DECLICK_S: f32 = 0.0015;
// Time constant of the smoothing of the bend, so that a wheel sending coarse steps does not
// zipper
const BEND_SMOOTHING_S: f32 = 0.005;
// How far the bend moves before the loop's gain follows it
const BEND_FOLLOW_ST: f32 = 0.01;

// No two plucks are the same: each one moves where it is plucked by up to this much of the
// string, how round its edges are by up to this share, and the pick's click by up to this share.
// Where it is plucked and how round, each to the other side of the last pluck and by at least
// `PLUCK_JITTER_LEAST` of the most, so that two plucks in a row are never nearly alike
const PLUCK_JITTER_ALONG: f32 = 0.015;
const PLUCK_JITTER_ROUNDING: f32 = 0.15;
const PLUCK_JITTER_CLICK: f32 = 0.25;
const PLUCK_JITTER_LEAST: f32 = 0.4;
// A string pulled hard is stretched: it starts this many cents sharp at full velocity (less by
// the square of a softer one) and falls to pitch with this time constant
const SHARP_START_CENTS: f32 = 6.0;
const SHARP_START_S: f32 = 0.06;
// Of what was ringing on the same note, what a new pluck leaves under it: a pick lands on a
// moving string
const PLUCK_KEEPS: f32 = 0.25;
// Metal's drive gets less hot as the note falls: how long its gain takes to fall by 60 dB,
// against the fundamental's T60. Three quarters of what it loses is made up after it, so the
// drive cleans up without taking the note's level down with it
const DRIVE_FOLLOW: f32 = 0.25;

/// The loop's low-pass and what it does at one pitch
#[derive(Clone, Copy)]
struct Damping {
    /// Coefficient of the one-pole low-pass
    loop_lp: f32,
    /// What it adds to the length of the loop at the note's pitch
    filter_delay: f32,
    /// Its gain at the note's pitch
    gain: f32,
}

impl Damping {
    /// The low-pass `loop_lp` at a pitch
    fn at(loop_lp: f32, freq_hz: f32, sample_rate: f32) -> Self {
        let pole = 1.0 - loop_lp;
        let omega = std::f32::consts::TAU * freq_hz / sample_rate;
        let (sin, cos) = omega.sin_cos();
        let filter_delay = (pole * sin).atan2(1.0 - pole * cos) / omega.max(1e-6);
        let gain = loop_lp / (1.0 - 2.0 * pole * cos + pole * pole).sqrt();
        Self { loop_lp, filter_delay, gain }
    }

    /// The low-pass that lets the fundamental ring for `fundamental_t60_s` and the partial at
    /// `TOP_HZ` for `top_t60_s`, whatever the pitch: a low note goes round the loop fewer times
    /// a second than a high one, so it loses more of its top each time round.
    fn for_note(freq_hz: f32, fundamental_t60_s: f32, top_t60_s: f32, sample_rate: f32) -> Self {
        let top_hz = TOP_HZ.max(2.0 * freq_hz);
        if top_hz > 0.45 * sample_rate || top_t60_s >= fundamental_t60_s {
            return Self::at(1.0, freq_hz, sample_rate);
        }
        // The top's gain each time round, against the fundamental's, squared
        let ratio = 10.0f32.powf(-6.0 / freq_hz * (1.0 / top_t60_s - 1.0 / fundamental_t60_s));
        let cos_fundamental = (std::f32::consts::TAU * freq_hz / sample_rate).cos();
        let cos_top = (std::f32::consts::TAU * top_hz / sample_rate).cos();
        // |H|² = a² / (1 - 2b cos ω + b²) for b = 1 - a: the b where the two gains are `ratio`
        // apart solves b² - 2qb + 1 = 0
        let q = (cos_fundamental - ratio * cos_top) / (1.0 - ratio);
        let pole = if q > 1.0 { q - (q * q - 1.0).sqrt() } else { 1.0 };
        Self::at(1.0 - pole.clamp(0.0, 0.995), freq_hz, sample_rate)
    }
}

/// Phase delay in samples of the first-order all-pass `(a + z^-1) / (1 + a z^-1)` at `omega`
/// radians per sample. With `a` below 0 it is longest at low frequencies: partials above run
/// sharp
fn allpass_delay(a: f64, omega: f64) -> f64 {
    let (sin, cos) = omega.sin_cos();
    let phase = (-sin).atan2(a + cos) - (-a * sin).atan2(1.0 + a * cos);
    -phase / omega.max(1e-9)
}

/// The coefficient of the `STIFF_SECTIONS` all-passes that make a string of pitch `freq_hz` as
/// stiff as `STIFFNESS_E` says, or None where it is limp (too high, or too few partials below
/// Nyquist)
fn stiffness_coeff(freq_hz: f32, sample_rate: f32) -> Option<f32> {
    if freq_hz >= STIFF_MAX_HZ {
        return None;
    }
    let rate = sample_rate as f64;
    let freq = freq_hz as f64;
    let omega = std::f64::consts::TAU * freq / rate;
    let ratio = STIFFNESS_E_HZ / freq_hz;
    let stiffness = (STIFFNESS_E * ratio * ratio) as f64;
    // The partial the chain is fitted at, kept well under Nyquist
    let mut partial = STIFF_FIT_PARTIAL as f64;
    let at = |partial: f64| partial * omega * (1.0 + stiffness * partial * partial).sqrt();
    while at(partial) > 0.8 * std::f64::consts::PI && partial >= 2.0 {
        partial -= 1.0;
    }
    if partial < 2.0 {
        return None;
    }
    // Where the partial is, the loop is shorter than at the fundamental by this much
    let wanted = rate / freq * (1.0 - 1.0 / (1.0 + stiffness * partial * partial).sqrt());
    let short_by = |pole: f64| {
        STIFF_SECTIONS as f64 * (allpass_delay(-pole, omega) - allpass_delay(-pole, at(partial)))
    };
    let (mut low, mut high) = (0.0f64, STIFF_POLE_MAX);
    for _ in 0..40 {
        let middle = 0.5 * (low + high);
        if short_by(middle) < wanted {
            low = middle;
        } else {
            high = middle;
        }
    }
    Some((-0.5 * (low + high)) as f32)
}

/// One string, one note at a time: a delay line that feeds itself through a low-pass
/// (Karplus-Strong) and holds how fast the string moves, read by a pickup near the bridge; a
/// sine at the pitch of the string under it, in phase with its fundamental; the voice's
/// output stage, and a cabinet behind it. Volume is left to the plugin, which smooths it.
pub struct Bass {
    sample_rate: f32,
    settings: BassSettings,
    noise: Noise,
    // What the hand and the finger make, apart from `noise` so that where a pluck lands does not
    // depend on how long a scrape was
    touch_noise: Noise,

    line: Vec<f32>,
    // The same string's other plane, written and read with the line
    line_other: Vec<f32>,
    mask: usize,
    write: usize,
    // Scratch for shaping a pluck, as long as the line
    pluck: Vec<f32>,

    // Length of the loop in samples, and where a slide is taking it
    delay: f32,
    target_delay: f32,
    // The pitch the string is at, or sliding to, before the bend
    freq_hz: f32,
    // What the loop's low-pass adds to its length at that pitch
    filter_delay: f32,
    // And at that pitch bent as far as the loop's gain last followed the bend
    bent_filter_delay: f32,
    loop_lp: f32,
    loop_state: f32,
    // The all-passes that stiffen the string: their coefficient (None where it is limp, or
    // before the first pluck) and their states
    stiff: Option<f32>,
    stiff_state: [f32; STIFF_SECTIONS],
    feedback: f32,
    feedback_held: f32,
    feedback_released: f32,
    // The other plane's loop: the same low-pass and all-passes, its own states and gain
    loop_state_other: f32,
    stiff_state_other: [f32; STIFF_SECTIONS],
    feedback_other: f32,
    feedback_other_held: f32,
    feedback_other_released: f32,
    glide: f32,

    // How sharp the string still is from a hard pluck, in cents, and how fast that falls
    sharp_cents: f32,
    sharp_decay: f32,
    // Where the last pluck was moved to and how its edges were rounded, -1.0 to 1.0 of the
    // most they move
    along_offset: f32,
    rounding_offset: f32,

    // The wheel, -1.0 to 1.0, and the bend it makes in semitones, smoothed, with the ratio
    // of frequencies that is
    wheel: f32,
    bend_st: f32,
    bend_ratio: f32,
    bend_smoothing: f32,
    // The bend the loop's gain was last worked out for, and its ratio
    followed_bend_st: f32,
    followed_ratio: f32,

    note: Option<u8>,
    held: bool,
    // Where the pickup reads the ringing note, as a part of the length that rings
    pickup_at: f32,
    // The voice and velocity the ringing string was plucked with
    string_voice: Voice,
    string_velocity: f32,

    sub_phase: f32,
    sub_env: f32,
    sub_attack: f32,
    // Per sample: the fade in, and the decay while the note is held and once it is let go
    sub_attack_step: f32,
    sub_decay_held: f32,
    sub_decay_released: f32,

    // How hot Metal's drive is, 1.0 at the pluck and falling with the string, held and let go
    drive_env: f32,
    drive_decay_held: f32,
    drive_decay_released: f32,

    // The tick of letting go: how loud it still is, how fast that falls, how loud it started and
    // its smoothed noise
    tick_env: f32,
    tick_decay: f32,
    tick_smooth: f32,
    tick_level: f32,
    tick_state: f32,

    // The step a new pluck makes over a ringing string, fading out
    last_raw: f32,
    declick: f32,
    declick_pending: bool,
    declick_decay: f32,

    pickup: Svf,
    // Metal's output stage
    split_low: Svf,
    drive_lp: Svf,
    mid_peak: Svf,
    // Groove's
    tone: Svf,
    dc: Svf,
    // The cabinet behind both
    cabinet_body: Svf,
    cabinet_presence: Svf,
    cabinet_top: Svf,
}

impl Bass {
    pub fn new() -> Self {
        let mut bass = Self {
            sample_rate: 44100.0,
            settings: BassSettings::default(),
            noise: Noise::new(1),
            touch_noise: Noise::new(2),
            line: Vec::new(),
            line_other: Vec::new(),
            mask: 0,
            write: 0,
            pluck: Vec::new(),
            delay: 800.0,
            target_delay: 800.0,
            freq_hz: 55.0,
            filter_delay: 0.0,
            bent_filter_delay: 0.0,
            loop_lp: 0.4,
            loop_state: 0.0,
            stiff: None,
            stiff_state: [0.0; STIFF_SECTIONS],
            feedback: 0.0,
            feedback_held: 0.0,
            feedback_released: 0.0,
            loop_state_other: 0.0,
            stiff_state_other: [0.0; STIFF_SECTIONS],
            feedback_other: 0.0,
            feedback_other_held: 0.0,
            feedback_other_released: 0.0,
            glide: 1.0,
            sharp_cents: 0.0,
            sharp_decay: 0.0,
            along_offset: 0.0,
            rounding_offset: 0.0,
            wheel: 0.0,
            bend_st: 0.0,
            bend_ratio: 1.0,
            bend_smoothing: 1.0,
            followed_bend_st: 0.0,
            followed_ratio: 1.0,
            note: None,
            held: false,
            pickup_at: PICKUP,
            string_voice: Voice::Metal,
            string_velocity: 1.0,
            sub_phase: 0.0,
            sub_env: 0.0,
            sub_attack: 1.0,
            sub_attack_step: 1.0,
            sub_decay_held: 0.0,
            sub_decay_released: 0.0,
            drive_env: 0.0,
            drive_decay_held: 0.0,
            drive_decay_released: 0.0,
            tick_env: 0.0,
            tick_decay: 0.0,
            tick_smooth: 0.0,
            tick_level: 0.0,
            tick_state: 0.0,
            last_raw: 0.0,
            declick: 0.0,
            declick_pending: false,
            declick_decay: 0.0,
            pickup: Svf::new(),
            split_low: Svf::new(),
            drive_lp: Svf::new(),
            mid_peak: Svf::new(),
            tone: Svf::new(),
            dc: Svf::new(),
            cabinet_body: Svf::new(),
            cabinet_presence: Svf::new(),
            cabinet_top: Svf::new(),
        };
        bass.set_sample_rate(44100.0);
        bass
    }

    /// Allocates the string. Not for the audio thread.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let size = STRING_SAMPLES * ((sample_rate / 48000.0).ceil() as usize).next_power_of_two();
        self.line = vec![0.0; size];
        self.line_other = vec![0.0; size];
        self.pluck = vec![0.0; size];
        self.mask = size - 1;
        self.write = 0;

        // A magnetic pickup's own resonance
        self.pickup.set(2800.0, 1.2, sample_rate);
        // Critically damped: the lows and what is left over sum to the input
        self.split_low.set(CROSSOVER_HZ, 0.5, sample_rate);
        self.mid_peak.set(900.0, 1.0, sample_rate);
        self.cabinet_body.set(CABINET_BODY.0, CABINET_BODY.1, sample_rate);
        self.cabinet_presence.set(CABINET_PRESENCE.0, CABINET_PRESENCE.1, sample_rate);
        self.cabinet_top.set(CABINET_TOP.0, CABINET_TOP.1, sample_rate);
        self.declick_decay = decay_coeff(DECLICK_S, sample_rate);
        self.tick_decay = decay_coeff(RELEASE_TICK_S, sample_rate);
        self.tick_smooth = 1.0 - (-std::f32::consts::TAU * RELEASE_TICK_HZ / sample_rate).exp();
        self.bend_smoothing = 1.0 - decay_coeff(BEND_SMOOTHING_S, sample_rate);
        self.sharp_decay = decay_coeff(SHARP_START_S, sample_rate);
        self.set_voice_filters();
        self.reset();
    }

    pub fn reset(&mut self) {
        self.line.fill(0.0);
        self.line_other.fill(0.0);
        self.loop_state = 0.0;
        self.stiff_state = [0.0; STIFF_SECTIONS];
        self.loop_state_other = 0.0;
        self.stiff_state_other = [0.0; STIFF_SECTIONS];
        self.tick_env = 0.0;
        self.note = None;
        self.held = false;
        self.sub_env = 0.0;
        self.sharp_cents = 0.0;
        self.drive_env = 0.0;
        self.last_raw = 0.0;
        self.declick = 0.0;
        self.declick_pending = false;
        // The wheel back at centre
        self.wheel = 0.0;
        self.bend_st = 0.0;
        self.bend_ratio = 1.0;
        self.followed_bend_st = 0.0;
        self.followed_ratio = 1.0;
        for filter in [
            &mut self.pickup,
            &mut self.split_low,
            &mut self.drive_lp,
            &mut self.mid_peak,
            &mut self.tone,
            &mut self.dc,
            &mut self.cabinet_body,
            &mut self.cabinet_presence,
            &mut self.cabinet_top,
        ] {
            filter.reset();
        }
    }

    /// Once per block. A ringing note keeps the string it was plucked with: another voice
    /// changes what stands behind the string at once, and the string at the next pluck.
    pub fn set(&mut self, settings: &BassSettings) {
        if *settings != self.settings {
            self.change(settings);
        }
        self.follow_bend();
    }

    fn change(&mut self, settings: &BassSettings) {
        let voice_changed = settings.voice != self.settings.voice;
        let brightness_changed = settings.brightness != self.settings.brightness;
        let decay_changed = voice_changed || settings.sustain_s != self.settings.sustain_s;
        self.settings = *settings;
        self.set_voice_filters();
        if voice_changed {
            // The other voice's filters start from rest
            for filter in [&mut self.drive_lp, &mut self.tone, &mut self.dc] {
                filter.reset();
            }
        }
        if self.note.is_some() && (decay_changed || brightness_changed) {
            // The note that is ringing follows Sustain, and Brightness while it is the
            // voice it was plucked with; held or let go
            if brightness_changed && self.settings.voice == self.string_voice {
                let before = self.filter_delay;
                self.damp_string();
                // The loop keeps its length, and the note its pitch
                self.delay += before - self.filter_delay;
                self.target_delay += before - self.filter_delay;
            } else {
                self.set_feedback();
            }
        }
    }

    /// Where the pitch-bend wheel is: -1.0 all the way down, 0.0 at centre, 1.0 all the way up
    pub fn set_wheel(&mut self, wheel: f32) {
        self.wheel = wheel.clamp(-1.0, 1.0);
    }

    /// The loop's gain, and what its low-pass adds to the loop's length, follow the bend once
    /// it has moved far enough to matter, so that Sustain is the same T60 on a bent note. The
    /// low-pass stays the one the note was plucked with. Once per block: on every sample it
    /// would make the level jump
    fn follow_bend(&mut self) {
        // Or once it has got where the wheel is, wherever that is
        let settled = self.bend_st == self.wheel * self.settings.bend;
        let moved = self.bend_st != self.followed_bend_st
            && (settled || (self.bend_st - self.followed_bend_st).abs() >= BEND_FOLLOW_ST);
        if moved && self.note.is_some() {
            self.followed_bend_st = self.bend_st;
            self.followed_ratio = self.bend_ratio;
            self.set_feedback();
        }
    }

    /// One sample of the bend's smoothing, towards where the wheel is
    fn smooth_bend(&mut self) {
        let target = self.wheel * self.settings.bend;
        if self.bend_st == target {
            return;
        }
        self.bend_st += (target - self.bend_st) * self.bend_smoothing;
        // There, rather than ever nearer: at centre the string is exactly as unbent
        if (self.bend_st - target).abs() < 1e-5 {
            self.bend_st = target;
        }
        self.bend_ratio = if self.bend_st == 0.0 { 1.0 } else { (self.bend_st / 12.0).exp2() };
    }

    fn set_voice_filters(&mut self) {
        let rate = self.sample_rate;
        let tone_hz = self.settings.tone_hz;
        let (sub_attack_s, release_s) = match self.settings.voice {
            Voice::Metal => (0.004, RELEASE_METAL_S),
            Voice::Groove => (0.006, RELEASE_GROOVE_S),
        };
        // T60s as time constants: 60 dB is ln(1000) of them
        let t60 = |seconds: f32| decay_coeff(seconds / 1000.0f32.ln(), rate);
        self.sub_attack_step = 1.0 / (sub_attack_s * rate);
        self.sub_decay_held = t60(self.settings.sustain_s * SUB_DECAY);
        self.sub_decay_released = t60(release_s * SUB_DECAY);
        self.drive_decay_held = t60(self.settings.sustain_s * DRIVE_FOLLOW);
        self.drive_decay_released = t60(release_s * DRIVE_FOLLOW);
        match self.settings.voice {
            Voice::Metal => {
                self.drive_lp.set(tone_hz, 0.8, rate);
                self.dc.set(25.0, 0.7, rate);
            }
            Voice::Groove => {
                self.tone.set(tone_hz, 0.75, rate);
                self.dc.set(18.0, 0.7, rate);
            }
        }
        // A third of the slide time as the time constant: there after the whole of it
        self.glide = if self.settings.slide_ms > 0.0 {
            1.0 - (-1.0 / (self.settings.slide_ms * 0.001 / 3.0 * rate)).exp()
        } else {
            1.0
        };
    }

    /// Velocity is 0.0-1.0
    pub fn note_on(&mut self, note: u8, velocity: f32) {
        if velocity <= 0.0 {
            return self.note_off(note);
        }
        let sliding =
            self.settings.slide_ms > 0.0 && self.held && self.note.is_some_and(|held| held != note);
        if sliding {
            self.slide_to(note);
        } else {
            self.pluck(note, velocity.min(1.0));
        }
    }

    pub fn note_off(&mut self, note: u8) {
        if self.note == Some(note) {
            self.release();
        }
    }

    /// Lets go of whatever note is held
    pub fn release(&mut self) {
        // The finger leaves the string: a tick, as loud as the note was plucked
        if self.held && self.note.is_some() {
            self.tick_env = 1.0;
            self.tick_level = RELEASE_TICK_LEVEL * (0.5 + 0.5 * self.string_velocity);
        }
        self.held = false;
        self.feedback = self.feedback_released;
        self.feedback_other = self.feedback_other_released;
    }

    fn freq_hz(&self, note: u8) -> f32 {
        note_hz(note as f32 + self.settings.transpose)
    }

    /// Length of the loop for a note, less what its low-pass adds
    fn loop_delay(&self, freq_hz: f32) -> f32 {
        let longest = (self.line.len() - 4) as f32;
        (self.sample_rate / (freq_hz - PLANE_FLAT_HZ) - self.filter_delay).clamp(2.0, longest)
    }

    /// The loop's low-pass for the ringing note: how long its top rings follows Brightness,
    /// and a softer touch is darker
    fn damp_string(&mut self) {
        let top_t60_s = (TOP_T60_MIN_S
            + (TOP_T60_MAX_S - TOP_T60_MIN_S) * self.settings.brightness)
            * (0.6 + 0.4 * self.string_velocity);
        let damping = Damping::for_note(
            self.freq_hz * self.followed_ratio,
            self.settings.sustain_s,
            top_t60_s,
            self.sample_rate,
        );
        self.loop_lp = damping.loop_lp;
        self.set_feedback();
    }

    /// What the all-passes add to the loop's length at a pitch
    fn stiff_delay(&self, freq_hz: f32) -> f32 {
        match self.stiff {
            Some(pole) => {
                let omega = std::f64::consts::TAU * (freq_hz / self.sample_rate) as f64;
                (STIFF_SECTIONS as f64 * allpass_delay(pole as f64, omega)) as f32
            }
            None => 0.0,
        }
    }

    /// The loop's gain, which sets how long the fundamental rings, held and let go, at the
    /// pitch the string is bent to
    fn set_feedback(&mut self) {
        self.filter_delay = Damping::at(self.loop_lp, self.freq_hz, self.sample_rate).filter_delay
            + self.stiff_delay(self.freq_hz);
        let freq_hz = self.freq_hz * self.followed_ratio;
        let damping = Damping::at(self.loop_lp, freq_hz, self.sample_rate);
        self.bent_filter_delay = damping.filter_delay + self.stiff_delay(freq_hz);
        let release_s = match self.settings.voice {
            Voice::Metal => RELEASE_METAL_S,
            Voice::Groove => RELEASE_GROOVE_S,
        };
        let gain =
            |t60_s: f32| (10.0f32.powf(-3.0 / (t60_s * freq_hz)) / damping.gain).min(0.99999);
        self.feedback_held = gain(self.settings.sustain_s);
        self.feedback_released = gain(release_s);
        self.feedback_other_held = gain(self.settings.sustain_s * PLANE_T60);
        self.feedback_other_released = gain(release_s * PLANE_T60);
        (self.feedback, self.feedback_other) = if self.held {
            (self.feedback_held, self.feedback_other_held)
        } else {
            (self.feedback_released, self.feedback_other_released)
        };
    }

    fn slide_to(&mut self, note: u8) {
        self.freq_hz = self.freq_hz(note);
        self.note = Some(note);
        self.held = true;
        // A slid-to note is as stiff as that note plucked (the ringing state carries on)
        self.stiff = stiffness_coeff(self.freq_hz, self.sample_rate);
        self.set_feedback();
        self.target_delay = self.loop_delay(self.freq_hz);
    }

    fn pluck(&mut self, note: u8, velocity: f32) {
        let rate = self.sample_rate;
        let brightness = self.settings.brightness;
        let freq_hz = self.freq_hz(note);
        // A pick lands on a string that may still move: what rings on the same note is damped,
        // anything else is stopped
        let keeps = if self.note.is_some() && self.freq_hz == freq_hz { PLUCK_KEEPS } else { 0.0 };
        self.freq_hz = freq_hz;
        self.note = Some(note);
        self.held = true;
        self.string_voice = self.settings.voice;
        self.string_velocity = velocity;
        // A stiff string: the all-passes start from rest with the pluck
        self.stiff = stiffness_coeff(freq_hz, rate);
        self.stiff_state = [0.0; STIFF_SECTIONS];
        self.stiff_state_other = [0.0; STIFF_SECTIONS];
        // Where the note is played: the pick, the finger and the pickup are further along the
        // shorter string of a fret
        let fretted = Self::fretted(note as f32 + self.settings.transpose);
        self.pickup_at = (PICKUP * fretted).min(ALONG_MOST);
        // The string starts at the bent pitch
        self.followed_bend_st = self.bend_st;
        self.followed_ratio = self.bend_ratio;
        self.damp_string();
        self.delay = self.loop_delay(self.freq_hz);
        self.target_delay = self.delay;
        // Pulled hard, the string is stretched and starts sharp
        self.sharp_cents = SHARP_START_CENTS * velocity * velocity;

        // What the line holds is how fast the string moves. A string pulled to a point and
        // let go moves one way on one side of that point and the other way on the other:
        // two levels, whose edges are rounded by what plucks it.
        let period = self.read_delay();
        let length = (period.ceil() as usize).clamp(2, self.line.len() - 8);
        let (mut along, level, sharpness, mut rounding_s) = match self.settings.voice {
            Voice::Metal => {
                // A pick: harder and brighter is a sharper edge
                let sharpness = 0.6 * brightness + 0.4 * velocity;
                (PICK * fretted, velocity.powf(1.2), sharpness, 0.00012 * (1.0 - 0.85 * sharpness))
            }
            Voice::Groove => {
                // A finger, softer
                let sharpness = velocity * (0.4 + 0.6 * brightness);
                (FINGER * fretted, velocity, sharpness, 0.00015 * (1.0 - 0.5 * sharpness))
            }
        };
        // No hand plucks twice in the same place or the same way
        self.along_offset = self.away(self.along_offset);
        self.rounding_offset = self.away(self.rounding_offset);
        along = (along + PLUCK_JITTER_ALONG * self.along_offset).min(ALONG_MOST);
        rounding_s *= 1.0 + PLUCK_JITTER_ROUNDING * self.rounding_offset;
        let high = 1.0 / along;
        let low = -1.0 / (1.0 - along);
        for (index, sample) in self.pluck[..length].iter_mut().enumerate() {
            *sample = if (index as f32) < along * period { high } else { low };
        }
        // Rounded twice round the string, so that its ends meet
        let smoothing = 1.0 - decay_coeff(rounding_s, rate);
        let mut smooth = 0.0;
        for _ in 0..2 {
            for sample in &mut self.pluck[..length] {
                smooth += (*sample - smooth) * smoothing;
                *sample = smooth;
            }
        }
        // What the hand makes besides the pluck
        match self.settings.voice {
            Voice::Metal => {
                // The pick scrapes the string as it leaves it: noise that swells and dies
                let scrape = ((SCRAPE_S * rate) as usize).clamp(2, length);
                let level = 2.0
                    * SCRAPE_LEVEL
                    * sharpness
                    * (1.0 + PLUCK_JITTER_CLICK * self.touch_noise.next());
                let mut previous = 0.0;
                for (index, sample) in self.pluck[..scrape].iter_mut().enumerate() {
                    let at = index as f32 / scrape as f32;
                    let swell = if at < 0.2 { at / 0.2 } else { (1.0 - at) / 0.8 };
                    let noise = self.touch_noise.next();
                    *sample += (noise - previous) * level * swell;
                    previous = noise;
                }
            }
            Voice::Groove => {
                // The finger lands: a thump, and a little noise that dies at once
                let thump = ((THUMP_S * rate) as usize).clamp(2, length);
                let level = THUMP_LEVEL * velocity;
                for (index, sample) in self.pluck[..thump].iter_mut().enumerate() {
                    *sample += level * sine(index as f32 / thump as f32);
                }
                let noise_length = ((4.0 * FINGER_NOISE_S * rate) as usize).clamp(2, length);
                let falls = decay_coeff(FINGER_NOISE_S / 4.0, rate);
                let level = velocity * (1.0 + PLUCK_JITTER_CLICK * self.touch_noise.next());
                // A knock and some noise through a resonance at `FINGER_NOISE_HZ`: a thud more
                // than a hiss
                let theta = std::f32::consts::TAU * FINGER_NOISE_HZ / rate;
                let (feed1, feed2) =
                    (2.0 * FINGER_NOISE_POLE * theta.cos(), -FINGER_NOISE_POLE * FINGER_NOISE_POLE);
                let (mut envelope, mut y1, mut y2) = (1.0f32, 0.0f32, 0.0f32);
                let (mut x1, mut x2) = (0.0f32, 0.0f32);
                for (index, sample) in self.pluck[..noise_length].iter_mut().enumerate() {
                    // Two zeros, at 0 and at Nyquist: none of it is below the string's own range
                    let knock = if index == 0 { FINGER_KNOCK_LEVEL } else { 0.0 };
                    let x = knock + FINGER_NOISE_LEVEL * self.touch_noise.next();
                    let y = x - x2 + feed1 * y1 + feed2 * y2;
                    (x2, x1, y2, y1) = (x1, x, y1, y);
                    *sample += y * level * envelope;
                    envelope *= falls;
                }
            }
        }

        // Into the line, in the order it is read, over what is left of the ringing string
        let scale = level * STRING_GAIN / std::f32::consts::TAU;
        let start = self.write + self.line.len() - length;
        for index in 0..length {
            let position = (start + index) & self.mask;
            self.line[position] = self.pluck[index] * scale + self.line[position] * keeps;
        }
        // The other plane is plucked too, less, and a quarter of a period later: the two start
        // a quarter of a beat apart, so that the level of a note ripples about where it would
        // be rather than falling first
        let quarter = length / 4;
        for index in 0..length {
            let position = (start + (index + quarter) % length) & self.mask;
            self.line_other[position] =
                self.pluck[index] * scale * PLANE_LEVEL + self.line_other[position] * keeps;
        }

        // The fundamental of what the pickup will read over the first time round, which the
        // sine under the string starts in phase with
        let tap = (self.pickup_at * self.ring()).round() as usize;
        let step = std::f32::consts::TAU / period;
        let (step_sin, step_cos) = step.sin_cos();
        let (mut sin, mut cos) = (0.0f32, 1.0f32);
        let (mut real, mut imag) = (0.0, 0.0);
        let at = |index: usize| self.line[(start + index) & self.mask];
        for index in 0..length {
            let read = at((index + tap) % length) - at(index);
            real += read * cos;
            imag -= read * sin;
            (sin, cos) = (sin * step_cos + cos * step_sin, cos * step_cos - sin * step_sin);
        }
        let pickup_gain = self.pickup_gain();
        let amplitude = 2.0 * (real * real + imag * imag).sqrt() / length as f32 * pickup_gain;
        let phase = imag.atan2(real);
        self.sub_phase =
            (phase + std::f32::consts::FRAC_PI_2) / std::f32::consts::TAU - 1.0 / period;
        self.sub_phase -= self.sub_phase.floor();
        self.sub_env = amplitude;
        self.sub_attack = 0.0;

        self.drive_env = 1.0;
        self.loop_state = 0.0;
        self.loop_state_other = 0.0;
        self.declick_pending = true;
    }

    /// A random offset, -1.0 to 1.0, on the other side of the last one and at least
    /// `PLUCK_JITTER_LEAST` from the middle: two plucks in a row are never alike
    fn away(&mut self, last: f32) -> f32 {
        let size = PLUCK_JITTER_LEAST + (1.0 - PLUCK_JITTER_LEAST) * self.noise.next_unipolar();
        if last > 0.0 {
            -size
        } else {
            size
        }
    }

    /// How much longer than the open string's the part of a pick, a finger or the pickup is on
    /// the note `sounding` (a MIDI note, maybe between two): `2^(fret/12)` on the highest
    /// open string at or below it. Below the low E it is played open
    fn fretted(sounding: f32) -> f32 {
        let open = OPEN_STRINGS
            .iter()
            .rev()
            .find(|&&open| open <= sounding)
            .copied()
            .unwrap_or(OPEN_STRINGS[0]);
        ((sounding - open).max(0.0) / 12.0).exp2()
    }

    /// How long the string takes to go round once at the pitch it has: the line, and the
    /// low-pass and the all-passes that stand in for part of it. The pickup is a part of this
    /// away from the bridge, as it is a part of the string
    fn ring(&self) -> f32 {
        self.read_delay() + self.bent_filter_delay
    }

    /// What makes the fundamental come out of the pickup as loud as it is on the string
    fn pickup_gain(&self) -> f32 {
        1.0 / (2.0 * (std::f32::consts::PI * self.pickup_at).sin())
    }

    /// Where the loop is read: its length, as a slide has it, at the pitch the wheel bends it
    /// to, less what its low-pass adds there
    fn read_delay(&self) -> f32 {
        let ratio = self.pitch_ratio();
        if ratio == 1.0 {
            return self.delay;
        }
        let longest = (self.line.len() - 4) as f32;
        ((self.delay + self.filter_delay) / ratio - self.bent_filter_delay).clamp(2.0, longest)
    }

    /// How far the string is above the pitch it is at, or sliding to: the bend, and what is
    /// left of a hard pluck's sharp start (a few cents, where 2^(c/1200) is 1 + c ln 2 / 1200
    /// within a hundredth of a cent)
    fn pitch_ratio(&self) -> f32 {
        if self.sharp_cents == 0.0 {
            return self.bend_ratio;
        }
        self.bend_ratio * (1.0 + self.sharp_cents * (std::f32::consts::LN_2 / 1200.0))
    }

    /// What a line held `delay` samples ago, between two samples
    fn read_from(&self, line: &[f32], delay: f32) -> f32 {
        let whole = delay.ceil();
        let fraction = whole - delay;
        let position = self.write + line.len() - whole as usize;
        let first = line[position & self.mask];
        let second = line[(position + 1) & self.mask];
        first * (1.0 - fraction) + second * fraction
    }

    /// What the other plane's loop is, in samples, where the first's is `delay`: a little
    /// shorter, so that it is sharp of it by `PLANE_BEAT_HZ`
    fn other_delay(&self, delay: f32) -> f32 {
        let round = delay + self.bent_filter_delay;
        (round / (1.0 + PLANE_BEAT_HZ * round / self.sample_rate) - self.bent_filter_delay).max(2.0)
    }

    /// One sample of a loop's low-pass and all-passes, which is what the loop writes back
    /// (before its gain)
    fn disperse(
        loop_lp: f32,
        stiff: Option<f32>,
        state: &mut f32,
        stiff_state: &mut [f32; STIFF_SECTIONS],
        input: f32,
    ) -> f32 {
        *state += (input - *state) * loop_lp;
        let mut dispersed = *state;
        if let Some(pole) = stiff {
            for section in stiff_state.iter_mut() {
                let out = pole * dispersed + *section;
                *section = dispersed - pole * out;
                dispersed = out;
            }
        }
        dispersed
    }

    /// The string as the pickup reads it, and the sine under it at the level Sub gives it
    fn string_and_sub(&mut self) -> (f32, f32) {
        let rate = self.sample_rate;

        // Only moves during a slide, and while the wheel moves
        self.delay += (self.target_delay - self.delay) * self.glide;
        self.smooth_bend();
        if self.sharp_cents != 0.0 {
            self.sharp_cents *= self.sharp_decay;
            // At pitch, rather than ever nearer
            if self.sharp_cents < 1e-3 {
                self.sharp_cents = 0.0;
            }
        }
        let freq_hz = rate / (self.delay + self.filter_delay) * self.pitch_ratio();
        let delay = self.read_delay();

        // The string, and the pickup: where the string is read now, against what reaches it
        // the pickup's part of the way round later. The same for the string's other plane
        let ahead_by =
            |delay: f32| (delay - self.pickup_at * (delay + self.bent_filter_delay)).max(1.0);
        let string = self.read_from(&self.line, delay);
        let ahead = self.read_from(&self.line, ahead_by(delay));
        let delay_other = self.other_delay(delay);
        let string_other = self.read_from(&self.line_other, delay_other);
        let ahead_other = self.read_from(&self.line_other, ahead_by(delay_other));
        let (loop_lp, stiff) = (self.loop_lp, self.stiff);
        let dispersed =
            Self::disperse(loop_lp, stiff, &mut self.loop_state, &mut self.stiff_state, string);
        let dispersed_other = Self::disperse(
            loop_lp,
            stiff,
            &mut self.loop_state_other,
            &mut self.stiff_state_other,
            string_other,
        );
        self.line[self.write] = dispersed * self.feedback;
        self.line_other[self.write] = dispersed_other * self.feedback_other;
        self.write = (self.write + 1) & self.mask;
        let picked = self
            .pickup
            .process(((ahead - string) + (ahead_other - string_other)) * self.pickup_gain())
            .low;

        // The sine under it
        advance(&mut self.sub_phase, freq_hz / rate);
        if self.sub_attack < 1.0 {
            self.sub_attack = (self.sub_attack + self.sub_attack_step).min(1.0);
        }
        let sub = sine(self.sub_phase) * self.sub_env * self.sub_attack;
        self.sub_env *= if self.held { self.sub_decay_held } else { self.sub_decay_released };
        (picked, sub * self.settings.sub * SUB_GAIN)
    }

    /// One sample, the same for both channels, before Volume: the voice's output stage, and
    /// the cabinet behind it
    pub fn process(&mut self) -> f32 {
        // The tick of letting go joins what the voice's stage makes, before the cabinet
        let voiced = self.voice_stage() + self.tick();
        self.cabinet(voiced)
    }

    /// The cabinet: two lifts, each the input with a share of its band added, which is the
    /// lift's gain at its centre, then the cut
    fn cabinet(&mut self, input: f32) -> f32 {
        let lift = |filter: &mut Svf, (_, q, db): (f32, f32, f32), input: f32| {
            input + (10.0f32.powf(db / 20.0) - 1.0) / q * filter.process(input).band
        };
        let body = lift(&mut self.cabinet_body, CABINET_BODY, input);
        let presence = lift(&mut self.cabinet_presence, CABINET_PRESENCE, body);
        self.cabinet_top.process(presence).low * CABINET_GAIN
    }

    /// The tick of letting go of the string: smoothed noise, differenced, that dies at once
    fn tick(&mut self) -> f32 {
        if self.tick_env < 1e-4 {
            self.tick_env = 0.0;
            return 0.0;
        }
        let before = self.tick_state;
        self.tick_state += (self.touch_noise.next() - self.tick_state) * self.tick_smooth;
        let tick = (self.tick_state - before) * self.tick_level * self.tick_env;
        self.tick_env *= self.tick_decay;
        tick
    }

    /// One sample of the voice's output stage
    fn voice_stage(&mut self) -> f32 {
        let (picked, sub) = self.string_and_sub();
        let mut raw = picked + sub;
        if self.declick_pending {
            self.declick = self.last_raw - raw;
            self.declick_pending = false;
        }
        raw += self.declick;
        self.declick *= self.declick_decay;
        self.last_raw = raw;

        let settings = &self.settings;
        match settings.voice {
            Voice::Metal => {
                // The lows stay clean; what is over them is distorted and laid on top
                let low = self.split_low.process(raw).low;
                let high = raw - low;
                // Hot on the attack, cleaner as the string falls, so the top dies with it; with
                // three quarters of the gain it loses made up after it
                let most = 1.0 + settings.drive * 14.0;
                let gain = 1.0 + settings.drive * 14.0 * self.drive_env;
                self.drive_env *=
                    if self.held { self.drive_decay_held } else { self.drive_decay_released };
                let lost = most / gain;
                let driven = (high * gain).tanh() * (lost * lost.sqrt()).sqrt();
                let driven = self.drive_lp.process(driven).low;
                let driven = driven + self.mid_peak.process(driven).band * 0.6;
                let mixed = low * 1.1 + driven * (0.25 + settings.drive * 0.9) * 0.5;
                (self.dc.process(mixed).high * 1.2).tanh() * 0.8
            }
            Voice::Groove => {
                let gain = 1.0 + settings.drive * 3.0;
                let warm = self.tone.process(raw).low;
                let driven = (warm * gain).tanh() / gain * (1.0 + settings.drive);
                self.dc.process(driven).high * 0.9
            }
        }
    }
}

#[cfg(test)]
mod tests;
