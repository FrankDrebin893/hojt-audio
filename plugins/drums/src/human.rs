//! Human: a small random change in loudness and a small delay on every hit, so a part drawn
//! on a grid stops sounding like one.
//!
//! Events go in with the sample they were sent for and come out at the sample they are due:
//! the same sample or later, never earlier, so nothing looks ahead and latency stays zero.
//! One draw is made per instant, and every event of that instant shares it (a kick and a
//! crash that arrive together stay one hit). An event never comes out before one that went in
//! ahead of it, so a cymbal's note-off still follows its note-on and a pedal move still lands
//! before the hat it was meant for; the queue is therefore a plain first-in first-out ring.

/// Events that can wait at once. Made in `new`, never resized
pub const CAPACITY: usize = 256;
/// The longest a hit is delayed, at Human 100 %
pub const MAX_DELAY_S: f32 = 0.008;
/// How far a hit's velocity is moved at most, at Human 100 %: a fraction of the velocity
pub const VELOCITY_SPREAD: f32 = 0.15;
/// The generator's seed, the same after every `reset`: a render repeats
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// What the plugin hands the kit
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    NoteOn {
        note: u8,
        velocity: f32,
    },
    NoteOff {
        note: u8,
    },
    /// The hi-hat pedal: 0.0 shut, 1.0 open
    Pedal {
        openness: f32,
    },
}

/// One instant's draw
#[derive(Clone, Copy, Debug)]
pub struct Draw {
    /// How much of the longest delay, 0.0 to 1.0, mostly small
    pub delay: f32,
    /// Where in the velocity spread, -1.0 to 1.0, bell-shaped around 0
    pub spread: f32,
}

pub struct Humanizer {
    state: u64,
    sample_rate: f32,
    queue: [(u64, Event); CAPACITY],
    head: usize,
    len: usize,
    // The instant (in samples) the last event came for, with its draw
    instant: Option<(u64, Draw)>,
    // When the event that went in last is due: the next is never due before it
    last_due: u64,
}

impl Humanizer {
    pub fn new() -> Self {
        Self {
            state: SEED,
            sample_rate: 48_000.0,
            queue: [(0, Event::NoteOff { note: 0 }); CAPACITY],
            head: 0,
            len: 0,
            instant: None,
            last_due: 0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    /// Empties the queue and takes the seed again: the same events give the same hits
    pub fn reset(&mut self) {
        self.state = SEED;
        self.head = 0;
        self.len = 0;
        self.instant = None;
        self.last_due = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// SplitMix64, 24 bits of it as 0.0 to under 1.0
    fn uniform(&mut self) -> f32 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }

    fn draw(&mut self) -> Draw {
        let u = self.uniform();
        // Squared: most hits are a little late, a few are later
        let delay = u * u;
        // Three uniforms added: a bell that stays between -1 and 1
        let spread = (self.uniform() + self.uniform() + self.uniform() - 1.5) / 1.5;
        Draw { delay, spread }
    }

    /// An event for the sample `at` (counted from `reset`), with Human at `amount` (0.0 to
    /// 1.0). The event is kept for `pop_due`. If there is no room, the oldest waiting event is
    /// returned instead, to be played at once at sample `at`, and the new one takes its place:
    /// nothing is dropped and nothing is played ahead of an older event
    pub fn push(&mut self, at: u64, event: Event, amount: f32) -> Option<(u64, Event)> {
        let draw = match self.instant {
            Some((instant, draw)) if instant == at => draw,
            _ => {
                let draw = self.draw();
                self.instant = Some((at, draw));
                draw
            }
        };
        let event = match event {
            Event::NoteOn { note, velocity } => {
                let factor = 1.0 + VELOCITY_SPREAD * amount * draw.spread;
                Event::NoteOn { note, velocity: (velocity * factor).min(1.0) }
            }
            other => other,
        };
        let delay = (draw.delay * amount * MAX_DELAY_S * self.sample_rate).round() as u64;
        let due = (at + delay).max(self.last_due);
        let flushed = if self.len == CAPACITY {
            let (_, oldest) = self.queue[self.head];
            self.head = (self.head + 1) % CAPACITY;
            self.len -= 1;
            Some((at, oldest))
        } else {
            None
        };
        self.last_due = due;
        self.queue[(self.head + self.len) % CAPACITY] = (due, event);
        self.len += 1;
        flushed
    }

    /// The oldest event if it is due at sample `now` or before
    pub fn pop_due(&mut self, now: u64) -> Option<(u64, Event)> {
        if self.len == 0 || self.queue[self.head].0 > now {
            return None;
        }
        let entry = self.queue[self.head];
        self.head = (self.head + 1) % CAPACITY;
        self.len -= 1;
        Some(entry)
    }
}

impl Default for Humanizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn on(note: u8) -> Event {
        Event::NoteOn { note, velocity: 0.5 }
    }

    fn velocity(event: Event) -> f32 {
        match event {
            Event::NoteOn { velocity, .. } => velocity,
            other => panic!("not a note-on: {other:?}"),
        }
    }

    /// Everything that is waiting, in the order it comes out, with its due sample
    fn drain(humanizer: &mut Humanizer) -> Vec<(u64, Event)> {
        let mut out = Vec::new();
        while let Some(entry) = humanizer.pop_due(u64::MAX) {
            out.push(entry);
        }
        out
    }

    #[test]
    fn test_every_delay_is_between_0_and_the_longest_and_scales_with_the_knob() {
        for amount in [0.25, 0.5, 1.0] {
            let mut humanizer = Humanizer::new();
            let max = (MAX_DELAY_S * amount * RATE).round() as u64;
            let mut longest = 0;
            for i in 0..5000u64 {
                // Far enough apart that the order rule never moves a due time
                let at = i * 1000;
                assert!(humanizer.push(at, on(36), amount).is_none());
                let (due, _) = humanizer.pop_due(u64::MAX).unwrap();
                assert!(due >= at, "early");
                assert!(due - at <= max, "{} late at {amount}", due - at);
                longest = longest.max(due - at);
            }
            assert!(longest as f32 > max as f32 * 0.9, "the range is not used at {amount}");
        }
    }

    #[test]
    fn test_the_delay_favours_small_values() {
        let mut humanizer = Humanizer::new();
        let max = (MAX_DELAY_S * RATE) as u64;
        let mut under_a_quarter = 0;
        for i in 0..4000u64 {
            humanizer.push(i * 1000, on(36), 1.0);
            let (due, _) = humanizer.pop_due(u64::MAX).unwrap();
            under_a_quarter += (due - i * 1000 < max / 4) as usize;
        }
        // Squared draw: half of the hits are in the first quarter of the range
        assert!((1800..2200).contains(&under_a_quarter), "{under_a_quarter} of 4000");
    }

    #[test]
    fn test_events_at_the_same_instant_share_one_delay() {
        let mut humanizer = Humanizer::new();
        for i in 0..200u64 {
            let at = 500 + i * 2000;
            humanizer.push(at, on(36), 1.0);
            humanizer.push(at, on(49), 1.0);
            humanizer.push(at, Event::Pedal { openness: 1.0 }, 1.0);
            let out = drain(&mut humanizer);
            assert_eq!(out.len(), 3);
            assert!(out.iter().all(|(due, _)| *due == out[0].0), "the instant was split");
            // And the same shade of velocity
            assert_eq!(velocity(out[0].1), velocity(out[1].1));
        }
    }

    #[test]
    fn test_velocity_is_inside_its_bounds_and_spread_as_aimed() {
        let mut humanizer = Humanizer::new();
        let mut factors = Vec::new();
        for i in 0..20_000u64 {
            humanizer.push(i * 1000, Event::NoteOn { note: 38, velocity: 0.5 }, 1.0);
            let (_, event) = humanizer.pop_due(u64::MAX).unwrap();
            factors.push(velocity(event) / 0.5);
        }
        let low = factors.iter().cloned().fold(f32::MAX, f32::min);
        let high = factors.iter().cloned().fold(f32::MIN, f32::max);
        assert!(low >= 1.0 - VELOCITY_SPREAD - 1e-4 && high <= 1.0 + VELOCITY_SPREAD + 1e-4);
        assert!(low < 0.9 && high > 1.1, "the spread is not used: {low}..{high}");
        let mean = factors.iter().sum::<f32>() / factors.len() as f32;
        let std = (factors.iter().map(|f| (f - mean) * (f - mean)).sum::<f32>()
            / factors.len() as f32)
            .sqrt();
        // Three uniforms added: a standard deviation of a third of the range, 15 % / 3
        let target = VELOCITY_SPREAD / 3.0;
        assert!((std - target).abs() < target * 0.1, "std {std}, aimed at {target}");
        assert!((mean - 1.0).abs() < 0.005, "the hits are louder or softer on the whole: {mean}");
        // Not the same twice in a row
        assert!(factors.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn test_velocity_never_goes_over_1_or_to_nothing() {
        let mut humanizer = Humanizer::new();
        for i in 0..5000u64 {
            humanizer.push(i * 1000, Event::NoteOn { note: 38, velocity: 1.0 }, 1.0);
            humanizer.push(i * 1000 + 1, Event::NoteOn { note: 38, velocity: 0.01 }, 1.0);
        }
        for (_, event) in drain(&mut humanizer) {
            let velocity = velocity(event);
            assert!(velocity > 0.0 && velocity <= 1.0);
        }
    }

    #[test]
    fn test_nothing_comes_out_ahead_of_what_went_in_before_it() {
        // A note-on delayed the longest, and its note-off a sample later with a draw of 0:
        // the off must not come first. Many events, close together and far apart
        let mut humanizer = Humanizer::new();
        let mut at = 0u64;
        let mut sent = Vec::new();
        for i in 0..3000u32 {
            at += [1, 2, 7, 300, 5000][(i % 5) as usize];
            let event = Event::NoteOff { note: (i % 100) as u8 };
            sent.push(event);
            // Taken as it goes, as the plugin does, so the queue stays short
            assert!(humanizer.push(at, event, 1.0).is_none());
            if i % 3 == 0 {
                let out = drain(&mut humanizer);
                for pair in out.windows(2) {
                    assert!(pair[0].0 <= pair[1].0, "due times out of order");
                }
            }
        }
        let mut humanizer = Humanizer::new();
        for (i, event) in sent.iter().take(200).enumerate() {
            humanizer.push(i as u64 * 3, *event, 1.0);
        }
        let out = drain(&mut humanizer);
        let order: Vec<Event> = out.iter().map(|(_, event)| *event).collect();
        assert_eq!(order, sent[..200].to_vec(), "the order changed");
        for pair in out.windows(2) {
            assert!(pair[0].0 <= pair[1].0);
        }
    }

    #[test]
    fn test_a_full_queue_flushes_the_oldest_first_and_drops_nothing() {
        let mut humanizer = Humanizer::new();
        // A crash is struck and its off follows, with the queue already full behind them:
        // the on must reach the kit before the off
        let mut played = Vec::new();
        let mut sent = vec![on(49), Event::NoteOff { note: 49 }];
        for i in 0..CAPACITY + 50 {
            if i >= 2 {
                sent.push(on((i % 40) as u8));
            }
        }
        for event in &sent {
            if let Some((at, flushed)) = humanizer.push(100, *event, 1.0) {
                assert_eq!(at, 100, "not played at the sample it came for");
                played.push(flushed);
            }
        }
        played.extend(drain(&mut humanizer).into_iter().map(|(_, event)| event));
        // Velocities are shaded, so compare what was played by kind and note
        let kind = |event: &Event| match event {
            Event::NoteOn { note, .. } => (true, *note),
            Event::NoteOff { note } => (false, *note),
            Event::Pedal { .. } => unreachable!(),
        };
        let played: Vec<_> = played.iter().map(kind).collect();
        let sent: Vec<_> = sent.iter().map(kind).collect();
        assert_eq!(played, sent, "something was dropped or came out of order");
        assert_eq!(played[..2], [(true, 49), (false, 49)]);
    }

    #[test]
    fn test_reset_empties_the_queue_and_takes_the_seed_again() {
        let mut humanizer = Humanizer::new();
        let run = |humanizer: &mut Humanizer| {
            for i in 0..50u64 {
                humanizer.push(i * 500, on(38), 1.0);
            }
            drain(humanizer)
        };
        let first = run(&mut humanizer);
        humanizer.push(10, on(36), 1.0);
        humanizer.reset();
        assert!(humanizer.is_empty());
        assert_eq!(run(&mut humanizer), first);
    }

    #[test]
    fn test_a_part_on_the_grid_does_not_stay_on_it() {
        let mut humanizer = Humanizer::new();
        let mut delays = std::collections::HashSet::new();
        let mut velocities = std::collections::HashSet::new();
        for i in 0..32u64 {
            humanizer.push(i * 12_000, Event::NoteOn { note: 42, velocity: 0.7 }, 1.0);
            let (due, event) = humanizer.pop_due(u64::MAX).unwrap();
            delays.insert(due - i * 12_000);
            velocities.insert(velocity(event).to_bits());
        }
        assert!(delays.len() > 16 && velocities.len() == 32);
    }
}
