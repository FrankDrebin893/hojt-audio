//! The suite's shared look: every plugin is a painted stompbox.
//!
//! - `theme`: colours, embedded fonts, text helpers
//! - `widgets`: knob, footswitch, LED, signal lamp, label tape, stepper, the plate of rows a
//!   display drops open, tuner display, logo badge
//! - `raster`: the surfaces lit pixel by pixel: powder coat, knobs and their caps, screws,
//!   tolex, grille cloth and mesh, plates
//! - `led`: the built-in display (flat dark glass, a crisp readout in cool white): `led_display`
//!   and the `tuner_display` on it
//! - `amp`: the half stack of a rig, covered and not painted: `amp_head` and `cabinet`
//! - `art`: what is painted on each mini pedal of a rig under its controls
//! - `frame`: the enclosure every editor is drawn on. `pedal` is one stompbox filling the window;
//!   `rig` is the bare bench for a `strip` with `mini_pedal`s below it,
//!   joined by `coupler`s and `patch_cable`s
//! - `plate`: the hardware plate that opens from an icon or a display, and `icons`: the icon with
//!   a lamp and a switch that opens one
//! - `ring`: a ring of lamps round a control, for a loop (`lamp_ring`)
//! - `sheet`: the sheet that rises from below, with the plus, the cross and the dashed place of a
//!   rig's chain
//! - `learn`: the hook of MIDI learn on the knobs and switches (a click picks one, a right-click
//!   forgets its controller, a ring shows what waits), asked for by the editor once a frame
//! - `update_line`: the line of label tape that says a newer version is out, with its download
//!   link, its off switch and its cross
//! - `audio_lamp`: the red lamp with NO AUDIO under it, for a standalone whose audio is the dummy
//! - `snapshot` (feature `snapshot`, tests only): draws an editor off screen into a picture

pub mod amp;
pub mod art;
pub mod artwork;
pub mod audio_lamp;
pub mod frame;
pub mod hardware;
pub mod host_keys;
pub mod icons;
pub mod learn;
pub mod led;
pub mod plate;
pub mod raster;
pub mod ring;
pub mod sheet;
#[cfg(feature = "snapshot")]
pub mod snapshot;
pub mod theme;
pub mod update_line;
pub mod widgets;

pub use amp::{
    amp_head, cab_faces, cabinet, head_faces, CabFaces, CabStyle, Corners, HeadFaces, HeadStyle,
    Speakers,
};
pub use art::Motif;
pub use audio_lamp::{
    audio_lamp, audio_lamp_bare, audio_lamp_beside, audio_lamp_left, notice_for, AudioNotice,
    HostAudio,
};
pub use frame::{
    coupler, enclosure, jack_socket, lifted, mini_pedal, patch_cable, pedal, rig, rig_layer,
    slim_print, slim_strip, strip, strip_plate, title_grip, MiniStyle, Ornament, PedalStyle,
    SlimPrint, BENCH_MARGIN, MODEL_LEFT, PLUG_Y, SLIM_JACK_X, SLIM_JACK_Y, SLIM_KNOBS,
    SLIM_KNOB_RADIUS, SLIM_LED, SLIM_PRINT_REACH, SLIM_STRIP_HEIGHT, SLIM_SWITCH, STRIP_BAND_WIDTH,
};
pub use icons::{amps_icon, icon_switch, Icon, IconSwitch, AMPS_GLYPH_GAP, AMPS_GLYPH_SIZE};
pub use led::{led_display, led_width, tuner_display, LED_HEIGHT};
pub use plate::{plate, Close};
pub use raster::{Grille, KnobCap, Plate, Tolex};
pub use ring::{lamp_ring, Ring, RING_AMBER, RING_GREEN, RING_LAMPS, RING_RED};
pub use sheet::{
    aside, center_label, cross_button, dashed_frame, dashed_rule, floating, left_label, miniature,
    plus_button, pointer_over, sheet, BRASS, PLUS_RADIUS,
};
pub use theme::install;
pub use update_line::{update_line, UPDATE_LINE_HEIGHT};
pub use widgets::{
    bare_param_knob, beat_lamp, beat_lamp_sized, cents_meter, chrome_button, crossfader,
    drop_panel, drop_panel_with, footswitch, led, led_stepper, level_meter, level_meter_wide,
    lit_level_lamps, menu_stepper, nameplate_opener, nameplate_opener_reach, nameplate_stepper,
    nameplate_stepper_sized, paint_small_footswitch, panel_caption, panel_knob, panel_knob_rect,
    param_knob, param_knob_as, param_knob_with, param_switch, print_label, rig_led, rocker,
    scroll_bar, signal_lamp, silk_frame, silk_label, small_footswitch, stepper, type_into,
    type_on_tape, value_knob, value_switch, Beat, Column, Pick, Picked, Row, Step, Typed,
    LEVEL_MIN_DB, SCROLL_BAR_THICKNESS,
};
