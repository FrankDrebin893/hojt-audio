//! Nothing allocates on the audio thread (Code Conventions in AGENTS.md)

use crate::drum_kit::{DrumKit, Stems};
use crate::kit::Kit;
use crate::DrumSynthPlugin;
use suite_common::host::{self, NoAlloc};

#[global_allocator]
static ALLOC: NoAlloc = NoAlloc;

#[test]
fn test_process_allocates_nothing() {
    // Every drum of the General MIDI map, a few at a time
    let notes = host::notes(35, 47, 1, 2, 1);
    let mut plugin = DrumSynthPlugin::default();
    // A host starts the smoothers at the parameters' values; the test host cannot
    plugin.params.kick.smoothed.reset(plugin.params.kick.value());
    plugin.params.snare.smoothed.reset(plugin.params.snare.value());
    plugin.params.toms.smoothed.reset(plugin.params.toms.value());
    plugin.params.hihat.smoothed.reset(plugin.params.hihat.value());
    plugin.params.cymbals.smoothed.reset(plugin.params.cymbals.value());
    plugin.params.room.smoothed.reset(plugin.params.room.value());
    plugin.params.gain.smoothed.reset(plugin.params.gain.value());
    host::assert_process_allocates_nothing("Drum Synth", &mut plugin, 200, &notes);
}

/// The default kit is Rock: every kit, through the kit itself
#[test]
fn test_every_kit_allocates_nothing() {
    let mut drums = DrumKit::new();
    drums.set_sample_rate(host::SAMPLE_RATE);
    for kit in Kit::ALL {
        host::assert_allocates_nothing(&format!("The {kit:?} kit"), || {
            drums.set_kit(kit);
            for note in 35..=81 {
                drums.note_on(note, 0.9);
                for _ in 0..1000 {
                    drums.process_stems(&mut Stems::default());
                }
            }
        });
    }
}
