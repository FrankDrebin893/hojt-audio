//! MIDI into the amp (#479): a footcontroller's program change picks a preset, and its control
//! changes (an expression pedal, a footswitch) move the knobs they are bound to (`midi_map`).
//!
//! The audio thread (`process`) only reads the events the host gives it and puts each program
//! change in `MidiInbox`, a fixed-size queue of one writer and one reader that takes no lock and
//! allocates nothing. A control change is not queued: the inbox keeps the latest value of each of
//! the 128 controllers, so a pedal swept faster than the window paints loses no end point. Presets are loaded by the editor (`presets::set_together`, one gesture, as
//! a click does), never by the audio thread, so the editor drains the queue. The editor paints
//! only when something asks (ADR 0006), so a program change also asks it to: not from `process`
//! (egui's own repaint request locks and may allocate) but by unparking a thread of the queue's
//! own, which holds the window's context and does the asking.
//!
//! A program change is the wire's number, counted from 0, and the channel is the wire's 0 to 15.
//! The player's choice counts channels from 1, with 0 for Omni (`standalone::Prefs::midi_channel`).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread::Thread;

use nih_plug::prelude::NoteEvent;
use nih_plug_egui::egui::Context;

/// How many program changes wait for the editor. A footcontroller sends a few a second at the
/// most; a change that finds the queue full is dropped
pub const QUEUE: usize = 64;

/// How many controllers there are on the wire
pub const CONTROLLERS: usize = 128;

/// The channel choice that takes every channel
pub const OMNI: u8 = 0;

/// The channels a player can choose between, Omni and then 1 to 16
pub const CHOICES: u8 = 17;

/// Does a program change on the wire channel `channel` (0 to 15) pass the player's `choice`?
pub fn passes(choice: u8, channel: u8) -> bool {
    choice == OMNI || choice == channel.saturating_add(1)
}

/// What the Channel row of the settings plate shows
pub fn channel_name(choice: u8) -> String {
    if choice == OMNI {
        "Omni".to_owned()
    } else {
        format!("{choice}")
    }
}

/// The next choice round the list (Omni, 1 to 16), or the previous
pub fn step_channel(choice: u8, forward: bool) -> u8 {
    let choice = choice % CHOICES;
    if forward {
        (choice + 1) % CHOICES
    } else {
        (choice + CHOICES - 1) % CHOICES
    }
}

/// A program change as it waits in the queue
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Program {
    /// The wire's channel, 0 to 15
    pub channel: u8,
    /// The wire's program number, 0 to 127
    pub number: u8,
}

/// Set in a controller's slot while it holds a value nobody has read
const PENDING: u32 = 1 << 31;

/// What wakes the editor: its context and the thread that asks it to paint
#[derive(Default)]
struct Wake {
    context: Mutex<Option<Context>>,
    /// Set once, by `attach`, before any program change can ask for it. The audio thread only
    /// reads it: an atomic load, with no lock and no lazy set-up, which a `Mutex` would have on
    /// platforms whose mutex is made on first use
    thread: OnceLock<Thread>,
}

/// The program changes on their way from the audio thread to the editor
pub struct MidiInbox {
    slots: [AtomicU32; QUEUE],
    /// The latest value each controller was set to and not yet read: `PENDING` and the wire's
    /// channel and the value in sixteen bits, or 0 for nothing waiting
    controllers: [AtomicU32; CONTROLLERS],
    /// Some controller has a value waiting
    controllers_waiting: AtomicBool,
    /// How many were ever put in, and how many were taken out: the writer moves the first and
    /// the reader the second
    written: AtomicUsize,
    read: AtomicUsize,
    wake: Arc<Wake>,
}

impl Default for MidiInbox {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| AtomicU32::new(0)),
            controllers: std::array::from_fn(|_| AtomicU32::new(0)),
            controllers_waiting: AtomicBool::new(false),
            written: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
            wake: Arc::default(),
        }
    }
}

impl MidiInbox {
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    /// The audio thread's side: keeps the event if it is a program change or a control change
    /// and ignores the rest
    pub fn take(&self, event: &NoteEvent<()>) {
        match *event {
            NoteEvent::MidiProgramChange { channel, program, .. } => {
                self.put(Program { channel, number: program });
            }
            NoteEvent::MidiCC { channel, cc, value, .. } => self.put_controller(channel, cc, value),
            _ => {}
        }
    }

    /// Sets controller `cc` (0 to 127) of the wire's `channel` to `value` (0 to 1) for the editor
    /// to read, over a value it has not read yet, and wakes it. Never blocks, never allocates
    pub fn put_controller(&self, channel: u8, cc: u8, value: f32) {
        let value = (value.clamp(0.0, 1.0) * f32::from(u16::MAX)).round() as u32;
        let entry = PENDING | u32::from(channel & 15) << 16 | value;
        self.controllers[usize::from(cc) % CONTROLLERS].store(entry, Ordering::Release);
        self.controllers_waiting.store(true, Ordering::Release);
        self.wake();
    }

    fn wake(&self) {
        if let Some(thread) = self.wake.thread.get() {
            thread.unpark();
        }
    }

    /// Puts a program change in the queue and wakes the editor. Never blocks, never allocates.
    /// A full queue drops it; returns whether it was kept. Only one thread may call this
    pub fn put(&self, program: Program) -> bool {
        let written = self.written.load(Ordering::Relaxed);
        if written.wrapping_sub(self.read.load(Ordering::Acquire)) >= QUEUE {
            return false;
        }
        let packed = u32::from(program.channel) << 8 | u32::from(program.number);
        self.slots[written % QUEUE].store(packed, Ordering::Relaxed);
        self.written.store(written.wrapping_add(1), Ordering::Release);
        self.wake();
        true
    }

    /// Is nothing waiting
    pub fn is_empty(&self) -> bool {
        self.written.load(Ordering::Acquire) == self.read.load(Ordering::Relaxed)
            && !self.controllers_waiting.load(Ordering::Acquire)
    }

    /// The editor's side: hands each waiting program change that passes the channel `choice`
    /// (`OMNI` or 1 to 16) to `each`, oldest first, and drops the others
    pub fn drain(&self, choice: u8, mut each: impl FnMut(u8)) {
        loop {
            let read = self.read.load(Ordering::Relaxed);
            if read == self.written.load(Ordering::Acquire) {
                return;
            }
            let packed = self.slots[read % QUEUE].load(Ordering::Relaxed);
            self.read.store(read.wrapping_add(1), Ordering::Release);
            let program = Program { channel: (packed >> 8) as u8, number: packed as u8 };
            if passes(choice, program.channel) {
                each(program.number);
            }
        }
    }

    /// The editor's side: hands each controller that was set since it last looked, and passes the
    /// channel `choice`, to `each` as its number and its value from 0 to 1, in the order of the
    /// numbers. A controller set twice since gives its last value only
    pub fn drain_controllers(&self, choice: u8, mut each: impl FnMut(u8, f32)) {
        if !self.controllers_waiting.swap(false, Ordering::AcqRel) {
            return;
        }
        for (cc, slot) in self.controllers.iter().enumerate() {
            let entry = slot.swap(0, Ordering::AcqRel);
            if entry & PENDING != 0 && passes(choice, (entry >> 16) as u8 & 15) {
                each(cc as u8, f32::from(entry as u16) / f32::from(u16::MAX));
            }
        }
    }

    /// Throws away what is waiting: program changes that arrived while no window was open are
    /// stale by the time one opens
    pub fn clear(&self) {
        self.read.store(self.written.load(Ordering::Acquire), Ordering::Release);
        self.controllers_waiting.store(false, Ordering::Release);
        for slot in &self.controllers {
            slot.store(0, Ordering::Release);
        }
    }

    /// The window opened with `context`: from now on a program change asks it to paint. The
    /// first call starts the thread that does the asking; it ends with the queue
    pub fn attach(&self, context: Context) {
        // Held to the end: only one call starts the thread
        let mut held = self.wake.context.lock().unwrap_or_else(|e| e.into_inner());
        *held = Some(context);
        if self.wake.thread.get().is_some() {
            return;
        }
        let wake: Weak<Wake> = Arc::downgrade(&self.wake);
        let spawned = std::thread::Builder::new().name("amp-midi-wake".into()).spawn(move || {
            loop {
                std::thread::park();
                // The queue is gone: so is the thread's reason
                let Some(wake) = wake.upgrade() else {
                    return;
                };
                let context = wake.context.lock().unwrap_or_else(|e| e.into_inner()).clone();
                if let Some(context) = context {
                    context.request_repaint();
                }
            }
        });
        if let Ok(spawned) = spawned {
            let _ = self.wake.thread.set(spawned.thread().clone());
        }
    }
}

impl Drop for MidiInbox {
    fn drop(&mut self) {
        // Let the thread find the queue gone
        let thread = self.wake.thread.get().cloned();
        drop(std::mem::take(&mut self.wake));
        if let Some(thread) = thread {
            thread.unpark();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(channel: u8, number: u8) -> NoteEvent<()> {
        NoteEvent::MidiProgramChange { timing: 0, channel, program: number }
    }

    fn drained(inbox: &MidiInbox, choice: u8) -> Vec<u8> {
        let mut numbers = Vec::new();
        inbox.drain(choice, |number| numbers.push(number));
        numbers
    }

    #[test]
    fn test_program_changes_come_out_in_the_order_they_went_in() {
        let inbox = MidiInbox::default();
        assert!(inbox.is_empty());
        for number in [3, 0, 127, 5] {
            inbox.take(&program(0, number));
        }
        assert!(!inbox.is_empty());
        assert_eq!(drained(&inbox, OMNI), [3, 0, 127, 5]);
        assert!(inbox.is_empty());
        assert_eq!(drained(&inbox, OMNI), Vec::<u8>::new());
    }

    #[test]
    fn test_only_program_changes_are_kept() {
        let inbox = MidiInbox::default();
        inbox.take(&NoteEvent::NoteOn {
            timing: 0,
            voice_id: None,
            channel: 0,
            note: 60,
            velocity: 0.5,
        });
        inbox.take(&NoteEvent::MidiChannelPressure { timing: 0, channel: 0, pressure: 0.5 });
        assert!(inbox.is_empty());
    }

    fn cc(channel: u8, cc: u8, value: f32) -> NoteEvent<()> {
        NoteEvent::MidiCC { timing: 0, channel, cc, value }
    }

    fn controllers(inbox: &MidiInbox, choice: u8) -> Vec<(u8, f32)> {
        let mut seen = Vec::new();
        inbox.drain_controllers(choice, |cc, value| seen.push((cc, value)));
        seen
    }

    #[test]
    fn test_a_controller_keeps_its_last_value_and_each_comes_out_once_in_order() {
        let inbox = MidiInbox::default();
        for value in [0.0, 0.25, 1.0, 0.5] {
            inbox.take(&cc(0, 11, value));
        }
        inbox.take(&cc(0, 7, 1.0));
        assert!(!inbox.is_empty());
        let seen = controllers(&inbox, OMNI);
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], (7, 1.0));
        assert_eq!(seen[1].0, 11);
        assert!((seen[1].1 - 0.5).abs() < 1.0 / 60_000.0);
        assert!(inbox.is_empty());
        assert!(controllers(&inbox, OMNI).is_empty());
    }

    #[test]
    fn test_a_controller_value_keeps_its_ends_exactly() {
        let inbox = MidiInbox::default();
        inbox.take(&cc(0, 1, 0.0));
        inbox.take(&cc(0, 2, 1.0));
        assert_eq!(controllers(&inbox, OMNI), [(1, 0.0), (2, 1.0)]);
    }

    #[test]
    fn test_controllers_pass_the_channel_choice_and_clear_drops_them() {
        let inbox = MidiInbox::default();
        inbox.take(&cc(0, 1, 0.5));
        inbox.take(&cc(4, 2, 0.5));
        assert_eq!(controllers(&inbox, 5).iter().map(|c| c.0).collect::<Vec<_>>(), [2]);
        assert!(inbox.is_empty());
        inbox.take(&cc(4, 2, 0.5));
        inbox.clear();
        assert!(inbox.is_empty());
        assert!(controllers(&inbox, OMNI).is_empty());
    }

    #[test]
    fn test_a_channel_choice_passes_its_own_channel_and_omni_every_one() {
        let inbox = MidiInbox::default();
        for channel in [0, 1, 2, 15] {
            inbox.take(&program(channel, channel + 10));
        }
        // Omni takes all four; the wire's channel 1 is the player's 2
        assert_eq!(drained(&inbox, OMNI), [10, 11, 12, 25]);
        for channel in [0, 1, 2, 15] {
            inbox.take(&program(channel, channel + 10));
        }
        assert_eq!(drained(&inbox, 2), [11]);
        // What did not pass is gone, not kept for another choice
        assert!(inbox.is_empty());
        assert!(passes(1, 0) && passes(16, 15) && !passes(1, 1) && !passes(16, 0));
    }

    #[test]
    fn test_a_full_queue_drops_the_newest_and_keeps_the_oldest() {
        let inbox = MidiInbox::default();
        for number in 0..QUEUE as u8 + 5 {
            assert_eq!(inbox.put(Program { channel: 0, number }), (number as usize) < QUEUE);
        }
        let all = drained(&inbox, OMNI);
        assert_eq!(all.len(), QUEUE);
        assert_eq!(all[0], 0);
        assert_eq!(all[QUEUE - 1], QUEUE as u8 - 1);
        // Room again once it was read, and the slots are reused round the ring
        for number in 0..QUEUE as u8 {
            assert!(inbox.put(Program { channel: 3, number: 100 - number % 50 }));
        }
        assert_eq!(drained(&inbox, OMNI).len(), QUEUE);
    }

    #[test]
    fn test_clear_drops_what_waited_for_a_window() {
        let inbox = MidiInbox::default();
        inbox.take(&program(0, 4));
        inbox.clear();
        assert!(inbox.is_empty());
        inbox.take(&program(0, 6));
        assert_eq!(drained(&inbox, OMNI), [6]);
    }

    #[test]
    fn test_the_channel_choice_goes_round_omni_and_one_to_sixteen() {
        assert_eq!(channel_name(OMNI), "Omni");
        assert_eq!(channel_name(16), "16");
        assert_eq!(step_channel(OMNI, true), 1);
        assert_eq!(step_channel(16, true), OMNI);
        assert_eq!(step_channel(OMNI, false), 16);
        assert_eq!(step_channel(1, false), OMNI);
    }

    #[test]
    fn test_a_program_change_asks_the_window_to_paint() {
        let inbox = MidiInbox::default();
        let context = Context::default();
        let asked = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = asked.clone();
        context.set_request_repaint_callback(move |_| flag.store(true, Ordering::SeqCst));
        inbox.attach(context.clone());
        assert!(!asked.load(Ordering::SeqCst));
        inbox.take(&program(0, 1));
        let begun = std::time::Instant::now();
        while !asked.load(Ordering::SeqCst) {
            assert!(begun.elapsed().as_secs() < 5, "the window was never asked to paint");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn test_the_wake_thread_ends_with_the_queue() {
        let inbox = MidiInbox::default();
        inbox.attach(Context::default());
        let weak = Arc::downgrade(&inbox.wake);
        drop(inbox);
        let begun = std::time::Instant::now();
        while weak.strong_count() > 0 {
            assert!(begun.elapsed().as_secs() < 5, "the wake thread is still holding the queue");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
