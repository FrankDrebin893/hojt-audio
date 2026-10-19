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
