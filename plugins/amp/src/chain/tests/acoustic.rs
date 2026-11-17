//! Træ, the amp that makes an electric guitar sound acoustic: what holds it to being clear,
//! and the report its constants were tuned with

use super::*;

/// Two notes a major third apart held together, as loud together as `level_db`
fn two_held_notes(level_db: f32) -> Vec<f32> {
    let len = 2 * SAMPLE_RATE as usize;
    let (low, high) = (sine(196.0, 1.0, SAMPLE_RATE, len), sine(247.0, 1.0, SAMPLE_RATE, len));
    let both: Vec<f32> = low.iter().zip(&high).map(|(a, b)| a + b).collect();
    at_rms_db(&both, level_db)
}

/// What an amp adds between two held notes and their harmonics, in dB below the whole: the
/// sum and difference tones a stage makes when it bends, which is what blurs a chord
fn blur_db(settings: &AmpSettings, level_db: f32) -> f64 {
    let output = run(settings, &two_held_notes(level_db), SAMPLE_RATE);
    let mut partials = harmonics_of(196.0, SAMPLE_RATE);
    partials.extend(harmonics_of(247.0, SAMPLE_RATE));
    fit_partials(&output[SAMPLE_RATE as usize..], SAMPLE_RATE, &partials).1
}

fn trae(gain: f32) -> AmpSettings {
    AmpSettings { gate_on: false, ..with_amp(Amp::Trae, gain) }
}

#[test]
fn test_trae_keeps_a_chord_clear() {
    // Two notes held together come out as those two notes, with nothing between them: at
    // the level a guitar is recorded at up to Gain 7.5, and 2 dB over it up to Gain 2.5.
    // Above that the stages and the power stage start to round off, which is what keeps
    // pick attacks under the output's safety clip at the loudness every amp is matched to
    for (gain, hotter_db) in [(0.0, 2.0), (0.25, 2.0), (0.5, 0.0), (0.75, 0.0)] {
        for level_db in [REFERENCE_DI_DB, REFERENCE_DI_DB + hotter_db] {
            let blur = blur_db(&trae(gain), level_db);
            assert!(blur < -60.0, "Gain {gain} at {level_db} dBFS RMS: {blur:.1} dB");
        }
    }
    // Which no other amp does: Klar, the cleanest of them, has bent the chord by then
    let klar = AmpSettings { gate_on: false, ..with_amp(Amp::Klar, 0.5) };
    let (ours, klars) = (blur_db(&trae(0.5), REFERENCE_DI_DB), blur_db(&klar, REFERENCE_DI_DB));
    assert!(ours < klars - 40.0, "Træ {ours:.1} dB, Klar {klars:.1} dB");
    // And a single held note has no distortion to speak of
    let held = sine(220.0, HELD_NOTE, SAMPLE_RATE, 19_200);
    for gain in [0.0, 0.5, 0.75] {
        let thd = thd_db(&run(&trae(gain), &held, SAMPLE_RATE)[9600..], SAMPLE_RATE, 220.0);
        assert!(thd < -60.0, "Gain {gain}: {thd:.1} dB");
    }
}

/// What the whole amp does to a quiet sine at Gain 5, in dB
fn response_db(amp: Amp, freq_hz: f32) -> f32 {
    let input = sine(freq_hz, 0.005, SAMPLE_RATE, 9600);
    let settings = AmpSettings { gate_on: false, ..with_amp(amp, 0.5) };
    to_db(rms(&run(&settings, &input, SAMPLE_RATE)[4800..]) / rms(&input[4800..]))
}

#[test]
fn test_trae_has_the_response_of_an_acoustic_guitar() {
    // With the dials at their centres: the strings' top, which a magnetic pickup loses, is
    // well over the mids from 3 kHz to 8 kHz and still there at 10 kHz, the body is a
    // little over them, and the mids around 700 Hz are the lowest of it
    let level = |freq_hz: f32| response_db(Amp::Trae, freq_hz) - response_db(Amp::Trae, 700.0);
    for freq_hz in [3000.0, 4500.0, 6000.0, 8000.0] {
        assert!(level(freq_hz) > 4.0, "{freq_hz} Hz: {:.1} dB over 700 Hz", level(freq_hz));
    }
    assert!(level(10000.0) > -3.0, "10 kHz: {:.1} dB over 700 Hz", level(10000.0));
    assert!((1.5..6.0).contains(&level(200.0)), "200 Hz: {:.1} dB over 700 Hz", level(200.0));
    // No other amp reaches up there: at 8 kHz every one of them is under its own mids
    for amp in Amp::ALL {
        let top = response_db(amp, 8000.0) - response_db(amp, 700.0);
        assert!(amp == Amp::Trae || top < 0.0, "{amp:?} at 8 kHz: {top:.1} dB over 700 Hz");
    }
}

/// What Træ was tuned with:
///   cargo test -p amp --release trae_report -- --ignored --nocapture
#[test]
#[ignore]
fn trae_report() {
    let riffs = guitar_di(SAMPLE_RATE);
    let clean = clean_di(SAMPLE_RATE);
    let chords = at_rms_db(&power_chords(SAMPLE_RATE, 1.5), REFERENCE_DI_DB);
    println!("\nLoudness (LUFS), peak (dBFS) and crest (dB) per Gain, riffs and clean playing");
    println!("amp   gain  level   riffs: LUFS   peak  crest   clean: LUFS   peak  crest");
    for amp in [Amp::Klar, Amp::Trae] {
        for gain in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for level_db in [REFERENCE_DI_DB, REFERENCE_DI_DB + 6.0] {
                print!("{:<5} {:>4.1} {:>6.0}", format!("{amp:?}"), gain * 10.0, level_db);
                for recording in [&riffs, &clean] {
                    let output =
                        run(&with_amp(amp, gain), &at_rms_db(recording, level_db), SAMPLE_RATE);
                    print!(
                        "   {:>11.1} {:>6.1} {:>6.1}",
                        loudness_lufs(&[&output], SAMPLE_RATE),
                        to_db(peak(&output)),
                        to_db(peak(&output) / rms(&output))
                    );
                }
                println!();
            }
        }
    }
    for (name, recording) in [("riffs", &riffs), ("clean", &clean), ("chords", &chords)] {
        println!(
            "The guitar itself, {name}: crest {:.1} dB, {:.1} LU over its RMS",
            to_db(peak(recording) / rms(recording)),
            loudness_lufs(&[recording], SAMPLE_RATE) - to_db(rms(recording))
        );
    }
    println!("\nTræ at Gain 5 between two held notes, per level of the two together (dBFS RMS)");
    for level_db in [-34.0, -32.0, -30.0, -28.0, -26.0, -24.0, -22.0, -20.0] {
        let output = run(&trae(0.5), &two_held_notes(level_db), SAMPLE_RATE);
        println!(
            "{:>6.0} {:>7.1} dB, peak {:>6.1} dBFS",
            level_db,
            blur_db(&trae(0.5), level_db),
            to_db(peak(&output[SAMPLE_RATE as usize..]))
        );
    }
    println!("\nBetween two held notes (196 and 247 Hz), dB below the whole, and THD of one");
    println!("amp   gain   -36    -30    -24    -18 dBFS RMS    THD 220 Hz");
    let held = sine(220.0, HELD_NOTE, SAMPLE_RATE, 19_200);
    for amp in [Amp::Klar, Amp::Trae] {
        for gain in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let settings = AmpSettings { gate_on: false, ..with_amp(amp, gain) };
            print!("{:<5} {:>4.1}", format!("{amp:?}"), gain * 10.0);
            for level_db in [-36.0, -30.0, -24.0, -18.0] {
                print!(" {:>6.1}", blur_db(&settings, level_db));
            }
            let thd = thd_db(&run(&settings, &held, SAMPLE_RATE)[9600..], SAMPLE_RATE, 220.0);
            println!("             {thd:>6.1}");
        }
    }
    println!("\nChords at Gain 5, octave bands in dB relative to the whole signal");
    println!(
        "Hz from  {:>7}{:>7}{:>7}{:>7}{:>7}{:>7}{:>7}{:>7}   {:>6}",
        62, 125, 250, 500, 1000, 2000, 4000, 8000, ">7k"
    );
    let whole = |signal: &[f32]| {
        for level in band_levels_db(signal, SAMPLE_RATE) {
            print!("{level:>7.1}");
        }
        println!("   {:>6.1}", spectrum_level_db(signal, SAMPLE_RATE, 7000.0, 16000.0));
    };
    print!("{:<9}", "guitar");
    whole(&chords);
    for amp in [Amp::Klar, Amp::Gnist, Amp::Trae] {
        print!("{:<9}", format!("{amp:?}"));
        whole(&run(&with_amp(amp, 0.5), &chords, SAMPLE_RATE));
    }
    println!("\nResponse of the whole amp at Gain 5 to a quiet sine, dB against 1 kHz");
    let level = |freq_hz: f32| {
        let input = sine(freq_hz, 0.005, SAMPLE_RATE, 9600);
        to_db(rms(&run(&trae(0.5), &input, SAMPLE_RATE)[4800..]))
    };
    let reference = level(1000.0);
    for freq_hz in [
        60.0, 80.0, 100.0, 150.0, 200.0, 300.0, 500.0, 750.0, 1000.0, 1500.0, 2000.0, 3000.0,
        4000.0, 6000.0, 8000.0, 10000.0, 12000.0, 15000.0, 18000.0,
    ] {
        println!("{:>7} Hz {:>6.1}", freq_hz, level(freq_hz) - reference);
    }
    println!("\nAliasing at Gain 10, 44.1 kHz, dB below the whole: plain, drive flat out, fuzz flat out");
    for freq_hz in ALIAS_TONES_HZ {
        println!(
            "{:>5} Hz {:>7.1} {:>7.1} {:>7.1}",
            freq_hz,
            aliasing_db_at(&with_amp(Amp::Trae, 1.0), freq_hz, 44100.0),
            aliasing_db_at(&with_drive(Amp::Trae, 1.0, 1.0, 1.0, 1.0), freq_hz, 44100.0),
            aliasing_db_at(
                &AmpSettings { fuzz: fuzz_on(1.0, 1.0), ..with_amp(Amp::Trae, 1.0) },
                freq_hz,
                44100.0
            ),
        );
    }
}

