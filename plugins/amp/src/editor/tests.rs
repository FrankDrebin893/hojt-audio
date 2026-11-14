use super::*;
#[cfg(target_os = "linux")]
use std::cell::RefCell;
#[cfg(target_os = "linux")]
use std::rc::Rc;
#[cfg(target_os = "linux")]
use suite_common::ui::snapshot::{pedal_harness, Harness, NoHost};

/// The tests that point the presets at a folder of their own, one at a time: the folder is
/// named in the environment, which every test shares
#[cfg(target_os = "linux")]
static FOLDER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// The picture on the face is compiled in: a build without it is a build of other sources
#[test]
fn test_the_pedal_is_drawn_with_its_art() {
    assert!(STYLE.art == Some(ui::artwork::AMP), "the pedal has lost its art");
}

#[cfg(target_os = "linux")]
thread_local! {
    /// The looper the window of this test reads: empty until a test shows another state
    static LOOPER: Arc<LooperControl> = Arc::new(LooperControl::default());
}

/// The looper the window of this test is drawn with
#[cfg(target_os = "linux")]
fn test_looper() -> Arc<LooperControl> {
    LOOPER.with(Arc::clone)
}

/// The editor's tests that draw the window have the looper in this state
#[cfg(target_os = "linux")]
fn loop_shows(state: LoopState, length: u32, position: u32, capacity: u32) {
    test_looper().show(state, length, position, capacity);
}

/// A view as the window has it after it opened: nothing open, the bank the params name
fn fresh_view(params: &GuitarAmpParams, lamp_reading: &LampReading) -> View {
    View {
        lamps: LampView::new(lamp_reading),
        bank: Bank::read(params, None),
        plate: None,
        saving: None,
        note: None,
        cab_status: CabStatus::Fine,
        beat: BeatView::default(),
        scroll: 0.0,
        effects: None,
    }
}

/// The window as a snapshot test draws it: with `params`, the tuner hearing `tuner_hz`,
/// and `view` as it is after the window opened and `open` did what a click would. The
/// lamps and meters are at rest, the cabinet the amp's own
#[cfg(target_os = "linux")]
fn harness_of<'a>(
    params: &'a GuitarAmpParams,
    setter: &'a ParamSetter<'a>,
    tuner_hz: Option<f32>,
    open: impl FnOnce(&mut View),
) -> Harness<'a> {
    harness_with(params, setter, tuner_hz, None, open)
}

/// `harness_of` in the standalone program, if there is a `host` for the machine's audio
#[cfg(target_os = "linux")]
fn harness_with<'a>(
    params: &'a GuitarAmpParams,
    setter: &'a ParamSetter<'a>,
    tuner_hz: Option<f32>,
    audio_host: Option<&'a dyn standalone::Host>,
    open: impl FnOnce(&mut View),
) -> Harness<'a> {
    harness_shared(params, setter, tuner_hz, audio_host, open).0
}

/// `harness_with` that also hands back the view the window draws from, to look at what a click
/// did to it
#[cfg(target_os = "linux")]
fn harness_shared<'a>(
    params: &'a GuitarAmpParams,
    setter: &'a ParamSetter<'a>,
    tuner_hz: Option<f32>,
    audio_host: Option<&'a dyn standalone::Host>,
    open: impl FnOnce(&mut View),
) -> (Harness<'a>, Rc<RefCell<View>>) {
    let tuner = TunerReading::new();
    tuner.set(tuner_hz);
    let lamp_reading = LampReading::new();
    let cab_loader = CabLoader::new(Arc::new(crate::cab_stage::CabStage::new()), 48_000.0);
    let mut view = fresh_view(params, &lamp_reading);
    open(&mut view);
    let view = Rc::new(RefCell::new(view));
    let harness = harness_at_size(
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        params,
        setter,
        tuner,
        lamp_reading,
        cab_loader,
        audio_host,
        view.clone(),
    );
    (harness, view)
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn harness_at_size<'a>(
    width: u32,
    height: u32,
    params: &'a GuitarAmpParams,
    setter: &'a ParamSetter<'a>,
    tuner: TunerReading,
    lamp_reading: LampReading,
    cab_loader: CabLoader,
    audio_host: Option<&'a dyn standalone::Host>,
    view: Rc<RefCell<View>>,
) -> Harness<'a> {
    pedal_harness(width, height, move |ctx| {
        let shared = Shared {
            tuner: &tuner,
            lamp_reading: &lamp_reading,
            beats: &BeatReading::default(),
            looper: &test_looper(),
            cab_loader: &cab_loader,
            load_cabinet: &|| {},
            audio_host,
            update: &UpdateCheck::resting(params.update.clone()),
        };
        draw(ctx, setter, params, &mut view.borrow_mut(), &shared);
    })
}

/// A click of the primary button at `at`, in window points at full size: the pointer arrives,
/// presses and lets go, a frame each
#[cfg(target_os = "linux")]
fn click(harness: &mut Harness, at: Pos2) {
    use nih_plug_egui::egui::{Event, Modifiers, PointerButton};
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    for event in [Event::PointerMoved(at), button(true), button(false)] {
        harness.input_mut().events.push(event);
        harness.run_ok();
    }
}

/// Text typed, and keys pressed, in the next frame
#[cfg(target_os = "linux")]
fn type_text(harness: &mut Harness, text: &str) {
    use nih_plug_egui::egui::Event;
    harness.input_mut().events.push(Event::Text(text.to_owned()));
    harness.run_ok();
}

// egui's debug build checks that a widget stays on one layer in a frame (#325). The window as
// the host has it, in its resizable window, over the frames that matter
#[test]
#[cfg(target_os = "linux")]
fn test_the_window_keeps_each_widget_on_one_layer() {
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let tuner = TunerReading::new();
    let lamp_reading = LampReading::new();
    let cab_loader = CabLoader::new(Arc::new(crate::cab_stage::CabStage::new()), 48_000.0);
    let view = Rc::new(RefCell::new(fresh_view(&params, &lamp_reading)));
    let state = EguiState::from_size(WINDOW_WIDTH, WINDOW_HEIGHT);
    let update = UpdateCheck::resting(params.update.clone());
    let mut harness = pedal_harness(WINDOW_WIDTH, WINDOW_HEIGHT, {
        let view = view.clone();
        let state = state.clone();
        move |ctx| {
            let shared = Shared {
                tuner: &tuner,
                lamp_reading: &lamp_reading,
                beats: &BeatReading::default(),
                looper: &test_looper(),
                cab_loader: &cab_loader,
                load_cabinet: &|| {},
                audio_host: None,
                update: &update,
            };
            window(ctx, &state, &setter, &params, &mut view.borrow_mut(), &shared);
        }
    });
    harness.run_ok();
    click(&mut harness, Pos2::new(WINDOW_WIDTH as f32 - 4.0, WINDOW_HEIGHT as f32 - 4.0));
    harness.run_ok();
}

// What the editor draws in its window, every parameter at its default, against the
// picture in tests/snapshots/editor.png. On a change it leaves editor.new.png and
// editor.diff.png there to look at (see suite_common::ui::snapshot)
#[test]
#[cfg(target_os = "linux")]
fn test_the_editor_looks_as_it_did() {
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    harness_of(&params, &setter, None, |_| {}).snapshot("editor");
}

// The update line is drawn when a newer version is known, and only then: with the check off,
// or the line closed for that version, the window is the one it was (pixel for pixel)
#[test]
#[cfg(target_os = "linux")]
fn test_the_update_line_is_drawn_when_a_newer_version_is_known() {
    type Settings = suite_common::update::UpdateSettings;
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let picture = |change: &dyn Fn(&mut Settings)| {
        let params = GuitarAmpParams::default();
        change(&mut params.update.lock().unwrap());
        let mut harness = harness_of(&params, &setter, None, |_| {});
        // Not returned directly: the harness must go before the params it borrows
        #[allow(clippy::let_and_return)]
        let raw = harness.render().unwrap().into_raw();
        raw
    };
    let none = picture(&|_| {});
    let newer = |settings: &mut Settings| settings.latest = "99.0.0".into();
    assert_ne!(picture(&newer), none, "no line with a newer version known");
    assert!(
        picture(&|settings| {
            newer(settings);
            settings.enabled = false;
        }) == none,
        "a line with the check off"
    );
    assert!(
        picture(&|settings| {
            newer(settings);
            settings.dismissed = settings.latest.clone();
        }) == none,
        "a line after it was closed"
    );
}

// The window with the line, against tests/snapshots/editor_update.png
#[test]
#[cfg(target_os = "linux")]
fn test_the_editor_with_the_update_line_looks_as_it_did() {
    let params = GuitarAmpParams::default();
    params.update.lock().unwrap().latest = "99.0.0".into();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    harness_of(&params, &setter, None, |_| {}).snapshot("editor_update");
}

// The line has the room it needs on the strip at its narrowest and at the widest it opens,
// clear of the settings icon and of the output knob's scale
#[test]
fn test_the_update_line_has_room_on_the_strip() {
    let needed = ui::update_line::parts(Pos2::ZERO, [96.0, 51.0, 19.0]).0.width();
    for width in [STRIP_MIN_WIDTH, WINDOW_WIDTH as f32, 3840.0] {
        let shift = width - WINDOW_WIDTH as f32;
        let room = (UPDATE_ROOM_RIGHT + shift) - (UPDATE_ROOM_LEFT + shift / 2.0) - STRIP_GAP;
        assert!(room >= needed, "{room} of the strip's room at {width} wide");
    }
}

// The window at the size it opens at (0.55 zoom, 1876 x 321): every pedal, label and the
// strip scaled down together, nothing clipped
#[test]
#[cfg(target_os = "linux")]
fn test_the_editor_at_default_zoom_looks_as_it_did() {
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let tuner = TunerReading::new();
    let lamp_reading = LampReading::new();
    let cab_loader = CabLoader::new(Arc::new(crate::cab_stage::CabStage::new()), 48_000.0);
    let view = Rc::new(RefCell::new(fresh_view(&params, &lamp_reading)));
    let (width, height) = size_for_screen(FALLBACK_SCREEN_WIDTH);
    harness_at_size(width, height, &params, &setter, tuner, lamp_reading, cab_loader, None, view)
        .snapshot("editor_default_zoom");
}

/// A window `width` by `height` points drawing `params`, scrolled `scroll` points, the way
/// a window of that size does, with everything else at rest
#[cfg(target_os = "linux")]
fn line_harness<'a>(
    width: u32,
    height: u32,
    params: &'a GuitarAmpParams,
    setter: &'a ParamSetter<'a>,
    scroll: f32,
) -> (Harness<'a>, Rc<RefCell<View>>) {
    let lamp_reading = LampReading::new();
    let cab_loader = CabLoader::new(Arc::new(crate::cab_stage::CabStage::new()), 48_000.0);
    let mut view = fresh_view(params, &lamp_reading);
    view.scroll = scroll;
    let view = Rc::new(RefCell::new(view));
    let harness = harness_at_size(
        width,
        height,
        params,
        setter,
        TunerReading::new(),
        lamp_reading,
        cab_loader,
        None,
        view.clone(),
    );
    (harness, view)
}

/// A chain of `kinds` in front of the amps then `kinds` behind them, each on the slot its
/// pedal's own parameters are on if that is free, a slot of its own otherwise
fn chain_of(front: &[EffectKind], back: &[EffectKind]) -> Chain {
    let mut chain = Chain::empty();
    for &kind in front {
        chain.add(kind, crate::order::Zone::Front, 99).unwrap();
    }
    for &kind in back {
        chain.add(kind, crate::order::Zone::Back, 99).unwrap();
    }
    chain
}

/// The chain of the longest line: sixteen blocks, a second of eight of the kinds among them,
/// so that half of them keep their values in the parameters of their slot
fn sixteen_blocks() -> Chain {
    use EffectKind::*;
    chain_of(
        &[Wah, Pitch, Fuzz, Comp, Comp, Drive, Eq, Phaser],
        &[Chorus, Chorus, Flanger, Tremolo, Delay, Delay, Reverb, Reverb],
    )
}

// A short chain, a compressor in front of the amps and a chorus behind them: the line is as
// long as its blocks, the strip as wide as the window
#[test]
#[cfg(target_os = "linux")]
fn test_a_short_chain_looks_as_it_did() {
    use EffectKind::*;
    let params = GuitarAmpParams::default();
    params.chain.put(chain_of(&[Comp], &[Chorus]));
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    line_harness(1430, 321, &params, &setter, 0.0).0.snapshot("line_short");
    let _ = (Comp, Chorus);
}

// Today's chain in the smallest window: the zoom the line is at when it is the least readable
// there is, and the line still all there
#[test]
#[cfg(target_os = "linux")]
fn test_the_twelve_pedals_in_the_smallest_window_look_as_they_did() {
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    line_harness(MIN_WIDTH, MIN_HEIGHT, &params, &setter, 0.0).0.snapshot("line_small");
}

// Today's chain on a large screen: full size, the strip as wide as the window
#[test]
#[cfg(target_os = "linux")]
fn test_the_twelve_pedals_in_a_large_window_look_as_they_did() {
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    line_harness(3840, 640, &params, &setter, 0.0).0.snapshot("line_large");
}

// Sixteen blocks in the window that shows them whole at the least zoom (4239 points at 0.5): the
// second of each kind, eight of them, on the parameters of their slots
#[test]
#[cfg(target_os = "linux")]
fn test_sixteen_blocks_look_as_they_did() {
    let params = GuitarAmpParams::default();
    params.chain.put(sixteen_blocks());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    line_harness(2120, 292, &params, &setter, 0.0).0.snapshot("line_sixteen");
}

// The same sixteen in the smallest window: they do not fit at the zoom it stops at, so the
// line is scrolled, here to its start and to its end
#[test]
#[cfg(target_os = "linux")]
fn test_sixteen_blocks_scroll_in_the_smallest_window() {
    let params = GuitarAmpParams::default();
    params.chain.put(sixteen_blocks());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (width, height) = (MIN_WIDTH, MIN_HEIGHT);
    line_harness(width, height, &params, &setter, 0.0).0.snapshot("line_scrolled_start");
    line_harness(width, height, &params, &setter, 1.0e6).0.snapshot("line_scrolled_end");
}

// The longest chain there is: sixteen blocks (and the gate in front of them) and both amps on (Torden and Klar), in the window
// that shows them whole and in the smallest one scrolled to the amps. Both lanes stay readable
// at the least zoom, and the line at rest still asks for no repaint
#[test]
#[cfg(target_os = "linux")]
fn test_sixteen_blocks_and_both_amps_look_as_they_did() {
    let params = GuitarAmpParams {
        amp: EnumParam::new("Amp", Amp::Torden),
        amp2_on: BoolParam::new("Amp 2", true),
        ..GuitarAmpParams::default()
    };
    params.chain.put(sixteen_blocks());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    line_harness(2120, 292, &params, &setter, 0.0).0.snapshot("line_sixteen_two_amps");
    // The stack stands after the eight blocks in front of it: scrolled so that it is in view
    let stack_at = Line::of_chain(&sixteen_blocks()).places_for_test()[8 + 1];
    let (mut harness, _) = line_harness(MIN_WIDTH, MIN_HEIGHT, &params, &setter, stack_at - 200.0);
    harness.snapshot("line_two_amps_scrolled");
    for _ in 0..3 {
        harness.step();
    }
    let delays: Vec<_> =
        harness.output().viewport_output.values().map(|v| v.repaint_delay).collect();
    assert!(!delays.is_empty());
    assert!(delays.iter().all(|delay| *delay == std::time::Duration::MAX), "{delays:?}");
}

/// The wheel turned over `at`, by `delta` points
#[cfg(target_os = "linux")]
fn wheel(harness: &mut Harness, at: Pos2, delta: Vec2) {
    use nih_plug_egui::egui::{Event, Modifiers, MouseWheelUnit};
    for event in [
        Event::PointerMoved(at),
        Event::MouseWheel { unit: MouseWheelUnit::Point, delta, modifiers: Modifiers::NONE },
    ] {
        harness.input_mut().events.push(event);
        harness.run_ok();
    }
}

// A line that is wider than the window scrolls with the wheel over it, either way the wheel
// is turned, and stops at both ends; a window that shows it all has nothing to scroll
#[test]
#[cfg(target_os = "linux")]
fn test_the_wheel_scrolls_a_line_that_does_not_fit_and_stops_at_its_ends() {
    let params = GuitarAmpParams::default();
    params.chain.put(sixteen_blocks());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = line_harness(MIN_WIDTH, MIN_HEIGHT, &params, &setter, 0.0);
    let over_the_line = Pos2::new(300.0, 200.0);
    wheel(&mut harness, over_the_line, vec2(0.0, -100.0));
    // 100 points of wheel at 0.5 is 200 points of the layer
    assert!((view.borrow().scroll - 200.0).abs() < 0.01, "{}", view.borrow().scroll);
    wheel(&mut harness, over_the_line, vec2(-40.0, 0.0));
    assert!((view.borrow().scroll - 280.0).abs() < 0.01);
    wheel(&mut harness, over_the_line, vec2(0.0, 1000.0));
    assert_eq!(view.borrow().scroll, 0.0);
    let furthest = Fit::of(vec2(MIN_WIDTH as f32, MIN_HEIGHT as f32), 4239.0).furthest();
    wheel(&mut harness, over_the_line, vec2(0.0, -100_000.0));
    assert_eq!(view.borrow().scroll, furthest);
    // Over the strip the wheel is left alone
    wheel(&mut harness, Pos2::new(300.0, 10.0), vec2(0.0, 1000.0));
    assert_eq!(view.borrow().scroll, furthest);

    // The twelve-pedal line in a window that shows it all does not scroll
    let params = GuitarAmpParams::default();
    let (mut harness, view) = line_harness(WINDOW_WIDTH, WINDOW_HEIGHT, &params, &setter, 0.0);
    wheel(&mut harness, over_the_line, vec2(0.0, -500.0));
    assert_eq!(view.borrow().scroll, 0.0);
}

// A press in the groove of the scroll bar brings its thumb there, and the thumb is dragged
#[test]
#[cfg(target_os = "linux")]
fn test_the_scroll_bar_takes_the_line_where_it_is_pressed_and_dragged() {
    let params = GuitarAmpParams::default();
    params.chain.put(sixteen_blocks());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = line_harness(MIN_WIDTH, MIN_HEIGHT, &params, &setter, 0.0);
    let furthest = Fit::of(vec2(MIN_WIDTH as f32, MIN_HEIGHT as f32), 4239.0).furthest();
    let bar_y = WINDOW_HEIGHT as f32 * MIN_ZOOM - 2.0;
    // Pressed at the groove's far end: the line is at its end
    click(&mut harness, Pos2::new(MIN_WIDTH as f32 - 25.0, bar_y));
    assert_eq!(view.borrow().scroll, furthest);
    // At its near end: back at the start
    click(&mut harness, Pos2::new(25.0, bar_y));
    assert_eq!(view.borrow().scroll, 0.0);
}

/// The pointer pressed at `from`, carried to `to` and let go there, a frame at each step
#[cfg(target_os = "linux")]
fn drag(harness: &mut Harness, from: Pos2, to: Pos2) {
    use nih_plug_egui::egui::{Event, Modifiers, PointerButton};
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let middle = from + (to - from) * 0.5;
    let events = [
        Event::PointerMoved(from),
        button(from, true),
        Event::PointerMoved(from + vec2(4.0, 0.0)),
        Event::PointerMoved(middle),
        Event::PointerMoved(to),
        button(to, false),
    ];
    for event in events {
        harness.input_mut().events.push(event);
        harness.run_ok();
    }
}

// A block is dragged by its title to another place in the line and lands there, across the
// amps if it is carried across them, with its slot
#[test]
#[cfg(target_os = "linux")]
fn test_a_block_dragged_by_its_title_lands_in_the_nearest_place() {
    use crate::order::Zone;
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, _) = line_harness(WINDOW_WIDTH, WINDOW_HEIGHT, &params, &setter, 0.0);
    let line = Line::of_chain(&params.chain.get());
    let title = |at: usize| {
        let left = line.places_for_test()[at];
        Pos2::new(left + PEDAL_WIDTH / 2.0, PEDAL_TOP + 30.0)
    };
    // The chorus, first behind the amps (nine pieces in), to the far end of the window
    let chorus = line.pieces_for_test().iter().position(|piece| {
            matches!(piece, crate::board::Piece::Block(block) if block.kind == EffectKind::Chorus)
        });
    let chorus = chorus.unwrap();
    drag(&mut harness, title(chorus), Pos2::new(WINDOW_WIDTH as f32 - 60.0, PEDAL_TOP + 30.0));
    for _ in 0..3 {
        harness.run_ok();
    }
    let chain = params.chain.get();
    assert_eq!(chain.back().last().map(|block| block.kind), Some(EffectKind::Chorus));
    assert_eq!(chain.back().len(), 6);
    assert_eq!(chain.front().len(), 6);
    // It is on its own slot still
    assert!(chain.blocks().iter().all(|&block| Chain::is_home(block)));
    // And back across the amps to right behind the gate
    let line = Line::of_chain(&chain);
    let last = line.pieces_for_test().len() - 1;
    drag(&mut harness, title(last), Pos2::new(BENCH_MARGIN + PEDAL_WIDTH + 40.0, PEDAL_TOP + 30.0));
    for _ in 0..3 {
        harness.run_ok();
    }
    let chain = params.chain.get();
    assert_eq!(chain.front().first().map(|block| block.kind), Some(EffectKind::Chorus));
    assert_eq!(chain.front().len(), 7);
    let _ = Zone::Front;
}

// Nothing asks for a repaint while nobody touches the window: a drawn line is painted only
// when something changes (STYLE.md)
#[test]
#[cfg(target_os = "linux")]
fn test_a_line_at_rest_asks_for_no_repaint() {
    let params = GuitarAmpParams::default();
    params.chain.put(sixteen_blocks());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, _) = line_harness(MIN_WIDTH, MIN_HEIGHT, &params, &setter, 300.0);
    for _ in 0..3 {
        harness.step();
    }
    let delays: Vec<_> =
        harness.output().viewport_output.values().map(|viewport| viewport.repaint_delay).collect();
    assert!(!delays.is_empty());
    assert!(delays.iter().all(|delay| *delay == std::time::Duration::MAX), "{delays:?}");
}

// The second amp on beside the first: Torden and Klar, each through a cabinet of its own
// (Torden's own, Klar through a 1x15), a Blend of 5.0 and the amps on their way
#[test]
#[cfg(target_os = "linux")]
fn test_the_second_amp_looks_as_it_did() {
    let params = GuitarAmpParams {
        amp: EnumParam::new("Amp", Amp::Torden),
        amp2_on: BoolParam::new("Amp 2", true),
        ..GuitarAmpParams::default()
    };
    params.cabinet_second.put(crate::amp::model::speaker_index("1x15-deep"));
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    harness_of(&params, &setter, None, |_| {}).snapshot("dual");
}

// The wah on, in Auto, with the rocker at the toe: the lamp, the Auto lamp and the cap at the
// top of its slot (the default picture has them at rest)
#[test]
#[cfg(target_os = "linux")]
fn test_the_wah_looks_as_it_did() {
    let params = GuitarAmpParams {
        wah_on: BoolParam::new("Wah", true),
        wah_mode: EnumParam::new("Wah Mode", WahMode::Auto),
        wah_pedal: crate::dial_param("Wah Pedal", 1.0),
        ..GuitarAmpParams::default()
    };
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    harness_of(&params, &setter, None, |_| {}).snapshot("wah");
}

#[test]
fn test_the_wahs_controls_stand_clear_of_each_other_and_on_the_face() {
    let knob = WAH_KNOB_RADIUS + TICKS;
    let [bite, sens] = WAH_KNOBS;
    // Across the rocker's row: Bite's scale, the rocker with its ticks, the tape of its value
    // and the face's edge
    in_a_row(&[
        ("Bite's scale", bite.x - knob, bite.x + knob),
        ("the rocker's ticks", -ui::widgets::ROCKER_REACH, ui::widgets::ROCKER_REACH),
        ("the rocker's tape", ROCKER_TAPE.x - 22.0, ROCKER_TAPE.x + 22.0),
        ("the face's edge", PEDAL_WIDTH / 2.0, PEDAL_WIDTH),
    ]);
    // The cap at either end of its slot stays inside the face between the captions
    let cap = ui::widgets::ROCKER_CAP_HEIGHT / 2.0;
    in_a_row(&[
        ("Toe", ROCKER_TOE.y - 6.0, ROCKER_TOE.y + 6.0),
        ("the rocker", ROCKER_TOP.y - cap, ROCKER_TOP.y + ROCKER_LENGTH + cap),
        ("Heel", ROCKER_HEEL.y - 6.0, ROCKER_HEEL.y + 6.0),
        ("the On switch", PEDAL_SWITCH.y - SWITCH, PEDAL_SWITCH.y + SWITCH),
    ]);
    // Down the right: the lamp, the rocker's name, its tape, then the Auto button with its
    // lamp above and its caption under it
    in_a_row(&[
        ("the lamp", WAH_LED.y - LAMP, WAH_LED.y + LAMP),
        ("the rocker's caption", ROCKER_CAPTION.y - 6.0, ROCKER_CAPTION.y + 6.0),
        ("the rocker's tape", ROCKER_TAPE.y, ROCKER_TAPE.y + 17.0),
        ("the Auto lamp", AUTO_LED.y - LAMP, AUTO_LED.y + LAMP),
        ("the Auto button", AUTO_BUTTON.y - BUTTON, AUTO_BUTTON.y + BUTTON + 2.5),
        ("the Auto caption", AUTO_CAPTION.y - 7.0, AUTO_CAPTION.y + 7.0),
    ]);
    assert_eq!(AUTO_LED.x, AUTO_BUTTON.x);
    assert_eq!(AUTO_CAPTION.x, AUTO_BUTTON.x);
    // The Auto button stands beside the On switch, not over it, and inside the face
    in_a_row(&[
        ("the On switch", -SWITCH, SWITCH),
        ("the Auto button", AUTO_BUTTON.x - BUTTON, AUTO_BUTTON.x + BUTTON),
        ("the face's edge", PEDAL_WIDTH / 2.0 - 10.0, PEDAL_WIDTH / 2.0),
    ]);
    // Down the left: Bite's tape is over Sens's scale
    assert!(bite.y + 68.0 <= sens.y - knob, "Bite's tape runs into Sens");
    assert_eq!(bite.x, sens.x);
}

/// A machine with two interfaces, for the standalone window's tests
#[cfg(target_os = "linux")]
struct TwoInterfaces;
#[cfg(target_os = "linux")]
impl standalone::Host for TwoInterfaces {
    fn drivers(&self) -> Vec<String> {
        ["auto", "alsa", "jack"].map(String::from).to_vec()
    }
    fn devices(&self, _: &str) -> standalone::DriverDevices {
        standalone::DriverDevices {
            inputs: vec!["Scarlett 2i2 USB".into(), "Built-in Microphone".into()],
            outputs: vec!["Scarlett 2i2 USB".into(), "Speakers".into()],
        }
    }
    fn restart(&self, _: &Prefs) -> std::io::Result<()> {
        Ok(())
    }
    fn channels(&self, _: &str, _: bool, device: Option<&str>) -> u32 {
        // The interface has four of each, the machine's own devices a pair
        if device == Some("Scarlett 2i2 USB") {
            4
        } else {
            2
        }
    }
}

// The settings plate in a plugin: the cabinet files alone, under the icon
#[test]
#[cfg(target_os = "linux")]
fn test_the_settings_plate_looks_as_it_did() {
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    harness_of(&params, &setter, None, |view| view.plate = Some(Open::Settings(None)))
        .snapshot("settings");
}

// The standalone program's settings plate: the cabinet files and the machine's audio
#[test]
#[cfg(target_os = "linux")]
fn test_the_standalone_audio_settings_look_as_they_did() {
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let machine = TwoInterfaces;
    let prefs = Prefs {
        backend: "alsa".into(),
        input_device: Some("Scarlett 2i2 USB".into()),
        input_channel: 3,
        ..Prefs::default()
    };
    harness_with(&params, &setter, None, Some(&machine), |view| {
        view.plate = Some(Open::Settings(Some(AudioChoice::open(&machine, prefs))));
    })
    .snapshot("audio_panel");
}

// The tuner in the place of the preset display, hearing a low E a little flat
#[test]
#[cfg(target_os = "linux")]
fn test_the_tuner_looks_as_it_did() {
    let params = GuitarAmpParams {
        tuner_on: BoolParam::new("Tuner", true).non_automatable(),
        ..GuitarAmpParams::default()
    };
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    harness_of(&params, &setter, Some(81.0), |_| {}).snapshot("tuner");
}

// The metronome running, its plate open: its lamp dim between beats, Sync on, six eighths, the
// Click and its output apart
#[test]
#[cfg(target_os = "linux")]
fn test_the_metronome_looks_as_it_did() {
    let params = GuitarAmpParams {
        metronome_on: BoolParam::new("Metronome", true),
        metronome_sync: BoolParam::new("Metronome Sync", true),
        metronome_time_sig: EnumParam::new("Metronome Time Signature", TimeSignature::SixEight),
        metronome_sound: EnumParam::new("Metronome Sound", Sound::Click),
        // The longer of the click output's two names, the one that must fit its tape
        metronome_out: EnumParam::new("Metronome Output", ClickOut::Separate),
        ..GuitarAmpParams::default()
    };
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    harness_of(&params, &setter, None, |view| view.plate = Some(Open::Metronome))
        .snapshot("metronome");
}

/// The looper's plate open with the looper in `state`, for the pictures: a 40 second buffer,
/// recording 12 seconds of it, or a 10 second loop three tenths of the way round
#[cfg(target_os = "linux")]
fn looper_picture(name: &str, state: LoopState) {
    let rate = 48_000;
    let (length, position) = match state {
        LoopState::Recording => (12 * rate, 0),
        LoopState::Empty => (0, 0),
        _ => (10 * rate, 10 * rate * 3 / 10),
    };
    loop_shows(state, length, position, 40 * rate);
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    harness_of(&params, &setter, None, |view| view.plate = Some(Open::Looper)).snapshot(name);
}

// The looper's plate open, in each state it can be in: the ring, the words in it and under
// the switch, Stop's name, and the lamp of the strip's icon
#[test]
#[cfg(target_os = "linux")]
fn test_the_looper_looks_as_it_did_empty() {
    looper_picture("looper_empty", LoopState::Empty);
}

#[test]
#[cfg(target_os = "linux")]
fn test_the_looper_looks_as_it_did_recording() {
    looper_picture("looper_recording", LoopState::Recording);
}

#[test]
#[cfg(target_os = "linux")]
fn test_the_looper_looks_as_it_did_playing() {
    looper_picture("looper_playing", LoopState::Playing);
}

#[test]
#[cfg(target_os = "linux")]
fn test_the_looper_looks_as_it_did_overdubbing() {
    looper_picture("looper_overdubbing", LoopState::Overdubbing);
}

#[test]
#[cfg(target_os = "linux")]
fn test_the_looper_looks_as_it_did_muted() {
    looper_picture("looper_muted", LoopState::Stopped);
}

// The plate's buttons are the looper's commands: the big switch presses, Stop stops, Clear
// clears, and the strip icon's own switch presses as the big one does
#[test]
#[cfg(target_os = "linux")]
fn test_the_looper_plates_buttons_and_the_icons_switch_send_the_commands() {
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let plate = Pos2::new(LOOPER_X - ui::icons::ICON_REACH, PLATE_TOP);
    let (mut harness, _) =
        harness_shared(&params, &setter, None, None, |view| view.plate = Some(Open::Looper));
    let control = test_looper();
    assert_eq!(control.pending(), (0, false, false));
    click(&mut harness, plate + LOOPER_RING);
    assert_eq!(control.pending(), (1, false, false), "the big switch");
    click(&mut harness, plate + vec2(LOOPER_BUTTONS_X, LOOPER_STOP[1]));
    assert_eq!(control.pending(), (1, true, false), "Stop");
    click(&mut harness, plate + vec2(LOOPER_BUTTONS_X, LOOPER_CLEAR[1]));
    assert_eq!(control.pending(), (1, true, true), "Clear");
    // The strip's switch is the same press (and, being outside the plate, closes it)
    click(&mut harness, Pos2::new(LOOPER_X + ui::icons::SWITCH_DX, STRIP_Y));
    assert_eq!(control.pending().0, 2, "the icon's switch");
}

// What the plate says of each state: the ring, the two words and Stop's name
#[test]
fn test_the_looper_plate_says_what_the_loop_is_doing_and_what_a_press_does_next() {
    use LoopState::*;
    let all = [Empty, Recording, Playing, Overdubbing, Stopped];
    // A press walks Record, Play, Overdub and back to Play
    let next: Vec<_> = all.iter().map(|&state| loop_words(state).1).collect();
    assert_eq!(next, ["Record", "Play", "Overdub", "Play", "Play"]);
    assert_eq!(stop_word(Playing), "Mute");
    assert_eq!(stop_word(Overdubbing), "Mute");
    assert_eq!(stop_word(Stopped), "Resume");
    assert_eq!(stop_word(Empty), "Stop");
    // Nothing burns when there is nothing to hear; a recording fills against the buffer
    assert_eq!(loop_ring(Empty, 0, 0, 100), ui::Ring::Dark);
    assert_eq!(loop_ring(Stopped, 50, 10, 100), ui::Ring::Dark);
    assert_eq!(
        loop_ring(Recording, 25, 0, 100),
        ui::Ring::Filled { fraction: 0.25, color: ui::RING_RED }
    );
    // A loop that plays has its head where its position is, in its length
    assert_eq!(
        loop_ring(Playing, 100, 50, 400),
        ui::Ring::Head { fraction: 0.5, color: ui::RING_GREEN }
    );
    assert_eq!(
        loop_ring(Overdubbing, 100, 25, 400),
        ui::Ring::Head { fraction: 0.25, color: ui::RING_AMBER }
    );
    // No length, no capacity: no dividing by nothing
    assert_eq!(
        loop_ring(Playing, 0, 0, 0),
        ui::Ring::Head { fraction: 0.0, color: ui::RING_GREEN }
    );
    // The strip's lamp: only a loop that sounds or records lights it
    assert_eq!(loop_lamp(Empty), Beat::Dark);
    assert_eq!(loop_lamp(Stopped), Beat::Dark);
    assert_ne!(loop_lamp(Recording), loop_lamp(Playing));
    assert_ne!(loop_lamp(Playing), loop_lamp(Overdubbing));
}

// Nothing on the looper's plate runs into anything else or over the plate's edge: the ring
// with its lamps and the switch in it, the buttons with their captions, the knob
#[test]
fn test_the_looper_plate_stands_clear_of_itself_and_of_its_edges() {
    let ring_reach = LOOPER_RING_RADIUS + ui::ring::RING_LAMP_RADIUS;
    let around = |at: Vec2, half: Vec2| Rect::from_center_size(at.to_pos2(), 2.0 * half);
    let parts = [
        ("the ring", around(LOOPER_RING, Vec2::splat(ring_reach))),
        ("the stop caption", around(vec2(LOOPER_BUTTONS_X, LOOPER_STOP[0]), vec2(24.0, 7.0))),
        ("the stop button", around(vec2(LOOPER_BUTTONS_X, LOOPER_STOP[1]), Vec2::splat(BUTTON))),
        ("the clear caption", around(vec2(LOOPER_BUTTONS_X, LOOPER_CLEAR[0]), vec2(24.0, 7.0))),
        ("the clear button", around(vec2(LOOPER_BUTTONS_X, LOOPER_CLEAR[1]), Vec2::splat(BUTTON))),
        (
            "the level knob and its tape",
            Rect::from_min_max(
                (LOOPER_LEVEL - Vec2::splat(LOOPER_LEVEL_RADIUS + TICKS)).to_pos2(),
                (LOOPER_LEVEL + vec2(LOOPER_LEVEL_RADIUS + TICKS, LOOPER_LEVEL_RADIUS + 46.0))
                    .to_pos2(),
            ),
        ),
    ];
    let plate = Rect::from_min_size(Pos2::ZERO, vec2(LOOPER_WIDTH, LOOPER_HEIGHT));
    for (index, (name, rect)) in parts.iter().enumerate() {
        for (other, other_rect) in &parts[index + 1..] {
            assert!(!rect.expand(2.0).intersects(*other_rect), "{name} runs into {other}");
        }
        assert!(plate.shrink(6.0).contains_rect(*rect), "{name} is over the plate's edge");
    }
    // The state's word is in the ring over the switch (74 across), the next action's under
    // it, both inside the ring's lamps
    const { assert!(-LOOPER_STATE_DY > 37.0 && LOOPER_NEXT_DY > 37.0) };
    const { assert!(-LOOPER_STATE_DY + 7.0 < LOOPER_RING_RADIUS - 3.6) };
    const { assert!(LOOPER_NEXT_DY + 7.0 < LOOPER_RING_RADIUS - 3.6) };
    // The title is above the ring
    assert!(LOOPER_RING.y - ring_reach > ui::plate::PLATE_TITLE_HEIGHT - 4.0);
}

/// The presets folder of a test: one that is not there, so the banks are the two every player
/// has, whatever is on this machine
#[cfg(target_os = "linux")]
fn no_presets_folder() {
    let nowhere = std::env::temp_dir().join("hojt-snapshot-no-presets");
    std::env::set_var(user_presets::FOLDER_ENV, &nowhere);
    assert!(!nowhere.exists(), "{nowhere:?} is there: the list would show its banks");
}

/// The preset plate on the factory bank
fn presets_open(view: &mut View) {
    view.plate = Some(Open::Presets(Menu {
        listed: user_presets::FACTORY.to_owned(),
        files: Vec::new(),
        naming: None,
    }));
}

// The preset plate open on the factory bank, under the display, with its foot
#[test]
#[cfg(target_os = "linux")]
fn test_the_preset_menu_looks_as_it_did() {
    let _folder = FOLDER_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    no_presets_folder();
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    harness_of(&params, &setter, None, presets_open).snapshot("preset_menu");
}

/// How far a stepper's buttons stand out from the middle of its tape
fn reach(width: f32) -> f32 {
    width / 2.0 + 22.0
}

// A stepper's button and a small footswitch, half across; how far a knob's ticks reach
// out from its rim, and how far a lamp of a meter or a signal lamp reaches from its middle
const BUTTON: f32 = 11.0;
const SWITCH: f32 = 25.0;
const TICKS: f32 = 13.0;
const LAMP: f32 = 11.0;

/// From left to right, the room each control of a wing takes across, from the window's
/// left edge: each one must end before the next begins
fn in_a_row(wing: &[(&str, f32, f32)]) {
    for pair in wing.windows(2) {
        let ((a, _, a_right), (b, b_left, _)) = (pair[0], pair[1]);
        assert!(a_right + 4.0 < b_left, "{a} runs into {b}");
    }
}

/// The top face of the strip, in window points
fn strip_face() -> Rect {
    strip_face_shifted(0.0)
}

/// The top face of a strip `shift` points wider than the default
fn strip_face_shifted(shift: f32) -> Rect {
    Rect::from_min_max(
        Pos2::new(BENCH_MARGIN, BENCH_MARGIN),
        Pos2::new(
            WINDOW_WIDTH as f32 + shift - BENCH_MARGIN,
            BENCH_MARGIN + SLIM_STRIP_HEIGHT - 5.0,
        ),
    )
}

/// The strips of the widths a window may give it: the narrowest, today's, a very wide one
fn strip_shifts() -> [f32; 3] {
    [STRIP_MIN_WIDTH - WINDOW_WIDTH as f32, 0.0, 2600.0]
}

/// What stands on the strip, and the room each part takes, in points from the window's top
/// left corner, from the left to the right of the strip: the print of `ui::slim_strip` (the
/// jacks with their captions, the model number, the logo) and every control
/// `strip_parts` of a strip `shift` points wider than the default: what is at its right end
/// moves with it, and what is in the middle half as far
fn strip_parts_shifted(shift: f32) -> Vec<(&'static str, Rect)> {
    let print = ui::slim_print(strip_face_shifted(shift), &STYLE);
    let (right, middle) = (vec2(shift, 0.0), vec2(shift / 2.0, 0.0));
    let around = |at: Vec2, half: Vec2| Rect::from_center_size(at.to_pos2(), 2.0 * half);
    let meter =
        |at: Vec2| around(at, vec2(METER_WIDTH / 2.0, ui::widgets::WIDE_METER_HEIGHT / 2.0 + 2.0));
    let caption =
        |at: Vec2, width: f32| around(vec2(at.x, METER_CAPTION_Y), vec2(width / 2.0, 7.0));
    let knob = |at: Vec2| around(at, Vec2::splat(KNOB_REACH));
    let icon = |x: f32, switch: bool| {
        Rect::from_min_max(
            Pos2::new(x - ui::icons::ICON_REACH, STRIP_Y - ui::icons::ICON_SIZE / 2.0),
            Pos2::new(
                x + if switch { ui::icons::SWITCH_REACH } else { ui::icons::ICON_REACH },
                STRIP_Y + ui::icons::ICON_SIZE / 2.0,
            ),
        )
    };
    vec![
        ("the input jack", Rect::from_center_size(print.jacks[0], Vec2::splat(16.0))),
        ("the input jack's caption", print.captions[0]),
        ("the input meter", meter(INPUT_METER)),
        ("the input meter's caption", caption(INPUT_METER, 52.0)),
        ("the Input knob", knob(INPUT_KNOB)),
        (
            "the preset display",
            around(vec2(PRESET_X, STRIP_Y), vec2(PRESET_WIDTH / 2.0, LED_HEIGHT / 2.0)),
        ),
        ("the metronome", icon(METRONOME_X + middle.x, true)),
        ("the looper", icon(LOOPER_X + middle.x, true)),
        ("the tuner", icon(TUNER_X + middle.x, true)),
        ("the settings", icon(SETTINGS_X + middle.x, false)),
        ("the output knob", knob(OUTPUT_KNOB + right)),
        ("the output meter", meter(OUTPUT_METER + right)),
        ("the output meter's caption", caption(OUTPUT_METER + right, 62.0)),
        ("the clip lamp", around(CLIP_LAMP + right, Vec2::splat(LAMP))),
        ("the clip lamp's caption", around(CLIP_LAMP + CLIP_CAPTION + right, vec2(24.0, 7.0))),
        ("the model number", print.model),
        ("the logo", print.logo),
        ("the output jack", Rect::from_center_size(print.jacks[1], Vec2::splat(16.0))),
        ("the output jack's caption", print.captions[1]),
    ]
}

#[test]
fn test_nothing_on_the_strip_overlaps_and_it_reads_from_the_left_to_the_right() {
    for shift in strip_shifts() {
        nothing_overlaps_on_a_strip_wider_by(shift);
    }
}

fn nothing_overlaps_on_a_strip_wider_by(shift: f32) {
    let parts = strip_parts_shifted(shift);
    for (index, (name, rect)) in parts.iter().enumerate() {
        for (other, other_rect) in &parts[index + 1..] {
            assert!(
                !rect.expand(2.0).intersects(*other_rect),
                "{name} {rect:?} runs into {other} {other_rect:?}"
            );
        }
    }
    // The signal runs from left to right: what comes in, the preset, the icons, what goes out
    let at = |name: &str| parts.iter().find(|part| part.0 == name).unwrap().1.center().x;
    let order = [
        "the input jack",
        "the input meter",
        "the Input knob",
        "the preset display",
        "the metronome",
        "the looper",
        "the tuner",
        "the settings",
        "the output knob",
        "the output meter",
        "the clip lamp",
        "the model number",
        "the logo",
        "the output jack",
    ];
    for pair in order.windows(2) {
        assert!(at(pair[0]) < at(pair[1]), "{} is not left of {}", pair[0], pair[1]);
    }
    // Room to spare between the groups, as many points before the icons as after them
    let room = |from: &str, to: &str| {
        let find = |name: &str| parts.iter().find(|part| part.0 == name).unwrap().1;
        find(to).left() - find(from).right()
    };
    let (before, after) =
        (room("the preset display", "the metronome"), room("the settings", "the output knob"));
    assert!((before - after).abs() < 0.5 && before > 40.0, "{before} {after} at {shift}");
}

#[test]
fn test_nothing_on_the_strip_reaches_its_edges_or_the_line_under_it() {
    for shift in strip_shifts() {
        let face = strip_face_shifted(shift);
        for (name, rect) in strip_parts_shifted(shift) {
            assert!(
                face.shrink(2.0).contains_rect(rect),
                "{name} {rect:?} is not on the strip's face {face:?} wider by {shift}"
            );
        }
    }
    // The strip is slim, and the line starts under it, a bench margin away from its box
    assert_eq!(STRIP_HEIGHT, 64.0);
    assert_eq!(STACK_TOP, BENCH_MARGIN + STRIP_HEIGHT + BENCH_MARGIN);
    // The plates open under the box, never over the strip's controls
    const { assert!(PLATE_TOP > BENCH_MARGIN + STRIP_HEIGHT) };
    // The controls end where the print begins
    let model = ui::slim_print(strip_face(), &STYLE).model;
    assert!((MODEL_LEFT - model.left()).abs() < 0.01);
    // The center line: every row of controls is on it, as the icons are
    assert_eq!(STRIP_Y, INPUT_KNOB.y);
    assert_eq!(STRIP_Y, OUTPUT_KNOB.y);
}

#[test]
fn test_the_strip_holds_the_preset_display_the_tuner_takes_the_place_of() {
    // The tuner's display is the preset display's size, in its place
    assert_eq!(led_width(PRESET_CHARS), PRESET_WIDTH);
    // What the display says is cut two characters short of its end, for the marker
    const { assert!(PRESET_CHARS > BANK_SHOWN_CHARS + 3 + USER_NAME_CHARS + 1 - 3) };
}

// The input and the output are made alike: a meter with its caption over it, a knob beside it
#[test]
fn test_the_input_and_the_output_are_made_alike() {
    assert_eq!(INPUT_METER.y, OUTPUT_METER.y);
    assert_eq!(INPUT_KNOB.y, OUTPUT_KNOB.y);
    // The input's knob is right of its meter, the output's left of its meter
    const { assert!(INPUT_KNOB.x > INPUT_METER.x && OUTPUT_KNOB.x < OUTPUT_METER.x) };
    let gap = |meter: Vec2, knob: Vec2| (meter.x - knob.x).abs() - METER_REACH - KNOB_REACH;
    assert_eq!(gap(INPUT_METER, INPUT_KNOB), gap(OUTPUT_METER, OUTPUT_KNOB));
}

#[test]
fn test_the_icons_stand_a_pitch_apart_between_the_preset_display_and_the_output() {
    // The room left is shared: as much before the icons as after them
    let before = METRONOME_X - ui::icons::ICON_REACH - PRESET_END;
    let after = OUTPUT_KNOB.x - KNOB_REACH - (SETTINGS_X + ui::icons::ICON_REACH);
    assert!((before - after).abs() < 0.01 && before > 40.0, "{before} {after}");
    assert_eq!(LOOPER_X - METRONOME_X, ICON_PITCH);
    assert_eq!(TUNER_X - LOOPER_X, SETTINGS_X - TUNER_X);
    assert_eq!(TUNER_X - LOOPER_X, ICON_PITCH);
    // An icon with a switch reaches the next icon's room, no further
    const { assert!(METRONOME_X + ui::icons::SWITCH_REACH + 4.0 < LOOPER_X - ui::icons::ICON_REACH) };
    const { assert!(LOOPER_X + ui::icons::SWITCH_REACH + 4.0 < TUNER_X - ui::icons::ICON_REACH) };
    const { assert!(TUNER_X + ui::icons::SWITCH_REACH + 4.0 < SETTINGS_X - ui::icons::ICON_REACH) };
}

/// The plate each of the three opens, where it stands in the window
fn plate_rects() -> [(&'static str, Rect, Rect); 4] {
    let preset_plate_width = DROP_BANK_WIDTH + DROP_PRESET_WIDTH + 2.0 * (5.0 + 4.0) + 12.0 + 20.0;
    let below = |left: f32, width: f32, height: f32| {
        Rect::from_min_size(Pos2::new(left, PLATE_TOP), vec2(width, height))
    };
    [
        (
            "the preset plate",
            below(PRESET_X - preset_plate_width / 2.0, preset_plate_width, 260.0),
            Rect::from_center_size(Pos2::new(PRESET_X, STRIP_Y), vec2(PRESET_WIDTH, LED_HEIGHT)),
        ),
        (
            "the metronome's plate",
            below(METRONOME_X - ui::icons::ICON_REACH, METRONOME_WIDTH, METRONOME_HEIGHT),
            Rect::from_center_size(
                Pos2::new(METRONOME_X, STRIP_Y),
                Vec2::splat(ui::icons::ICON_SIZE),
            ),
        ),
        (
            "the looper's plate",
            below(LOOPER_X - ui::icons::ICON_REACH, LOOPER_WIDTH, LOOPER_HEIGHT),
            Rect::from_center_size(Pos2::new(LOOPER_X, STRIP_Y), Vec2::splat(ui::icons::ICON_SIZE)),
        ),
        (
            "the settings plate",
            below(
                SETTINGS_X + ui::icons::ICON_REACH - SETTINGS_WIDTH,
                SETTINGS_WIDTH,
                settings_height(true, 9),
            ),
            Rect::from_center_size(
                Pos2::new(SETTINGS_X, STRIP_Y),
                Vec2::splat(ui::icons::ICON_SIZE),
            ),
        ),
    ]
}

#[test]
fn test_each_plate_opens_under_the_control_that_opens_it_and_inside_the_window() {
    let window = Rect::from_min_size(Pos2::ZERO, vec2(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32));
    for (name, plate, opener) in plate_rects() {
        assert!(window.contains_rect(plate), "{name} {plate:?} is out of the window");
        // Under the strip's box, and its opener is over it, not in its way
        assert!(plate.top() > BENCH_MARGIN + STRIP_HEIGHT, "{name}");
        assert!(opener.bottom() < plate.top(), "{name}");
        // Its edge lines up with the opener's: the display's left edge, or under the icon
        assert!(
            plate.left() <= opener.center().x && opener.center().x <= plate.right(),
            "{name} is not under its opener"
        );
    }
}

#[test]
fn test_the_preset_plates_foot_holds_the_name_the_folder_and_save_apart() {
    let foot = DROP_BANK_WIDTH + DROP_PRESET_WIDTH + 2.0 * (5.0 + 4.0) + 12.0;
    let save = foot - FOOT_SAVE_REACH + 3.0;
    let folder = save - FOOT_BUTTON_SPREAD;
    in_a_row(&[
        ("the name display", 0.0, led_width(FOOT_CHARS)),
        ("the Folder button", folder - BUTTON, folder + BUTTON),
        ("the Save button", save - BUTTON, save + BUTTON),
        ("the plate's edge", foot, foot),
    ]);
    // Their captions are under them and clear of each other
    in_a_row(&[
        ("the Folder caption", folder - 20.0, folder + 20.0),
        ("the Save caption", save - 15.0, save + 15.0),
    ]);
    // The display and the buttons with their captions fit the foot's height
    const { assert!(20.0 + LED_HEIGHT / 2.0 < FOOT_HEIGHT) };
    const { assert!(14.0 + 22.0 + 7.0 < FOOT_HEIGHT) };
    // A name typed fits the display, with the caret
    const { assert!(NEW_PRESET_SHOWN < FOOT_CHARS) };
}

#[test]
fn test_the_metronomes_plate_stands_clear_of_itself_and_of_its_edges() {
    let stepper_reach = |width: f32| width / 2.0 + STEPPER_REACH;
    in_a_row(&[
        ("the plate's edge", 0.0, METRONOME_PAD - 8.0),
        ("the tempo", TEMPO_X - stepper_reach(TEMPO_WIDTH), TEMPO_X + stepper_reach(TEMPO_WIDTH)),
        ("Tap", TAP_X - SAVE_REACH, TAP_X + SAVE_REACH),
        ("the steppers", STEPPERS_X - COLUMN_WIDTH / 2.0, STEPPERS_X + COLUMN_WIDTH / 2.0),
        ("the volume", CLICK_VOLUME.x - CLICK_REACH, CLICK_VOLUME.x + CLICK_REACH),
        ("Sync", METRONOME_SYNC_X - SAVE_REACH, METRONOME_SYNC_X + SAVE_REACH),
        ("the plate's edge", METRONOME_WIDTH - METRONOME_PAD + 8.0, METRONOME_WIDTH),
    ]);
    // The narrower stepper is inside the column the wider one makes
    const { assert!(SIGNATURE_WIDTH < SOUND_WIDTH) };
    // Under the sound, the meter and the click's output side by side, clear of each other
    // and inside the column, which the sound's stepper is centred over
    in_a_row(&[
        (
            "the meter",
            SIGNATURE_X - stepper_reach(SIGNATURE_WIDTH),
            SIGNATURE_X + stepper_reach(SIGNATURE_WIDTH),
        ),
        (
            "the click's output",
            CLICK_OUT_X - stepper_reach(OUT_WIDTH),
            CLICK_OUT_X + stepper_reach(OUT_WIDTH),
        ),
    ]);
    assert!(SIGNATURE_X - stepper_reach(SIGNATURE_WIDTH) >= STEPPERS_X - COLUMN_WIDTH / 2.0 - 0.01);
    assert!(CLICK_OUT_X + stepper_reach(OUT_WIDTH) <= STEPPERS_X + COLUMN_WIDTH / 2.0 + 0.01);
    // Top to bottom in the column: the title, then caption, tape, caption, tape, each clear
    // of the next, all on the plate
    let tape = 11.0;
    let caption = 7.0;
    let column = [
        ui::plate::PLATE_TITLE_HEIGHT,
        SOUND_CAPTION_Y - caption,
        SOUND_CAPTION_Y + caption,
        SOUND_Y - tape,
        SOUND_Y + tape,
        SIGNATURE_CAPTION_Y - caption,
        SIGNATURE_CAPTION_Y + caption,
        SIGNATURE_Y - tape,
        SIGNATURE_Y + tape,
    ];
    assert!(column.windows(2).all(|pair| pair[0] < pair[1]), "{column:?}");
    assert!(column[8] + 8.0 < METRONOME_HEIGHT);
    // The tempo's display and its caption, and Tap, on the same rows as the sound's caption
    assert_eq!(TEMPO_CAPTION_Y, SOUND_CAPTION_Y);
    const { assert!(TEMPO_CAPTION_Y + 7.0 < TEMPO_Y - LED_HEIGHT / 2.0) };
    const { assert!(TEMPO_Y + LED_HEIGHT / 2.0 < METRONOME_HEIGHT) };
    // The volume knob, with its caption and its tape, ends on the plate
    let tape_bottom = CLICK_VOLUME.y + CLICK_RADIUS + 18.0 + 15.0 + 2.0 + 16.0;
    assert!(tape_bottom + 4.0 < METRONOME_HEIGHT, "{tape_bottom}");
    // The Sync button under its LED under its caption, on the plate
    let [caption, led, button] = SYNC_STACK;
    assert!(caption + 8.0 < led - 12.0 && led + 12.0 < button - BUTTON - 2.0);
    assert!(button + BUTTON < METRONOME_HEIGHT);
}

#[test]
fn test_the_settings_plate_holds_every_row_the_note_and_the_buttons() {
    // A row's stepper reaches its buttons past its tape, and all of it is on the plate
    let row = SETTINGS_PAD
        + SETTINGS_CAPTION_WIDTH
        + SETTINGS_OVERHANG
        + SETTINGS_TAPE_WIDTH
        + SETTINGS_OVERHANG;
    assert!(row + SETTINGS_PAD <= SETTINGS_WIDTH + 0.01);
    // The audio's eight rows and the cabinet's, the note and the buttons grow the plate
    let (plugin, standalone) = (settings_height(false, 1), settings_height(true, 9));
    assert!(plugin < standalone);
    assert_eq!(standalone - plugin, 8.0 * SETTINGS_ROW_HEIGHT + SETTINGS_FOOT_HEIGHT,);
    // The longest cabinet name fits the tape
    assert!(SETTINGS_CAB_CHARS as f32 * 8.5 < SETTINGS_TAPE_WIDTH);
    // The plate is on the window under the icon, with room under it for the pedals' titles
    assert!(PLATE_TOP + standalone < WINDOW_HEIGHT as f32);
}

#[test]
fn test_a_stepper_goes_round_its_list() {
    use TimeSignature::*;
    assert_eq!(neighbour(&TimeSignature::ALL, SixEight, Step::Next), TwoFour);
    assert_eq!(neighbour(&TimeSignature::ALL, TwoFour, Step::Previous), SixEight);
    assert_eq!(neighbour(&Sound::ALL, Sound::Beep, Step::Next), Sound::Click);
}

// One plate at a time, by the shape of the view: a field holds the plate that is open, so
// opening one is closing the other
#[test]
fn test_opening_a_plate_closes_the_one_that_was_open() {
    let params = GuitarAmpParams::default();
    let mut view = fresh_view(&params, &LampReading::new());
    assert!(view.plate.is_none());
    for which in
        [Plates::Presets, Plates::Metronome, Plates::Looper, Plates::Settings, Plates::Metronome]
    {
        let open = |which| {
            move || match which {
                Plates::Presets => Open::Presets(Menu {
                    listed: user_presets::FACTORY.to_owned(),
                    files: Vec::new(),
                    naming: None,
                }),
                Plates::Metronome => Open::Metronome,
                Plates::Looper => Open::Looper,
                Plates::Settings => Open::Settings(None),
            }
        };
        view.toggle_plate(which, None, open(which));
        // It is the plate that is open, and no other
        for other in [Plates::Presets, Plates::Metronome, Plates::Looper, Plates::Settings] {
            assert_eq!(view.is_open(other), other == which, "{other:?} after {which:?}");
        }
    }
    // Its own icon again closes it
    view.toggle_plate(Plates::Metronome, None, || Open::Metronome);
    assert!(view.plate.is_none());
    // A name being typed does not outlast the plate it was typed on
    view.saving = Some("Big".to_owned());
    view.close_plate(None);
    assert!(view.saving.is_none());
}

/// Where the icon of `plate` is, in window points
#[cfg(target_os = "linux")]
fn opener(plate: Plates) -> Pos2 {
    match plate {
        Plates::Presets => Pos2::new(PRESET_X, STRIP_Y),
        Plates::Metronome => Pos2::new(METRONOME_X, STRIP_Y),
        Plates::Looper => Pos2::new(LOOPER_X, STRIP_Y),
        Plates::Settings => Pos2::new(SETTINGS_X, STRIP_Y),
    }
}

// The same by clicks in the window: an icon opens its plate and the plate that was open is
// gone, the same icon closes it, and so do a click outside and escape
#[test]
#[cfg(target_os = "linux")]
fn test_no_plate_opens_beside_another() {
    let _folder = FOLDER_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    no_presets_folder();
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = harness_shared(&params, &setter, None, None, |_| {});
    let open = |view: &Rc<RefCell<View>>| -> Vec<Plates> {
        [Plates::Presets, Plates::Metronome, Plates::Looper, Plates::Settings]
            .into_iter()
            .filter(|&plate| view.borrow().is_open(plate))
            .collect()
    };
    assert!(open(&view).is_empty());
    for plate in [
        Plates::Metronome,
        Plates::Looper,
        Plates::Settings,
        Plates::Presets,
        Plates::Metronome,
        Plates::Presets,
    ] {
        click(&mut harness, opener(plate));
        assert_eq!(open(&view), [plate], "after a click on the icon of {plate:?}");
    }
    // The icon of the plate that is open closes it
    click(&mut harness, opener(Plates::Presets));
    assert!(open(&view).is_empty());
    // A click outside it closes it: on the bench between the strip and the line
    click(&mut harness, opener(Plates::Metronome));
    assert_eq!(open(&view), [Plates::Metronome]);
    click(&mut harness, Pos2::new(3.0, 300.0));
    assert!(open(&view).is_empty(), "a click outside left {:?}", open(&view));
    // And so does escape
    click(&mut harness, opener(Plates::Settings));
    assert_eq!(open(&view), [Plates::Settings]);
    harness.press_key(nih_plug_egui::egui::Key::Escape);
    harness.run_ok();
    assert!(open(&view).is_empty(), "escape left {:?}", open(&view));
}

// A click inside a plate is the plate's: it neither closes it nor opens another
#[test]
#[cfg(target_os = "linux")]
fn test_a_click_on_a_plate_leaves_it_open() {
    let params = GuitarAmpParams::default();
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) =
        harness_shared(&params, &setter, None, None, |view| view.plate = Some(Open::Metronome));
    // On its title, where there is no control
    click(&mut harness, Pos2::new(METRONOME_X + 60.0, PLATE_TOP + 19.0));
    assert!(view.borrow().is_open(Plates::Metronome));
}

/// A folder for a test of its own, made empty, which the presets are kept in while `body` runs
#[cfg(target_os = "linux")]
fn with_presets_in(name: &str, body: impl FnOnce(&std::path::Path)) {
    let _folder = FOLDER_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let dir = crate::user_cab::tests::test_dir(name);
    std::env::set_var(user_presets::FOLDER_ENV, &dir);
    body(&dir);
    let _ = std::fs::remove_dir_all(&dir);
}

// #264: a name for a new preset may hold spaces. Save is pressed, the name is typed in the
// plate's foot with its space as the host hands it over, and enter keeps it
#[test]
#[cfg(target_os = "linux")]
fn test_a_new_preset_is_saved_under_a_name_with_spaces() {
    use nih_plug_egui::egui::Key;
    with_presets_in("editor-names-with-spaces", |dir| {
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = harness_shared(&params, &setter, None, None, |view| {
            presets_open(view);
            view.saving = Some(String::new());
        });
        // A space that came as text
        type_text(&mut harness, "Big");
        type_text(&mut harness, " ");
        type_text(&mut harness, "Mud");
        assert_eq!(view.borrow().saving.as_deref(), Some("Big Mud"));
        harness.press_key(Key::Enter);
        harness.run_ok();
        assert!(view.borrow().saving.is_none());
        let saved = crate::user_cab::list_with_extension(dir, user_presets::EXTENSION);
        assert_eq!(saved, ["Big Mud.preset"]);
        // The list of the bank has it, with the space
        assert_eq!(view.borrow().bank.files, ["Big Mud.preset"]);
    });
}

// The same for a host that takes the space bar for itself and only tells the window the key
// was pressed, with no character: the name still has its space
#[test]
#[cfg(target_os = "linux")]
fn test_a_space_the_host_hands_over_as_a_key_press_is_in_the_name() {
    use nih_plug_egui::egui::Key;
    with_presets_in("editor-names-space-key", |dir| {
        let params = GuitarAmpParams::default();
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = harness_shared(&params, &setter, None, None, |view| {
            presets_open(view);
            view.saving = Some(String::new());
        });
        type_text(&mut harness, "Big");
        harness.press_key(Key::Space);
        harness.run_ok();
        type_text(&mut harness, "Mud");
        assert_eq!(view.borrow().saving.as_deref(), Some("Big Mud"));
        harness.press_key(Key::Enter);
        harness.run_ok();
        let saved = crate::user_cab::list_with_extension(dir, user_presets::EXTENSION);
        assert_eq!(saved, ["Big Mud.preset"]);
    });
}

// A name is kept as typed when the Save button is pressed a second time: names with spaces
// are written under file names with spaces, and read back as they were typed
#[test]
fn test_a_name_with_spaces_makes_a_file_with_spaces() {
    let taken = vec!["Big Mud.preset".to_owned()];
    assert_eq!(user_presets::named_file_name("Big Mud", &[]).as_deref(), Some("Big Mud.preset"));
    // A name that is taken gets a number, after its space
    assert_eq!(
        user_presets::named_file_name("Big Mud", &taken).as_deref(),
        Some("Big Mud 2.preset")
    );
    // The spaces at its ends are not part of it, those inside are
    assert_eq!(
        user_presets::named_file_name("  Big  Mud ", &[]).as_deref(),
        Some("Big  Mud.preset")
    );
    assert_eq!(user_presets::display_name("Big Mud.preset", 17), "BIG MUD");
}

#[test]
fn test_the_delays_sync_button_stands_clear_of_the_knobs_and_on_the_face() {
    let knob = TRIO_RADIUS + TICKS;
    let mix = TRIO_KNOBS[2];
    // Across, right of the Mix knob's scale and its tape, inside the face
    for (name, y) in [("caption", SYNC_CAPTION.y), ("LED", SYNC_LED.y), ("button", SYNC_BUTTON.y)] {
        if (y - mix.y).abs() < knob + 4.0 {
            in_a_row(&[
                ("the Mix knob", mix.x - knob, mix.x + knob),
                (name, SYNC_LED.x - 20.0, SYNC_LED.x + 20.0),
                ("the face's edge", PEDAL_WIDTH / 2.0, PEDAL_WIDTH),
            ]);
        }
    }
    in_a_row(&[
        ("the Mix knob's tape", mix.x - 24.0, mix.x + 24.0),
        ("the Sync button", SYNC_BUTTON.x - BUTTON, SYNC_BUTTON.x + BUTTON),
        ("the face's edge", PEDAL_WIDTH / 2.0, PEDAL_WIDTH),
    ]);
    assert_eq!(SYNC_CAPTION.x, SYNC_BUTTON.x);
    assert_eq!(SYNC_LED.x, SYNC_BUTTON.x);
    // From top to bottom: the Repeat knob's tape, the caption, the LED, the button, and
    // the On switch's place
    let repeat_tape_bottom = TRIO_KNOBS[1].y + TRIO_RADIUS + 18.0 + 15.0 + 2.0 + 16.0;
    in_a_row(&[
        ("the Repeat knob's tape", 0.0, repeat_tape_bottom),
        ("the Sync caption", SYNC_CAPTION.y - 7.0, SYNC_CAPTION.y + 7.0),
        ("the Sync LED", SYNC_LED.y - LAMP, SYNC_LED.y + LAMP),
        ("the Sync button", SYNC_BUTTON.y - BUTTON, SYNC_BUTTON.y + BUTTON + 2.5),
        ("the On switch", PEDAL_SWITCH.y - SWITCH, PEDAL_SWITCH.y + SWITCH),
    ]);
}

/// The faces of every amp's head in the lane it stands in (the first or the second)
fn heads() -> impl Iterator<Item = (Amp, Rect, ui::HeadFaces)> {
    let head = Rect::from_min_size(Pos2::new(0.0, STACK_TOP), vec2(LANE_HEAD_WIDTH, LANE_HEIGHT));
    Amp::ALL.into_iter().map(move |amp| (amp, head, ui::head_faces(head, &looks::look(amp).head)))
}

// The jack's nut, and the pilot lamp in its bezel, half across
const JACK: f32 = 8.0;
const PILOT: f32 = 9.0;

#[test]
fn test_the_lanes_fill_the_stack_and_stand_on_the_floor_with_the_pedals() {
    assert_eq!(2.0 * LANE_HEIGHT + LANE_GAP, HEAD_HEIGHT + PEDAL_HEIGHT);
    assert_eq!(PEDAL_TOP + PEDAL_HEIGHT + BENCH_MARGIN, WINDOW_HEIGHT as f32);
    // Head, cabinet and plate across the stack, each clear of the next, with room for
    // the couplers
    let cab_left = LANE_HEAD_WIDTH + LANE_CAB_GAP;
    in_a_row(&[
        ("the head", 0.0, LANE_HEAD_WIDTH),
        ("the cabinet", cab_left, cab_left + LANE_CAB_WIDTH),
        ("the plate", MIX_X, STACK_WIDTH),
    ]);
    assert_eq!(MIX_X - (cab_left + LANE_CAB_WIDTH), MIX_GAP - LANE_CAB_GAP);
}

#[test]
fn test_the_head_panel_holds_the_jack_the_dials_and_the_lamp_in_a_row() {
    for (amp, _, faces) in heads() {
        let corner = faces.panel.left_top();
        let jack = corner + vec2(LANE_JACK_X, DIAL_Y);
        let jewel = faces.panel.right() - ui::amp::JEWEL_INSET;
        let mut row = vec![
            ("the panel's edge", 0.0, faces.panel.left()),
            ("the jack", jack.x - JACK, jack.x + JACK),
        ];
        for (index, label) in DIALS.iter().enumerate() {
            let at = corner + vec2(LANE_DIAL_FIRST_X + LANE_DIAL_SPACING * index as f32, DIAL_Y);
            let room = ui::panel_knob_rect(at, LANE_DIAL_RADIUS);
            // Its scale, caption and tape all on the panel
            assert!(faces.panel.contains_rect(room), "{amp:?}: {label} is off the panel");
            row.push((label, room.left(), room.right()));
        }
        row.push(("the pilot lamp", jewel - PILOT, jewel + PILOT));
        row.push(("the panel's edge", faces.panel.right(), LANE_HEAD_WIDTH));
        in_a_row(&row);
        // The jack's caption over the jack, on the panel
        let first = corner + vec2(LANE_DIAL_FIRST_X, DIAL_Y);
        let caption = ui::panel_caption(first, LANE_DIAL_RADIUS).y;
        assert!(caption + 7.0 < corner.y + DIAL_Y - JACK);
        assert!(caption - 7.0 > faces.panel.top(), "{amp:?}");
    }
}

#[test]
fn test_the_line_under_the_dials_holds_the_name_the_cabinet_and_the_switch_apart() {
    for (amp, _, faces) in heads() {
        let line = DIAL_Y + LANE_LINE_DY;
        let stepper = reach(LANE_CAB_STEPPER_WIDTH) + BUTTON;
        let right = faces.panel.width();
        // Across: the name, the cabinet's stepper, the switch's caption, LED and switch
        in_a_row(&[
            ("the panel's edge", 0.0, 0.0),
            ("the name", LANE_NAME_X - 32.0, LANE_NAME_X + 32.0),
            ("the cabinet", LANE_CAB_X - stepper, LANE_CAB_X + stepper),
            (
                "the On caption",
                right + LANE_ON_CAPTION_DX - 10.0,
                right + LANE_ON_CAPTION_DX + 10.0,
            ),
            ("the On LED", right + LANE_ON_LED_DX - LAMP, right + LANE_ON_LED_DX + LAMP),
            (
                "the On switch",
                right + LANE_ON_SWITCH_DX - SWITCH,
                right + LANE_ON_SWITCH_DX + SWITCH,
            ),
            ("the panel's edge", right, right),
        ]);
        // Down: the dials' tapes, then the cabinet's caption, then its stepper, and the
        // switch inside the panel
        let tape = ui::panel_knob_rect(Pos2::new(0.0, DIAL_Y), LANE_DIAL_RADIUS).bottom();
        let caption_top = line + LANE_CAB_CAPTION_DY - 7.0;
        let caption_bottom = line + LANE_CAB_CAPTION_DY + 7.0;
        assert!(tape + 2.0 < caption_top, "{amp:?}: the caption is under the dials' tapes");
        assert!(caption_bottom + 4.0 < line - 11.0, "{amp:?}");
        assert!(line + SWITCH < faces.panel.height(), "{amp:?}: the switch is off the panel");
    }
}

#[test]
fn test_the_plate_holds_the_fader_and_what_the_cabinets_share_in_order() {
    let knob = MIX_KNOB_RADIUS + TICKS;
    let half = MIX_WIDTH / 2.0 - PLATE_FRAME_INSET;
    let (cap, ticks) = (ui::widgets::FADER_CAP_HALF_WIDTH, ui::widgets::FADER_REACH);
    let stepper = reach(MIC_STEPPER_WIDTH) + BUTTON;
    // Across, inside the frames: the Blend zone's fader with its tape beside it, the
    // Cabinets zone's knobs, stepper, LED and switch
    let tape = 19.0;
    for (name, from, to) in [
        ("the fader's cap", FADER_DX - cap, FADER_DX + cap),
        ("the fader's ticks", FADER_DX - ticks, FADER_DX + ticks),
        ("the Blend tape", MIX_BLEND_TAPE_DX - tape, MIX_BLEND_TAPE_DX + tape),
        ("the Mic knob", -MIX_KNOB_DX - knob, -MIX_KNOB_DX + knob),
        ("the Res knob", MIX_KNOB_DX - knob, MIX_KNOB_DX + knob),
        ("the microphone's stepper", -stepper, stepper),
        ("the Cabs LED", MIX_LED_DX - LAMP, MIX_LED_DX + LAMP),
        ("the Cabs switch", MIX_SWITCH_DX - SWITCH, MIX_SWITCH_DX + SWITCH),
    ] {
        assert!(-half + 4.0 < from && to < half - 4.0, "{name} is outside its frame");
    }
    // The tape is clear of the fader's ticks, the LED of the switch, the knobs of each other
    assert!(FADER_DX + ticks + 4.0 < MIX_BLEND_TAPE_DX - tape);
    const {
        assert!(MIX_LED_DX + LAMP + 4.0 < MIX_SWITCH_DX - SWITCH);
    }
    assert!(-MIX_KNOB_DX + knob + 4.0 < MIX_KNOB_DX - knob);
    // Down, from the top, each clear of the next; a frame's title is on its top line
    let cap_half = ui::widgets::FADER_CAP_HEIGHT / 2.0;
    let end = MIX_END_CAPTION_SIZE / 2.0;
    let knob_tape = MIX_KNOB_Y + MIX_KNOB_RADIUS + 18.0 + 15.0 + 2.0 + 16.0;
    in_a_row(&[
        ("the plate's top", 0.0, 0.0),
        ("the Blend frame's title", BLEND_FRAME_TOP - 8.0, BLEND_FRAME_TOP + 8.0),
        ("the Amp 1 caption", MIX_AMP1_CAPTION_Y - end, MIX_AMP1_CAPTION_Y + end),
        ("the fader", FADER_TOP - cap_half, FADER_TOP + FADER_LENGTH + cap_half),
        ("the Amp 2 caption", MIX_AMP2_CAPTION_Y - end, MIX_AMP2_CAPTION_Y + end),
        ("the Blend frame", BLEND_FRAME_BOTTOM, BLEND_FRAME_BOTTOM),
        ("the Cabinets frame's title", CABS_FRAME_TOP - 8.0, CABS_FRAME_TOP + 8.0),
        ("the Mic and Res knobs", MIX_KNOB_Y - MIX_KNOB_RADIUS - TICKS, knob_tape),
        ("the microphone's caption", MIC_CAPTION_Y - 7.0, MIC_CAPTION_Y + 7.0),
        ("the microphone's stepper", MIC_STEPPER_Y - 11.0, MIC_STEPPER_Y + 11.0),
        ("the Cabs caption", MIX_SWITCH_CAPTION_Y - 7.0, MIX_SWITCH_CAPTION_Y + 7.0),
        ("the Cabs LED", MIX_LED_Y - LAMP, MIX_LED_Y + LAMP),
        ("the Cabinets frame", CABS_FRAME_BOTTOM, CABS_FRAME_BOTTOM),
        ("the output jack", MIX_JACK_Y - JACK, MIX_JACK_Y + JACK),
        ("the plate's bottom", 2.0 * LANE_HEIGHT + LANE_GAP, 2.0 * LANE_HEIGHT + LANE_GAP),
    ]);
    // The switch beside its LED is under the stepper and inside its frame
    const {
        assert!(MIC_STEPPER_Y + 11.0 + 4.0 < MIX_SWITCH_Y - SWITCH);
    }
    const {
        assert!(MIX_SWITCH_Y + SWITCH + 4.0 < CABS_FRAME_BOTTOM);
    }
    // The jack stays inside the plate's screws, and its caption is beside it
    const {
        assert!(MIX_JACK_CAPTION_DX < -JACK - 12.0);
    }
    assert!(MIX_JACK_DX + JACK < half - 4.0);
    assert!(MIX_JACK_DX + MIX_JACK_CAPTION_DX - 14.0 > -half);
}

#[test]
fn test_the_nameplate_and_its_buttons_sit_on_the_grille_clear_of_the_handle() {
    for (amp, head, faces) in heads() {
        let nameplate = faces.grille.center();
        let plate =
            Rect::from_center_size(nameplate, vec2(NAMEPLATE_WIDTH, ui::widgets::NAMEPLATE_HEIGHT));
        assert!(faces.grille.contains_rect(plate), "{amp:?}: {plate:?}");
        let reach = reach(NAMEPLATE_WIDTH) - 2.0;
        let buttons = Rect::from_center_size(nameplate, vec2(2.0 * (reach + BUTTON), 2.0 * BUTTON));
        // Inside the grille, away from its edge and the piping round it
        assert!(faces.grille.shrink(4.0).contains_rect(buttons), "{amp:?}: {buttons:?}");
        // The handle stands on the box, over the grille
        assert!(buttons.top() > head.top() + ui::amp::HANDLE_HEIGHT + 4.0, "{amp:?}");
    }
}

#[test]
fn test_the_list_holds_every_bank_and_the_row_that_makes_one() {
    let banks = vec![user_presets::FACTORY.to_owned(), user_presets::MINE.to_owned()];
    let rows = bank_rows(&banks, user_presets::MINE, None);
    let texts: Vec<&str> = rows.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(texts, ["FACTORY", "MINE", NEW_BANK_ROW]);
    // The bank that is listed is the lit one, and nothing else
    assert_eq!(rows.iter().filter(|row| row.lit).count(), 1);
    assert!(rows[1].lit);
    // While a bank is being named, its row holds what is typed with the caret after it
    let naming = bank_rows(&banks, user_presets::MINE, Some("Jaz"));
    assert_eq!(naming[2].text, "Jaz_");
    assert!(naming[2].lit);
}

#[test]
fn test_the_list_holds_the_presets_of_the_bank_it_lists_with_the_one_loaded_lit() {
    // The factory bank: every built-in preset, the one loaded lit
    let rows = preset_rows(None, Loaded::Factory(3));
    assert_eq!(rows.len(), PRESETS.len());
    assert_eq!(rows[3].text, PRESETS[3].name);
    assert_eq!(rows.iter().filter(|row| row.lit).count(), 1);
    assert!(rows[3].lit);

    // A bank of the player's: its files, without the extension and in capitals
    let files = vec!["Smooth.preset".to_owned(), "Tåge 2.preset".to_owned()];
    let rows = preset_rows(Some(&files), Loaded::File("Tåge 2.preset"));
    assert_eq!(rows[0].text, "SMOOTH");
    assert!(!rows[0].lit && rows[1].lit);

    // Nothing is lit while the sound came out of another bank
    let rows = preset_rows(Some(&files), Loaded::Nothing);
    assert!(rows.iter().all(|row| !row.lit && !row.dim));
    // A bank with nothing in it says so, and that row is not picked
    let empty = preset_rows(Some(&[]), Loaded::Nothing);
    assert_eq!(empty.len(), 1);
    assert!(empty[0].dim);
}

#[test]
fn test_mic_stepper_goes_round_the_list_both_ways() {
    let mut mic = MicType::ALL[0];
    for expected in
        [MicType::Ribbon, MicType::Condenser, MicType::Close, MicType::Air, MicType::Dynamic]
    {
        mic = neighbour_mic(mic, Step::Next);
        assert_eq!(mic, expected);
    }
    for one in MicType::ALL {
        assert_eq!(neighbour_mic(neighbour_mic(one, Step::Previous), Step::Next), one);
    }
}

#[test]
fn test_amp_stepper_goes_round_the_list_both_ways() {
    let mut amp = Amp::ALL[0];
    let mut seen = vec![amp];
    for _ in 0..Amp::ALL.len() {
        amp = neighbour_amp(amp, Step::Next);
        seen.push(amp);
    }
    // Every amp once, and back where it started
    assert_eq!(seen.first(), seen.last());
    for one in Amp::ALL {
        assert_eq!(
            seen.iter().filter(|&&seen| seen == one).count(),
            if one == seen[0] { 2 } else { 1 }
        );
    }
    for one in Amp::ALL {
        assert_eq!(neighbour_amp(neighbour_amp(one, Step::Next), Step::Previous), one);
    }
}

#[test]
fn test_default_window_scales_with_screen_and_fits_inside_each_screen() {
    assert_eq!(size_for_screen(1366), (1322, MIN_HEIGHT));
    assert_eq!(size_for_screen(1920), (1876, 321));
    assert_eq!(size_for_screen(2560), (2516, 431));
    assert_eq!(size_for_screen(2752), (2708, 464));
    assert_eq!(size_for_screen(3440), (3396, 581));
    assert_eq!(size_for_screen(3840), (WINDOW_WIDTH, WINDOW_HEIGHT));
    for screen_w in [1366, 1536, 1920, 2560, 2752, 3440, 3840] {
        let (width, height) = size_for_screen(screen_w);
        assert!(
            (MIN_WIDTH..=WINDOW_WIDTH).contains(&width),
            "width {width} for screen {screen_w} is outside {MIN_WIDTH}..={WINDOW_WIDTH}"
        );
        assert!(
            (MIN_HEIGHT..=WINDOW_HEIGHT).contains(&height),
            "height {height} for screen {screen_w} is outside {MIN_HEIGHT}..={WINDOW_HEIGHT}"
        );
        if screen_w >= MIN_WIDTH + SCREEN_MARGIN {
            assert!(
                width + SCREEN_MARGIN <= screen_w,
                "width {width} leaves less than {SCREEN_MARGIN} px on a {screen_w} px screen"
            );
        }
    }
    let (width, height) = default_state().size();
    assert!((MIN_WIDTH..=WINDOW_WIDTH).contains(&width));
    assert!((MIN_HEIGHT..=WINDOW_HEIGHT).contains(&height));
}

/// A window of `width` by `height` points for a line `line` wide
fn fit(width: f32, height: f32, line: f32) -> Fit {
    Fit::of(vec2(width, height), line)
}

#[test]
fn test_the_zoom_fits_the_layer_in_the_window_and_stays_between_its_limits() {
    let default_line = WINDOW_WIDTH as f32;
    // The window that opens on a 1920 screen shows the twelve-pedal line whole, at 0.55
    let opening = fit(1876.0, 321.0, default_line);
    assert!((opening.zoom - DEFAULT_ZOOM).abs() < 0.001);
    assert_eq!(opening.furthest(), 0.0);
    assert_eq!(opening.layer, default_line);
    // Wider than the line needs: no bigger than full size, the strip as wide as the window
    let wide = fit(5000.0, 900.0, default_line);
    assert_eq!(wide.zoom, MAX_ZOOM);
    assert_eq!(wide.strip_width(), 5000.0);
    assert_eq!(wide.furthest(), 0.0);
    // A window as wide as the line wants but too low for it: the height decides
    let low = fit(3000.0, 400.0, default_line);
    assert!((low.zoom - 400.0 / WINDOW_HEIGHT as f32).abs() < 0.001);
    assert!(low.visible > default_line);
    // Nothing is scaled below the minimum, nor made above the maximum
    for width in [100.0, 1301.0, 1500.0, 2400.0, 4000.0, 9000.0] {
        for height in [50.0, 234.0, 584.0, 2000.0] {
            for line in [default_line / 2.0, default_line, 4239.0, 9000.0] {
                let fit = fit(width, height, line);
                assert!((MIN_ZOOM..=MAX_ZOOM).contains(&fit.zoom), "{fit:?}");
                assert!(fit.strip_width() >= STRIP_MIN_WIDTH);
                assert!(fit.layer >= line.min(STRIP_MIN_WIDTH.max(line)));
            }
        }
    }
}

#[test]
fn test_a_line_too_long_for_the_minimum_zoom_scrolls_and_shorter_ones_do_not() {
    // Sixteen blocks and a gate, the stack and the gaps: 4239 points
    let long = Line::of_chain(&full_chain_of_sixteen()).width();
    assert_eq!(long, 4239.0);
    // A window of 2120 shows it whole, at the least zoom there is
    let fits = fit(2120.0, 292.0, long);
    assert!((fits.zoom - MIN_ZOOM).abs() < 0.001, "{fits:?}");
    assert_eq!(fits.furthest(), 0.0, "{fits:?}");
    // On the window of a 1920 screen it would be under the minimum: scrolled instead, the
    // blocks no smaller than the minimum makes them
    let scrolls = fit(1876.0, 321.0, long);
    assert_eq!(scrolls.zoom, MIN_ZOOM);
    assert!((scrolls.furthest() - (long - 1876.0 / MIN_ZOOM)).abs() < 0.01);
    assert!(scrolls.furthest() > 0.0);
    // Today's twelve-pedal line scrolls only below 1706 points
    assert_eq!(fit(1706.0, 292.0, 3411.0).furthest(), 0.0);
    assert!(fit(1500.0, 292.0, 3411.0).furthest() > 0.0);
    // The strip is what the window shows, never less than its controls need
    assert!(scrolls.strip_width() >= STRIP_MIN_WIDTH);
    // The window's smallest size shows a whole strip
    let smallest = fit(MIN_WIDTH as f32, MIN_HEIGHT as f32, long);
    assert!(smallest.visible >= STRIP_MIN_WIDTH, "{smallest:?}");
    // The strip's places are those of the default width where the layer is that wide
    assert_eq!(fit(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32, 3411.0).shift(), 0.0);
}

// Sixteen blocks, eight on each side, with no more than `MAX_PER_KIND` of a kind
fn full_chain_of_sixteen() -> Chain {
    sixteen_blocks()
}

// The effects bank (#272): the plus in a gap, the sheet that rises, the card dragged into a
// gap, the cross on a block, the empty chain and the full one

/// The pointer moved to `at` (window points at full size), and a frame drawn
#[cfg(target_os = "linux")]
fn hover(harness: &mut Harness, at: Pos2) {
    use nih_plug_egui::egui::Event;
    harness.input_mut().events.push(Event::PointerMoved(at));
    harness.run_ok();
    harness.run_ok();
}

/// The button pressed at `at`, the pointer carried to `to`, and not let go
#[cfg(target_os = "linux")]
fn carry(harness: &mut Harness, from: Pos2, to: Pos2) {
    use nih_plug_egui::egui::{Event, Modifiers, PointerButton};
    let press = Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    };
    for event in [
        Event::PointerMoved(from),
        press,
        Event::PointerMoved(from + vec2(6.0, -6.0)),
        Event::PointerMoved(from + (to - from) * 0.5),
        Event::PointerMoved(to),
    ] {
        harness.input_mut().events.push(event);
        harness.run_ok();
    }
    harness.run_ok();
}

/// The button let go at `at`
#[cfg(target_os = "linux")]
fn release(harness: &mut Harness, at: Pos2) {
    use nih_plug_egui::egui::{Event, Modifiers, PointerButton};
    harness.input_mut().events.push(Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run_ok();
    harness.run_ok();
}

#[cfg(target_os = "linux")]
fn press_escape(harness: &mut Harness) {
    use nih_plug_egui::egui::{Event, Key, Modifiers};
    harness.input_mut().events.push(Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    });
    harness.run_ok();
}

/// The bank open from `from`, risen: as it is a moment after a plus was clicked
#[cfg(target_os = "linux")]
fn risen(view: &Rc<RefCell<View>>, from: (crate::order::Zone, usize)) {
    let mut sheet = crate::bank::Sheet::opened(from, -10.0, None);
    sheet.fresh = false;
    view.borrow_mut().effects = Some(sheet);
}

/// The effects most of the bank's frames are made with: seven of the twelve, the rest not in
/// the chain
fn half_chain() -> Chain {
    use EffectKind::*;
    chain_of(&[Fuzz, Comp, Drive, Phaser], &[Chorus, Delay, Reverb])
}

/// Where the plus of gap `place` of `zone` stands, in window points at full size
fn gap_plus(chain: &Chain, zone: crate::order::Zone, place: usize) -> Pos2 {
    let gap = Line::of_chain(chain)
        .gaps()
        .into_iter()
        .find(|gap| gap.zone == zone && gap.place == place)
        .unwrap();
    Pos2::new(gap.x, PEDAL_TOP + ui::PLUG_Y)
}

#[cfg(target_os = "linux")]
fn bank_harness<'a>(
    params: &'a GuitarAmpParams,
    setter: &'a ParamSetter<'a>,
) -> (Harness<'a>, Rc<RefCell<View>>) {
    line_harness(WINDOW_WIDTH, WINDOW_HEIGHT, params, setter, 0.0)
}

// The pointer over a gap shows its plus and where it would add; a click on the plus raises the
// bank, which stays open for that gap
#[test]
#[cfg(target_os = "linux")]
fn test_a_gap_shows_its_plus_under_the_pointer_and_the_plus_opens_the_bank() {
    use crate::order::Zone;
    let params = GuitarAmpParams::default();
    params.chain.put(half_chain());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = bank_harness(&params, &setter);
    // Between the drive and the phaser
    let plus = gap_plus(&params.chain.get(), Zone::Front, 3);
    hover(&mut harness, plus + vec2(0.0, -40.0));
    harness.snapshot("bank_hover_gap");
    assert!(view.borrow().effects.is_none());
    click(&mut harness, plus);
    let opened = view.borrow().effects.clone().expect("the plus opens the bank");
    assert_eq!(opened.from, (Zone::Front, 3));
    // The click that opened it did not close it again
    harness.run_ok();
    assert!(view.borrow().effects.is_some());
    // It stays where the pointer left; the gap it adds to keeps its plus
    hover(&mut harness, Pos2::new(300.0, 100.0));
    assert!(view.borrow().effects.is_some());
}

// The pointer over a block outlines it and shows the cross on its corner
#[test]
#[cfg(target_os = "linux")]
fn test_a_block_under_the_pointer_shows_its_cross() {
    let params = GuitarAmpParams::default();
    params.chain.put(half_chain());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, _) = bank_harness(&params, &setter);
    let line = Line::of_chain(&params.chain.get());
    // The comp, second of the pieces in front of the amps
    let comp = line.places_for_test()[2];
    hover(&mut harness, Pos2::new(comp + PEDAL_WIDTH / 2.0, PEDAL_TOP + 120.0));
    harness.snapshot("bank_hover_block");
}

// The cross takes the block out with its settings kept, raises the bank with the note, and
// Undo puts it back where it stood
#[test]
#[cfg(target_os = "linux")]
fn test_the_cross_takes_a_block_out_and_undo_puts_it_back() {
    use crate::order::Zone;
    let params = GuitarAmpParams::default();
    params.chain.put(half_chain());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = bank_harness(&params, &setter);
    let line = Line::of_chain(&params.chain.get());
    let comp = line.places_for_test()[2];
    let corner = Pos2::new(comp + PEDAL_WIDTH - 24.0, PEDAL_TOP + 24.0);
    hover(&mut harness, Pos2::new(comp + PEDAL_WIDTH / 2.0, PEDAL_TOP + 120.0));
    click(&mut harness, corner);
    let chain = params.chain.get();
    assert_eq!(chain.blocks().len(), 6);
    assert!(!chain.blocks().iter().any(|block| block.kind == EffectKind::Comp));
    let kept = view.borrow().effects.as_ref().and_then(|sheet| sheet.kept);
    assert_eq!(
        kept,
        Some(crate::bank::Kept { kind: EffectKind::Comp, zone: Zone::Front, place: 1 })
    );
    // Risen: the sheet and its note in a picture
    view.borrow_mut().effects.as_mut().unwrap().since = -10.0;
    view.borrow_mut().effects.as_mut().unwrap().fresh = false;
    hover(&mut harness, Pos2::new(1600.0, 60.0));
    harness.snapshot("bank_taken_out");

    // Undo, on the note's right
    let sheet = crate::bank::risen_rect(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
    click(&mut harness, crate::bank::undo_rect(sheet).center());
    let chain = params.chain.get();
    assert_eq!(chain.blocks().len(), 7);
    assert_eq!(chain.front()[1].kind, EffectKind::Comp);
    assert!(view.borrow().effects.as_ref().is_some_and(|sheet| sheet.kept.is_none()));
}

// The sheet covers what is under it: the layer under the pointer is the sheet's, a click on
// its empty metal reaches nothing below, and a click on a card adds that effect where the
// bank was opened from
#[test]
#[cfg(target_os = "linux")]
fn test_the_sheet_covers_the_line_and_a_click_on_a_card_adds_the_effect() {
    use crate::order::Zone;
    use nih_plug_egui::egui::Order;
    let params = GuitarAmpParams::default();
    params.chain.put(half_chain());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = bank_harness(&params, &setter);
    risen(&view, (Zone::Front, 3));
    harness.run_ok();
    harness.run_ok();
    harness.snapshot("bank_open");
    let top = WINDOW_HEIGHT as f32 - crate::bank::SHEET_HEIGHT;
    let order_at =
        |harness: &Harness, at: Pos2| harness.ctx.layer_id_at(at).map(|layer| layer.order);
    assert_eq!(order_at(&harness, Pos2::new(400.0, top + 100.0)), Some(Order::Foreground));
    // The foot switch of a pedal is under the sheet
    assert_eq!(order_at(&harness, Pos2::new(400.0, PEDAL_TOP + 273.0)), Some(Order::Foreground));
    assert_ne!(order_at(&harness, Pos2::new(400.0, top - 40.0)), Some(Order::Foreground));
    // A click on the bare metal between the two rows of cards, over a foot switch
    click(&mut harness, Pos2::new(400.0, top + 52.0 + 64.0 + 4.0));
    assert!(view.borrow().effects.is_some(), "a click on the sheet leaves it open");
    assert_eq!(params.chain.get().blocks().len(), 7);

    // The first card is the pitch shifter: not in the chain, so it takes the slot of its own
    let sheet = crate::bank::risen_rect(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
    click(&mut harness, crate::bank::card_rect(sheet, 0).center());
    let chain = params.chain.get();
    assert_eq!(chain.blocks().len(), 8);
    assert_eq!(chain.front()[3].kind, EffectKind::Pitch);
    assert!(Chain::is_home(chain.front()[3]));
    // The gap it adds to moves on past it, so the next card follows it
    assert_eq!(view.borrow().effects.as_ref().map(|sheet| sheet.from), Some((Zone::Front, 4)));
}

// A card dragged out of the sheet opens a place in the nearest gap and lands there when it is
// let go over the line; let go over the sheet it lands nowhere
#[test]
#[cfg(target_os = "linux")]
fn test_a_card_dragged_into_a_gap_opens_a_place_and_lands_there() {
    use crate::order::Zone;
    let params = GuitarAmpParams::default();
    params.chain.put(half_chain());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = bank_harness(&params, &setter);
    risen(&view, (Zone::Front, 3));
    harness.run_ok();
    let sheet = crate::bank::risen_rect(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
    // The flanger: second row, third card
    let flanger = crate::bank::card_rect(sheet, 8).center();
    let target = Pos2::new(gap_plus(&params.chain.get(), Zone::Front, 2).x, PEDAL_TOP + 40.0);
    carry(&mut harness, flanger, target);
    harness.snapshot("bank_dragging");
    let dragged = view.borrow().effects.as_ref().and_then(|sheet| sheet.drag);
    assert!(dragged.is_some_and(|drag| drag.kind != EffectKind::Gate));
    release(&mut harness, target);
    for _ in 0..4 {
        harness.run_ok();
    }
    let chain = params.chain.get();
    assert_eq!(chain.blocks().len(), 8, "{:?}", chain.blocks());
    assert!(view.borrow().effects.as_ref().is_some_and(|sheet| sheet.drag.is_none()));
    let kinds: Vec<_> = chain.front().iter().map(|block| block.kind).collect();
    assert_eq!(kinds.len(), 5);
    assert_eq!(kinds[2], dragged.unwrap().kind);

    // Let go over the sheet: nothing is added
    carry(&mut harness, flanger, flanger + vec2(300.0, 20.0));
    release(&mut harness, flanger + vec2(300.0, 20.0));
    assert_eq!(params.chain.get().blocks().len(), 8);
}

// Escape, a click outside and the sheet's handle close it; a plate takes its place and the
// other way round, so one is open at a time; typed text does not close it
#[test]
#[cfg(target_os = "linux")]
fn test_the_sheet_closes_on_escape_a_click_outside_and_its_handle() {
    use crate::order::Zone;
    let params = GuitarAmpParams::default();
    params.chain.put(half_chain());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = bank_harness(&params, &setter);
    let top = WINDOW_HEIGHT as f32 - crate::bank::SHEET_HEIGHT;

    risen(&view, (Zone::Front, 3));
    harness.run_ok();
    type_text(&mut harness, "abc");
    assert!(view.borrow().effects.is_some());
    press_escape(&mut harness);
    assert!(view.borrow().effects.is_none());

    risen(&view, (Zone::Front, 3));
    harness.run_ok();
    click(&mut harness, Pos2::new(1500.0, 40.0));
    assert!(view.borrow().effects.is_none());

    risen(&view, (Zone::Front, 3));
    harness.run_ok();
    click(&mut harness, Pos2::new(WINDOW_WIDTH as f32 / 2.0, top + 11.0));
    assert!(view.borrow().effects.is_none());

    // The sheet's own Close
    risen(&view, (Zone::Front, 3));
    harness.run_ok();
    click(
        &mut harness,
        crate::bank::close_rect(crate::bank::risen_rect(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32))
            .center(),
    );
    assert!(view.borrow().effects.is_none());

    // Opening a plate takes the sheet down
    risen(&view, (Zone::Front, 3));
    harness.run_ok();
    click(&mut harness, Pos2::new(SETTINGS_X, STRIP_Y));
    assert!(view.borrow().effects.is_none());
    assert!(view.borrow().is_open(Plates::Settings));
    // And the sheet takes the plate down
    let plus = gap_plus(&params.chain.get(), Zone::Front, 3);
    hover(&mut harness, plus + vec2(0.0, -40.0));
    click(&mut harness, plus);
    assert!(view.borrow().effects.is_some());
    assert!(view.borrow().plate.is_none());
}

// The bank is a sheet with no text field: typed text is neither taken nor does it close the
// sheet, and the place text does go (the preset name) still takes it once the sheet is down
#[test]
#[cfg(target_os = "linux")]
fn test_typed_text_still_works_with_the_bank_in_the_window() {
    use crate::order::Zone;
    with_presets_in("editor-typing-with-the-bank", |_| {
        let params = GuitarAmpParams::default();
        params.chain.put(half_chain());
        let host = NoHost;
        let setter = ParamSetter::new(&host);
        let (mut harness, view) = bank_harness(&params, &setter);
        risen(&view, (Zone::Front, 3));
        harness.run_ok();
        type_text(&mut harness, "Big Mud");
        assert!(view.borrow().effects.is_some(), "typing does not close the sheet");
        assert!(!harness.ctx.wants_keyboard_input(), "the sheet takes no keyboard focus");
        assert_eq!(params.chain.get().blocks().len(), 7);
        // A plate opens under its icon, takes the sheet down, and then takes the typing
        click(&mut harness, Pos2::new(PRESET_X, STRIP_Y));
        assert!(view.borrow().effects.is_none());
        assert!(view.borrow().is_open(Plates::Presets));
        view.borrow_mut().saving = Some(String::new());
        type_text(&mut harness, "Big");
        type_text(&mut harness, " ");
        type_text(&mut harness, "Mud");
        assert_eq!(view.borrow().saving.as_deref(), Some("Big Mud"));
    });
}

// The pointer over the amps shows the plus that adds one while the chain has fewer lanes than
// the engine has (here one of two; with two of two nothing is offered, and amp 1 has no cross)
#[test]
#[cfg(target_os = "linux")]
fn test_the_amps_show_a_plus_while_the_engine_has_room_for_another() {
    let params = GuitarAmpParams::default();
    params.chain.put(half_chain());
    assert_eq!(params.chain.get().amps(), 1);
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, _) = bank_harness(&params, &setter);
    let line = Line::of_chain(&params.chain.get());
    let stack = line.places_for_test()[5];
    let over_amp = Pos2::new(stack + 200.0, STACK_TOP + 100.0);
    hover(&mut harness, over_amp);
    harness.snapshot("bank_hover_amp");
    // Its plus, near the stack's foot
    let plus = Pos2::new(stack + STACK_WIDTH / 2.0 - 80.0, STACK_TOP + STACK_HEIGHT - 18.0);
    click(&mut harness, plus);
    assert_eq!(params.chain.get().amps(), 2);
    // Two of two: the pointer over the amps offers nothing
    hover(&mut harness, over_amp);
    click(&mut harness, plus);
    assert_eq!(params.chain.get().amps(), 2);
    let mut chain = params.chain.get();
    assert!(!chain.remove_amp(0), "amp 1 cannot be taken out");
}

// The line at rest with a chain of seven, the bank down
#[test]
#[cfg(target_os = "linux")]
fn test_the_line_at_rest_with_the_bank_down_looks_as_it_did() {
    let params = GuitarAmpParams::default();
    params.chain.put(half_chain());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    bank_harness(&params, &setter).0.snapshot("bank_rest");
}

// The print on a card reads as well as on the pedal it stands for
#[test]
fn test_the_print_on_every_card_reads_as_well_as_on_a_pedal() {
    for kind in crate::bank::CARDS {
        let ground = crate::bank::card_ground(kind);
        let read = theme::contrast(theme::SILK, ground);
        assert!(read >= theme::least_contrast(), "{kind:?}: {read}");
    }
}

// A chain with no blocks is a dashed place on each side of the amps, each with a plus; a
// card dropped on one lands there
#[test]
#[cfg(target_os = "linux")]
fn test_an_empty_chain_invites_the_first_effect() {
    use crate::order::Zone;
    let params = GuitarAmpParams::default();
    params.chain.put(Chain::empty());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = bank_harness(&params, &setter);
    harness.snapshot("bank_empty");
    let line = Line::of_chain(&params.chain.get());
    let slots: Vec<_> = line.gaps().into_iter().filter(|gap| gap.slot).collect();
    assert_eq!(slots.len(), 2);
    // The plus of the first
    click(&mut harness, Pos2::new(slots[0].x, PEDAL_TOP + PEDAL_HEIGHT / 2.0 - 40.0));
    assert_eq!(view.borrow().effects.as_ref().map(|sheet| sheet.from), Some((Zone::Front, 0)));
    view.borrow_mut().effects.as_mut().unwrap().fresh = false;
    harness.run_ok();
    // A card clicked lands in front of the amps
    view.borrow_mut().effects.as_mut().unwrap().since = -10.0;
    harness.run_ok();
    // The fuzz, fourth card
    let sheet = crate::bank::risen_rect(WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);
    click(&mut harness, crate::bank::card_rect(sheet, 3).center());
    let chain = params.chain.get();
    assert_eq!(chain.blocks().len(), 1);
    assert_eq!(chain.front().len(), 1);
    assert_eq!(chain.front()[0].kind, EffectKind::Fuzz);
}

// Sixteen blocks: the plus is not offered, and the line says why
#[test]
#[cfg(target_os = "linux")]
fn test_sixteen_blocks_offer_no_plus_and_say_why() {
    let params = GuitarAmpParams::default();
    params.chain.put(sixteen_blocks());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = line_harness(2120, 292, &params, &setter, 0.0);
    let plus = gap_plus(&params.chain.get(), crate::order::Zone::Front, 3) * MIN_ZOOM;
    hover(&mut harness, plus + vec2(0.0, -20.0));
    harness.snapshot("bank_full");
    click(&mut harness, plus);
    assert!(view.borrow().effects.is_none());
}

// An open sheet, a gap with its plus and a window at rest ask for no repaint: the sheet's
// rise is the only thing that moves, and it is over once it has risen
#[test]
#[cfg(target_os = "linux")]
fn test_an_open_sheet_at_rest_asks_for_no_repaint() {
    use crate::order::Zone;
    let params = GuitarAmpParams::default();
    params.chain.put(half_chain());
    let host = NoHost;
    let setter = ParamSetter::new(&host);
    let (mut harness, view) = bank_harness(&params, &setter);
    risen(&view, (Zone::Front, 3));
    // Past the sheet's own fade in
    for _ in 0..20 {
        harness.step();
    }
    let delays: Vec<_> =
        harness.output().viewport_output.values().map(|viewport| viewport.repaint_delay).collect();
    assert!(!delays.is_empty());
    assert!(delays.iter().all(|delay| *delay == std::time::Duration::MAX), "{delays:?}");
}

// The line the chain makes and the gaps it has: one after the gate, one after each block,
// one on either side of the stack's neighbours, one at the end; a card lands in the nearest
#[test]
fn test_the_gaps_of_a_line_and_the_one_a_card_lands_in() {
    use crate::order::Zone;
    let chain = half_chain();
    let line = Line::of_chain(&chain);
    let gaps = line.gaps();
    // gate + 4 blocks in front, the stack, 3 blocks behind: eight pieces, a gap after each
    assert_eq!(gaps.len(), 9);
    assert_eq!((gaps[0].zone, gaps[0].place), (Zone::Front, 0));
    assert_eq!((gaps[4].zone, gaps[4].place), (Zone::Front, 4));
    assert_eq!((gaps[5].zone, gaps[5].place), (Zone::Back, 0));
    assert_eq!((gaps[8].zone, gaps[8].place), (Zone::Back, 3));
    for pair in gaps.windows(2) {
        assert!(pair[0].x < pair[1].x);
    }
    let nearest = |x: f32| line.drop_target(EffectKind::Delay, x).map(|gap| (gap.zone, gap.place));
    assert_eq!(nearest(gaps[2].x + 5.0), Some((Zone::Front, 2)));
    assert_eq!(nearest(-100.0), Some((Zone::Front, 0)));
    assert_eq!(nearest(1.0e6), Some((Zone::Back, 3)));
    // The last gap stays inside the layer
    assert!(gaps[8].x + suite_common::ui::PLUS_RADIUS < line.width());
    // An empty chain has a place on each side of the amps and no other gap
    let empty = Line::of_chain(&Chain::empty());
    let slots = empty.gaps();
    assert_eq!(slots.len(), 2);
    assert!(slots.iter().all(|gap| gap.slot && gap.place == 0));
    assert_eq!(slots[0].zone, Zone::Front);
    assert_eq!(slots[1].zone, Zone::Back);
}
