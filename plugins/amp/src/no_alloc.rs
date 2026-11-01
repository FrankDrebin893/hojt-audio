//! Nothing allocates on the audio thread (Code Conventions in AGENTS.md)

use crate::chain::AmpChain;
use crate::presets::PRESETS;
use crate::GuitarAmpPlugin;
use suite_common::host::{self, NoAlloc};

#[global_allocator]
static ALLOC: NoAlloc = NoAlloc;

#[test]
fn test_process_allocates_nothing() {
    host::assert_process_allocates_nothing("Guitar Amp", &mut GuitarAmpPlugin::default(), 200, &[]);
}

/// The defaults leave most pedals off: every factory preset, which between them turn every
/// pedal and every amp on, through the chain itself, in stereo and in mono
#[test]
fn test_every_preset_allocates_nothing() {
    let mut chain = AmpChain::new();
    chain.set_sample_rate(host::SAMPLE_RATE);
    let mut left = vec![0.0f32; host::BLOCKS[0]];
    let mut right = vec![0.0f32; host::BLOCKS[0]];
    for preset in &PRESETS {
        let settings = preset.settings();
        host::assert_allocates_nothing(&format!("The preset {}", preset.name), || {
            for block in 0..100 {
                let len = host::BLOCKS[block % host::BLOCKS.len()];
                for i in 0..len {
                    left[i] = (i as f32 * 0.0144).sin() * 0.5;
                    right[i] = left[i];
                }
                chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
                chain.process(&settings, &mut left[..len], None);
            }
        });
    }
}

/// The second amp: off, on, switched on and off in the middle of a block, and its amp
/// switched while it plays (and the first amp's, beside it)
#[test]
fn test_the_second_amp_allocates_nothing() {
    use crate::chain::AmpSettings;
    use crate::second::SecondSettings;
    use crate::Amp;
    let mut chain = AmpChain::new();
    chain.set_sample_rate(host::SAMPLE_RATE);
    let mut left = vec![0.0f32; host::BLOCKS[0]];
    let mut right = vec![0.0f32; host::BLOCKS[0]];
    let second = |on: bool, amp: Amp| AmpSettings {
        amp: Amp::Torden,
        second: SecondSettings { on, amp, ..SecondSettings::default() },
        blend: 0.4,
        ..AmpSettings::default()
    };
    host::assert_allocates_nothing("The second amp", || {
        for block in 0..400 {
            let len = host::BLOCKS[block % host::BLOCKS.len()];
            for i in 0..len {
                left[i] = (i as f32 * 0.0144).sin() * 0.5;
                right[i] = left[i];
            }
            // Off for a stretch, on, switched on and off again every few blocks, and its
            // amp (and the first one's) switched while it plays
            let settings = match block / 25 % 4 {
                0 => second(false, Amp::Klar),
                1 => second(true, Amp::ALL[block / 5 % Amp::ALL.len()]),
                2 => second(block % 3 != 0, Amp::Klar),
                _ => AmpSettings {
                    amp: Amp::ALL[block % Amp::ALL.len()],
                    ..second(true, Amp::ALL[(block + 2) % Amp::ALL.len()])
                },
            };
            chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
            chain.process(&settings, &mut left[..len], None);
        }
    });
}

/// The pitch shifter pedal: off, on at each shift, switched on and off in the middle of a block
/// and its shift and tracking turned while it plays, in front of the amp, at the rates that
/// oversample 4 and 2 times
#[test]
fn test_the_pitch_shifter_allocates_nothing() {
    use crate::chain::AmpSettings;
    use crate::pitch::PitchSettings;
    for sample_rate in [44_100.0, 192_000.0] {
        let mut chain = AmpChain::new();
        chain.set_sample_rate(sample_rate);
        let mut left = vec![0.0f32; host::BLOCKS[0]];
        let mut right = vec![0.0f32; host::BLOCKS[0]];
        host::assert_allocates_nothing("The pitch shifter", || {
            for block in 0..600 {
                let len = host::BLOCKS[block % host::BLOCKS.len()];
                for i in 0..len {
                    left[i] = (i as f32 * 0.0144).sin() * 0.5;
                    right[i] = left[i];
                }
                // Off for a stretch, on at one shift after the other, then switched on and off
                // every few blocks, with the shift and the tracking turning
                let pitch = match block / 50 % 4 {
                    0 => PitchSettings::default(),
                    1 => PitchSettings {
                        on: true,
                        semitones: (block as i32 / 5) % 25 - 12,
                        ..PitchSettings::default()
                    },
                    2 => PitchSettings {
                        on: block % 3 != 0,
                        semitones: -5,
                        blend: 0.5,
                        latency_ms: 2.0 + (block % 49) as f32,
                    },
                    _ => PitchSettings { on: true, semitones: 7, ..PitchSettings::default() },
                };
                let settings = AmpSettings { pitch, ..AmpSettings::default() };
                chain.process(&settings, &mut left[..len], Some(&mut right[..len]));
                chain.process(&settings, &mut left[..len], None);
            }
        });
    }
}
