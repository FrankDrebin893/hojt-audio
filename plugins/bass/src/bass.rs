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
                drive: 0.5,
                brightness: 0.55,
                sub: 0.25,
                tone_hz: 3200.0,
                sustain_s: 3.0,
                slide_ms: 0.0,
                transpose: 0.0,
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
    // The pitch the string is at, or sliding to
    freq_hz: f32,
    // What the loop's low-pass adds to its length at that pitch
    filter_delay: f32,
    loop_lp: f32,
    loop_state: f32,
    feedback: f32,
    feedback_held: f32,
    feedback_released: f32,
    glide: f32,

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
            loop_lp: 0.4,
            loop_state: 0.0,
            feedback: 0.0,
            feedback_held: 0.0,
            feedback_released: 0.0,
            glide: 1.0,
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
        self.set_voice_filters();
        self.reset();
    }

    pub fn reset(&mut self) {
        self.line.fill(0.0);
        self.loop_state = 0.0;
        self.note = None;
        self.held = false;
        self.sub_env = 0.0;
        self.last_raw = 0.0;
        self.declick = 0.0;
        self.declick_pending = false;
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
        if *settings == self.settings {
            return;
        }
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
        let damping =
            Damping::for_note(self.freq_hz, self.settings.sustain_s, top_t60_s, self.sample_rate);
        self.loop_lp = damping.loop_lp;
        self.set_feedback();
    }

    /// The loop's gain, which sets how long the fundamental rings, held and let go
    fn set_feedback(&mut self) {
        let damping = Damping::at(self.loop_lp, self.freq_hz, self.sample_rate);
        self.filter_delay = damping.filter_delay;
        let release_s = match self.settings.voice {
            Voice::Metal => RELEASE_METAL_S,
            Voice::Groove => RELEASE_GROOVE_S,
        };
        let gain =
            |t60_s: f32| (10.0f32.powf(-3.0 / (t60_s * self.freq_hz)) / damping.gain).min(0.99999);
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
        self.freq_hz = self.freq_hz(note);
        self.note = Some(note);
        self.held = true;
        self.string_voice = self.settings.voice;
        self.string_velocity = velocity;
        self.damp_string();
        self.delay = self.loop_delay(self.freq_hz);
        self.target_delay = self.delay;

        // What the line holds is how fast the string moves. A string pulled to a point and
        // let go moves one way on one side of that point and the other way on the other:
        // two levels, whose edges are rounded by what plucks it.
        let period = self.delay;
        let length = (period.ceil() as usize).clamp(2, self.line.len() - 8);
        let (along, level, sharpness, rounding_s) = match self.settings.voice {
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
            let mut previous = 0.0;
            for (index, sample) in self.pluck[..click].iter_mut().enumerate() {
                let noise = self.noise.next();
                *sample +=
                    (noise - previous) * 2.0 * sharpness * (1.0 - index as f32 / click as f32);
                previous = noise;
            }
        }

        // Into the line, in the order it is read: what is ringing is stopped by the pluck
        let scale = level * STRING_GAIN / std::f32::consts::TAU;
        let start = self.write + self.line.len() - length;
        for index in 0..length {
            self.line[(start + index) & self.mask] = self.pluck[index] * scale;
        }

        // The fundamental of what the pickup will read over the first time round, which the
        // sine under the string starts in phase with
        let tap = (PICKUP * period).round() as usize;
        let step = std::f32::consts::TAU / period;
        let (step_sin, step_cos) = step.sin_cos();
        let (mut sin, mut cos) = (0.0f32, 1.0f32);
        let (mut real, mut imag) = (0.0, 0.0);
        for index in 0..length {
            let read = (self.pluck[(index + tap) % length] - self.pluck[index]) * scale;
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

        self.loop_state = 0.0;
        self.declick_pending = true;
    }

    /// What makes the fundamental come out of the pickup as loud as it is on the string
    fn pickup_gain() -> f32 {
        1.0 / (2.0 * (std::f32::consts::PI * PICKUP).sin())
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

        // Only moves during a slide
        self.delay += (self.target_delay - self.delay) * self.glide;
        let freq_hz = rate / (self.delay + self.filter_delay);

        // The string, and the pickup: where the string is read now, against what reaches it
        // a fifth of the way round later
        let string = self.read(self.delay);
        let ahead = self.read((self.delay * (1.0 - PICKUP)).max(1.0));
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
                let driven = (high * (1.0 + settings.drive * 14.0)).tanh();
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

    #[test]
    fn test_the_same_note_sounds_the_same() {
        for midi in MEASURED {
            let mut bass = bass(Voice::Metal, SAMPLE_RATE);
            bass.set(&BassSettings { sub: 0.0, ..Voice::Metal.settings() });
            let mut pluck = || {
                bass.note_on(midi, 0.9);
                let output = render(&mut bass, SAMPLE_RATE, 0.3);
                bass.note_off(midi);
                render(&mut bass, SAMPLE_RATE, 0.5);
                output
            };
            let (first, second) = (pluck(), pluck());
            let freq_hz = note_hz(midi as f32);
            for harmonic in 1..=8 {
                let hz = harmonic as f32 * freq_hz;
                let apart =
                    partial_db(&first, 0.05, hz, freq_hz) - partial_db(&second, 0.05, hz, freq_hz);
                assert!(
                    apart.abs() < 1.0,
                    "note {} harmonic {}: {} dB apart",
                    midi,
                    harmonic,
                    apart
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
        for voice in Voice::ALL {
            for sample_rate in [44100.0, 192000.0] {
                let mut bass = bass(voice, sample_rate);
                bass.set(&BassSettings {
                    voice,
                    volume_db: 0.0,
                    drive: 1.0,
                    brightness: 1.0,
                    sub: 1.0,
                    tone_hz: TONE_MAX_HZ,
                    sustain_s: SUSTAIN_MAX_S,
                    slide_ms: 5.0,
                    transpose: -TRANSPOSE_RANGE,
                });
                let mut output = Vec::new();
                for midi in [0u8, 28, 127, 40, 28] {
                    bass.note_on(midi, 1.0);
                    output.extend(render(&mut bass, sample_rate, 0.2));
                }
                assert!(output.iter().all(|sample| sample.is_finite()));
                assert!(
                    peak(&output) <= 2.0,
                    "{:?} at {} Hz peaks at {}",
                    voice,
                    sample_rate,
                    peak(&output)
                );
            }
        }
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
