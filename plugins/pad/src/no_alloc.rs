//! Nothing allocates on the audio thread (Code Conventions in AGENTS.md)

use crate::ShimmerPadPlugin;
use nih_plug::prelude::NoteEvent;
use suite_common::host::{self, NoAlloc};

#[global_allocator]
static ALLOC: NoAlloc = NoAlloc;

#[test]
fn test_process_allocates_nothing() {
    // More notes held than there are voices
    let mut notes = host::notes(40, 16, 3, 4, 100);
    // And the mod wheel moved in the middle of the notes, up, down and back
    for (block, value) in [(13, 0.8), (14, 1.0), (37, 0.2), (38, 0.0), (61, 0.5)] {
        notes.push((block, NoteEvent::MidiCC { timing: 30, channel: 0, cc: 1, value }));
    }
    notes.sort_by_key(|(block, event)| (*block, event.timing()));
    let mut plugin = ShimmerPadPlugin::default();
    // A host starts the smoothers at the parameters' values; the test host cannot
    plugin.params.volume.smoothed.reset(plugin.params.volume.value());
    host::assert_process_allocates_nothing("Shimmer Pad", &mut plugin, 300, &notes);
}
