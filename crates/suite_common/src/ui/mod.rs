//! The suite's shared look: every plugin is a painted stompbox.
//!
//! - `theme`: colours, embedded fonts, text helpers
//! - `widgets`: knob, footswitch, LED, signal lamp, label tape, stepper, the plate of rows a
//!   display drops open, tuner display, logo badge
//! - `raster`: the surfaces lit pixel by pixel: powder coat, knobs and their caps, screws,
//!   tolex, grille cloth and mesh, plates
//! - `amp`: the half stack of a rig, covered and not painted: `amp_head` and `cabinet`
//! - `art`: what is painted on each mini pedal of a rig under its controls
//! - `frame`: the enclosure every editor is drawn on. `pedal` is one stompbox filling the window;
//!   `rig` is the bare bench for a `strip` with `mini_pedal`s and `slim_pedal`s below it,
//!   joined by `coupler`s and `patch_cable`s

pub mod amp;
pub mod art;
pub mod artwork;
pub mod frame;
pub mod hardware;
pub mod raster;
pub mod theme;
pub mod widgets;

pub use amp::{
    amp_head, cab_faces, cabinet, head_faces, CabFaces, CabStyle, Corners, HeadFaces, HeadStyle,
    Speakers,
};
pub use art::Motif;
pub use frame::{
    coupler, enclosure, lifted, mini_pedal, patch_cable, pedal, rig, slim_pedal, strip, title_grip,
    MiniStyle, Ornament, PedalStyle, BENCH_MARGIN, PLUG_Y, SLIM_KNOBS, SLIM_KNOB_RADIUS, SLIM_LED,
    SLIM_PEDAL_WIDTH, SLIM_SWITCH, STRIP_BAND_WIDTH,
};
pub use raster::{Grille, KnobCap, Plate, Tolex};
pub use theme::install;
pub use widgets::{
    cents_meter, chrome_button, drop_panel, footswitch, led, level_meter, lit_level_lamps,
    menu_stepper, nameplate_stepper, panel_caption, panel_knob, panel_knob_rect, param_knob,
    param_knob_with, param_switch, print_label, rig_led, signal_lamp, silk_frame, silk_label,
    small_footswitch, stepper, tuner_display, type_into, type_on_tape, value_knob, value_switch,
    Column, Pick, Picked, Row, Step, Typed, LEVEL_MIN_DB,
};
