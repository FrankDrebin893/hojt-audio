//! Nothing allocates on the audio thread (Code Conventions in AGENTS.md)

use crate::ShimmerPadPlugin;
use suite_common::host::{self, NoAlloc};

#[global_allocator]
static ALLOC: NoAlloc = NoAlloc;

#[test]
fn test_process_allocates_nothing() {
    // More notes held than there are voices
    let notes = host::notes(40, 16, 3, 4, 100);
    let mut plugin = ShimmerPadPlugin::default();
    // A host starts the smoothers at the parameters' values; the test host cannot
    plugin.params.volume.smoothed.reset(plugin.params.volume.value());
    host::assert_process_allocates_nothing("Shimmer Pad", &mut plugin, 300, &notes);
}
