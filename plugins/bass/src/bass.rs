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
                brightness: 0.55,
                sub: 0.25,
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
                brightness: 0.35,
                sub: 0.3,
                tone_hz: 1100.0,
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

// Where the pick, the finger and the pickup are, as parts of the string from the bridge
const PICK: f32 = 0.18;
const FINGER: f32 = 0.12;
const PICKUP: f32 = 0.2;

// How loud the string is, before the output stage
const STRING_GAIN: f32 = 0.7;
// The sine under the string at Sub 100 %, against the string's own fundamental
const SUB_GAIN: f32 = 4.0;
// The sine's T60 against the fundamental's: it never outlasts the string
const SUB_DECAY: f32 = 0.85;
// Where Metal splits the clean lows from the driven mids
const CROSSOVER_HZ: f32 = 120.0;
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

/// One string, one note at a time: a delay line that feeds itself through a low-pass
/// (Karplus-Strong) and holds how fast the string moves, read by a pickup near the bridge; a
/// sine at the pitch of the string under it, in phase with its fundamental; and the voice's
/// output stage. Volume is left to the plugin, which smooths it.
pub struct Bass {
    sample_rate: f32,
    settings: BassSettings,
    noise: Noise,

    line: Vec<f32>,
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
    feedback: f32,
    feedback_held: f32,
    feedback_released: f32,
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
}

impl Bass {
    pub fn new() -> Self {
        let mut bass = Self {
            sample_rate: 44100.0,
            settings: BassSettings::default(),
            noise: Noise::new(1),
            line: Vec::new(),
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
            feedback: 0.0,
            feedback_held: 0.0,
            feedback_released: 0.0,
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
        };
        bass.set_sample_rate(44100.0);
        bass
    }

    /// Allocates the string. Not for the audio thread.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let size = STRING_SAMPLES * ((sample_rate / 48000.0).ceil() as usize).next_power_of_two();
        self.line = vec![0.0; size];
        self.pluck = vec![0.0; size];
        self.mask = size - 1;
        self.write = 0;

        // A magnetic pickup's own resonance
        self.pickup.set(2800.0, 1.2, sample_rate);
        // Critically damped: the lows and what is left over sum to the input
        self.split_low.set(CROSSOVER_HZ, 0.5, sample_rate);
        self.mid_peak.set(900.0, 1.0, sample_rate);
        self.declick_decay = decay_coeff(DECLICK_S, sample_rate);
        self.bend_smoothing = 1.0 - decay_coeff(BEND_SMOOTHING_S, sample_rate);
        self.sharp_decay = decay_coeff(SHARP_START_S, sample_rate);
        self.set_voice_filters();
        self.reset();
    }

    pub fn reset(&mut self) {
        self.line.fill(0.0);
        self.loop_state = 0.0;
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
        self.held = false;
        self.feedback = self.feedback_released;
    }

    fn freq_hz(&self, note: u8) -> f32 {
        note_hz(note as f32 + self.settings.transpose)
    }

    /// Length of the loop for a note, less what its low-pass adds
    fn loop_delay(&self, freq_hz: f32) -> f32 {
        let longest = (self.line.len() - 4) as f32;
        (self.sample_rate / freq_hz - self.filter_delay).clamp(2.0, longest)
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

    /// The loop's gain, which sets how long the fundamental rings, held and let go, at the
    /// pitch the string is bent to
    fn set_feedback(&mut self) {
        self.filter_delay = Damping::at(self.loop_lp, self.freq_hz, self.sample_rate).filter_delay;
        let freq_hz = self.freq_hz * self.followed_ratio;
        let damping = Damping::at(self.loop_lp, freq_hz, self.sample_rate);
        self.bent_filter_delay = damping.filter_delay;
        let release_s = match self.settings.voice {
            Voice::Metal => RELEASE_METAL_S,
            Voice::Groove => RELEASE_GROOVE_S,
        };
        let gain =
            |t60_s: f32| (10.0f32.powf(-3.0 / (t60_s * freq_hz)) / damping.gain).min(0.99999);
        self.feedback_held = gain(self.settings.sustain_s);
        self.feedback_released = gain(release_s);
        self.feedback = if self.held { self.feedback_held } else { self.feedback_released };
    }

    fn slide_to(&mut self, note: u8) {
        self.freq_hz = self.freq_hz(note);
        self.note = Some(note);
        self.held = true;
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
                (PICK, velocity.powf(1.2), sharpness, 0.00012 * (1.0 - 0.85 * sharpness))
            }
            Voice::Groove => {
                // A finger, softer
                let sharpness = velocity * (0.4 + 0.6 * brightness);
                (FINGER, velocity, sharpness, 0.00015 * (1.0 - 0.5 * sharpness))
            }
        };
        // No hand plucks twice in the same place or the same way
        self.along_offset = self.away(self.along_offset);
        self.rounding_offset = self.away(self.rounding_offset);
        along += PLUCK_JITTER_ALONG * self.along_offset;
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
        // The pick's click, where it leaves the string
        if self.settings.voice == Voice::Metal {
            let click = ((0.0004 * rate) as usize).min(length);
            let level = 2.0 * sharpness * (1.0 + PLUCK_JITTER_CLICK * self.noise.next());
            let mut previous = 0.0;
            for (index, sample) in self.pluck[..click].iter_mut().enumerate() {
                let noise = self.noise.next();
                *sample += (noise - previous) * level * (1.0 - index as f32 / click as f32);
                previous = noise;
            }
        }

        // Into the line, in the order it is read, over what is left of the ringing string
        let scale = level * STRING_GAIN / std::f32::consts::TAU;
        let start = self.write + self.line.len() - length;
        for index in 0..length {
            let position = (start + index) & self.mask;
            self.line[position] = self.pluck[index] * scale + self.line[position] * keeps;
        }

        // The fundamental of what the pickup will read over the first time round, which the
        // sine under the string starts in phase with
        let tap = (PICKUP * period).round() as usize;
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
        let pickup_gain = Self::pickup_gain();
        let amplitude = 2.0 * (real * real + imag * imag).sqrt() / length as f32 * pickup_gain;
        let phase = imag.atan2(real);
        self.sub_phase =
            (phase + std::f32::consts::FRAC_PI_2) / std::f32::consts::TAU - 1.0 / period;
        self.sub_phase -= self.sub_phase.floor();
        self.sub_env = amplitude;
        self.sub_attack = 0.0;

        self.drive_env = 1.0;
        self.loop_state = 0.0;
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

    /// What makes the fundamental come out of the pickup as loud as it is on the string
    fn pickup_gain() -> f32 {
        1.0 / (2.0 * (std::f32::consts::PI * PICKUP).sin())
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

    /// What the line held `delay` samples ago, between two samples
    fn read(&self, delay: f32) -> f32 {
        let whole = delay.ceil();
        let fraction = whole - delay;
        let position = self.write + self.line.len() - whole as usize;
        let first = self.line[position & self.mask];
        let second = self.line[(position + 1) & self.mask];
        first * (1.0 - fraction) + second * fraction
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
        // a fifth of the way round later
        let string = self.read(delay);
        let ahead = self.read((delay * (1.0 - PICKUP)).max(1.0));
        self.loop_state += (string - self.loop_state) * self.loop_lp;
        self.line[self.write] = self.loop_state * self.feedback;
        self.write = (self.write + 1) & self.mask;
        let picked = self.pickup.process((ahead - string) * Self::pickup_gain()).low;

        // The sine under it
        advance(&mut self.sub_phase, freq_hz / rate);
        if self.sub_attack < 1.0 {
            self.sub_attack = (self.sub_attack + self.sub_attack_step).min(1.0);
        }
        let sub = sine(self.sub_phase) * self.sub_env * self.sub_attack;
        self.sub_env *= if self.held { self.sub_decay_held } else { self.sub_decay_released };
        (picked, sub * self.settings.sub * SUB_GAIN)
    }

    /// One sample, the same for both channels, before Volume
    pub fn process(&mut self) -> f32 {
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
mod tests {
    use super::*;
    use suite_common::measure::{brightness, peak, pitch_hz, rms, tone_level};

    const SAMPLE_RATE: f32 = 48000.0;

    fn bass(voice: Voice, sample_rate: f32) -> Bass {
        let mut bass = Bass::new();
        bass.set_sample_rate(sample_rate);
        bass.set(&voice.settings());
        bass
    }

    fn render(bass: &mut Bass, sample_rate: f32, seconds: f32) -> Vec<f32> {
        (0..(seconds * sample_rate) as usize).map(|_| bass.process()).collect()
    }

    fn note(voice: Voice, note: u8, velocity: f32, seconds: f32) -> Vec<f32> {
        let mut bass = bass(voice, SAMPLE_RATE);
        bass.note_on(note, velocity);
        render(&mut bass, SAMPLE_RATE, seconds)
    }

    /// As the plugin plays: the settings given once a block of 64 samples, which is when the
    /// string follows the bend
    fn play(bass: &mut Bass, settings: &BassSettings, sample_rate: f32, seconds: f32) -> Vec<f32> {
        let mut output = Vec::new();
        while output.len() < (seconds * sample_rate) as usize {
            bass.set(settings);
            output.extend((0..64).map(|_| bass.process()));
        }
        output
    }

    fn pitch(samples: &[f32], sample_rate: f32) -> f32 {
        pitch_hz(samples, sample_rate, 25.0, 500.0)
    }

    /// One note from the string alone: the voice's settings with Sub at 0
    fn string_alone(voice: Voice, midi: u8, velocity: f32, seconds: f32) -> Vec<f32> {
        let mut bass = bass(voice, SAMPLE_RATE);
        bass.set(&BassSettings { sub: 0.0, ..voice.settings() });
        bass.note_on(midi, velocity);
        render(&mut bass, SAMPLE_RATE, seconds)
    }

    // What a partial is measured over: 170 ms, which tells apart the partials of a low E
    const WINDOW: usize = 8192;

    /// The partial of a note nearest 1.5 kHz, where a string's top is heard
    fn top_partial_hz(freq_hz: f32) -> f32 {
        (1500.0 / freq_hz).round().max(2.0) * freq_hz
    }

    /// Where the partial near `freq_hz` really is: a string's partials sit a little off the
    /// harmonics. Looked for within a quarter of the spacing of the note's harmonics.
    fn find_partial(samples: &[f32], start_s: f32, freq_hz: f32, spacing_hz: f32) -> f32 {
        let start = (start_s * SAMPLE_RATE) as usize;
        let window = &samples[start..start + WINDOW];
        (-8..=8)
            .map(|step| freq_hz + step as f32 * spacing_hz / 32.0)
            .map(|hz| (hz, tone_level(window, SAMPLE_RATE, hz)))
            .fold((freq_hz, 0.0), |best, found| if found.1 > best.1 { found } else { best })
            .0
    }

    /// Level in dB of the partial near `freq_hz` in the window that starts at `start_s`
    fn partial_db(samples: &[f32], start_s: f32, freq_hz: f32, spacing_hz: f32) -> f32 {
        let freq_hz = find_partial(samples, start_s, freq_hz, spacing_hz);
        let start = (start_s * SAMPLE_RATE) as usize;
        20.0 * tone_level(&samples[start..start + WINDOW], SAMPLE_RATE, freq_hz).max(1e-9).log10()
    }

    /// Time for the partial near `freq_hz` to fall by 60 dB: the slope of its level every
    /// 20 ms from 50 ms on, while it is within 30 dB of where it started
    fn partial_t60(samples: &[f32], freq_hz: f32, spacing_hz: f32) -> f32 {
        let freq_hz = find_partial(samples, 0.05, freq_hz, spacing_hz);
        let mut points: Vec<(f32, f32)> = Vec::new();
        let mut start_s = 0.05;
        while ((start_s * SAMPLE_RATE) as usize) + WINDOW <= samples.len() {
            let start = (start_s * SAMPLE_RATE) as usize;
            let level = tone_level(&samples[start..start + WINDOW], SAMPLE_RATE, freq_hz);
            let level_db = 20.0 * level.max(1e-12).log10();
            if points.first().is_some_and(|&(_, first)| level_db < first - 30.0)
                || level_db < -120.0
            {
                break;
            }
            points.push((start_s, level_db));
            start_s += 0.02;
        }
        let count = points.len() as f32;
        let mean_t = points.iter().map(|point| point.0).sum::<f32>() / count;
        let mean_db = points.iter().map(|point| point.1).sum::<f32>() / count;
        let slope = points.iter().map(|&(t, db)| (t - mean_t) * (db - mean_db)).sum::<f32>()
            / points.iter().map(|&(t, _)| (t - mean_t) * (t - mean_t)).sum::<f32>().max(1e-9);
        if slope >= 0.0 {
            f32::INFINITY
        } else {
            -60.0 / slope
        }
    }

    #[test]
    fn test_silent_until_a_note() {
        for voice in Voice::ALL {
            let mut bass = bass(voice, SAMPLE_RATE);
            assert!(render(&mut bass, SAMPLE_RATE, 0.1).iter().all(|&sample| sample == 0.0));
        }
    }

    #[test]
    fn test_notes_are_in_tune_at_every_rate() {
        for voice in Voice::ALL {
            for sample_rate in [44100.0, 48000.0, 96000.0, 192000.0] {
                // Low E, A, and the G an octave and a third above it
                for midi in [28u8, 33, 55] {
                    let mut bass = bass(voice, sample_rate);
                    bass.note_on(midi, 0.9);
                    let output = render(&mut bass, sample_rate, 0.5);
                    let found = pitch(&output[(0.2 * sample_rate) as usize..], sample_rate);
                    let cents = 1200.0 * (found / note_hz(midi as f32)).log2();
                    assert!(
                        cents.abs() < 6.0,
                        "{:?} note {} at {} Hz: {} cents off",
                        voice,
                        midi,
                        sample_rate,
                        cents
                    );
                }
            }
        }
    }

    #[test]
    fn test_transpose_moves_the_pitch() {
        let mut bass = bass(Voice::Groove, SAMPLE_RATE);
        bass.set(&BassSettings { transpose: -12.0, ..Voice::Groove.settings() });
        bass.note_on(45, 0.9);
        let output = render(&mut bass, SAMPLE_RATE, 0.8);
        let found = pitch(&output[9600..], SAMPLE_RATE);
        assert!((found / note_hz(33.0) - 1.0).abs() < 0.005, "{} Hz", found);
    }

    #[test]
    fn test_a_held_note_rings_and_a_released_one_stops() {
        for voice in Voice::ALL {
            let held = note(voice, 33, 0.9, 1.0);
            assert!(rms(&held[38400..]) > 0.02, "{:?} held: {}", voice, rms(&held[38400..]));

            let mut bass = bass(voice, SAMPLE_RATE);
            bass.note_on(33, 0.9);
            render(&mut bass, SAMPLE_RATE, 0.3);
            bass.note_off(33);
            let released = render(&mut bass, SAMPLE_RATE, 0.7);
            assert!(
                rms(&released[24000..]) < 0.002,
                "{:?} released: {}",
                voice,
                rms(&released[24000..])
            );
        }
    }

    #[test]
    fn test_sustain_reaches_the_note_that_is_ringing() {
        // Without the sine under the string, which has its own decay
        let string_alone = BassSettings { sub: 0.0, ..Voice::Groove.settings() };
        let level_after = |sustain_s: f32| {
            let mut bass = bass(Voice::Groove, SAMPLE_RATE);
            bass.set(&string_alone);
            bass.note_on(33, 0.9);
            render(&mut bass, SAMPLE_RATE, 0.2);
            bass.set(&BassSettings { sustain_s, ..string_alone });
            rms(&render(&mut bass, SAMPLE_RATE, 1.0)[38400..])
        };
        assert!(level_after(SUSTAIN_MIN_S) < level_after(SUSTAIN_MAX_S) * 0.3);

        // And one that was let go stays let go
        let mut bass = bass(Voice::Groove, SAMPLE_RATE);
        bass.note_on(33, 0.9);
        render(&mut bass, SAMPLE_RATE, 0.2);
        bass.note_off(33);
        bass.set(&BassSettings { sustain_s: SUSTAIN_MAX_S, ..Voice::Groove.settings() });
        assert!(rms(&render(&mut bass, SAMPLE_RATE, 0.7)[24000..]) < 0.002);
    }

    #[test]
    fn test_a_ringing_note_keeps_its_string_and_its_pitch_when_the_voice_changes() {
        for (from, to) in [(Voice::Metal, Voice::Groove), (Voice::Groove, Voice::Metal)] {
            let mut bass = bass(from, SAMPLE_RATE);
            bass.note_on(33, 0.9);
            render(&mut bass, SAMPLE_RATE, 0.2);
            let loop_lp = bass.loop_lp;
            bass.set(&to.settings());
            assert_eq!(bass.loop_lp, loop_lp, "{:?} to {:?}", from, to);

            let found = pitch(&render(&mut bass, SAMPLE_RATE, 0.5)[4800..], SAMPLE_RATE);
            let cents = 1200.0 * (found / note_hz(33.0)).log2();
            assert!(cents.abs() < 6.0, "{:?} to {:?}: {} cents off", from, to, cents);

            // The next pluck is the new voice's
            bass.note_off(33);
            bass.note_on(33, 0.9);
            assert_ne!(bass.loop_lp, loop_lp);
        }
    }

    #[test]
    fn test_letting_go_of_another_note_changes_nothing() {
        let mut bass = bass(Voice::Metal, SAMPLE_RATE);
        bass.note_on(33, 0.9);
        render(&mut bass, SAMPLE_RATE, 0.1);
        bass.note_off(40);
        assert!(rms(&render(&mut bass, SAMPLE_RATE, 0.5)[19200..]) > 0.02);
    }

    #[test]
    fn test_overlapping_notes_slide_on_groove_and_are_plucked_on_metal() {
        // The level right after the second note: a pluck is a new attack, a slide is not
        let second_note = |voice: Voice| {
            let mut bass = bass(voice, SAMPLE_RATE);
            bass.note_on(33, 0.9);
            render(&mut bass, SAMPLE_RATE, 1.5);
            let before = peak(&render(&mut bass, SAMPLE_RATE, 0.05));
            bass.note_on(38, 0.9);
            let output = render(&mut bass, SAMPLE_RATE, 0.8);
            (peak(&output[..2400]) / before, pitch(&output[14400..], SAMPLE_RATE))
        };

        let (attack, found) = second_note(Voice::Groove);
        assert!(attack < 1.3, "Groove attack: {}", attack);
        assert!((found / note_hz(38.0) - 1.0).abs() < 0.01, "Groove slid to {} Hz", found);

        let (attack, found) = second_note(Voice::Metal);
        assert!(attack > 1.5, "Metal attack: {}", attack);
        assert!((found / note_hz(38.0) - 1.0).abs() < 0.01, "Metal plucked {} Hz", found);
    }

    #[test]
    fn test_velocity_and_brightness_open_the_sound() {
        for voice in Voice::ALL {
            let soft = note(voice, 33, 0.3, 0.3);
            let hard = note(voice, 33, 1.0, 0.3);
            // Less than the velocity says where the drive evens the level out
            assert!(
                peak(&hard) > peak(&soft) * 1.2,
                "{:?}: {} vs {}",
                voice,
                peak(&hard),
                peak(&soft)
            );

            let with = |brightness: f32| {
                let mut bass = bass(voice, SAMPLE_RATE);
                bass.set(&BassSettings { brightness, ..voice.settings() });
                bass.note_on(33, 0.9);
                self::brightness(&render(&mut bass, SAMPLE_RATE, 0.3))
            };
            // Groove's tone filter keeps most of it back
            assert!(with(1.0) > with(0.0) * 1.1, "{:?}: {} vs {}", voice, with(1.0), with(0.0));
        }
    }

    #[test]
    fn test_the_voices_differ_and_metal_is_the_brighter() {
        let metal = note(Voice::Metal, 33, 0.9, 0.5);
        let groove = note(Voice::Groove, 33, 0.9, 0.5);
        assert!(brightness(&metal) > brightness(&groove) * 2.0);
    }

    // The notes the tone is measured on: low E, A, and the G above
    const MEASURED: [u8; 3] = [28, 33, 43];

    /// What the pickup reads from the string alone (Sub at 0), before the voice's output
    /// stage: Metal's drive makes new partials at the top out of the ones under it for as
    /// long as they ring
    fn pickup_alone(voice: Voice, midi: u8, velocity: f32, seconds: f32) -> Vec<f32> {
        let mut bass = bass(voice, SAMPLE_RATE);
        bass.set(&BassSettings { sub: 0.0, ..voice.settings() });
        bass.note_on(midi, velocity);
        (0..(seconds * SAMPLE_RATE) as usize).map(|_| bass.string_and_sub().0).collect()
    }

    #[test]
    fn test_the_top_dies_before_the_fundamental() {
        for voice in Voice::ALL {
            for midi in MEASURED {
                let string = pickup_alone(voice, midi, 0.9, 3.0);
                let freq_hz = note_hz(midi as f32);
                let fundamental = partial_t60(&string, freq_hz, freq_hz);
                let top = partial_t60(&string, top_partial_hz(freq_hz), freq_hz);
                assert!(
                    top <= 0.25 * fundamental,
                    "{:?} note {}: the top rings {} s, the fundamental {} s",
                    voice,
                    midi,
                    top,
                    fundamental
                );
            }
        }
    }

    /// Level in dB of harmonics `first` to `last` of a note together, at 50 ms
    fn harmonics_db(samples: &[f32], freq_hz: f32, first: usize, last: usize) -> f32 {
        let power: f32 = (first..=last)
            .map(|harmonic| harmonic as f32 * freq_hz)
            .map(|hz| 10.0f32.powf(partial_db(samples, 0.05, hz, freq_hz) / 10.0))
            .sum();
        10.0 * power.log10()
    }

    #[test]
    fn test_the_same_note_sounds_the_same() {
        // Two plucks in a row are not copies: each is plucked a little further from or nearer
        // to the bridge. Harmonics 1 to 3 hold the note's body and stay within 1 dB. From 4 up a
        // harmonic can sit near a node of where the pick or the finger is (Metal's 4th and 6th,
        // Groove's 8th), so a move of 1 % of the string moves it by several dB, as on a real
        // string: those are held together, harmonics 4 to 8 within 3 dB
        for voice in Voice::ALL {
            for midi in MEASURED {
                let mut bass = bass(voice, SAMPLE_RATE);
                bass.set(&BassSettings { sub: 0.0, ..voice.settings() });
                let mut pluck = || {
                    bass.note_on(midi, 0.9);
                    let output = render(&mut bass, SAMPLE_RATE, 0.3);
                    bass.note_off(midi);
                    render(&mut bass, SAMPLE_RATE, 0.5);
                    output
                };
                let (first, second) = (pluck(), pluck());
                let freq_hz = note_hz(midi as f32);
                for harmonic in 1..=3 {
                    let hz = harmonic as f32 * freq_hz;
                    let apart = partial_db(&first, 0.05, hz, freq_hz)
                        - partial_db(&second, 0.05, hz, freq_hz);
                    assert!(
                        apart.abs() < 1.0,
                        "{:?} note {} harmonic {}: {} dB apart",
                        voice,
                        midi,
                        harmonic,
                        apart
                    );
                }
                let apart =
                    harmonics_db(&first, freq_hz, 4, 8) - harmonics_db(&second, freq_hz, 4, 8);
                assert!(
                    apart.abs() < 3.0,
                    "{:?} note {}: harmonics 4 to 8 {} dB apart",
                    voice,
                    midi,
                    apart
                );
            }
        }
    }

    /// The first 50 ms of each of `count` plucks of a note at one velocity, a sixteenth at
    /// 150 bpm apart, held for most of it as the Metal line plays them
    fn sixteenths(voice: Voice, midi: u8, count: usize) -> Vec<Vec<f32>> {
        let mut bass = bass(voice, SAMPLE_RATE);
        let sixteenth = SAMPLE_RATE * 60.0 / 150.0 / 4.0;
        let attack = (0.05 * SAMPLE_RATE) as usize;
        (0..count)
            .map(|_| {
                bass.note_on(midi, 0.8);
                let output = render(&mut bass, SAMPLE_RATE, sixteenth * 0.8 / SAMPLE_RATE);
                bass.note_off(midi);
                render(&mut bass, SAMPLE_RATE, sixteenth * 0.2 / SAMPLE_RATE);
                output[..attack].to_vec()
            })
            .collect()
    }

    fn correlation(first: &[f32], second: &[f32]) -> f32 {
        let both: f32 = first.iter().zip(second).map(|(a, b)| a * b).sum();
        let first: f32 = first.iter().map(|a| a * a).sum();
        let second: f32 = second.iter().map(|b| b * b).sum();
        both / (first * second).sqrt().max(1e-12)
    }

    #[test]
    fn test_two_plucks_in_a_row_are_not_copies() {
        for voice in Voice::ALL {
            for midi in MEASURED {
                let plucks = sixteenths(voice, midi, 6);
                // From the second on, each lands on a ringing string, as the ones before it
                for pair in plucks[1..].windows(2) {
                    let alike = correlation(&pair[0], &pair[1]);
                    let apart_db = 20.0 * (rms(&pair[1]) / rms(&pair[0])).log10();
                    assert!(
                        alike < 0.98,
                        "{:?} note {}: two plucks correlate {}",
                        voice,
                        midi,
                        alike
                    );
                    assert!(
                        apart_db.abs() < 1.0,
                        "{:?} note {}: two plucks {} dB apart",
                        voice,
                        midi,
                        apart_db
                    );
                }
            }
        }
    }

    #[test]
    fn test_metals_drive_lets_the_note_die() {
        // At Metal's output, with Sub at 0: the top dies soon, and the fundamental rings as
        // long as it does on the string, not held up by the drive
        for midi in [28u8, 33, 38, 43, 50, 55] {
            let string = pickup_alone(Voice::Metal, midi, 0.9, 3.0);
            let out = string_alone(Voice::Metal, midi, 0.9, 3.0);
            let freq_hz = note_hz(midi as f32);
            let top = partial_t60(&out, top_partial_hz(freq_hz), freq_hz);
            assert!(top <= 0.6, "note {}: the top rings {} s at the output", midi, top);
            let on_string = partial_t60(&string, freq_hz, freq_hz);
            let at_output = partial_t60(&out, freq_hz, freq_hz);
            assert!(
                (at_output / on_string - 1.0).abs() <= 0.15,
                "note {}: the fundamental rings {} s at the output, {} s on the string",
                midi,
                at_output,
                on_string
            );
        }
    }

    /// The period the string plays at over the 20 ms that start at `start_s`: where what
    /// follows matches it best, between two samples
    fn period_over_20_ms(samples: &[f32], start_s: f32, about: f32) -> f32 {
        let start = (start_s * SAMPLE_RATE) as usize;
        let window = &samples[start..start + (0.02 * SAMPLE_RATE) as usize];
        let match_at = |lag: usize| -> f32 {
            let later = &samples[start + lag..start + lag + window.len()];
            correlation(window, later)
        };
        let lags = (0.97 * about) as usize..(1.03 * about) as usize;
        let best = lags.clone().max_by(|&a, &b| match_at(a).total_cmp(&match_at(b))).unwrap();
        let (before, at, after) = (match_at(best - 1), match_at(best), match_at(best + 1));
        best as f32 + 0.5 * (before - after) / (before - 2.0 * at + after).min(-1e-12)
    }

    #[test]
    fn test_a_hard_pluck_starts_sharp_and_settles() {
        for voice in Voice::ALL {
            for midi in MEASURED {
                let freq_hz = note_hz(midi as f32);
                let period = SAMPLE_RATE / freq_hz;
                // Cents sharp over the first 20 ms, against the same pluck played at pitch
                let sharp_at = |velocity: f32| {
                    let mut sharp = bass(voice, SAMPLE_RATE);
                    sharp.note_on(midi, velocity);
                    let sharp = render(&mut sharp, SAMPLE_RATE, 0.1);
                    let mut at_pitch = bass(voice, SAMPLE_RATE);
                    at_pitch.note_on(midi, velocity);
                    at_pitch.sharp_cents = 0.0;
                    let at_pitch = render(&mut at_pitch, SAMPLE_RATE, 0.1);
                    1200.0
                        * (period_over_20_ms(&at_pitch, 0.0, period)
                            / period_over_20_ms(&sharp, 0.0, period))
                        .log2()
                };
                let hard = sharp_at(1.0);
                assert!(
                    (3.0..=8.0).contains(&hard),
                    "{:?} note {}: {} cents sharp at full velocity",
                    voice,
                    midi,
                    hard
                );
                let soft = sharp_at(0.3);
                assert!(soft < 1.5, "{:?} note {}: {} cents sharp at 0.3", voice, midi, soft);

                let output = note(voice, midi, 1.0, 0.8);
                let settled = cents_off(&output, SAMPLE_RATE, 0.3, freq_hz);
                assert!(
                    settled.abs() < 1.0,
                    "{:?} note {}: {} cents off after 300 ms",
                    voice,
                    midi,
                    settled
                );
            }
        }
    }

    #[test]
    fn test_groove_has_harmonics() {
        for midi in MEASURED {
            let output = note(Voice::Groove, midi, 0.9, 0.3);
            let freq_hz = note_hz(midi as f32);
            let fundamental = partial_db(&output, 0.05, freq_hz, freq_hz);
            for harmonic in 2..=4 {
                let level =
                    partial_db(&output, 0.05, harmonic as f32 * freq_hz, freq_hz) - fundamental;
                assert!(
                    (-15.0..=-3.0).contains(&level),
                    "note {} harmonic {}: {} dB",
                    midi,
                    harmonic,
                    level
                );
            }
        }
    }

    #[test]
    fn test_the_sine_under_the_string_does_not_take_over() {
        for voice in Voice::ALL {
            for midi in MEASURED {
                let mut bass = bass(voice, SAMPLE_RATE);
                bass.note_on(midi, 0.9);
                let parts: Vec<(f32, f32)> =
                    (0..(2.2 * SAMPLE_RATE) as usize).map(|_| bass.string_and_sub()).collect();
                let share = |start_s: f32| {
                    let window = &parts[(start_s * SAMPLE_RATE) as usize..][..4800];
                    let string: Vec<f32> = window.iter().map(|part| part.0).collect();
                    let sub: Vec<f32> = window.iter().map(|part| part.1).collect();
                    rms(&sub) / rms(&string)
                };
                let (early, late) = (share(0.1), share(2.0));
                assert!(
                    late <= 1.5 * early,
                    "{:?} note {}: the sine is {} of the string at 0.1 s and {} at 2 s",
                    voice,
                    midi,
                    early,
                    late
                );
            }
        }
    }

    #[test]
    fn test_a_new_pluck_over_a_ringing_string_does_not_click() {
        // The sixteenths of the Metal line at 150 bpm: no sample steps further than the
        // attack of the first note does
        let mut bass = bass(Voice::Metal, SAMPLE_RATE);
        let sixteenth = (SAMPLE_RATE * 60.0 / 150.0 / 4.0) as usize;
        let mut output = Vec::new();
        for _ in 0..16 {
            bass.note_on(28, 0.9);
            output.extend(render(&mut bass, SAMPLE_RATE, sixteenth as f32 * 0.8 / SAMPLE_RATE));
            bass.note_off(28);
            output.extend(render(&mut bass, SAMPLE_RATE, sixteenth as f32 * 0.2 / SAMPLE_RATE));
        }
        let step = |samples: &[f32]| {
            samples.windows(2).fold(0.0f32, |most, pair| most.max((pair[1] - pair[0]).abs()))
        };
        let first = step(&output[..sixteenth]);
        let later = step(&output[sixteenth..]);
        assert!(later <= first * 1.5, "steps of {} after the first note, {} in it", later, first);
    }

    #[test]
    fn test_output_is_bounded_at_every_setting() {
        for (voice, wheel) in Voice::ALL.into_iter().flat_map(|v| [(v, 0.0), (v, -1.0), (v, 1.0)]) {
            for sample_rate in [44100.0, 192000.0] {
                let mut bass = bass(voice, sample_rate);
                bass.set_wheel(wheel);
                let settings = BassSettings {
                    voice,
                    volume_db: 0.0,
                    drive: 1.0,
                    brightness: 1.0,
                    sub: 1.0,
                    tone_hz: TONE_MAX_HZ,
                    sustain_s: SUSTAIN_MAX_S,
                    slide_ms: 5.0,
                    transpose: -TRANSPOSE_RANGE,
                    bend: BEND_MAX as f32,
                };
                let mut output = Vec::new();
                for midi in [0u8, 28, 127, 40, 28] {
                    bass.note_on(midi, 1.0);
                    output.extend(play(&mut bass, &settings, sample_rate, 0.2));
                }
                assert!(output.iter().all(|sample| sample.is_finite()));
                assert!(
                    peak(&output) <= 2.0,
                    "{:?} at {} Hz, wheel at {}, peaks at {}",
                    voice,
                    sample_rate,
                    wheel,
                    peak(&output)
                );
            }
        }
    }

    /// Cents from `expected_hz` of what is played from `start_s` on
    fn cents_off(output: &[f32], sample_rate: f32, start_s: f32, expected_hz: f32) -> f32 {
        let found = pitch(&output[(start_s * sample_rate) as usize..], sample_rate);
        1200.0 * (found / expected_hz).log2()
    }

    // The rates the bend is tested at
    const BEND_RATES: [f32; 3] = [44100.0, 48000.0, 96000.0];

    #[test]
    fn test_the_wheel_bends_a_held_note_by_the_bend_range() {
        for voice in Voice::ALL {
            for sample_rate in BEND_RATES {
                // Up and down at Bend 2, and up an octave at Bend 12
                for (bend, wheel) in [(2.0, 1.0), (2.0, -1.0), (12.0, 1.0)] {
                    let settings = BassSettings { bend, ..voice.settings() };
                    let mut bass = bass(voice, sample_rate);
                    bass.set(&settings);
                    bass.note_on(33, 0.9);
                    play(&mut bass, &settings, sample_rate, 0.1);
                    bass.set_wheel(wheel);
                    let output = play(&mut bass, &settings, sample_rate, 0.6);
                    let cents = cents_off(&output, sample_rate, 0.1, note_hz(33.0 + wheel * bend));
                    assert!(
                        cents.abs() < 2.0,
                        "{:?} at {} Hz, Bend {}, wheel {}: {} cents off",
                        voice,
                        sample_rate,
                        bend,
                        wheel,
                        cents
                    );
                }
            }
        }
    }

    #[test]
    fn test_the_wheel_back_at_centre_brings_the_note_back() {
        for voice in Voice::ALL {
            for sample_rate in BEND_RATES {
                let settings = voice.settings();
                // The same note, never bent, measured over the same time
                let mut unbent = bass(voice, sample_rate);
                unbent.note_on(33, 0.9);
                let unbent = play(&mut unbent, &settings, sample_rate, 1.0);

                let mut bass = bass(voice, sample_rate);
                bass.note_on(33, 0.9);
                let mut output = play(&mut bass, &settings, sample_rate, 0.1);
                bass.set_wheel(1.0);
                output.extend(play(&mut bass, &settings, sample_rate, 0.3));
                bass.set_wheel(0.0);
                output.extend(play(&mut bass, &settings, sample_rate, 0.6));

                let start = (0.5 * sample_rate) as usize;
                let found = pitch(&output[start..], sample_rate);
                let expected = pitch(&unbent[start..], sample_rate);
                let cents = 1200.0 * (found / expected).log2();
                assert!(
                    cents.abs() < 1.0,
                    "{:?} at {} Hz: {} cents off",
                    voice,
                    sample_rate,
                    cents
                );
                assert_eq!(bass.bend_ratio, 1.0, "the string is exactly unbent again");
            }
        }
    }

    #[test]
    fn test_a_note_plucked_with_the_wheel_up_starts_bent() {
        for voice in Voice::ALL {
            for sample_rate in BEND_RATES {
                let settings = voice.settings();
                let mut bass = bass(voice, sample_rate);
                bass.set_wheel(0.5);
                play(&mut bass, &settings, sample_rate, 0.05);
                bass.note_on(33, 0.9);
                // From its start: half of Bend 2 up
                let output = play(&mut bass, &settings, sample_rate, 0.25);
                let cents = cents_off(&output, sample_rate, 0.0, note_hz(34.0));
                assert!(
                    cents.abs() < 2.0,
                    "{:?} at {} Hz: {} cents off",
                    voice,
                    sample_rate,
                    cents
                );
            }
        }
    }

    #[test]
    fn test_the_bend_moves_the_pitch_smoothly() {
        let step = |samples: &[f32]| {
            samples.windows(2).fold(0.0f32, |most, pair| most.max((pair[1] - pair[0]).abs()))
        };
        for voice in Voice::ALL {
            for sample_rate in BEND_RATES {
                let settings = voice.settings();
                let mut bass = bass(voice, sample_rate);
                bass.note_on(33, 0.9);
                play(&mut bass, &settings, sample_rate, 0.3);
                // How far the held note moves from one sample to the next on its own
                let held = step(&play(&mut bass, &settings, sample_rate, 0.1));
                // A wheel that sends coarse steps, a quarter of the way a block, up and back
                let mut bent = Vec::new();
                for wheel in [0.25, 0.5, 0.75, 1.0, 1.0, 0.75, 0.5, 0.25, 0.0, 0.0] {
                    bass.set_wheel(wheel);
                    bent.extend(play(&mut bass, &settings, sample_rate, 64.0 / sample_rate));
                }
                bent.extend(play(&mut bass, &settings, sample_rate, 0.05));
                // A whole tone up makes the note's own steps an eighth larger
                assert!(
                    step(&bent) <= held * 1.2,
                    "{:?} at {} Hz: steps of {} while bending, {} held",
                    voice,
                    sample_rate,
                    step(&bent),
                    held
                );
            }
        }
    }

    #[test]
    fn test_a_slide_with_the_wheel_up_ends_bent() {
        for sample_rate in BEND_RATES {
            // Groove slides
            let settings = Voice::Groove.settings();
            let mut bass = bass(Voice::Groove, sample_rate);
            bass.set_wheel(1.0);
            bass.note_on(33, 0.9);
            play(&mut bass, &settings, sample_rate, 0.3);
            bass.note_on(38, 0.9);
            let output = play(&mut bass, &settings, sample_rate, 0.6);
            let cents = cents_off(&output, sample_rate, 0.3, note_hz(40.0));
            assert!(cents.abs() < 2.0, "at {} Hz: {} cents off", sample_rate, cents);
        }
    }

    #[test]
    fn test_a_bent_note_rings_as_long_as_sustain_says() {
        for voice in Voice::ALL {
            let settings = BassSettings { sub: 0.0, ..voice.settings() };
            let mut bass = bass(voice, SAMPLE_RATE);
            bass.set(&settings);
            bass.note_on(33, 0.9);
            bass.set_wheel(1.0);
            // What the pickup reads, before the output stage, the settings once a block
            let mut string = Vec::new();
            while string.len() < (3.0 * SAMPLE_RATE) as usize {
                bass.set(&settings);
                string.extend((0..64).map(|_| bass.string_and_sub().0));
            }
            let freq_hz = note_hz(35.0);
            let t60 = partial_t60(&string, freq_hz, freq_hz);
            assert!(
                (t60 / settings.sustain_s - 1.0).abs() < 0.1,
                "{:?}: the bent note rings {} s, Sustain is {} s",
                voice,
                t60,
                settings.sustain_s
            );
        }
    }

    #[test]
    fn test_reset_puts_the_wheel_back_at_centre() {
        let mut bass = bass(Voice::Metal, SAMPLE_RATE);
        bass.set_wheel(1.0);
        bass.note_on(33, 0.9);
        play(&mut bass, &Voice::Metal.settings(), SAMPLE_RATE, 0.1);
        bass.reset();
        bass.note_on(33, 0.9);
        let output = play(&mut bass, &Voice::Metal.settings(), SAMPLE_RATE, 0.5);
        let cents = cents_off(&output, SAMPLE_RATE, 0.2, note_hz(33.0));
        assert!(cents.abs() < 2.0, "{} cents off", cents);
    }

    /// Level, pitch error and brightness per voice and note. Then, with Sub at 0: from what the
    /// pickup reads, before the voice's output stage, how long the fundamental and the top
    /// ring and how loud harmonics 2 and 3 are (the columns `string`); and how long the two ring
    /// at the voice's output, through its drive (the columns `out`):
    ///   cargo test -p bass --release bass_report -- --ignored --nocapture
    #[test]
    #[ignore]
    fn bass_report() {
        for voice in Voice::ALL {
            println!("\n{:?}", voice);
            println!(
                "{:<8}{:>8}{:>8}{:>10}{:>12}{:>12}{:>12}{:>10}{:>10}{:>10}{:>10}",
                "",
                "",
                "",
                "",
                "",
                "string T60",
                "string T60",
                "string",
                "string",
                "out T60",
                "out T60"
            );
            println!(
                "{:<8}{:>8}{:>8}{:>10}{:>12}{:>12}{:>12}{:>10}{:>10}{:>10}{:>10}",
                "note",
                "peak",
                "rms",
                "cents",
                "brightness",
                "f0",
                "top",
                "h2 dB",
                "h3 dB",
                "f0",
                "top"
            );
            for midi in [28u8, 33, 38, 43, 50, 55] {
                let output = note(voice, midi, 0.9, 1.0);
                let found = pitch(&output[9600..], SAMPLE_RATE);
                let string = pickup_alone(voice, midi, 0.9, 3.0);
                let out = string_alone(voice, midi, 0.9, 3.0);
                let freq_hz = note_hz(midi as f32);
                let top_hz = top_partial_hz(freq_hz);
                let fundamental = partial_db(&string, 0.05, freq_hz, freq_hz);
                println!(
                    "{:<8}{:>8.3}{:>8.3}{:>10.1}{:>12.4}{:>12.2}{:>12.2}{:>10.1}{:>10.1}{:>10.2}{:>10.2}",
                    midi,
                    peak(&output),
                    rms(&output),
                    1200.0 * (found / note_hz(midi as f32)).log2(),
                    brightness(&output),
                    partial_t60(&string, freq_hz, freq_hz),
                    partial_t60(&string, top_hz, freq_hz),
                    partial_db(&string, 0.05, 2.0 * freq_hz, freq_hz) - fundamental,
                    partial_db(&string, 0.05, 3.0 * freq_hz, freq_hz) - fundamental,
                    partial_t60(&out, freq_hz, freq_hz),
                    partial_t60(&out, top_hz, freq_hz),
                );
            }
        }

        let mut bass = bass(Voice::Metal, SAMPLE_RATE);
        bass.note_on(28, 1.0);
        let frames = 480_000;
        let started = std::time::Instant::now();
        let mut sum = 0.0;
        for _ in 0..frames {
            sum += bass.process();
        }
        let per_frame_ns = started.elapsed().as_nanos() as f64 / frames as f64;
        println!(
            "\nOne note at 48 kHz: {:.0} ns per frame ({:.2} % of a core) ({})",
            per_frame_ns,
            per_frame_ns * 48000.0 / 1e7,
            sum.is_finite()
        );
    }

    /// A line to render: (beat, length in beats, note, velocity), and its tempo
    type Line = (Vec<(f32, f32, u8, f32)>, f32);

    /// Sixteenths on the low E at 150 bpm, the octave on the 15th, and a long note to end
    fn metal_line() -> Line {
        let mut line = Vec::new();
        for bar in 0..4 {
            let start = bar as f32 * 4.0;
            let root = [28, 28, 31, 26][bar];
            for sixteenth in 0..16 {
                let accent = if sixteenth % 4 == 0 { 1.0 } else { 0.8 };
                let midi = if sixteenth == 14 { root + 12 } else { root };
                line.push((start + sixteenth as f32 * 0.25, 0.2, midi, accent));
            }
        }
        line.push((16.0, 4.0, 28, 1.0));
        (line, 150.0)
    }

    /// A held root, a dead note, a slide up to the fifth and the octave, at 88 bpm
    fn groove_line() -> Line {
        let mut line = Vec::new();
        for bar in 0..4 {
            let start = bar as f32 * 4.0;
            let root = [33, 33, 38, 36][bar];
            line.push((start, 1.4, root, 0.85));
            line.push((start + 1.5, 0.1, root, 0.3));
            line.push((start + 2.0, 0.8, root + 5, 0.8));
            line.push((start + 2.5, 0.75, root + 7, 0.8));
            line.push((start + 3.25, 0.5, root + 12, 0.7));
        }
        line.push((16.0, 4.0, 33, 0.85));
        (line, 88.0)
    }

    /// Quarter notes walking through F, Bb, C and F at 132 bpm, each let go before the next
    fn walking_line() -> Line {
        let notes = [29u8, 33, 36, 38, 34, 38, 41, 40, 36, 40, 43, 42, 41, 36, 33, 31];
        let mut line: Vec<_> = notes
            .iter()
            .enumerate()
            .map(|(beat, &midi)| (beat as f32, 0.85, midi, if beat % 2 == 0 { 0.8 } else { 0.7 }))
            .collect();
        line.push((16.0, 3.0, 29, 0.8));
        (line, 132.0)
    }

    /// A one drop at 76 bpm: the root on beat three, the fifth and the octave around it
    fn reggae_line() -> Line {
        let mut line = Vec::new();
        for bar in 0..4 {
            let start = bar as f32 * 4.0;
            let root = [28, 28, 33, 35][bar];
            line.push((start + 0.5, 0.4, root, 0.75));
            line.push((start + 2.0, 1.2, root, 0.9));
            line.push((start + 3.25, 0.2, root + 7, 0.7));
            line.push((start + 3.5, 0.4, root + 12, 0.7));
        }
        line.push((16.0, 3.0, 28, 0.9));
        (line, 76.0)
    }

    /// Sixteenths at 100 bpm with dead notes (soft and very short) between the notes
    fn funk_line() -> Line {
        // (sixteenth, semitones over the root, or None for a dead note)
        const PATTERN: [(usize, Option<u8>); 10] = [
            (0, Some(0)),
            (2, None),
            (3, Some(12)),
            (5, None),
            (6, Some(0)),
            (8, Some(10)),
            (9, None),
            (10, Some(12)),
            (13, None),
            (14, Some(7)),
        ];
        let mut line = Vec::new();
        for bar in 0..4 {
            let start = bar as f32 * 4.0;
            let root = [31, 31, 36, 38][bar];
            for (sixteenth, interval) in PATTERN {
                let beat = start + sixteenth as f32 * 0.25;
                match interval {
                    Some(interval) => line.push((beat, 0.2, root + interval, 0.85)),
                    None => line.push((beat, 0.04, root, 0.25)),
                }
            }
        }
        line.push((16.0, 2.0, 31, 0.9));
        (line, 100.0)
    }

    fn render_line(voice: Voice, (line, bpm): &Line, path: &std::path::Path) {
        let samples_per_beat = SAMPLE_RATE * 60.0 / bpm;
        let mut events: Vec<(usize, u8, f32)> = Vec::new();
        for &(beat, length, midi, velocity) in line {
            events.push(((beat * samples_per_beat) as usize, midi, velocity));
            events.push((((beat + length) * samples_per_beat) as usize, midi, 0.0));
        }
        events.sort_by_key(|event| event.0);
        let length = events.last().unwrap().0 + SAMPLE_RATE as usize;

        let mut bass = bass(voice, SAMPLE_RATE);
        let mut next = 0;
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: SAMPLE_RATE as u32,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for sample in 0..length {
            while next < events.len() && events[next].0 <= sample {
                let (_, midi, velocity) = events[next];
                if velocity > 0.0 {
                    bass.note_on(midi, velocity);
                } else {
                    bass.note_off(midi);
                }
                next += 1;
            }
            writer.write_sample((bass.process().clamp(-1.0, 1.0) * 32767.0) as i16).unwrap();
        }
        writer.finalize().unwrap();
    }

    /// Writes WAV files to target/renders for listening: each voice's own line
    /// (bass_<voice>.wav), and a walking, a reggae and a funk line on both voices
    /// (bass_<line>_<voice>.wav):
    ///   cargo test -p bass --release render_wavs -- --ignored
    #[test]
    #[ignore]
    fn render_wavs() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/renders");
        std::fs::create_dir_all(&dir).unwrap();

        for voice in Voice::ALL {
            let name = format!("{:?}", voice).to_lowercase();
            let own = match voice {
                Voice::Metal => metal_line(),
                Voice::Groove => groove_line(),
            };
            render_line(voice, &own, &dir.join(format!("bass_{}.wav", name)));
            for (style, line) in
                [("walking", walking_line()), ("reggae", reggae_line()), ("funk", funk_line())]
            {
                render_line(voice, &line, &dir.join(format!("bass_{}_{}.wav", style, name)));
            }
        }
        println!("Wrote renders to {}", dir.display());
    }
}
