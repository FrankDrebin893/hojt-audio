//! MIDI into the amp (#479): a footcontroller's program change picks a preset.
//!
//! The audio thread (`process`) only reads the events the host gives it and puts each program
//! change in `MidiInbox`, a fixed-size queue of one writer and one reader that takes no lock and
//! allocates nothing. Presets are loaded by the editor (`presets::set_together`, one gesture, as
//! a click does), never by the audio thread, so the editor drains the queue. The editor paints
//! only when something asks (ADR 0006), so a program change also asks it to: not from `process`
//! (egui's own repaint request locks and may allocate) but by unparking a thread of the queue's
//! own, which holds the window's context and does the asking.
//!
//! A program change is the wire's number, counted from 0, and the channel is the wire's 0 to 15.
//! The player's choice counts channels from 1, with 0 for Omni (`standalone::Prefs::midi_channel`).

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread::Thread;

use nih_plug::prelude::NoteEvent;
use nih_plug_egui::egui::Context;

/// How many program changes wait for the editor. A footcontroller sends a few a second at the
/// most; a change that finds the queue full is dropped
pub const QUEUE: usize = 64;

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

/// What wakes the editor: its context and the thread that asks it to paint
#[derive(Default)]
struct Wake {
    context: Mutex<Option<Context>>,
    thread: Mutex<Option<Thread>>,
}

/// The program changes on their way from the audio thread to the editor
pub struct MidiInbox {
    slots: [AtomicU32; QUEUE],
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

    /// The audio thread's side: keeps the event if it is a program change and ignores the
    /// rest (Control changes are milestone 2's)
    pub fn take(&self, event: &NoteEvent<()>) {
        if let NoteEvent::MidiProgramChange { channel, program, .. } = *event {
            self.put(Program { channel, number: program });
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
        // `try_lock`, so the audio thread never waits for the editor opening: a change that
        // arrives in that instant is painted with the next thing that does
        if let Ok(thread) = self.wake.thread.try_lock() {
            if let Some(thread) = thread.as_ref() {
                thread.unpark();
            }
        }
        true
    }

    /// Is nothing waiting
    pub fn is_empty(&self) -> bool {
        self.written.load(Ordering::Acquire) == self.read.load(Ordering::Relaxed)
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

    /// Throws away what is waiting: program changes that arrived while no window was open are
    /// stale by the time one opens
    pub fn clear(&self) {
        self.read.store(self.written.load(Ordering::Acquire), Ordering::Release);
    }

    /// The window opened with `context`: from now on a program change asks it to paint. The
    /// first call starts the thread that does the asking; it ends with the queue
    pub fn attach(&self, context: Context) {
        *self.wake.context.lock().unwrap_or_else(|e| e.into_inner()) = Some(context);
        let mut thread = self.wake.thread.lock().unwrap_or_else(|e| e.into_inner());
        if thread.is_some() {
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
            *thread = Some(spawned.thread().clone());
        }
    }
}

impl Drop for MidiInbox {
    fn drop(&mut self) {
        // Let the thread find the queue gone
        let thread = self.wake.thread.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(thread) = thread {
            drop(std::mem::take(&mut self.wake));
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
        inbox.take(&NoteEvent::MidiCC { timing: 0, channel: 0, cc: 7, value: 0.5 });
        assert!(inbox.is_empty());
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
