//! Draws an editor off screen into a picture, for its snapshot test.
//!
//! Each editor draws through one function, `draw`, which its plugin calls in the window and its
//! test calls here. [`pedal_harness`] runs that in a window of the editor's size with egui's
//! wgpu renderer, and `harness.snapshot("editor")` compares the picture with
//! `tests/snapshots/editor.png` of the plugin. When they differ the test fails and leaves
//! `editor.new.png` and `editor.diff.png` beside it; `UPDATE_SNAPSHOTS=1` takes the new one as
//! the reference, only after somebody has looked at it (`STYLE.md`, "After changing an editor").
//!
//! The references are drawn on Linux, with Mesa's software Vulkan driver (lavapipe, the
//! package `mesa-vulkan-drivers`): other systems draw the letters and edges a little
//! differently, so the harness and the snapshot tests are only built there (`NoHost` is built
//! everywhere). Only for tests: behind the `snapshot` feature, which the plugins name in their
//! dev-dependencies.

#[cfg(target_os = "linux")]
pub use self::harness::{pedal_harness, Harness, SnapshotOptions};
use nih_plug::context::PluginApi;
use nih_plug::prelude::*;

/// A host that is never asked for anything: no input reaches the knobs
pub struct NoHost;

impl GuiContext for NoHost {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    fn request_resize(&self) -> bool {
        false
    }
    unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {}
    unsafe fn raw_set_parameter_normalized(&self, _: ParamPtr, _: f32) {}
    unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {}
    fn get_state(&self) -> PluginState {
        unreachable!()
    }
    fn set_state(&self, _: PluginState) {}
}

/// A check that knows a newer version is out (99.0.0), without asking anyone, for the editors'
/// tests of the update line. Its settings are its own, so a test may close or turn it off
pub fn newer_version() -> crate::update::UpdateCheck {
    crate::update::UpdateCheck::resting(std::sync::Arc::new(std::sync::Mutex::new(
        crate::update::UpdateSettings { latest: "99.0.0".into(), ..Default::default() },
    )))
}

#[cfg(target_os = "linux")]
mod harness {
    use nih_plug_egui::egui::Context;
    use std::cell::Cell;
    use std::panic::catch_unwind;
    use std::rc::Rc;

    pub use egui_kittest::SnapshotOptions;
    use std::ops::{Deref, DerefMut};
    use std::sync::{Mutex, MutexGuard};

    /// One editor is drawn at a time. The tests of a plugin run side by side, and with several
    /// of them drawing at once the software Vulkan driver crashed the whole test run on CI
    /// (SIGSEGV, no test named), more often the faster the tests around the drawing are
    static DRAWING: Mutex<()> = Mutex::new(());

    thread_local! {
        /// The harnesses alive in this thread and the turn they hold together, so that a test
        /// with two harnesses does not wait for itself
        static TURN: std::cell::RefCell<(usize, Option<MutexGuard<'static, ()>>)> =
            const { std::cell::RefCell::new((0, None)) };
    }

    /// egui_kittest's harness, holding this thread's turn to draw for as long as it lives
    pub struct Harness<'a>(Option<egui_kittest::Harness<'a>>);

    impl<'a> Deref for Harness<'a> {
        type Target = egui_kittest::Harness<'a>;
        fn deref(&self) -> &Self::Target {
            self.0.as_ref().expect("only taken when dropped")
        }
    }

    impl DerefMut for Harness<'_> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.0.as_mut().expect("only taken when dropped")
        }
    }

    impl Drop for Harness<'_> {
        fn drop(&mut self) {
            // The renderer goes before the turn is given up
            self.0 = None;
            end_turn();
        }
    }

    fn take_turn() {
        TURN.with_borrow_mut(|(alive, turn)| {
            if *alive == 0 {
                // A test that failed while drawing leaves the lock poisoned; the next may still draw
                *turn = Some(DRAWING.lock().unwrap_or_else(|poisoned| poisoned.into_inner()));
            }
            *alive += 1;
        });
    }

    fn end_turn() {
        TURN.with_borrow_mut(|(alive, turn)| {
            *alive -= 1;
            if *alive == 0 {
                *turn = None;
            }
        });
    }

    /// Gives the turn up when building a harness panics before there is one to drop
    struct TurnTaken;

    impl Drop for TurnTaken {
        fn drop(&mut self) {
            end_turn();
        }
    }

    /// How many frames the harness draws at most while it waits for egui to settle. An editor
    /// that asks for a repaint in every frame (the amp while it tunes) is drawn this many times
    const MAX_STEPS: u64 = 8;

    /// The time between two frames, in seconds: each frame is this much later than the one
    /// before, from 0, so what an editor reads from the clock is the same in every run
    const STEP_DT: f32 = 1.0 / 60.0;

    /// The widest texture egui may make. Its own default is 2048, which the amp's lit surfaces
    /// are wider than; a window on a graphics card offers more, and wgpu's default device limits
    /// are this
    const MAX_TEXTURE_SIDE: usize = 8192;

    /// A harness that draws `draw` in a window of `width` by `height` points, one pixel each,
    /// the way the plugin's window does, run until egui has settled. The fonts are installed
    /// first, in frames of their own, since egui only takes them from the next frame on
    ///
    /// # Panics
    ///
    /// When wgpu finds no adapter to draw with: on Linux, install `mesa-vulkan-drivers`
    pub fn pedal_harness<'a>(
        width: u32,
        height: u32,
        mut draw: impl FnMut(&Context) + 'a,
    ) -> Harness<'a> {
        take_turn();
        let turn = TurnTaken;
        let renderer =
            catch_unwind(egui_kittest::wgpu::WgpuTestRenderer::new).unwrap_or_else(|_| {
                panic!(
                    "wgpu found no adapter to draw the editor with. On Linux, install Mesa's \
                 software Vulkan driver: sudo apt-get install mesa-vulkan-drivers"
                )
            });
        // The harness draws as it is built, before its input can be set: nothing is drawn
        // until it has been
        let ready = Rc::new(Cell::new(false));
        let mut fonts_in = false;
        let mut harness = egui_kittest::Harness::builder()
            .with_size([width as f32, height as f32])
            .with_pixels_per_point(1.0)
            .with_step_dt(STEP_DT)
            .with_max_steps(MAX_STEPS)
            .renderer(renderer)
            .build({
                let ready = ready.clone();
                move |ctx| {
                    if !fonts_in {
                        crate::ui::install(ctx);
                        fonts_in = true;
                    } else if ready.get() {
                        draw(ctx);
                    }
                }
            });
        harness.input_mut().max_texture_side = Some(MAX_TEXTURE_SIDE);
        ready.set(true);
        harness.run_ok();
        // From here on the harness gives the turn up, when it is dropped
        std::mem::forget(turn);
        Harness(Some(harness))
    }
}
