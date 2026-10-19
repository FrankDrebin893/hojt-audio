//! A host for tests, and an allocator that tells when the audio thread allocates.
//!
//! Nothing may allocate, free or resize memory on the audio thread (Code Conventions in
//! AGENTS.md). Each plugin's `no_alloc` tests hold that: [`assert_process_allocates_nothing`]
//! plays the whole plugin the way a DAW does, and [`assert_allocates_nothing`] covers what its
//! defaults do not reach. Both need [`NoAlloc`] as the allocator of the test binary:
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOC: suite_common::host::NoAlloc = suite_common::host::NoAlloc;
//! ```

use crate::measure::peak;
use nih_plug::prelude::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, Ordering};

/// The system allocator, which also notes the first allocation on a thread that is inside
/// [`assert_allocates_nothing`], with where it was made
pub struct NoAlloc;

/// Whether [`NoAlloc`] is the allocator: it is once it has been asked for anything
static INSTALLED: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Whether this thread is inside [`assert_allocates_nothing`]
    static FORBIDDEN: Cell<bool> = const { Cell::new(false) };
    /// What this thread first allocated or freed there, and where
    static FOUND: RefCell<Option<Found>> = const { RefCell::new(None) };
}

/// An allocation on the audio thread: what was done, how much, and where. The trace is
/// read only when the failure is written, outside the allocator
struct Found {
    what: &'static str,
    bytes: usize,
    trace: std::backtrace::Backtrace,
}

impl NoAlloc {
    fn note(what: &'static str, layout: Layout) {
        INSTALLED.store(true, Ordering::Relaxed);
        // `try_with`: a thread that is ending may have lost its thread-locals already. The
        // flag is off while the find is written down, so writing it may allocate
        let _ = FORBIDDEN.try_with(|forbidden| {
            if forbidden.replace(false) {
                FOUND.with(|found| {
                    found.borrow_mut().get_or_insert_with(|| Found {
                        what,
                        bytes: layout.size(),
                        trace: std::backtrace::Backtrace::force_capture(),
                    });
                });
                forbidden.set(true);
            }
        });
    }
}

unsafe impl GlobalAlloc for NoAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Self::note("allocated", layout);
        System.alloc(layout)
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        Self::note("allocated", layout);
        System.alloc_zeroed(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        Self::note("freed", layout);
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        Self::note("resized", layout);
        System.realloc(ptr, layout, new_size)
    }
}

/// Runs `audio`, as the audio thread would, and fails if it allocates, frees or resizes
/// memory. `what` names it in the failure
pub fn assert_allocates_nothing<R>(what: &str, audio: impl FnOnce() -> R) -> R {
    assert!(
        INSTALLED.load(Ordering::Relaxed),
        "no allocation can be seen: declare `#[global_allocator] static ALLOC: \
         suite_common::host::NoAlloc = suite_common::host::NoAlloc;` in the crate's tests"
    );
    /// Lets allocations through again, also when `audio` panics
    struct Allow;
    impl Drop for Allow {
        fn drop(&mut self) {
            FORBIDDEN.with(|forbidden| forbidden.set(false));
        }
    }

    FOUND.with(|found| found.borrow_mut().take());
    FORBIDDEN.with(|forbidden| forbidden.set(true));
    let allow = Allow;
    let result = audio();
    drop(allow);

    if let Some(found) = FOUND.with(|found| found.borrow_mut().take()) {
        panic!(
            "{what} {} {} bytes, here:\n{}\n\nNothing may allocate, free or resize memory on the audio thread \
             (Code Conventions in AGENTS.md). Make the buffer at its largest size in `new` or \
             `initialize` and reuse it, and keep `Vec`, `Box`, `String`, `format!` and \
             collecting out of the audio path",
            found.what, found.bytes, found.trace
        );
    }
    result
}

/// The sample rate the host plays at
pub const SAMPLE_RATE: f32 = 48_000.0;

/// Lengths of the blocks the host asks for, in turn: its largest, a short one and an odd one
pub const BLOCKS: [usize; 3] = [512, 64, 333];

/// Plays `plugin` through `blocks` blocks of [`BLOCKS`] in its first audio layout, giving an
/// effect a tone at its input and an instrument `events`: each in the block numbered with it,
/// at its own timing in that block. Fails if a call of `process` allocates, or if the plugin
/// made no sound at all. `name` names the plugin in the failure.
///
/// Parameters stay at their defaults, and their smoothers where they are (nih-plug leaves
/// setting either to the host): a test starts the smoothers it needs itself, and reaches other
/// settings through the plugin's own DSP with [`assert_allocates_nothing`]
pub fn assert_process_allocates_nothing<P: Plugin>(
    name: &str,
    plugin: &mut P,
    blocks: usize,
    events: &[(usize, PluginNoteEvent<P>)],
) where
    PluginNoteEvent<P>: Copy,
{
    let layout = P::AUDIO_IO_LAYOUTS[0];
    let channels = layout.main_output_channels.map_or(0, |n| n.get() as usize);
    let config = BufferConfig {
        sample_rate: SAMPLE_RATE,
        min_buffer_size: None,
        max_buffer_size: BLOCKS[0] as u32,
        process_mode: ProcessMode::Realtime,
    };
    assert!(plugin.initialize(&layout, &config, &mut Host), "{name} did not initialize");
    plugin.reset();

    let mut audio = vec![vec![0.0f32; BLOCKS[0]]; channels];
    let mut played = 0;
    let mut loudest = 0.0f32;
    for block in 0..blocks {
        let len = BLOCKS[block % BLOCKS.len()];
        for channel in &mut audio {
            for (i, sample) in channel[..len].iter_mut().enumerate() {
                *sample = if layout.main_input_channels.is_some() {
                    let t = (played + i) as f32 / SAMPLE_RATE;
                    0.5 * (std::f32::consts::TAU * 110.0 * t).sin()
                } else {
                    0.0
                };
            }
        }
        played += len;

        let mut buffer = Buffer::default();
        // SAFETY: the slices are `audio`'s, which outlives `buffer`, and all `len` long
        unsafe {
            buffer.set_slices(len, |slices| {
                slices.clear();
                slices.extend(audio.iter_mut().map(|channel| &mut channel[..len]));
            });
        }
        let mut aux = AuxiliaryBuffers { inputs: &mut [], outputs: &mut [] };
        let mut context = Events { events, block, next: 0 };
        let what = format!("{name}'s process, in block {block},");
        assert_allocates_nothing(&what, || plugin.process(&mut buffer, &mut aux, &mut context));
        drop(buffer);
        loudest = audio.iter().fold(loudest, |loudest, channel| loudest.max(peak(&channel[..len])));
    }
    // A test that played nothing has shown nothing
    assert!(loudest > 0.0, "{name} made no sound: the test did not play it");
}

/// `count` notes from `first` up, `step` semitones apart: one every `every` blocks from the
/// first block on, each let go `hold` blocks after it started
pub fn notes<S>(
    first: u8,
    count: u8,
    step: u8,
    every: usize,
    hold: usize,
) -> Vec<(usize, NoteEvent<S>)> {
    let mut events: Vec<(usize, NoteEvent<S>)> = Vec::new();
    for i in 0..count {
        let note = first + i * step;
        let on = NoteEvent::NoteOn { timing: 10, voice_id: None, channel: 0, note, velocity: 0.8 };
        let off =
            NoteEvent::NoteOff { timing: 20, voice_id: None, channel: 0, note, velocity: 0.0 };
        events.push((i as usize * every, on));
        events.push((i as usize * every + hold, off));
    }
    // In order, as a host gives them: by block, then by timing
    events.sort_by_key(|(block, event)| (*block, event.timing()));
    events
}

/// What `initialize` may ask of the host. Tasks are not run: a host runs them on another
/// thread, and none of them is the audio thread's. So what one would bring (the amp's own
/// cabinet file) never reaches the plugin here, and a test that needs it gives it to the DSP
struct Host;

impl<P: Plugin> InitContext<P> for Host {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }
    fn execute(&self, _task: P::BackgroundTask) {}
    fn set_latency_samples(&self, _samples: u32) {}
    fn set_current_voice_capacity(&self, _capacity: u32) {}
}

/// What `process` may ask of the host: the events of one block
struct Events<'a, P: Plugin> {
    events: &'a [(usize, PluginNoteEvent<P>)],
    block: usize,
    next: usize,
}

impl<P: Plugin> ProcessContext<P> for Events<'_, P>
where
    PluginNoteEvent<P>: Copy,
{
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }
    fn execute_background(&self, _task: P::BackgroundTask) {}
    fn execute_gui(&self, _task: P::BackgroundTask) {}
    fn transport(&self) -> &Transport {
        unimplemented!("the test host has no transport")
    }
    fn next_event(&mut self) -> Option<PluginNoteEvent<P>> {
        while let Some((block, event)) = self.events.get(self.next) {
            self.next += 1;
            if *block == self.block {
                return Some(*event);
            }
        }
        None
    }
    fn send_event(&mut self, _event: PluginNoteEvent<P>) {}
    fn set_latency_samples(&self, _samples: u32) {}
    fn set_current_voice_capacity(&self, _capacity: u32) {}
}
