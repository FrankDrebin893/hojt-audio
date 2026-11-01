//! Metronome: a click on the beat, for practice. It keeps its own clock (a count of samples to
//! the next beat, so a tempo holds to the sample and never drifts) or follows the host's tempo
//! and transport. The click is added to the plugin's output after the whole chain, so the amp
//! never distorts it. Nothing here allocates: a voice is a few numbers.

use nih_plug::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// The tempo range, in beats per minute (a beat is a quarter note, and in 6/8 an eighth)
pub const BPM_MIN: f32 = 40.0;
pub const BPM_MAX: f32 = 300.0;
pub const BPM_DEFAULT: f32 = 120.0;

/// The click's level in dB; the bottom of the range is silence
pub const VOLUME_MIN_DB: f32 = -60.0;
pub const VOLUME_MAX_DB: f32 = 0.0;
pub const VOLUME_DEFAULT_DB: f32 = -12.0;

/// How loud a beat other than the first is, against the first (the downbeat)
const SECONDARY_LEVEL: f32 = 0.6;

// Frozen: the variant ids and their order are what DAW projects store. A new value goes at
// the end
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeSignature {
    #[id = "two_four"]
    #[name = "2/4"]
    TwoFour,
    #[id = "three_four"]
    #[name = "3/4"]
    ThreeFour,
    #[id = "four_four"]
    #[name = "4/4"]
    FourFour,
    #[id = "six_eight"]
    #[name = "6/8"]
    SixEight,
}

impl TimeSignature {
    pub const ALL: [Self; 4] = [Self::TwoFour, Self::ThreeFour, Self::FourFour, Self::SixEight];

    pub fn name(self) -> &'static str {
        match self {
            Self::TwoFour => "2/4",
            Self::ThreeFour => "3/4",
            Self::FourFour => "4/4",
            Self::SixEight => "6/8",
        }
    }

    /// Clicks in a bar
    pub fn beats(self) -> u32 {
        match self {
            Self::TwoFour => 2,
            Self::ThreeFour => 3,
            Self::FourFour => 4,
            Self::SixEight => 6,
        }
    }

    /// How long one click is, in quarter notes: a quarter in x/4, an eighth in 6/8
    pub fn quarters_per_beat(self) -> f64 {
        match self {
            Self::SixEight => 0.5,
            _ => 1.0,
        }
    }
}

/// Where the click is heard. Frozen like the time signatures
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickOut {
    /// Added to the main output, after the amp
    #[id = "mixed"]
    #[name = "Mixed"]
    Mixed,
    /// On the auxiliary output "Metronome" alone, so the DAW can route it by itself. Where
    /// the host gives no such output (the standalone program) it is mixed in, to stay audible
    #[id = "separate"]
    #[name = "Separate"]
    Separate,
}

impl ClickOut {
    pub const ALL: [Self; 2] = [Self::Mixed, Self::Separate];

    pub fn name(self) -> &'static str {
        match self {
            Self::Mixed => "Mixed",
            Self::Separate => "Separate",
        }
    }
}

// Frozen like the time signatures
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    #[id = "woodblock"]
    #[name = "Woodblock"]
    Woodblock,
    #[id = "beep"]
    #[name = "Beep"]
    Beep,
    #[id = "click"]
    #[name = "Click"]
    Click,
}

impl Sound {
    pub const ALL: [Self; 3] = [Self::Woodblock, Self::Beep, Self::Click];

    pub fn name(self) -> &'static str {
        match self {
            Self::Woodblock => "Woodblock",
            Self::Beep => "Beep",
            Self::Click => "Click",
        }
    }
}

/// What the metronome is told with every block
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetronomeSettings {
    pub on: bool,
    pub bpm: f32,
    /// Follow the host's tempo and transport, where it gives them
    pub sync: bool,
    pub time_signature: TimeSignature,
    /// Linear gain of the click
    pub gain: f32,
    pub sound: Sound,
    /// From the host: its tempo, whether it plays, and the position in quarter notes
    pub host_tempo: Option<f64>,
    pub host_playing: bool,
    pub host_beats: Option<f64>,
}

impl Default for MetronomeSettings {
    fn default() -> Self {
        Self {
            on: false,
            bpm: BPM_DEFAULT,
            sync: false,
            time_signature: TimeSignature::FourFour,
            gain: volume_gain(VOLUME_DEFAULT_DB),
            sound: Sound::Woodblock,
            host_tempo: None,
            host_playing: false,
            host_beats: None,
        }
    }
}

/// The click's gain for a level in dB; the bottom of the range is silence
pub fn volume_gain(db: f32) -> f32 {
    if db <= VOLUME_MIN_DB {
        0.0
    } else {
        10.0f32.powf(db.min(VOLUME_MAX_DB) / 20.0)
    }
}

/// The beats, as the editor can see them: how many have sounded, and whether the last was a
/// downbeat. Two stores per beat from the audio thread; nothing is read back
#[derive(Default)]
pub struct BeatReading {
    count: AtomicU32,
    accent: AtomicBool,
}

impl BeatReading {
    fn beat(&self, accent: bool) {
        self.accent.store(accent, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Release);
    }

    /// The number of beats so far (it wraps) and whether the last was a downbeat
    pub fn read(&self) -> (u32, bool) {
        let count = self.count.load(Ordering::Acquire);
        (count, self.accent.load(Ordering::Relaxed))
    }
}

/// One click sounding
#[derive(Clone, Copy)]
struct Voice {
    sound: Sound,
    level: f32,
    /// Samples since it began
    age: u32,
    phase: f32,
    /// The upper partial's phase, kept apart so that it does not jump when the first wraps
    upper: f32,
    step: f32,
    envelope: f32,
    decay: f32,
    /// Noise generator and the state of the resonator that shapes it
    noise: u32,
    low: f32,
    band: f32,
    /// The resonator's coefficients
    f: f32,
    q: f32,
    attack: u32,
}

impl Voice {
    const IDLE: Self = Self {
        sound: Sound::Woodblock,
        level: 0.0,
        age: u32::MAX,
        phase: 0.0,
        upper: 0.0,
        step: 0.0,
        envelope: 0.0,
        decay: 0.0,
        noise: 1,
        low: 0.0,
        band: 0.0,
        f: 0.0,
        q: 0.0,
        attack: 0,
    };

    fn start(sound: Sound, accent: bool, sample_rate: f32) -> Self {
        let level = if accent { 1.0 } else { SECONDARY_LEVEL };
        let tau = |seconds: f32| (-1.0 / (seconds * sample_rate)).exp();
        let mut voice = Self { sound, level, age: 0, envelope: 1.0, ..Self::IDLE };
        match sound {
            // Two partials of a struck block, the high one dying faster
            Sound::Woodblock => {
                let hz = if accent { 1900.0 } else { 1250.0 };
                voice.step = hz / sample_rate;
                voice.decay = tau(0.011);
            }
            // A plain tone with a soft start
            Sound::Beep => {
                let hz = if accent { 1568.0 } else { 988.0 };
                voice.step = hz / sample_rate;
                voice.decay = tau(0.020);
                voice.attack = (0.002 * sample_rate) as u32;
            }
            // A burst of noise through a resonance, over in a few milliseconds
            Sound::Click => {
                let hz = if accent { 3400.0 } else { 2300.0 };
                voice.f = 2.0 * (std::f32::consts::PI * hz / sample_rate).sin();
                voice.q = 0.35;
                voice.decay = tau(0.004);
                voice.noise = if accent { 0x9E37_79B9 } else { 0x85EB_CA6B };
            }
        }
        voice
    }

    fn live(&self) -> bool {
        self.age != u32::MAX && self.envelope > 0.0005
    }

    fn next(&mut self) -> f32 {
        // The sine is taken after the step, so the first sample is already off zero
        self.phase += self.step;
        self.phase -= self.phase.floor();
        self.upper += self.step * 2.43;
        self.upper -= self.upper.floor();
        let tone = (std::f32::consts::TAU * self.phase).sin();
        let shaped = match self.sound {
            Sound::Woodblock => {
                let upper = (std::f32::consts::TAU * self.upper).sin();
                // The upper partial dies as the square of the envelope
                tone * 0.8 + upper * 0.35 * self.envelope
            }
            Sound::Beep => {
                let rise = if self.attack > 0 {
                    (self.age as f32 / self.attack as f32).min(1.0)
                } else {
                    1.0
                };
                // The first sample is already off zero
                tone * rise.max(0.05)
            }
            Sound::Click => {
                self.noise = self.noise.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let white = (self.noise >> 8) as f32 / 8_388_608.0 - 1.0;
                self.low += self.f * self.band;
                let high = white - self.low - self.q * self.band;
                self.band += self.f * high;
                (self.band * 1.6 + white * 0.25).clamp(-1.0, 1.0)
            }
        };
        let out = shaped * self.envelope * self.level;
        self.envelope *= self.decay;
        self.age = self.age.saturating_add(1);
        out
    }
}

/// The metronome: its clock, the click sounding and what the editor reads
pub struct Metronome {
    sample_rate: f32,
    /// Samples to the next beat. Zero or below: it falls on this sample. Kept as a fraction
    /// of a sample, so beats that are not a whole number of samples apart never drift
    until_beat: f64,
    /// Which beat of the bar the next one is, from 0
    next_beat: u32,
    was_on: bool,
    voice: Voice,
    reading: Arc<BeatReading>,
}

impl Default for Metronome {
    fn default() -> Self {
        Self::new()
    }
}

impl Metronome {
    pub fn new() -> Self {
        Self {
            sample_rate: 48_000.0,
            until_beat: 0.0,
            next_beat: 0,
            was_on: false,
            voice: Voice::IDLE,
            reading: Arc::new(BeatReading::default()),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.reset();
    }

    /// Silence, and the next time it is switched on it starts on a downbeat
    pub fn reset(&mut self) {
        self.until_beat = 0.0;
        self.next_beat = 0;
        self.was_on = false;
        self.voice = Voice::IDLE;
    }

    pub fn reading(&self) -> Arc<BeatReading> {
        self.reading.clone()
    }

    /// True while there is nothing to hear or to count: off, and the last click died away
    pub fn is_idle(&self) -> bool {
        !self.was_on && !self.voice.live()
    }

    /// Samples from one beat to the next at `bpm`, for beats `quarters` long
    fn period(&self, bpm: f64, quarters: f64) -> f64 {
        60.0 / bpm * quarters * f64::from(self.sample_rate)
    }

    /// Adds the click to a block. A block of one channel is the left one
    pub fn process(
        &mut self,
        settings: &MetronomeSettings,
        left: &mut [f32],
        mut right: Option<&mut [f32]>,
    ) {
        let len = left.len();
        self.render(settings, len, |index, click| {
            left[index] = (left[index] + click).clamp(-1.0, 1.0);
            if let Some(right) = right.as_deref_mut() {
                right[index] = (right[index] + click).clamp(-1.0, 1.0);
            }
        });
    }

    /// Writes the click of a block, and only the click, to its own stereo pair (`right` is
    /// left out for a mono one): the pair is overwritten, silence where there is no click
    pub fn process_alone(
        &mut self,
        settings: &MetronomeSettings,
        left: &mut [f32],
        mut right: Option<&mut [f32]>,
    ) {
        left.fill(0.0);
        if let Some(right) = right.as_deref_mut() {
            right.fill(0.0);
        }
        let len = left.len();
        self.render(settings, len, |index, click| {
            let click = click.clamp(-1.0, 1.0);
            left[index] = click;
            if let Some(right) = right.as_deref_mut() {
                right[index] = click;
            }
        });
    }

    /// The clock and the voice for `len` samples; `emit` is given each sample that has a click
    fn render(
        &mut self,
        settings: &MetronomeSettings,
        len: usize,
        mut emit: impl FnMut(usize, f32),
    ) {
        if !settings.on {
            self.was_on = false;
        }
        if !settings.on && !self.voice.live() {
            return;
        }
        let signature = settings.time_signature;
        let beats = signature.beats();
        let quarters = signature.quarters_per_beat();
        let mut bpm = f64::from(settings.bpm.clamp(BPM_MIN, BPM_MAX));
        let mut following = false;
        if settings.on {
            if !self.was_on {
                // From the first sample, on the first beat of the bar
                self.was_on = true;
                self.until_beat = 0.0;
                self.next_beat = 0;
            }
            if settings.sync {
                if let Some(tempo) = settings.host_tempo.filter(|tempo| *tempo > 0.0) {
                    bpm = tempo.clamp(f64::from(BPM_MIN) / 2.0, f64::from(BPM_MAX) * 2.0);
                    following = true;
                }
            }
        }
        let period = self.period(bpm, quarters);
        // Following the host: a stopped song is silent, and where it says it is decides the
        // next beat, so a jump or a loop is followed. Every block is placed afresh, which
        // costs nothing: the host's position is exact and a beat is never counted twice
        let mut waiting = false;
        if settings.on && following {
            if !settings.host_playing {
                waiting = true;
                self.was_on = false;
                // A stopped song is silent at once: a click still ringing is cut, not
                // finished after the stop
                self.voice = Voice::IDLE;
            } else if let Some(position) = settings.host_beats {
                let units = position / quarters;
                let whole = units.ceil();
                self.until_beat = ((whole - units) * period).max(0.0);
                self.next_beat = (whole as i64).rem_euclid(i64::from(beats)) as u32;
            }
        }
        if self.next_beat >= beats {
            self.next_beat %= beats;
        }

        let active = settings.on && !waiting;
        for index in 0..len {
            if active {
                if self.until_beat <= 0.0 {
                    let accent = self.next_beat == 0;
                    self.voice = Voice::start(settings.sound, accent, self.sample_rate);
                    self.reading.beat(accent);
                    self.next_beat = (self.next_beat + 1) % beats;
                    self.until_beat += period;
                }
                self.until_beat -= 1.0;
            }
            if self.voice.live() {
                emit(index, self.voice.next() * settings.gain);
            }
        }
    }
}

/// Tap tempo: the tempo of the last few taps. A pause of two seconds starts it over
#[derive(Default, Clone, Debug)]
pub struct TapTempo {
    taps: [f64; Self::KEPT],
    count: usize,
}

impl TapTempo {
    const KEPT: usize = 5;
    const PAUSE_S: f64 = 2.0;

    /// A tap at `now` seconds. Gives the tempo once there are two taps
    pub fn tap(&mut self, now: f64) -> Option<f32> {
        if self.count > 0 && now - self.taps[self.count - 1] > Self::PAUSE_S {
            self.count = 0;
        }
        if self.count == Self::KEPT {
            self.taps.copy_within(1.., 0);
            self.count -= 1;
        }
        self.taps[self.count] = now;
        self.count += 1;
        if self.count < 2 {
            return None;
        }
        let span = self.taps[self.count - 1] - self.taps[0];
        if span <= 0.0 {
            return None;
        }
        let bpm = 60.0 * (self.count - 1) as f64 / span;
        Some((bpm as f32).clamp(BPM_MIN, BPM_MAX))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Plays `blocks` of the lengths in `lengths` in turn, up to `total` samples, and gives the
    /// first sample of each click
    pub(crate) fn onsets(
        metronome: &mut Metronome,
        settings: &MetronomeSettings,
        total: usize,
        lengths: &[usize],
    ) -> (Vec<usize>, Vec<f32>) {
        let mut played = 0;
        let mut out = Vec::with_capacity(total);
        let mut block = 0;
        while played < total {
            let len = lengths[block % lengths.len()].min(total - played);
            let mut left = vec![0.0f32; len];
            metronome.process(settings, &mut left, None);
            out.extend_from_slice(&left);
            played += len;
            block += 1;
        }
        // A click starts at a sample that is not zero after a stretch of zeros
        let mut starts = Vec::new();
        let mut quiet = 0;
        for (index, &sample) in out.iter().enumerate() {
            if sample != 0.0 {
                if quiet >= 40 || starts.is_empty() {
                    starts.push(index);
                }
                quiet = 0;
            } else {
                quiet += 1;
            }
        }
        (starts, out)
    }

    fn running(bpm: f32) -> MetronomeSettings {
        MetronomeSettings { on: true, bpm, gain: 1.0, ..MetronomeSettings::default() }
    }

    #[test]
    fn test_clicks_fall_on_the_beat_to_the_sample_at_every_rate_and_tempo() {
        for sample_rate in [44_100.0f32, 48_000.0, 96_000.0] {
            for bpm in [40.0f32, 77.0, 120.0, 133.3, 187.0, 300.0] {
                let mut metronome = Metronome::new();
                metronome.set_sample_rate(sample_rate);
                let beats = 64usize;
                let period = 60.0 / f64::from(bpm) * f64::from(sample_rate);
                let total = (period * beats as f64) as usize - 100;
                // Odd block lengths: the beat must not depend on them
                let (starts, _) = onsets(&mut metronome, &running(bpm), total, &[512, 64, 333, 1]);
                assert_eq!(starts.len(), beats, "{bpm} bpm at {sample_rate}");
                for (beat, &start) in starts.iter().enumerate() {
                    let wanted = beat as f64 * period;
                    assert!(
                        (start as f64 - wanted).abs() <= 1.0,
                        "{bpm} bpm at {sample_rate}: beat {beat} at {start}, wanted {wanted}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_ten_minutes_do_not_drift() {
        // 123.4 bpm is not a whole number of samples per beat
        let mut metronome = Metronome::new();
        let settings = running(123.4);
        let sample_rate = 48_000.0f64;
        let period = 60.0 / 123.4 * sample_rate;
        let mut played = 0usize;
        let mut last = None;
        let mut beats = 0usize;
        let block = 480usize;
        let mut buffer = vec![0.0f32; block];
        let total = (10.0 * 60.0 * sample_rate) as usize;
        while played < total {
            buffer.fill(0.0);
            metronome.process(&settings, &mut buffer, None);
            for (i, &sample) in buffer.iter().enumerate() {
                if sample != 0.0 && last.is_none_or(|at: usize| played + i - at > 2000) {
                    let wanted = beats as f64 * period;
                    assert!(
                        ((played + i) as f64 - wanted).abs() <= 1.0,
                        "beat {beats} at {}, wanted {wanted}",
                        played + i
                    );
                    beats += 1;
                }
                if sample != 0.0 {
                    last = Some(played + i);
                }
            }
            played += block;
        }
        assert!(beats > 1200);
    }

    #[test]
    fn test_the_downbeat_is_louder_and_higher_in_every_signature() {
        for (signature, beats) in [
            (TimeSignature::TwoFour, 2),
            (TimeSignature::ThreeFour, 3),
            (TimeSignature::FourFour, 4),
            (TimeSignature::SixEight, 6),
        ] {
            for sound in [Sound::Woodblock, Sound::Beep, Sound::Click] {
                let settings =
                    MetronomeSettings { time_signature: signature, sound, ..running(120.0) };
                let mut metronome = Metronome::new();
                let (starts, out) = onsets(&mut metronome, &settings, 48_000 * 8, &[256]);
                // Two beats a second for eight seconds, twice as many in 6/8 (an eighth each)
                let wanted = (16.0 / signature.quarters_per_beat()) as usize;
                assert_eq!(starts.len(), wanted);
                let peak = |from: usize| {
                    out[from..from + 1000].iter().fold(0.0f32, |peak, s| peak.max(s.abs()))
                };
                for (index, &start) in starts.iter().enumerate() {
                    let accent = index % beats == 0;
                    let level = peak(start);
                    if accent {
                        assert!(level > 0.5, "{sound:?} {signature:?}: downbeat {level}");
                    } else {
                        assert!(
                            level < peak(starts[0]) * 0.75,
                            "{sound:?} {signature:?} beat {index}: {level}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_the_click_is_clean_and_never_clips() {
        for sound in [Sound::Woodblock, Sound::Beep, Sound::Click] {
            for sample_rate in [44_100.0f32, 96_000.0] {
                let mut metronome = Metronome::new();
                metronome.set_sample_rate(sample_rate);
                let settings = MetronomeSettings { sound, ..running(240.0) };
                let (starts, out) =
                    onsets(&mut metronome, &settings, sample_rate as usize * 3, &[128]);
                assert!(starts.len() >= 11);
                let peak = out.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
                assert!(peak > 0.5 && peak <= 1.0, "{sound:?}: peak {peak}");
                // A click ends, and does not end on a step: its last samples are tiny
                let from = starts[0];
                let tail =
                    &out[from + (0.15 * sample_rate) as usize..from + (0.2 * sample_rate) as usize];
                assert!(tail.iter().all(|s| s.abs() < 0.002), "{sound:?} rings on");
                // No DC and nothing jumps at its start: the first sample is small
                assert!(out[from].abs() < 0.6, "{sound:?}: starts at {}", out[from]);
                let mean = out[from..from + sample_rate as usize / 4].iter().sum::<f32>();
                assert!(mean.abs() < 25.0, "{sound:?}: mean {mean}");
            }
        }
    }

    #[test]
    fn test_off_is_silent_and_leaves_the_signal_alone() {
        let mut metronome = Metronome::new();
        let mut left: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.01).sin() * 0.3).collect();
        let before = left.clone();
        metronome.process(&MetronomeSettings::default(), &mut left, None);
        assert_eq!(left, before);
        assert!(metronome.is_idle());
    }

    #[test]
    fn test_switching_off_lets_the_click_finish_and_on_starts_a_downbeat() {
        let mut metronome = Metronome::new();
        let mut buffer = vec![0.0f32; 300];
        metronome.process(&running(120.0), &mut buffer, None);
        // Off in the middle of a click: it rings out, no new one
        let off = MetronomeSettings { on: false, ..running(120.0) };
        let mut buffer = vec![0.0f32; 48_000];
        metronome.process(&off, &mut buffer, None);
        assert!(buffer[..100].iter().any(|s| *s != 0.0));
        assert!(buffer[10_000..].iter().all(|s| *s == 0.0));
        assert!(metronome.is_idle());
        // On again from the middle of a bar: the first click is the downbeat, at once
        let reading = metronome.reading();
        let (count, _) = reading.read();
        let mut buffer = vec![0.0f32; 64];
        metronome.process(&running(120.0), &mut buffer, None);
        assert!(buffer[0] != 0.0);
        let (now, accent) = reading.read();
        assert_eq!((now, accent), (count.wrapping_add(1), true));
    }

    #[test]
    fn test_sync_follows_the_hosts_tempo_and_position() {
        let mut metronome = Metronome::new();
        let sample_rate = 48_000.0f64;
        // The song is at 90 bpm, a quarter and a half into it; the knob says 200
        let settings = MetronomeSettings {
            sync: true,
            host_tempo: Some(90.0),
            host_playing: true,
            host_beats: Some(1.5),
            ..running(200.0)
        };
        let mut buffer = vec![0.0f32; 96_000];
        metronome.process(&settings, &mut buffer, None);
        // The first click is on beat 2 (the song's third quarter), half a beat in
        let period = 60.0 / 90.0 * sample_rate;
        let first = buffer.iter().position(|s| *s != 0.0).unwrap();
        assert!((first as f64 - 0.5 * period).abs() <= 1.0, "{first}");
        // Beats 2, 3 and 0 of the bar follow: the first is not a downbeat, the third is
        let peak =
            |from: usize| buffer[from..from + 1000].iter().fold(0.0f32, |p, s| p.max(s.abs()));
        assert!(peak(first) < 0.75);
        let third = (2.5 * period) as usize;
        assert!(buffer[third..third + 1000].iter().any(|s| s.abs() > 0.9));
        assert!(metronome.reading().read().1);

        // On beat 4 of the song (position 4.0) the click is a downbeat, now
        let mut metronome = Metronome::new();
        let on_the_bar = MetronomeSettings { host_beats: Some(4.0), ..settings };
        let mut buffer = vec![0.0f32; 4800];
        metronome.process(&on_the_bar, &mut buffer, None);
        assert!(buffer[0] != 0.0);
        assert!(metronome.reading().read().1);
    }

    #[test]
    fn test_sync_is_silent_while_the_host_is_stopped_and_without_tempo_the_knob_rules() {
        let mut metronome = Metronome::new();
        let stopped = MetronomeSettings {
            sync: true,
            host_tempo: Some(100.0),
            host_playing: false,
            host_beats: Some(0.0),
            ..running(200.0)
        };
        let mut buffer = vec![0.0f32; 20_000];
        metronome.process(&stopped, &mut buffer, None);
        assert!(buffer.iter().all(|s| *s == 0.0));
        // Playing again starts on the song's position
        let playing = MetronomeSettings { host_playing: true, ..stopped };
        metronome.process(&playing, &mut buffer, None);
        assert!(buffer[0] != 0.0);

        // No tempo from the host: the knob's
        let mut metronome = Metronome::new();
        let no_tempo = MetronomeSettings { sync: true, ..running(120.0) };
        let (starts, _) = onsets(&mut metronome, &no_tempo, 48_000 * 3, &[512]);
        assert_eq!(starts.len(), 6);
        assert!((starts[1] as i64 - 24_000).abs() <= 1);
    }

    #[test]
    fn test_a_click_still_ringing_is_cut_when_the_synced_song_stops() {
        let mut metronome = Metronome::new();
        let playing = MetronomeSettings {
            sync: true,
            host_tempo: Some(100.0),
            host_playing: true,
            host_beats: Some(0.0),
            ..running(120.0)
        };
        let mut buffer = vec![0.0f32; 300];
        metronome.process(&playing, &mut buffer, None);
        assert!(buffer[299] != 0.0, "the click is still ringing");
        let stopped = MetronomeSettings { host_playing: false, ..playing };
        let mut buffer = vec![0.0f32; 4800];
        metronome.process(&stopped, &mut buffer, None);
        assert!(buffer.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn test_the_click_alone_is_what_the_mixed_click_adds() {
        let (mut mixed, mut alone) = (Metronome::new(), Metronome::new());
        let settings = MetronomeSettings { sound: Sound::Click, ..running(200.0) };
        for _ in 0..30 {
            let (mut left, mut right) = (vec![0.0f32; 777], vec![0.0f32; 777]);
            mixed.process(&settings, &mut left, Some(&mut right));
            let (mut own_left, mut own_right) = (vec![9.0f32; 777], vec![9.0f32; 777]);
            alone.process_alone(&settings, &mut own_left, Some(&mut own_right));
            assert_eq!((left, right), (own_left, own_right));
        }
    }

    #[test]
    fn test_a_tempo_change_is_taken_at_the_next_beat() {
        let mut metronome = Metronome::new();
        let (first, _) = onsets(&mut metronome, &running(60.0), 48_000, &[256]);
        assert_eq!(first, vec![0]);
        let (next, _) = onsets(&mut metronome, &running(240.0), 48_000, &[256]);
        // The beat that was due a second after the first falls where it was due
        assert!(next[0] <= 1, "{next:?}");
        assert!((next[1] as i64 - 12_000).abs() <= 1, "{next:?}");
    }

    #[test]
    fn test_volume_goes_from_silence_to_full() {
        assert_eq!(volume_gain(VOLUME_MIN_DB), 0.0);
        assert!((volume_gain(0.0) - 1.0).abs() < 1e-6);
        assert!((volume_gain(-12.0) - 0.2512).abs() < 1e-3);
        let mut metronome = Metronome::new();
        let quiet = MetronomeSettings { gain: volume_gain(-60.0), ..running(120.0) };
        let (_, out) = onsets(&mut metronome, &quiet, 48_000, &[256]);
        assert!(out.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn test_tap_tempo_reads_the_taps_and_starts_over_after_a_pause() {
        let mut tap = TapTempo::default();
        assert_eq!(tap.tap(10.0), None);
        let bpm = tap.tap(10.5).unwrap();
        assert!((bpm - 120.0).abs() < 0.01);
        tap.tap(11.0);
        let bpm = tap.tap(11.5).unwrap();
        assert!((bpm - 120.0).abs() < 0.01);
        // Slower taps move it, a pause forgets it
        assert_eq!(tap.tap(20.0), None);
        let bpm = tap.tap(21.0).unwrap();
        assert!((bpm - 60.0).abs() < 0.01);
        // Clamped to the range
        let mut tap = TapTempo::default();
        tap.tap(0.0);
        assert_eq!(tap.tap(0.05), Some(BPM_MAX));
        let mut tap = TapTempo::default();
        tap.tap(0.0);
        assert_eq!(tap.tap(1.9), Some(BPM_MIN));
        // Many taps keep the last few
        let mut tap = TapTempo::default();
        for beat in 0..20 {
            tap.tap(f64::from(beat) * 0.4);
        }
        assert!((tap.tap(8.0).unwrap() - 150.0).abs() < 0.01);
    }

    /// Writes WAV files to target/renders for listening: eight bars of each sound in 4/4 and
    /// six eighths in 6/8, at 100 bpm, at -12 dB and at 0 dB
    #[test]
    #[ignore]
    fn render_metronome() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/renders");
        std::fs::create_dir_all(&dir).unwrap();
        for (sound, name) in
            [(Sound::Woodblock, "woodblock"), (Sound::Beep, "beep"), (Sound::Click, "click")]
        {
            for (signature, bars, label) in
                [(TimeSignature::FourFour, 8, "4-4"), (TimeSignature::SixEight, 8, "6-8")]
            {
                for (db, level) in [(-12.0, "-12db"), (0.0, "0db")] {
                    let settings = MetronomeSettings {
                        sound,
                        time_signature: signature,
                        gain: volume_gain(db),
                        ..running(100.0)
                    };
                    let beats = bars * signature.beats() as usize;
                    let total =
                        (60.0 / 100.0 * signature.quarters_per_beat() * 48_000.0 * beats as f64)
                            as usize;
                    let mut metronome = Metronome::new();
                    let (_, out) = onsets(&mut metronome, &settings, total, &[512]);
                    let peak = out.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
                    println!("{name} {label} {level}: peak {:.1} dBFS", 20.0 * peak.log10());
                    let path = dir.join(format!("amp_metronome_{name}_{label}_{level}.wav"));
                    crate::test_util::write_wav(&path, &[&out], 48_000);
                }
            }
        }
        println!("Wrote renders to {}", dir.display());
    }
}
