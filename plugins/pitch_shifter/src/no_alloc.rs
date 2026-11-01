//! Nothing allocates on the audio thread (Code Conventions in AGENTS.md)

use crate::GuitarPitchShifterPlugin;
use suite_common::dsp::pitch::PitchShifter;
use suite_common::host::{self, NoAlloc};

#[global_allocator]
static ALLOC: NoAlloc = NoAlloc;

#[test]
fn test_process_allocates_nothing() {
    let mut plugin = GuitarPitchShifterPlugin::default();
    host::assert_process_allocates_nothing("Pitch Shifter", &mut plugin, 200, &[]);
}

/// The default is a shift of 0: every other shift, through the shifter itself, at the
/// default blend and with the dry input mixed in
#[test]
fn test_every_shift_allocates_nothing() {
    let mut shifter = PitchShifter::new();
    shifter.set_sample_rate(host::SAMPLE_RATE);
    for (semitones, blend) in (-12..=12).flat_map(|st| [(st, 1.0), (st, 0.5)]) {
        host::assert_allocates_nothing(&format!("A shift of {semitones} at blend {blend}"), || {
            shifter.set_semitones(semitones);
            shifter.set_blend(blend);
            for i in 0..20_000 {
                let input = (i as f32 * 0.0144).sin() * 0.5;
                shifter.process_stereo(input, -input);
                shifter.process_sample(input);
            }
        });
    }
}
