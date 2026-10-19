//! Nothing allocates on the audio thread (Code Conventions in AGENTS.md)

use crate::GlassBellsPlugin;
use suite_common::host::{self, NoAlloc};

#[global_allocator]
static ALLOC: NoAlloc = NoAlloc;

#[test]
fn test_process_allocates_nothing() {
    // More bells ringing than there are voices
    let notes = host::notes(60, 16, 2, 4, 1);
    let mut plugin = GlassBellsPlugin::default();
    // A host starts the smoothers at the parameters' values; the test host cannot
    plugin.params.volume.smoothed.reset(plugin.params.volume.value());
    host::assert_process_allocates_nothing("Glass Bells", &mut plugin, 300, &notes);
}
