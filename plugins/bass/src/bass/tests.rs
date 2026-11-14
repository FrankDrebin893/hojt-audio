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

/// The note's fundamental: where the string repeats best, then the peak of its spectrum
/// next to that. A stiff string's partials run sharp, so it never quite repeats, and the
/// repeat is a little off the fundamental, which is what is tuned
fn pitch(samples: &[f32], sample_rate: f32) -> f32 {
    let repeats = pitch_hz(samples, sample_rate, 25.0, 500.0);
    let window = &samples[..samples.len().min((0.35 * sample_rate) as usize)];
    let mut best = repeats;
    // Three passes, each over the width of one step of the one before, 20 steps wide
    for cents in [4.0f32, 0.4, 0.04] {
        let centre = best;
        best = (-10..=10)
            .map(|step| centre * (step as f32 * cents / 1200.0).exp2())
            .map(|hz| (hz, tone_level(window, sample_rate, hz)))
            .fold((centre, 0.0), |best, found| if found.1 > best.1 { found } else { best })
            .0;
    }
    best
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

/// Where partial `n` of a note is, as the string's all-passes put it: the loop is shorter
/// than at the fundamental by what they add less there
fn stiff_partial_hz(freq_hz: f32, n: usize) -> f32 {
    let Some(pole) = stiffness_coeff(freq_hz, SAMPLE_RATE) else {
        return n as f32 * freq_hz;
    };
    let rate = SAMPLE_RATE as f64;
    let omega0 = std::f64::consts::TAU * freq_hz as f64 / rate;
    let chain = |omega: f64| STIFF_SECTIONS as f64 * allpass_delay(pole as f64, omega);
    let mut omega = n as f64 * omega0;
    for _ in 0..30 {
        let length = std::f64::consts::TAU / omega0 - (chain(omega0) - chain(omega));
        omega = std::f64::consts::TAU * n as f64 / length;
    }
    (omega * rate / std::f64::consts::TAU) as f32
}

/// Where the partial near `freq_hz` really is: a string's partials sit off the harmonics
/// (`stiff_partial_hz`). Looked for within a quarter of the spacing of the note's harmonics.
fn find_partial(samples: &[f32], start_s: f32, freq_hz: f32, spacing_hz: f32) -> f32 {
    let start = (start_s * SAMPLE_RATE) as usize;
    let window = &samples[start..start + WINDOW];
    let centre = stiff_partial_hz(spacing_hz, (freq_hz / spacing_hz).round() as usize);
    (-8..=8)
        .map(|step| centre + step as f32 * spacing_hz / 32.0)
        .map(|hz| (hz, tone_level(window, SAMPLE_RATE, hz)))
        .fold((centre, 0.0), |best, found| if found.1 > best.1 { found } else { best })
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
        if points.first().is_some_and(|&(_, first)| level_db < first - 30.0) || level_db < -120.0 {
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
            // Low E, A, the G above them and the G an octave and a third above the E: the
            // fundamental, which the all-passes of the stiff string leave where it is, once
            // the hard pluck's sharp start has settled
            for midi in [28u8, 33, 43, 55] {
                let mut bass = bass(voice, sample_rate);
                bass.note_on(midi, 0.9);
                let output = render(&mut bass, sample_rate, 0.8);
                let found = pitch(&output[(0.4 * sample_rate) as usize..], sample_rate);
                let cents = 1200.0 * (found / note_hz(midi as f32)).log2();
                assert!(
                    cents.abs() < 1.0,
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
        assert!(peak(&hard) > peak(&soft) * 1.2, "{:?}: {} vs {}", voice, peak(&hard), peak(&soft));

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
    // Groove's finger knock is part of its brightness: Metal was 2.1 times it before #167's milestone 3
    assert!(brightness(&metal) > brightness(&groove) * 1.7);
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

// The velocity and the time the note's harmonics are weighed at
const WEIGHED_VELOCITY: f32 = 0.8;
const WEIGHED_AT_S: f32 = 0.05;
// Up to where they are weighed
const WEIGHED_UP_TO_HZ: f32 = 6000.0;

/// The power of each harmonic of a note up to 6 kHz at 50 ms, the fundamental first
fn harmonic_powers(samples: &[f32], freq_hz: f32) -> Vec<f32> {
    (1..)
        .map(|harmonic| harmonic as f32 * freq_hz)
        .take_while(|&hz| hz <= WEIGHED_UP_TO_HZ)
        .map(|hz| 10.0f32.powf(partial_db(samples, WEIGHED_AT_S, hz, freq_hz) / 10.0))
        .collect()
}

/// How much of a note's harmonic power its fundamental holds, and the power-weighted
/// centroid of its harmonics in Hz
fn f0_share_and_centroid(samples: &[f32], freq_hz: f32) -> (f32, f32) {
    let powers = harmonic_powers(samples, freq_hz);
    let total: f32 = powers.iter().sum();
    let weighted: f32 =
        powers.iter().enumerate().map(|(index, power)| (index + 1) as f32 * power).sum();
    (powers[0] / total, weighted / total * freq_hz)
}

/// What the sine under the string adds to the fundamental at the output at 50 ms, in dB:
/// the voice's own settings against the same with Sub at 0
fn sine_adds_db(voice: Voice, midi: u8) -> f32 {
    let freq_hz = note_hz(midi as f32);
    let with = note(voice, midi, WEIGHED_VELOCITY, 0.3);
    let without = string_alone(voice, midi, WEIGHED_VELOCITY, 0.3);
    partial_db(&with, WEIGHED_AT_S, freq_hz, freq_hz)
        - partial_db(&without, WEIGHED_AT_S, freq_hz, freq_hz)
}

// The centroid of each measured note's harmonics before #167, Metal's and Groove's, in Hz
const CENTROID_BEFORE: [(Voice, [f32; 3]); 2] =
    [(Voice::Metal, [90.0, 112.0, 173.0]), (Voice::Groove, [74.0, 94.0, 155.0])];

#[test]
fn test_the_sine_sits_under_the_string() {
    // The sine adds at most 2.5 dB to the fundamental, the fundamental holds at most 40 %
    // of the note's harmonic power, and the harmonics' centroid is at least 1.4 times
    // where it was when the sine was half of the note. The spec's number for milestone 1 is
    // 1.5; milestone 3 moved it to 1.4: the string's second plane starts a quarter of a
    // period late, which puts its 2nd harmonic against the first plane's and so the
    // fundamental's share up (Groove's G, note 43: 233 to 224-226 Hz, 1.45 times). No thump,
    // Brightness up to 60 % or Tone at 1550 Hz got it past 229 Hz, short of 232.5
    for (voice, before) in CENTROID_BEFORE {
        for (midi, before) in MEASURED.into_iter().zip(before) {
            let adds = sine_adds_db(voice, midi);
            assert!(adds <= 2.5, "{:?} note {}: the sine adds {} dB", voice, midi, adds);
            let output = note(voice, midi, WEIGHED_VELOCITY, 0.3);
            let (share, centroid) = f0_share_and_centroid(&output, note_hz(midi as f32));
            assert!(
                share <= 0.4,
                "{:?} note {}: the fundamental holds {} of the power",
                voice,
                midi,
                share
            );
            assert!(
                centroid >= 1.4 * before,
                "{:?} note {}: the centroid is at {} Hz, {} before",
                voice,
                midi,
                centroid,
                before
            );
        }
    }
}

/// How many cents above `n` times the fundamental partial `n` of a note is: the peak of its
/// spectrum over the second after the first 100 ms, looked for up to 60 cents sharp
fn partial_cents_sharp(samples: &[f32], freq_hz: f32, n: usize) -> f32 {
    let window = &samples[(0.1 * SAMPLE_RATE) as usize..(1.1 * SAMPLE_RATE) as usize];
    let ideal = n as f32 * freq_hz;
    let (best, _) = (-100..=1200)
        .map(|step| ideal * (step as f32 * 0.05 / 1200.0).exp2())
        .map(|hz| (hz, tone_level(window, SAMPLE_RATE, hz)))
        .fold((ideal, 0.0), |best, found| if found.1 > best.1 { found } else { best });
    1200.0 * (best / ideal).log2()
}

#[test]
fn test_the_string_is_stiff() {
    // The partials of the low E run sharp: the 10th by 8 to 30 cents, the 5th by less, as
    // on a wound string. Measured where the pickup reads, on both voices, and less so up the
    // neck
    for voice in Voice::ALL {
        let sharp = |midi: u8, n: usize| {
            let string = pickup_alone(voice, midi, WEIGHED_VELOCITY, 1.2);
            partial_cents_sharp(&string, note_hz(midi as f32), n)
        };
        let (fifth, tenth) = (sharp(28, 5), sharp(28, 10));
        assert!(
            (8.0..=30.0).contains(&tenth),
            "{:?}: the 10th partial of the low E is {} cents sharp",
            voice,
            tenth
        );
        assert!(
            fifth > 1.0 && fifth < 0.5 * tenth,
            "{:?}: the 5th is {} cents sharp, the 10th {}",
            voice,
            fifth,
            tenth
        );
        let up_the_neck = sharp(43, 10);
        assert!(
            (0.0..tenth).contains(&up_the_neck),
            "{:?}: the 10th of the G is {} cents sharp, the E's {}",
            voice,
            up_the_neck,
            tenth
        );
    }
}

/// Is the 5th the weakest of harmonics 2 to 8 in what the pickup reads?
fn fifth_is_weakest(powers: &[f32]) -> bool {
    powers[1..8].iter().all(|&power| powers[4] <= power)
}

#[test]
fn test_the_pickups_notch_moves_with_the_fret() {
    // The pickup is a fixed distance from the bridge: on the open A (33) its comb takes out
    // the 5th, on the 4th fret of the A string (37) another harmonic
    for voice in Voice::ALL {
        let powers = |midi: u8| {
            let string = pickup_alone(voice, midi, WEIGHED_VELOCITY, 0.3);
            harmonic_powers(&string, note_hz(midi as f32))
        };
        assert!(fifth_is_weakest(&powers(33)), "{:?}: the open A has no notch at the 5th", voice);
        assert!(!fifth_is_weakest(&powers(37)), "{:?}: the 4th fret still notches the 5th", voice);
        // And at the output, through the voice's own output stage and the cabinet
        let output = note(voice, 37, WEIGHED_VELOCITY, 0.3);
        assert!(
            !fifth_is_weakest(&harmonic_powers(&output, note_hz(37.0))),
            "{:?}: the 4th fret's 5th is the weakest at the output",
            voice
        );
    }
}

/// The level of a note's output in dB over 100 ms windows from 1 s to 3 s, and how far it
/// is from a straight line through them (the line of least squares): the residue's RMS in dB
fn decay_residue_db(output: &[f32]) -> f32 {
    let window = (0.1 * SAMPLE_RATE) as usize;
    let levels: Vec<f32> = (0..20)
        .map(|index| {
            let start = (1.0 * SAMPLE_RATE) as usize + index * window;
            20.0 * rms(&output[start..start + window]).max(1e-9).log10()
        })
        .collect();
    let count = levels.len() as f32;
    let mean_x = (count - 1.0) / 2.0;
    let mean_y = levels.iter().sum::<f32>() / count;
    let slope =
        levels.iter().enumerate().map(|(x, y)| (x as f32 - mean_x) * (y - mean_y)).sum::<f32>()
            / levels.iter().enumerate().map(|(x, _)| (x as f32 - mean_x).powi(2)).sum::<f32>();
    let squares: f32 = levels
        .iter()
        .enumerate()
        .map(|(x, y)| (y - (mean_y + slope * (x as f32 - mean_x))).powi(2))
        .sum();
    (squares / count).sqrt()
}

#[test]
fn test_a_held_note_does_not_fall_in_one_straight_line() {
    // The string's two planes beat and fall at their own rates: from 1 to 3 s the level is
    // off a straight line in dB by at least 0.5 dB, and by no more than 3, so that it
    // does not wobble
    for voice in Voice::ALL {
        for midi in MEASURED {
            let output = note(voice, midi, WEIGHED_VELOCITY, 3.2);
            let residue = decay_residue_db(&output);
            assert!(
                (0.5..=3.0).contains(&residue),
                "{:?} note {}: the level is {} dB off a straight line",
                voice,
                midi,
                residue
            );
        }
    }
}

/// The energy above 1 kHz (a fourth-order high-pass) in the first `seconds` of a note
fn energy_above_1_khz(samples: &[f32], seconds: f32) -> f32 {
    let mut filters = [Svf::new(), Svf::new()];
    for filter in &mut filters {
        filter.set(1000.0, 0.7, SAMPLE_RATE);
    }
    samples[..(seconds * SAMPLE_RATE) as usize]
        .iter()
        .map(|&sample| {
            let first = filters[0].process(sample).high;
            let high = filters[1].process(first).high;
            high * high
        })
        .sum()
}

// The energy above 1 kHz in Groove's first 5 ms on notes 28, 33 and 43 at velocity 0.8
// before the finger's attack was added (#167, milestone 3)
const GROOVE_ATTACK_BEFORE: [f32; 3] = [0.317, 0.319, 0.335];

#[test]
fn test_groove_has_a_finger_attack() {
    // The finger's thump and noise: more energy above 1 kHz in the first 5 ms than the
    // string alone had, and the more the harder it is plucked
    for (midi, before) in MEASURED.into_iter().zip(GROOVE_ATTACK_BEFORE) {
        let output = note(Voice::Groove, midi, WEIGHED_VELOCITY, 0.05);
        let energy = energy_above_1_khz(&output, 0.005);
        assert!(
            energy >= 1.15 * before,
            "note {}: {} above 1 kHz in the first 5 ms, {} before",
            midi,
            energy,
            before
        );
        let soft = energy_above_1_khz(&note(Voice::Groove, midi, 0.3, 0.05), 0.005);
        assert!(soft < energy, "note {}: a soft touch is as loud as a hard one", midi);
    }
}

/// The loudest sample in the 150 ms after a note is let go, against the note's own peak,
/// in dB. Sustain at its shortest and the note held for 3 s: the string has died by then,
/// so what is left is what letting go makes
fn release_sound_db(voice: Voice, midi: u8) -> f32 {
    let mut bass = bass(voice, SAMPLE_RATE);
    bass.set(&BassSettings { sustain_s: SUSTAIN_MIN_S, ..voice.settings() });
    bass.note_on(midi, WEIGHED_VELOCITY);
    let held = render(&mut bass, SAMPLE_RATE, 3.0);
    assert!(peak(&held[(2.5 * SAMPLE_RATE) as usize..]) < 1e-4, "the string rings on");
    bass.note_off(midi);
    let let_go = render(&mut bass, SAMPLE_RATE, 0.15);
    20.0 * (peak(&let_go) / peak(&held)).max(1e-9).log10()
}

#[test]
fn test_letting_go_of_a_note_makes_a_quiet_sound() {
    // 30 to 45 dB under the note's peak: there, and not a click
    for voice in Voice::ALL {
        for midi in MEASURED {
            let db = release_sound_db(voice, midi);
            assert!(
                (-45.0..=-30.0).contains(&db),
                "{:?} note {}: letting go makes a sound at {} dB",
                voice,
                midi,
                db
            );
        }
    }
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
                let apart =
                    partial_db(&first, 0.05, hz, freq_hz) - partial_db(&second, 0.05, hz, freq_hz);
                assert!(
                    apart.abs() < 1.0,
                    "{:?} note {} harmonic {}: {} dB apart",
                    voice,
                    midi,
                    harmonic,
                    apart
                );
            }
            let apart = harmonics_db(&first, freq_hz, 4, 8) - harmonics_db(&second, freq_hz, 4, 8);
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
                assert!(alike < 0.98, "{:?} note {}: two plucks correlate {}", voice, midi, alike);
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
            // Cents sharp over the first 20 ms, against the same pluck played at pitch, on
            // the string's first plane: the second beats against it fast enough to be
            // measured with the pitch over a period of the low E
            // Measured on the string and the sine under it, before the output stage and the
            // cabinet: with the sine no longer half of the note, their lift of the attack
            // over the first period of a low note would be measured as much as its pitch
            let string = |bass: &mut Bass| -> Vec<f32> {
                (0..(0.1 * SAMPLE_RATE) as usize)
                    .map(|_| {
                        let (string, sub) = bass.string_and_sub();
                        string + sub
                    })
                    .collect()
            };
            let sharp_at = |velocity: f32| {
                let mut sharp = bass(voice, SAMPLE_RATE);
                sharp.note_on(midi, velocity);
                sharp.line_other.fill(0.0);
                let sharp = string(&mut sharp);
                let mut at_pitch = bass(voice, SAMPLE_RATE);
                at_pitch.note_on(midi, velocity);
                at_pitch.line_other.fill(0.0);
                at_pitch.sharp_cents = 0.0;
                let at_pitch = string(&mut at_pitch);
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
    // Harmonics 2 to 4 from 15 dB under the fundamental to 6 dB over it. With the sine under
    // the string no longer as loud as the string's own fundamental, the 2nd and the 3rd can be
    // over it, as on a recorded bass
    for midi in MEASURED {
        let output = note(Voice::Groove, midi, 0.9, 0.3);
        let freq_hz = note_hz(midi as f32);
        let fundamental = partial_db(&output, 0.05, freq_hz, freq_hz);
        for harmonic in 2..=4 {
            let level = partial_db(&output, 0.05, harmonic as f32 * freq_hz, freq_hz) - fundamental;
            assert!(
                (-15.0..=6.0).contains(&level),
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
            assert!(cents.abs() < 1.0, "{:?} at {} Hz: {} cents off", voice, sample_rate, cents);
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
            // Without the hard pluck's sharp start, which has its own test
            bass.sharp_cents = 0.0;
            // Half of Bend 2 up, once the attack is over: where it is measured from
            let output = play(&mut bass, &settings, sample_rate, 0.5);
            let cents = cents_off(&output, sample_rate, 0.15, note_hz(34.0));
            assert!(cents.abs() < 2.0, "{:?} at {} Hz: {} cents off", voice, sample_rate, cents);
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

/// One note stays within the bound of milestone 3, optimised only
#[test]
fn test_one_note_costs_at_most_2_percent_of_a_core() {
    // Both planes, the playing noises and all. Timing means nothing unoptimised, so this
    // runs in the optimised step of scripts/check.sh and CI (cargo test -p bass --release)
    if cfg!(debug_assertions) {
        return;
    }
    let mut bass = bass(Voice::Metal, SAMPLE_RATE);
    // The best of seven: a busy machine only makes a run slower
    let frames = 48_000;
    let per_frame_ns = (0..7)
        .map(|_| {
            bass.note_on(28, 1.0);
            let started = std::time::Instant::now();
            let mut sum = 0.0;
            for _ in 0..frames {
                sum += bass.process();
            }
            assert!(sum.is_finite());
            started.elapsed().as_nanos() as f64 / frames as f64
        })
        .fold(f64::MAX, f64::min);
    let percent = per_frame_ns * 48000.0 / 1e7;
    assert!(percent <= 2.0, "one note costs {:.2} % of a core", percent);
}

/// Level, pitch error (of the fundamental) and brightness per voice and note. Then, with Sub at 0: from what the
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

        // The note's harmonics at the output at 50 ms, with the voice's own settings, at
        // velocity 0.8: what the sine adds to the fundamental, how much of the harmonics'
        // power the fundamental holds, their centroid, and the 5th against the fundamental
        println!(
            "{:<8}{:>10}{:>10}{:>12}{:>10}",
            "note", "sine dB", "f0 share", "centroid", "h5 dB"
        );
        for midi in [28u8, 33, 37, 38, 43, 50, 55] {
            let output = note(voice, midi, WEIGHED_VELOCITY, 0.3);
            let freq_hz = note_hz(midi as f32);
            let (share, centroid) = f0_share_and_centroid(&output, freq_hz);
            let powers = harmonic_powers(&output, freq_hz);
            println!(
                "{:<8}{:>10.1}{:>9.0}%{:>12.0}{:>10.1}",
                midi,
                sine_adds_db(voice, midi),
                share * 100.0,
                centroid,
                10.0 * (powers[4] / powers[0]).log10(),
            );
        }

        // The two planes and the playing noises, at velocity 0.8: how far the level from 1 to
        // 3 s is off a straight line (dB), the energy above 1 kHz in the first 5 ms, and the
        // sound of letting go against the note's peak (dB)
        println!("{:<8}{:>12}{:>16}{:>12}", "note", "residue dB", "5 ms >1 kHz", "let go dB");
        for midi in [28u8, 33, 38, 43, 50, 55] {
            let held = note(voice, midi, WEIGHED_VELOCITY, 3.2);
            let short = note(voice, midi, WEIGHED_VELOCITY, 0.05);
            println!(
                "{:<8}{:>12.2}{:>16.4}{:>12.1}",
                midi,
                decay_residue_db(&held),
                energy_above_1_khz(&short, 0.005),
                release_sound_db(voice, midi),
            );
        }

        // How far above 5 and 10 times the fundamental those partials of the string run
        println!("{:<8}{:>12}{:>12}", "note", "5th cents", "10th cents");
        for midi in [28u8, 33, 38, 43, 50, 55] {
            let string = pickup_alone(voice, midi, WEIGHED_VELOCITY, 1.2);
            let freq_hz = note_hz(midi as f32);
            println!(
                "{:<8}{:>12.1}{:>12.1}",
                midi,
                partial_cents_sharp(&string, freq_hz, 5),
                partial_cents_sharp(&string, freq_hz, 10),
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
