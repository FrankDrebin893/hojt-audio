//! Nothing allocates on the audio thread (Code Conventions in AGENTS.md)

use crate::bass::{Bass, Voice};
use crate::BassSynthPlugin;
use suite_common::host::{self, NoAlloc};

#[global_allocator]
static ALLOC: NoAlloc = NoAlloc;

#[test]
fn test_process_allocates_nothing() {
    // Each note is still held when the next comes: it slides
    let notes = host::notes(28, 12, 2, 10, 15);
    let mut plugin = BassSynthPlugin::default();
    // A host starts the smoothers at the parameters' values; the test host cannot
    plugin.params.volume.smoothed.reset(plugin.params.volume.value());
    host::assert_process_allocates_nothing("Bass Synth", &mut plugin, 200, &notes);
}

/// The plugin starts with one voice: every voice, through the bass itself
#[test]
fn test_every_voice_allocates_nothing() {
    let mut bass = Bass::new();
    bass.set_sample_rate(host::SAMPLE_RATE);
    for voice in Voice::ALL {
        let settings = voice.settings();
        host::assert_allocates_nothing(&format!("The {voice:?} voice"), || {
            bass.set(&settings);
            for note in 28..52 {
                bass.note_on(note, 0.8);
                for _ in 0..2000 {
                    bass.process();
                }
                bass.note_off(note);
            }
        });
    }
}
