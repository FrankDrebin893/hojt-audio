//! Tracks 2 and 3 of the looper (#452): three tracks that play together from one position.

use super::*;

const RATE: f32 = 48_000.0;
const FADE: usize = 240;
/// The loop's length in these tests, in samples
const LEN: usize = 2400;

fn tone(at: usize, hz: f32, amp: f32) -> f32 {
    amp * (std::f32::consts::TAU * hz * at as f32 / RATE).sin()
}

/// A looper that holds one second, with a loop of `LEN` samples of a 330 Hz tone playing in it
fn with_loop() -> Looper {
    let mut looper = Looper::with_seconds(1.0);
    let control = looper.control();
    control.press();
    let mut left: Vec<f32> = (0..LEN).map(|i| tone(i, 330.0, 0.3)).collect();
    looper.process(&LooperSettings::default(), &mut left, None);
    control.press();
    looper.process(&LooperSettings::default(), &mut [0.0; 1], None);
    assert_eq!(looper.state(), State::Playing);
    assert_eq!(looper.len, LEN);
    looper
}

/// `blocks` of `size` samples of the input `live(position of the loop at that sample)`, mono; what
/// comes out is returned
fn run(
    looper: &mut Looper,
    settings: &LooperSettings,
    blocks: usize,
    size: usize,
    live: impl Fn(usize) -> f32,
) -> Vec<f32> {
    let mut out = Vec::new();
    for _ in 0..blocks {
        let pos = looper.pos;
        let mut block: Vec<f32> = (0..size).map(|i| live((pos + i) % LEN)).collect();
        looper.process(settings, &mut block, None);
        out.extend(block);
    }
    out
}

/// Plays the loop silent until it is about to start again, so a press lands before the wrap
fn to_the_start(looper: &mut Looper) {
    let settings = LooperSettings::default();
    let left = (LEN - looper.pos) % LEN;
    if left > 0 {
        run(looper, &settings, 1, left, |_| 0.0);
    }
    assert_eq!(looper.pos, 0);
}

fn heard_alone(looper: &Looper, track: usize, at: usize) -> f32 {
    match track {
        0 => looper.left[at],
        t => looper.extras[t - 1].left[at],
    }
}

#[test]
fn test_a_track_two_press_waits_for_the_loop_to_come_round_and_records_one_pass() {
    let mut looper = with_loop();
    let control = looper.control();
    let settings = LooperSettings::default();
    // Part way round: the press waits, the sound passes with the loop
    run(&mut looper, &settings, 1, 700, |_| 0.0);
    control.press_track(1);
    let before = looper.pos;
    run(&mut looper, &settings, 1, 100, |_| 0.2);
    assert!(control.track_waiting(1));
    assert_eq!(control.track_state(1), State::Empty);
    assert_eq!(looper.extras[0].state, State::Empty);
    // The wrap: it records from the first sample, one pass, and plays by itself after it
    let to_wrap = LEN - (before + 100);
    run(&mut looper, &settings, 1, to_wrap, |_| 0.2);
    assert_eq!(looper.pos, 0);
    run(&mut looper, &settings, 1, LEN - 1, |_| 0.2);
    assert_eq!(control.track_state(1), State::Recording);
    assert!(!control.track_waiting(1));
    run(&mut looper, &settings, 1, 1, |_| 0.2);
    assert_eq!(control.track_state(1), State::Playing);
    assert_eq!(looper.pos, 0);
    // In the middle of the pass the track holds what was played, at full level
    assert!((looper.extras[0].left[LEN / 2] - 0.2).abs() < 1e-6);
    // The ends are faded in and out, so the seam is silent
    assert_eq!(looper.extras[0].left[0], 0.0);
    assert!(looper.extras[0].left[LEN - 1].abs() < 0.2 * 2.0 / FADE as f32);
}

#[test]
fn test_a_second_press_while_waiting_takes_the_wait_back() {
    let mut looper = with_loop();
    let control = looper.control();
    let settings = LooperSettings::default();
    control.press_track(1);
    run(&mut looper, &settings, 1, 100, |_| 0.0);
    assert!(control.track_waiting(1));
    control.press_track(1);
    run(&mut looper, &settings, 1, LEN, |_| 0.0);
    assert!(!control.track_waiting(1));
    assert_eq!(control.track_state(1), State::Empty);
}

#[test]
fn test_a_press_during_the_pass_closes_it_early_with_the_rest_silent() {
    let mut looper = with_loop();
    let control = looper.control();
    let settings = LooperSettings::default();
    // Stale sound in the buffer must not be heard after the early close
    looper.extras[0].left.fill(0.9);
    looper.extras[0].right.fill(0.9);
    to_the_start(&mut looper);
    control.press_track(1);
    run(&mut looper, &settings, 1, 1000, |_| 0.2);
    assert_eq!(control.track_state(1), State::Recording);
    control.press_track(1);
    run(&mut looper, &settings, 1, 10, |_| 0.2);
    assert_eq!(control.track_state(1), State::Playing);
    let track = &looper.extras[0].left;
    assert!(track[1000..LEN].iter().all(|x| *x == 0.0));
    assert!((track[500] - 0.2).abs() < 1e-6);
    assert!(track[999].abs() < 0.2 * 2.0 / FADE as f32);
    // The next pass plays the played part and nothing else of it
    let pos = looper.pos;
    let out = run(&mut looper, &settings, 1, LEN, |_| 0.0);
    for (i, got) in out.iter().enumerate() {
        let at = (pos + i) % LEN;
        let expected = looper.left[at] + looper.extras[0].left[at];
        assert!((got - expected).abs() < 1e-5, "{at}");
    }
}

#[test]
fn test_a_pass_under_ten_milliseconds_is_thrown_away() {
    let mut looper = with_loop();
    let control = looper.control();
    to_the_start(&mut looper);
    control.press_track(1);
    run(&mut looper, &LooperSettings::default(), 1, 100, |_| 0.2);
    control.press_track(1);
    run(&mut looper, &LooperSettings::default(), 1, 10, |_| 0.2);
    assert_eq!(control.track_state(1), State::Empty);
}

#[test]
fn test_tracks_do_nothing_while_track_one_has_no_loop() {
    let mut looper = Looper::with_seconds(1.0);
    let control = looper.control();
    control.press_track(1);
    control.press_track(2);
    run(&mut looper, &LooperSettings::default(), 2, 100, |_| 0.1);
    assert!(!control.track_waiting(1) && !control.track_waiting(2));
    assert!(looper.is_idle());
    // Nor while it records
    control.press();
    run(&mut looper, &LooperSettings::default(), 1, 100, |_| 0.1);
    control.press_track(1);
    run(&mut looper, &LooperSettings::default(), 1, 100, |_| 0.1);
    assert!(!control.track_waiting(1));
}

/// Three tracks recorded in turn, each a pass of a tone of its own at its own position
fn three_tracks() -> Looper {
    let mut looper = with_loop();
    let control = looper.control();
    let settings = LooperSettings::default();
    for (track, hz) in [(1, 440.0), (2, 550.0)] {
        to_the_start(&mut looper);
        control.press_track(track);
        run(&mut looper, &settings, 1, LEN, |at| tone(at, hz, 0.2));
        assert_eq!(control.track_state(track), State::Playing);
    }
    looper
}

#[test]
fn test_three_tracks_come_out_as_the_sum_of_the_three_in_time_for_a_thousand_passes() {
    let mut looper = three_tracks();
    let settings = LooperSettings::default();
    // Each track recorded its tone against the loop's position
    for at in [FADE, 700, 1200, LEN - FADE - 1] {
        assert!((heard_alone(&looper, 1, at) - tone(at, 440.0, 0.2)).abs() < 1e-5, "{at}");
        assert!((heard_alone(&looper, 2, at) - tone(at, 550.0, 0.2)).abs() < 1e-5, "{at}");
    }
    let mut heard = Vec::new();
    let sizes = [64usize, 333, 512, 480];
    let mut size = 0;
    let first_pos = looper.pos;
    while heard.len() < LEN * 1000 {
        let n = sizes[size % sizes.len()];
        size += 1;
        heard.extend(run(&mut looper, &settings, 1, n, |_| 0.0));
    }
    // Every sample is the three tracks added, and a thousand passes later it is the same
    for (k, got) in heard.iter().enumerate().step_by(7) {
        let at = (first_pos + k) % LEN;
        let sum = looper.left[at] + looper.extras[0].left[at] + looper.extras[1].left[at];
        assert!((got - sum).abs() < 1e-5, "sample {k}");
    }
    let from = LEN - first_pos;
    let first_pass: Vec<f32> = heard[from..from + LEN].to_vec();
    for pass in 1..990 {
        let at = from + pass * LEN;
        assert_eq!(&heard[at..at + LEN], &first_pass[..], "pass {pass}");
    }
}

#[test]
fn test_mute_overdub_and_level_act_on_one_track_only() {
    let mut looper = three_tracks();
    let control = looper.control();
    let settings = LooperSettings::default();
    let sum_of = |looper: &Looper, tracks: &[usize], at: usize| -> f32 {
        tracks.iter().map(|t| heard_alone(looper, *t, at)).sum()
    };
    // Mute track 2: the others go on
    control.stop_track(1);
    run(&mut looper, &settings, 1, 2000, |_| 0.0);
    assert_eq!(control.track_state(1), State::Stopped);
    assert_eq!(control.track_state(0), State::Playing);
    assert_eq!(control.track_state(2), State::Playing);
    let pos = looper.pos;
    let out = run(&mut looper, &settings, 1, 500, |_| 0.0);
    for (i, got) in out.iter().enumerate() {
        let at = (pos + i) % LEN;
        assert!((got - sum_of(&looper, &[0, 2], at)).abs() < 1e-5);
    }
    // And it comes back in time
    control.stop_track(1);
    run(&mut looper, &settings, 1, 2000, |_| 0.0);
    let pos = looper.pos;
    let out = run(&mut looper, &settings, 1, 500, |_| 0.0);
    for (i, got) in out.iter().enumerate() {
        let at = (pos + i) % LEN;
        assert!((got - sum_of(&looper, &[0, 1, 2], at)).abs() < 1e-5);
    }
    // Overdub track 3 with a tone: only its buffer changes
    let (one, two) = (looper.left.clone(), looper.extras[0].left.clone());
    let three = looper.extras[1].left.clone();
    control.press_track(2);
    run(&mut looper, &settings, 1, 2 * LEN, |at| tone(at, 1000.0, 0.1));
    control.press_track(2);
    run(&mut looper, &settings, 1, LEN, |_| 0.0);
    assert_eq!(control.track_state(2), State::Playing);
    assert_eq!(looper.left, one);
    assert_eq!(looper.extras[0].left, two);
    assert_ne!(looper.extras[1].left, three);
    // Level: track 1 at -60 dB is silent, the others are not touched
    let quiet = LooperSettings { level: level_gain(-60.0), ..settings };
    run(&mut looper, &quiet, 1, 4 * LEN, |_| 0.0);
    let pos = looper.pos;
    let out = run(&mut looper, &quiet, 1, 500, |_| 0.0);
    for (i, got) in out.iter().enumerate() {
        let at = (pos + i) % LEN;
        assert!((got - sum_of(&looper, &[1, 2], at)).abs() < 1e-5);
    }
}

#[test]
fn test_clearing_a_track_leaves_the_others_and_clearing_track_one_clears_all() {
    let mut looper = three_tracks();
    let control = looper.control();
    let settings = LooperSettings::default();
    control.clear_track(1);
    run(&mut looper, &settings, 1, 1000, |_| 0.0);
    assert_eq!(control.track_state(1), State::Empty);
    assert_eq!(control.track_state(0), State::Playing);
    assert_eq!(control.track_state(2), State::Playing);
    control.clear();
    let out = run(&mut looper, &settings, 1, 2 * LEN, |_| 0.0);
    assert_eq!(control.track_state(0), State::Empty);
    assert_eq!(control.track_state(1), State::Empty);
    assert_eq!(control.track_state(2), State::Empty);
    assert!(looper.is_idle());
    // The sound is back to untouched, and a track needs a new loop to record on
    assert!(out[out.len() - 100..].iter().all(|x| *x == 0.0));
    let mut block = [0.3f32; 64];
    looper.process(&settings, &mut block, None);
    assert_eq!(block, [0.3; 64]);
}

#[test]
fn test_the_sum_of_three_loud_overdubs_cannot_run_away() {
    let mut looper = three_tracks();
    let control = looper.control();
    let settings = LooperSettings { extra_levels: [level_gain(6.0); 2], ..Default::default() };
    for track in 0..3 {
        control.press_track(track);
    }
    let out = run(&mut looper, &settings, 1, 20 * LEN, |at| tone(at, 200.0, 0.95));
    assert!(out.iter().all(|x| x.abs() <= 1.0));
    for track in 0..3 {
        let loud = (0..LEN).map(|at| heard_alone(&looper, track, at).abs()).fold(0.0, f32::max);
        assert!(loud < 1.0, "track {track}: {loud}");
    }
}

#[test]
fn test_a_change_of_sample_rate_empties_all_three_tracks() {
    let mut looper = three_tracks();
    let control = looper.control();
    looper.set_sample_rate(44_100.0);
    for track in 0..3 {
        assert_eq!(control.track_state(track), State::Empty);
        assert!(!control.track_waiting(track));
    }
    assert!(looper.is_idle());
}

#[test]
fn test_the_click_counts_a_track_that_records_as_recording() {
    let mut looper = with_loop();
    let control = looper.control();
    assert!(!control.extra_recording());
    control.press_track(2);
    run(&mut looper, &LooperSettings::default(), 1, 100, |_| 0.0);
    assert!(control.extra_recording(), "waiting counts");
}

#[test]
fn test_the_tuner_silences_all_tracks() {
    let mut looper = three_tracks();
    let settings = LooperSettings { muted: true, ..Default::default() };
    run(&mut looper, &settings, 1, 2000, |_| 0.0);
    let out = run(&mut looper, &settings, 1, 500, |_| 0.0);
    assert!(out.iter().all(|x| *x == 0.0));
}
