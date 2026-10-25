use nih_plug::prelude::{BoolParam, Param, ParamSetter};
use nih_plug_egui::egui::{
    vec2, Align2, Area, Color32, CornerRadius, Event, Id, Key, Order, Painter, Pos2, Rect,
    Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, WidgetInfo,
};

use super::raster::{KnobCap, Plate};
use super::theme::*;

// The knob travels 270 degrees, from bottom-left over the top to bottom-right
const KNOB_START_ANGLE: f32 = std::f32::consts::PI * 0.75;
const KNOB_SWEEP: f32 = std::f32::consts::PI * 1.5;

// Normalized parameter change per pixel of vertical drag. Shift slows it down.
const DRAG_SENSITIVITY: f32 = 0.005;
const FINE_DRAG_FACTOR: f32 = 0.1;

// Knobs at least this large get the larger label and value
const BIG_KNOB_RADIUS: f32 = 40.0;
const SMALL_KNOB_RADIUS: f32 = 24.0;

const LABEL_SPACING: f32 = 0.14;

// Diameter of a footswitch's washer
const FOOTSWITCH_SIZE: f32 = 74.0;
const SMALL_FOOTSWITCH_SIZE: f32 = 50.0;

// Size of a rig's signal lamp against its LEDs
const SIGNAL_LAMP_SCALE: f32 = 0.68;

// The display of a stepper: a strip of label tape with a push button at each end
const STEPPER_TEXT_SIZE: f32 = 13.0;
const STEPPER_TAPE_HEIGHT: f32 = 22.0;
const STEPPER_BUTTON_RADIUS: f32 = 11.0;
const STEPPER_BUTTON_GAP: f32 = 9.0;

// The plate that drops out of a display: the space round its columns, the caption over
// each of them, and its rows of label tape
const DROP_PADDING: f32 = 10.0;
const DROP_TITLE_SIZE: f32 = 13.0;
const DROP_TITLE_HEIGHT: f32 = 18.0;
const DROP_ROW_HEIGHT: f32 = 22.0;
// The tape of a row is painted a little over its rectangle, so the rows stand far enough
// apart for the strips not to overlap
const DROP_ROW_GAP: f32 = 6.0;
const DROP_COLUMN_GAP: f32 = 12.0;
const DROP_TEXT_SIZE: f32 = 13.0;
// The lamp of the row that is in use, and where the text starts after it
const DROP_LAMP_X: f32 = 9.0;
const DROP_LAMP_RADIUS: f32 = 3.5;
const DROP_TEXT_X: f32 = 19.0;
// The chrome bar of a column that holds more rows than it shows, and the space to it
const DROP_BAR_WIDTH: f32 = 5.0;
const DROP_BAR_GAP: f32 = 4.0;
/// The most rows a column of a `drop_panel` shows at once: the rest are scrolled to
pub const DROP_ROWS: usize = 7;

// The row of lamps of a `cents_meter`, on label tape
const METER_PADDING: f32 = 8.0;
// Lamps on each side of the middle one, which is lit alone while the note is in tune
const METER_SIDE_LAMPS: usize = 8;

fn polar(center: Pos2, radius: f32, angle: f32) -> Pos2 {
    Pos2::new(center.x + angle.cos() * radius, center.y + angle.sin() * radius)
}

/// Fluted amp knob with its printed scale. `scale` names the start, middle and end of the travel.
pub fn knob(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    normalized: f32,
    scale: Option<[&str; 3]>,
) {
    knob_with(painter, center, radius, normalized, scale, KnobCap::Fluted, SILK);
}

/// The ticks of a knob's scale: the angle of each, and how far past the rim and how wide
fn scale_ticks() -> impl Iterator<Item = (f32, f32, f32)> {
    (0..=10).map(|tick| {
        let angle = KNOB_START_ANGLE + KNOB_SWEEP * tick as f32 / 10.0;
        let (length, width) = if tick % 5 == 0 { (13.0, 2.2) } else { (10.0, 1.3) };
        (angle, length, width)
    })
}

/// A `knob` with the cap `cap`, its scale printed in `print`
pub fn knob_with(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    normalized: f32,
    scale: Option<[&str; 3]>,
    cap: KnobCap,
    print: Color32,
) {
    // Scale printed on the enclosure
    for (angle, length, width) in scale_ticks() {
        painter.line_segment(
            [polar(center, radius + 6.0, angle), polar(center, radius + length, angle)],
            Stroke::new(width, print),
        );
    }
    if let Some(scale) = scale {
        for (i, text) in scale.iter().enumerate() {
            let angle = KNOB_START_ANGLE + KNOB_SWEEP * i as f32 / 2.0;
            painter.text(
                polar(center, radius + 24.0, angle),
                Align2::CENTER_CENTER,
                text,
                display(15.0),
                print,
            );
        }
    }

    super::raster::paint_knob(painter, center, radius, cap);

    let angle = KNOB_START_ANGLE + KNOB_SWEEP * normalized.clamp(0.0, 1.0);
    let pointer = |reach: f32, color: Color32| {
        painter.line_segment(
            [polar(center, radius * 0.12, angle), polar(center, radius * reach, angle)],
            Stroke::new((radius * 0.1).max(2.6), color),
        );
    };
    match cap {
        KnobCap::Fluted => pointer(0.94, SILK),
        KnobCap::ChickenHead => chicken_wing(painter, center, radius, angle, BAKELITE, SILK),
        KnobCap::CreamChickenHead => {
            chicken_wing(painter, center, radius, angle, super::raster::CREAM_BAKELITE_COLOR, INK)
        }
        KnobCap::GoldTop => pointer(0.66, INK),
        KnobCap::ChromeSkirt => pointer(0.54, SILK),
    }
}

// The wing of a black chicken-head knob; a cream one is `raster::CREAM_BAKELITE_COLOR`
const BAKELITE: Color32 = Color32::from_rgb(34, 31, 28);

/// The wing of a chicken-head knob, pointing at `angle`: bakelite of `body` tapering to a
/// point past the base, with a line of `line` along it
fn chicken_wing(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    angle: f32,
    body: Color32,
    line: Color32,
) {
    let half_turn = std::f32::consts::PI;
    let wing = |at: Pos2| {
        vec![
            polar(at, radius * 1.04, angle),
            polar(at, radius * 0.34, angle + half_turn * 0.5),
            polar(at, radius * 0.62, angle + half_turn * 0.88),
            polar(at, radius * 0.62, angle - half_turn * 0.88),
            polar(at, radius * 0.34, angle - half_turn * 0.5),
        ]
    };
    painter.add(Shape::convex_polygon(wing(center + vec2(0.6, 1.8)), SHADOW, Stroke::NONE));
    painter.add(Shape::convex_polygon(
        wing(center),
        body,
        Stroke::new(1.0, Color32::from_white_alpha(28)),
    ));
    painter.line_segment(
        [polar(center, radius * 0.1, angle), polar(center, radius * 0.92, angle)],
        Stroke::new((radius * 0.08).max(2.0), line),
    );
}

/// Knob bound to a parameter, with its name and value below.
///
/// Drag up and down to change the parameter, hold Shift for fine steps, double-click for the default.
pub fn param_knob<P: Param>(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    center: Pos2,
    radius: f32,
    label: &str,
    scale: Option<[&str; 3]>,
) -> Response {
    param_knob_with(ui, setter, param, center, radius, label, scale, KnobCap::Fluted, SILK)
}

/// A `param_knob` with the cap `cap`, its scale and name printed in `print` (`SILK` on paint
/// and dark plates, `INK` on metal)
#[allow(clippy::too_many_arguments)]
pub fn param_knob_with<P: Param>(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    center: Pos2,
    radius: f32,
    label: &str,
    scale: Option<[&str; 3]>,
    cap: KnobCap,
    print: Color32,
) -> Response {
    let (response, normalized) = turn_param(ui, setter, param, center, radius);
    knob_with(ui.painter(), center, radius, normalized, scale, cap, print);
    let value = param.normalized_value_to_string(normalized, true);
    knob_caption(ui.painter(), center, radius, label, &value, print);

    response
}

/// What a knob of `radius` on `center` does to `param` when it is dragged or double-clicked.
/// Returns its response and where the parameter stands now
fn turn_param<P: Param>(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    center: Pos2,
    radius: f32,
) -> (Response, f32) {
    let id = Id::new(("param_knob", param.name()));
    let rect = Rect::from_center_size(center, Vec2::splat((radius + 6.0) * 2.0));
    let response = ui.interact(rect, id, Sense::click_and_drag());

    let normalized = param.unmodulated_normalized_value();

    // The drag position is kept apart from the parameter, so that stepped parameters
    // do not snap back to their current step on every frame
    if response.drag_started() {
        setter.begin_set_parameter(param);
        ui.data_mut(|data| data.insert_temp(id, normalized));
    }
    if response.dragged() {
        let fine = ui.input(|input| input.modifiers.shift);
        let sensitivity = if fine { DRAG_SENSITIVITY * FINE_DRAG_FACTOR } else { DRAG_SENSITIVITY };
        let delta = -response.drag_delta().y * sensitivity;
        let position = ui.data_mut(|data| {
            let position = data.get_temp_mut_or(id, normalized);
            *position = (*position + delta).clamp(0.0, 1.0);
            *position
        });
        setter.set_parameter_normalized(param, position);
    }
    if response.drag_stopped() {
        setter.end_set_parameter(param);
    }
    if response.double_clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter_normalized(param, param.default_normalized_value());
        setter.end_set_parameter(param);
    }

    (response, normalized)
}

/// How far a panel knob's caption is above its middle, past the top of the scale, and how
/// far its tape is under it, between the ends of the scale
const PANEL_CAPTION_ABOVE: f32 = 22.0;
const PANEL_TAPE_BELOW: f32 = 5.0;
const PANEL_CAPTION_SIZE: f32 = 13.0;
const PANEL_VALUE_SIZE: f32 = 10.0;
// The room a caption and a tape take
const PANEL_CAPTION_HALF: f32 = 7.0;
const PANEL_TAPE_HEIGHT: f32 = 17.0;

/// A knob on the panel of an amp head, where there is room for one line of controls: its name
/// printed in `print` over its scale, and its value on label tape under it, between the ends
/// of the scale. For the small knobs of a panel (radius about 19); otherwise as `param_knob_with`
#[allow(clippy::too_many_arguments)]
pub fn panel_knob<P: Param>(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &P,
    center: Pos2,
    radius: f32,
    label: &str,
    cap: KnobCap,
    print: Color32,
) -> Response {
    let (response, normalized) = turn_param(ui, setter, param, center, radius);
    let painter = ui.painter();
    knob_with(painter, center, radius, normalized, None, cap, print);
    spaced_text(
        painter,
        panel_caption(center, radius),
        Align2::CENTER_CENTER,
        &label.to_uppercase(),
        display(PANEL_CAPTION_SIZE),
        print,
        LABEL_SPACING,
    );
    let value = param.normalized_value_to_string(normalized, true);
    label_tape(
        painter,
        Pos2::new(center.x, center.y + radius + PANEL_TAPE_BELOW),
        &value,
        PANEL_VALUE_SIZE,
    );
    response
}

/// The middle of the caption of a `panel_knob` of `radius` on `center`: the line other
/// captions on its panel go on
pub fn panel_caption(center: Pos2, radius: f32) -> Pos2 {
    Pos2::new(center.x, center.y - radius - PANEL_CAPTION_ABOVE)
}

/// The room a `panel_knob` of `radius` on `center` takes: across, the ends of its scale's
/// ticks; from its caption's top down to its tape's bottom
pub fn panel_knob_rect(center: Pos2, radius: f32) -> Rect {
    let reach = scale_ticks()
        .map(|(angle, length, width)| angle.cos().abs() * (radius + length) + width / 2.0)
        .fold(0.0, f32::max);
    Rect::from_min_max(
        Pos2::new(center.x - reach, center.y - radius - PANEL_CAPTION_ABOVE - PANEL_CAPTION_HALF),
        Pos2::new(center.x + reach, center.y + radius + PANEL_TAPE_BELOW + PANEL_TAPE_HEIGHT),
    )
}

/// Knob that turns a plain value from 0 to 1 and not a parameter, with its name and value
/// below. It looks and is handled like a `param_knob`. `shown` words the value for the tape.
#[allow(clippy::too_many_arguments)]
pub fn value_knob(
    ui: &mut Ui,
    id_source: &str,
    value: &mut f32,
    default: f32,
    center: Pos2,
    radius: f32,
    label: &str,
    shown: impl Fn(f32) -> String,
) -> Response {
    let rect = Rect::from_center_size(center, Vec2::splat((radius + 6.0) * 2.0));
    let response = ui.interact(rect, Id::new(("value_knob", id_source)), Sense::click_and_drag());

    if response.dragged() {
        let fine = ui.input(|input| input.modifiers.shift);
        let sensitivity = if fine { DRAG_SENSITIVITY * FINE_DRAG_FACTOR } else { DRAG_SENSITIVITY };
        *value = (*value - response.drag_delta().y * sensitivity).clamp(0.0, 1.0);
    }
    if response.double_clicked() {
        *value = default;
    }

    knob(ui.painter(), center, radius, *value, None);
    knob_caption(ui.painter(), center, radius, label, &shown(*value), SILK);

    response
}

/// A knob's name printed under it in `print`, and under that its value on label tape
fn knob_caption(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    label: &str,
    value: &str,
    print: Color32,
) {
    let (label_size, value_size) = if radius >= BIG_KNOB_RADIUS {
        (19.0, 14.0)
    } else if radius < SMALL_KNOB_RADIUS {
        (13.0, 10.0)
    } else {
        (15.0, 11.0)
    };
    let label_rect = spaced_text(
        painter,
        Pos2::new(center.x, center.y + radius + 18.0),
        Align2::CENTER_TOP,
        &label.to_uppercase(),
        display(label_size),
        print,
        LABEL_SPACING,
    );
    label_tape(painter, Pos2::new(center.x, label_rect.bottom() + 2.0), value, value_size);
}

/// Value read-out: embossed label tape. `top_center` is the middle of its top edge.
pub fn label_tape(painter: &Painter, top_center: Pos2, text: &str, size: f32) {
    let galley = layout(painter, &text.to_uppercase(), mono(size), TAPE_TEXT, 0.1);
    let padding = vec2(size * 0.6, 2.0);
    let rect = Rect::from_min_size(
        Pos2::new(top_center.x - galley.size().x / 2.0 - padding.x, top_center.y),
        galley.size() + padding * 2.0,
    );
    super::hardware::paint_tape(painter, rect);
    painter.galley(rect.min + padding, galley, TAPE_TEXT);
}

/// Which way a `stepper` was asked to go
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Previous,
    Next,
}

/// What a click on a `menu_stepper` asked for
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// A button: the entry before or after the one showing
    Step(Step),
    /// The tape: the whole list, to be drawn with a `drop_panel`
    Open,
}

/// Display for picking one of a list: `text` on a strip of label tape `width` wide, centred
/// on `center`, with a small chrome push button at each end for the previous and the next
/// entry. A click on the tape also asks for the next one. The caller owns the list: it
/// passes the current entry's text and moves on when a step comes back.
///
/// The tape does not grow with the text: choose `width` for the longest entry.
pub fn stepper(ui: &mut Ui, center: Pos2, width: f32, text: &str, id_source: &str) -> Option<Step> {
    match stepper_parts(ui, center, width, text, id_source) {
        Some(Pick::Step(step)) => Some(step),
        // On a stepper that opens nothing, the tape asks for the next entry as well
        Some(Pick::Open) => Some(Step::Next),
        None => None,
    }
}

/// A `stepper` whose tape opens the whole list instead of stepping on, for a list too long
/// to click through one at a time: the buttons step as they do on any stepper, and a click
/// on the tape comes back as `Pick::Open`, which a `drop_panel` is drawn for.
pub fn menu_stepper(
    ui: &mut Ui,
    center: Pos2,
    width: f32,
    text: &str,
    id_source: &str,
) -> Option<Pick> {
    stepper_parts(ui, center, width, text, id_source)
}

/// A `menu_stepper` on an LED display (`led_display`) instead of label tape: `text` on a
/// window `chars` characters wide centred on `center`, a chrome push button at each end, and
/// a click on the window opens the whole list. `text` is cut off at the window's right end.
/// The window is as high as every LED display, `LED_HEIGHT`.
pub fn led_stepper(
    ui: &mut Ui,
    center: Pos2,
    chars: usize,
    text: &str,
    id_source: &str,
) -> Option<Pick> {
    let width = super::led::led_width(chars);
    let window = Rect::from_center_size(center, vec2(width, super::led::LED_HEIGHT));
    let reach = width / 2.0 + STEPPER_BUTTON_GAP + STEPPER_BUTTON_RADIUS;
    let clicked =
        ui.interact(window, Id::new(("stepper", id_source, "tape")), Sense::click()).clicked();
    let step = step_buttons(ui, center, reach, id_source)
        .map(Pick::Step)
        .or(clicked.then_some(Pick::Open));
    super::led::led_display(ui.painter(), center, chars, text);
    step
}

fn stepper_parts(
    ui: &mut Ui,
    center: Pos2,
    width: f32,
    text: &str,
    id_source: &str,
) -> Option<Pick> {
    let tape = Rect::from_center_size(center, vec2(width, STEPPER_TAPE_HEIGHT));
    let reach = width / 2.0 + STEPPER_BUTTON_GAP + STEPPER_BUTTON_RADIUS;
    let tape_clicked =
        ui.interact(tape, Id::new(("stepper", id_source, "tape")), Sense::click()).clicked();

    let step = step_buttons(ui, center, reach, id_source)
        .map(Pick::Step)
        .or(tape_clicked.then_some(Pick::Open));

    let painter = ui.painter();
    painter.rect_filled(tape.translate(vec2(0.0, 2.0)), 2.0, SHADOW);
    painter.rect_filled(tape, 2.0, TAPE);
    let galley = layout(painter, &text.to_uppercase(), mono(STEPPER_TEXT_SIZE), TAPE_TEXT, 0.1);
    // The letter spacing also follows the last letter: leave it out of the centring
    let text_width = galley.size().x - STEPPER_TEXT_SIZE * 0.1;
    painter.galley(
        Pos2::new(center.x - text_width / 2.0, center.y - galley.size().y / 2.0),
        galley,
        TAPE_TEXT,
    );

    step
}

/// The chrome push buttons of a stepper, `reach` left and right of `center`. Returns the step
/// a click on one asked for
fn step_buttons(ui: &mut Ui, center: Pos2, reach: f32, id_source: &str) -> Option<Step> {
    let mut step = None;
    for (direction, side) in [(Step::Previous, -1.0), (Step::Next, 1.0)] {
        let button = center + vec2(side * reach, 0.0);
        let rect = Rect::from_center_size(button, Vec2::splat(STEPPER_BUTTON_RADIUS * 2.0 + 4.0));
        let response =
            ui.interact(rect, Id::new(("stepper", id_source, side > 0.0)), Sense::click());
        push_button(
            ui.painter(),
            button,
            Engraving::Arrow(side),
            response.is_pointer_button_down_on(),
        );
        if response.clicked() {
            step = Some(direction);
        }
    }
    step
}

/// Height of the plate of a `nameplate_stepper`
pub const NAMEPLATE_HEIGHT: f32 = 30.0;
const NAMEPLATE_TEXT_SIZE: f32 = 22.0;

/// A nameplate for picking one of a few by name (the amp, on its head): `text` printed in
/// `print` on a plate of `plate` `width` wide, centred on `center`, with a chrome push button
/// each side of it for the previous and the next. A click on the plate also asks for the
/// next. As with a `stepper`, the caller owns the list.
pub fn nameplate_stepper(
    ui: &mut Ui,
    center: Pos2,
    width: f32,
    plate: Plate,
    print: Color32,
    text: &str,
    id_source: &str,
) -> Option<Step> {
    let rect = Rect::from_center_size(center, vec2(width, NAMEPLATE_HEIGHT));
    let reach = width / 2.0 + STEPPER_BUTTON_GAP + STEPPER_BUTTON_RADIUS;
    let plate_clicked =
        ui.interact(rect, Id::new(("stepper", id_source, "plate")), Sense::click()).clicked();
    let step = step_buttons(ui, center, reach, id_source);

    let painter = ui.painter();
    super::raster::paint_plate(painter, rect, 3.0, plate);
    spaced_text(
        painter,
        center + vec2(0.0, 1.0),
        Align2::CENTER_CENTER,
        &text.to_uppercase(),
        display(NAMEPLATE_TEXT_SIZE),
        print,
        0.2,
    );

    step.or(plate_clicked.then_some(Step::Next))
}

/// One row of a column of a `drop_panel`: a strip of label tape with `text` on it
#[derive(Clone, Default)]
pub struct Row {
    pub text: String,
    /// Lit: a small lamp at the left of the tape says this is the one in use
    pub lit: bool,
    /// Nothing to pick, for a column with nothing in it: darker, and never clicked
    pub dim: bool,
    /// What is typed goes on this row, so it asks egui for the keyboard (see `type_into`)
    pub typing: bool,
}

/// One column of a `drop_panel`: its caption, its rows, and how wide its tape is
pub struct Column<'a> {
    pub title: &'a str,
    /// Tells this list from another in the same panel: a column keeps how far down it
    /// stands under this, so a column that is given another list starts at its lit row
    pub key: &'a str,
    pub width: f32,
    pub rows: &'a [Row],
}

/// What came of a `drop_panel`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Picked {
    /// The column and the row in it that was clicked
    pub row: Option<(usize, usize)>,
    /// It was asked to close: a click outside it, or escape. A row that was clicked does
    /// not close it; the caller knows which of its lists ends the picking
    pub closed: bool,
}

/// A plate of label-tape rows that drops out of a display, for picking one of a list too
/// long to step through: the columns side by side, each under its own caption, on a dark
/// plate with a chrome edge. `top_center` is the middle of its top edge.
///
/// It is drawn above everything else in the window and takes every click in its own area,
/// so nothing behind it answers while it is open. A column of more than `DROP_ROWS` rows
/// shows that many and scrolls with the wheel.
pub fn drop_panel(ui: &Ui, top_center: Pos2, columns: &[Column], id_source: &str) -> Picked {
    let lane = DROP_BAR_WIDTH + DROP_BAR_GAP;
    let shown =
        columns.iter().map(|column| column.rows.len().clamp(1, DROP_ROWS)).max().unwrap_or(1);
    let rows_height =
        shown as f32 * DROP_ROW_HEIGHT + (shown.saturating_sub(1)) as f32 * DROP_ROW_GAP;
    let inner: f32 = columns.iter().map(|column| column.width + lane).sum::<f32>()
        + DROP_COLUMN_GAP * columns.len().saturating_sub(1) as f32;
    let size =
        vec2(inner + 2.0 * DROP_PADDING, rows_height + DROP_TITLE_HEIGHT + 2.0 * DROP_PADDING);
    let rect = Rect::from_min_size(Pos2::new(top_center.x - size.x / 2.0, top_center.y), size);

    let mut picked = Picked::default();
    // Its own layer above the window: it is drawn over what is under it, and the controls
    // under it are in a lower layer, so they answer no clicks while it is open
    Area::new(Id::new(("drop-panel", id_source)))
        .order(Order::Foreground)
        .fixed_pos(rect.min)
        .show(ui.ctx(), |ui| {
            // The plate takes the whole area up, so that the layer is known to cover it:
            // every click in it is the plate's, whether it lands on a row or not, and the
            // wheel over a column is the column's
            ui.allocate_rect(rect, Sense::click_and_drag());
            let painter = ui.painter().clone();
            painter.rect_filled(rect.translate(vec2(0.0, 5.0)).expand(2.0), 7.0, SHADOW);
            painter.rect_filled(rect, 5.0, PANEL);
            painter.rect_stroke(rect, 5.0, Stroke::new(1.0, CHROME_DARK), StrokeKind::Inside);

            let top = rect.top() + DROP_PADDING + DROP_TITLE_HEIGHT;
            let mut left = rect.left() + DROP_PADDING;
            for (index, column) in columns.iter().enumerate() {
                silk_label(
                    &painter,
                    Pos2::new(
                        left + column.width / 2.0,
                        rect.top() + DROP_PADDING + DROP_TITLE_HEIGHT / 2.0,
                    ),
                    column.title,
                    DROP_TITLE_SIZE,
                );
                let lane_rect = Rect::from_min_size(
                    Pos2::new(left, top),
                    vec2(column.width + lane, rows_height),
                );
                let first = column_scroll(ui, id_source, column, index, lane_rect, shown);
                for (at, row) in column.rows.iter().enumerate().skip(first).take(shown) {
                    let tape = Rect::from_min_size(
                        Pos2::new(
                            left,
                            top + (at - first) as f32 * (DROP_ROW_HEIGHT + DROP_ROW_GAP),
                        ),
                        vec2(column.width, DROP_ROW_HEIGHT),
                    );
                    let response = ui.interact(
                        tape,
                        Id::new(("drop-row", id_source, index, at)),
                        Sense::click(),
                    );
                    if row.typing {
                        ask_for_keyboard(&response, &row.text);
                    }
                    drop_row(&painter, tape, row, response.hovered());
                    if response.clicked() && !row.dim {
                        picked.row = Some((index, at));
                    }
                }
                if column.rows.len() > shown {
                    drop_bar(&painter, lane_rect, column.rows.len(), shown, first);
                }
                left += column.width + lane + DROP_COLUMN_GAP;
            }

            // A click anywhere else, or escape, asks it to close
            let away =
                ui.input(|input| input.pointer.any_click()) && !ui.rect_contains_pointer(rect);
            picked.closed = away || ui.input(|input| input.key_pressed(Key::Escape));
        });
    picked
}

/// How far down a column of a `drop_panel` stands: the wheel moves it while the pointer is
/// over it, and it is kept from frame to frame under the column's key
fn column_scroll(
    ui: &Ui,
    id_source: &str,
    column: &Column,
    index: usize,
    rect: Rect,
    shown: usize,
) -> usize {
    let most = column.rows.len().saturating_sub(shown) as f32;
    let id = Id::new(("drop-scroll", id_source, index, column.key));
    let lit = column.rows.iter().position(|row| row.lit).unwrap_or(0);
    let mut at: f32 = ui.data(|data| data.get_temp(id)).unwrap_or(lit as f32);
    if ui.rect_contains_pointer(rect) {
        at -= ui.input(|input| input.raw_scroll_delta.y) / DROP_ROW_HEIGHT;
    }
    at = at.clamp(0.0, most);
    ui.data_mut(|data| data.insert_temp(id, at));
    at.round() as usize
}

fn drop_row(painter: &Painter, tape: Rect, row: &Row, hovered: bool) {
    super::hardware::paint_tape(painter, tape);
    if hovered && !row.dim {
        painter.rect_filled(tape, 2.0, Color32::from_white_alpha(20));
    }
    if row.lit {
        painter.circle_filled(
            Pos2::new(tape.left() + DROP_LAMP_X, tape.center().y),
            DROP_LAMP_RADIUS,
            LED_ON,
        );
    }
    let color = if row.dim { blend(TAPE_TEXT, TAPE, 0.55) } else { TAPE_TEXT };
    // A name longer than the tape is cut off at its edge, not drawn over the plate
    let inside = painter.with_clip_rect(tape);
    let galley = layout(&inside, &row.text.to_uppercase(), mono(DROP_TEXT_SIZE), color, 0.1);
    inside.galley(
        Pos2::new(tape.left() + DROP_TEXT_X, tape.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
}

/// The chrome bar at the right of a column that holds more rows than it shows
fn drop_bar(painter: &Painter, rect: Rect, rows: usize, shown: usize, first: usize) {
    let track = Rect::from_min_size(
        Pos2::new(rect.right() - DROP_BAR_WIDTH, rect.top()),
        vec2(DROP_BAR_WIDTH, rect.height()),
    );
    painter.rect_filled(track, 2.0, blend(CHROME_DARK, PANEL, 0.6));
    let height = (track.height() * shown as f32 / rows as f32).max(DROP_ROW_HEIGHT);
    let travel = (track.height() - height) * first as f32 / (rows - shown) as f32;
    painter.rect_filled(
        Rect::from_min_size(
            Pos2::new(track.left(), track.top() + travel),
            vec2(track.width(), height),
        ),
        2.0,
        CHROME,
    );
}

/// Asks egui for the keyboard for the row a name is typed on, and tells it, for anything
/// reading the window out, that the row is a field with text in it.
///
/// On Windows this is what gets the letters at all: a plugin's editor is a child window of
/// the host's, which nothing gives the keyboard focus to by itself. egui-baseview takes the
/// focus when a widget asks egui for the keyboard like this, which is why the workspace
/// turns its `windows_keyboard_workaround` on (see the root `Cargo.toml`). Nothing else in
/// the suite asks, so the window never takes the focus away from the host for a knob.
fn ask_for_keyboard(response: &Response, text: &str) {
    if !response.has_focus() {
        response.request_focus();
    }
    response.widget_info(|| WidgetInfo::text_edit(true, text, text));
}

/// What came of a name being typed
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typed {
    /// Enter: the name stands as it is
    Done,
    /// Escape: nothing is named
    Cancelled,
}

/// Takes a name typed on the tape of a `stepper` centred on `center`, `width` wide: asks for
/// the keyboard for it, as the row of a new bank does, then `type_into`. The caller draws the
/// name (with its caret) on the stepper's tape.
pub fn type_on_tape(
    ui: &mut Ui,
    center: Pos2,
    width: f32,
    text: &mut String,
    max_chars: usize,
) -> Option<Typed> {
    let tape = Rect::from_center_size(center, vec2(width, STEPPER_TAPE_HEIGHT));
    // Focusable, or egui takes the focus back in the same frame and nothing is ever typed
    let response = ui.interact(tape, Id::new("typed-tape"), Sense::focusable_noninteractive());
    ask_for_keyboard(&response, text);
    type_into(ui, text, max_chars)
}

/// Takes what was typed this frame into `text`: letters, backspace, enter and escape, and
/// `max_chars` at most. For the one place in the suite where a name is typed (a new bank of
/// presets, on its row of a `drop_panel`), so that no egui text widget is drawn.
pub fn type_into(ui: &Ui, text: &mut String, max_chars: usize) -> Option<Typed> {
    let mut end = None;
    ui.input(|input| {
        for event in &input.events {
            match event {
                Event::Text(typed) => {
                    for letter in typed.chars().filter(|letter| !letter.is_control()) {
                        if text.chars().count() < max_chars {
                            text.push(letter);
                        }
                    }
                }
                Event::Key { key: Key::Backspace, pressed: true, .. } => {
                    text.pop();
                }
                Event::Key { key: Key::Enter, pressed: true, .. } => end = Some(Typed::Done),
                Event::Key { key: Key::Escape, pressed: true, .. } => end = Some(Typed::Cancelled),
                _ => {}
            }
        }
    });
    end
}

/// How far a note is from in tune, as a row of lamps on a strip of label tape `width` wide,
/// centred on `center`: flat to the left, sharp to the right, up to 50 cents. The lit spot
/// moves like a needle, two lamps sharing it between their places. Within 2 cents the
/// larger lamp in the middle is lit, and only then. `None` leaves all of them dark.
/// Give it about 13 px of width per lamp: there are 17.
pub fn cents_meter(painter: &Painter, center: Pos2, width: f32, cents: Option<f32>) {
    let tape = Rect::from_center_size(center, vec2(width, STEPPER_TAPE_HEIGHT));
    painter.rect_filled(tape.translate(vec2(0.0, 2.0)), 2.0, SHADOW);
    painter.rect_filled(tape, 2.0, TAPE);
    meter_lamps(painter, tape.shrink2(vec2(METER_PADDING, 0.0)), cents);
}

/// How many lamps a `level_meter` has, from the quietest to the one at full scale
pub const LEVEL_LAMPS: usize = 10;
/// The level of the quietest lamp, in dBFS; each lamp after it needs this much more
const LEVEL_FLOOR_DB: f32 = -45.0;
const LEVEL_STEP_DB: f32 = 5.0;
/// Lowest level a `level_meter` can show: below it every lamp is dark
pub const LEVEL_MIN_DB: f32 = LEVEL_FLOOR_DB - LEVEL_STEP_DB;

/// How many lamps of a `level_meter` are lit at `dbfs`: the lowest at -45 dBFS, the last at 0
pub fn lit_level_lamps(dbfs: f32) -> usize {
    if dbfs.is_nan() {
        return 0;
    }
    (((dbfs - LEVEL_FLOOR_DB) / LEVEL_STEP_DB).floor() + 1.0).clamp(0.0, LEVEL_LAMPS as f32)
        as usize
}

/// A signal level as a column of `LEVEL_LAMPS` lamps on a strip of label tape `height` high,
/// centred on `center`, filling from the bottom: the first is lit from -45 dBFS, each one
/// above it 5 dB higher, the top one at full scale. Give it about 11 px of height per lamp.
pub fn level_meter(painter: &Painter, center: Pos2, height: f32, dbfs: f32) {
    let tape = Rect::from_center_size(center, vec2(STEPPER_TAPE_HEIGHT, height));
    painter.rect_filled(tape.translate(vec2(0.0, 2.0)), 2.0, SHADOW);
    painter.rect_filled(tape, 2.0, TAPE);

    let rect = tape.shrink2(vec2(0.0, METER_PADDING));
    let lit = lit_level_lamps(dbfs);
    // Whole pixels from lamp to lamp and even sizes, so every lamp is as high as the next
    let pitch = (rect.height() / LEVEL_LAMPS as f32).floor();
    let even = |size: f32| (size / 2.0).round() * 2.0;
    let size = vec2(even(rect.width() * 0.5), even(pitch * 0.6));
    let middle_x = rect.center().x.round();
    let bottom_y = (rect.center().y + pitch * (LEVEL_LAMPS as f32 - 1.0) / 2.0).round();
    for lamp in 0..LEVEL_LAMPS {
        let on = lamp < lit;
        let lens =
            Rect::from_center_size(Pos2::new(middle_x, bottom_y - lamp as f32 * pitch), size);
        if on {
            for (grow, alpha) in [(3.5, 0.16), (1.8, 0.3)] {
                painter.rect_filled(lens.expand(grow), 1.5 + grow, LED_ON.gamma_multiply(alpha));
            }
        }
        painter.rect_filled(lens, 1.5, if on { LED_ON } else { LED_OFF });
        if on {
            let shine =
                Rect::from_min_size(lens.min + vec2(1.0, 1.0), vec2((size.x - 2.0).max(1.0), 2.0));
            painter.rect_filled(shine, 1.0, Color32::from_rgb(255, 208, 196));
        }
    }
}

/// The lamps of a `cents_meter`, spread over the width of `rect`
fn meter_lamps(painter: &Painter, rect: Rect, cents: Option<f32>) {
    let side = METER_SIDE_LAMPS as f32;
    // Whole pixels from lamp to lamp and even sizes, so every lamp is as wide as the next
    let pitch = (rect.width() / (2.0 * side + 1.0)).floor();
    let even = |size: f32| (size / 2.0).round() * 2.0;
    let middle_x = rect.center().x.round();
    let middle_y = rect.center().y.round();

    for lamp in -(METER_SIDE_LAMPS as i32)..=METER_SIDE_LAMPS as i32 {
        let middle = lamp == 0;
        let brightness = super::led::needle_brightness(lamp, cents);
        let size = if middle {
            vec2(even(pitch * 0.8), even(rect.height() * 0.72))
        } else {
            vec2(even(pitch * 0.5), even(rect.height() * 0.5))
        };
        let lens =
            Rect::from_center_size(Pos2::new(middle_x + lamp as f32 * pitch, middle_y), size);
        if brightness > 0.0 {
            for (grow, alpha) in [(3.5, 0.16), (1.8, 0.3)] {
                painter.rect_filled(
                    lens.expand(grow),
                    1.5 + grow,
                    LED_ON.gamma_multiply(alpha * brightness),
                );
            }
        }
        painter.rect_filled(lens, 1.5, blend(LED_OFF, LED_ON, brightness));
        if brightness >= 1.0 {
            // The same highlight as on a round lamp
            let shine =
                Rect::from_min_size(lens.min + vec2(1.0, 1.0), vec2((size.x - 2.0).max(1.0), 2.0));
            painter.rect_filled(shine, 1.0, Color32::from_rgb(255, 208, 196));
        }
    }
}

/// Small chrome push button with an arrow head engraved in its cap. `side` is -1 for the
/// arrow pointing left, 1 for right
/// What is engraved in the cap of a `push_button`
#[derive(Clone, Copy)]
enum Engraving {
    /// An arrow pointing left (-1.0) or right (1.0)
    Arrow(f32),
    /// A dot: a button that does one thing, named by the caption beside it
    Dot,
}

/// A small chrome push button on its own, like those at the ends of a `stepper`, with a dot
/// engraved in its cap. For an action, not a setting: it has no LED and stays up. Its
/// caption is a `silk_label` above it. Returns the click response.
pub fn chrome_button(ui: &mut Ui, center: Pos2, id_source: &str) -> Response {
    let rect = Rect::from_center_size(center, Vec2::splat(STEPPER_BUTTON_RADIUS * 2.0 + 4.0));
    let response = ui.interact(rect, Id::new(("push-button", id_source)), Sense::click());
    push_button(ui.painter(), center, Engraving::Dot, response.is_pointer_button_down_on());
    response
}

fn push_button(painter: &Painter, center: Pos2, engraving: Engraving, pressed: bool) {
    let radius = STEPPER_BUTTON_RADIUS;
    let ring = Stroke::new(1.0, Color32::from_black_alpha(110));
    painter.circle_filled(center + vec2(0.0, 2.5), radius + 0.5, SHADOW);
    painter.circle_filled(center, radius, CHROME_LIGHT);
    painter.circle_stroke(center, radius, ring);

    let cap = if pressed { center + vec2(0.0, 1.0) } else { center };
    painter.circle_filled(cap, radius - 2.5, CHROME_DARK);
    painter.circle_filled(
        cap + vec2(-0.5, -0.8),
        radius - 3.5,
        if pressed { CHROME } else { CHROME_LIGHT },
    );
    painter.circle_stroke(cap, radius - 2.5, ring);

    match engraving {
        Engraving::Arrow(side) => {
            let tip = cap + vec2(side * 4.0, 0.0);
            let base = cap - vec2(side * 2.5, 0.0);
            painter.add(Shape::convex_polygon(
                vec![tip, base + vec2(0.0, -4.2), base + vec2(0.0, 4.2)],
                INK,
                Stroke::NONE,
            ));
        }
        Engraving::Dot => {
            painter.circle_filled(cap, 3.0, INK);
        }
    }
}

/// Chrome stomp switch. Returns the click response.
pub fn footswitch(ui: &mut Ui, center: Pos2, id_source: &str) -> Response {
    stomp_switch(ui, center, id_source, FOOTSWITCH_SIZE)
}

/// The smaller stomp switch of a head or a mini pedal. Returns the click response.
pub fn small_footswitch(ui: &mut Ui, center: Pos2, id_source: &str) -> Response {
    stomp_switch(ui, center, id_source, SMALL_FOOTSWITCH_SIZE)
}

/// Small stomp switch with its LED, bound to an on/off parameter: lit while it is on, a click toggles it
pub fn param_switch(
    ui: &mut Ui,
    setter: &ParamSetter,
    param: &BoolParam,
    switch_center: Pos2,
    led_center: Pos2,
) {
    let on = param.value();
    rig_led(ui.painter(), led_center, on);
    if small_footswitch(ui, switch_center, param.name()).clicked() {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, !on);
        setter.end_set_parameter(param);
    }
}

/// Small stomp switch with its LED for a plain on/off value and not a parameter: lit while
/// it is on, a click toggles it
pub fn value_switch(
    ui: &mut Ui,
    id_source: &str,
    on: &mut bool,
    switch_center: Pos2,
    led_center: Pos2,
) {
    rig_led(ui.painter(), led_center, *on);
    if small_footswitch(ui, switch_center, id_source).clicked() {
        *on = !*on;
    }
}

/// Stomp switch `size` across. The measures in here are those of the full-size one
fn stomp_switch(ui: &mut Ui, center: Pos2, id_source: &str, size: f32) -> Response {
    let scale = size / FOOTSWITCH_SIZE;
    let rect = Rect::from_center_size(center, Vec2::splat(size));
    let response = ui.interact(rect, Id::new(("footswitch", id_source)), Sense::click());
    let pressed = response.is_pointer_button_down_on();

    super::hardware::paint_stomp(ui.painter(), center, scale, pressed);

    response
}

/// Indicator light in a chrome bezel
pub fn led(painter: &Painter, center: Pos2, on: bool) {
    lamp(painter, center, on, LED_OFF, false, 1.0);
}

/// The indicator light of a rig. On dark paint an unlit `led` looks like an empty bezel,
/// so this one has a black socket around a darker lens
pub fn rig_led(painter: &Painter, center: Pos2, on: bool) {
    lamp(painter, center, on, LED_OFF_DARK, true, 1.0);
}

/// A signal lamp of a rig: a `rig_led` at two thirds of the size, with no switch of its
/// own. It shows what the sound is doing (a gate that is open, an output that clips), not
/// what is switched on. Give it a `silk_label` of size 13 under it, 18 px down
pub fn signal_lamp(painter: &Painter, center: Pos2, on: bool) {
    lamp(painter, center, on, LED_OFF_DARK, true, SIGNAL_LAMP_SCALE);
}

/// The measures in here are those of the full-size light
fn lamp(painter: &Painter, center: Pos2, on: bool, off_color: Color32, socket: bool, scale: f32) {
    super::hardware::paint_lamp(painter, center, on, off_color, socket, scale);
}

/// Screen-printed caption, centred on `center`
pub fn silk_label(painter: &Painter, center: Pos2, text: &str, size: f32) -> Rect {
    print_label(painter, center, text, size, SILK)
}

/// A `silk_label` printed in `print`, for a plate that is not dark (`INK` on metal)
pub fn print_label(painter: &Painter, center: Pos2, text: &str, size: f32, print: Color32) -> Rect {
    spaced_text(
        painter,
        center,
        Align2::CENTER_CENTER,
        &text.to_uppercase(),
        display(size),
        print,
        0.16,
    )
}

/// Screen-printed frame around a group of controls, with its title set into the top line
pub fn silk_frame(painter: &Painter, rect: Rect, title: &str, paint: Color32) {
    painter.rect_stroke(rect, CornerRadius::same(12), Stroke::new(2.0, SILK), StrokeKind::Middle);

    let galley = layout(painter, &title.to_uppercase(), display(14.0), SILK, 0.2);
    let title_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 30.0, rect.top() - galley.size().y / 2.0),
        galley.size(),
    );
    painter.rect_filled(title_rect.expand2(vec2(8.0, 0.0)), 0.0, paint);
    painter.galley(title_rect.min, galley, SILK);
}

/// The HØJT logo on its black plate. The Ø is a knob and its slash the pointer.
/// `right_bottom` is the plate's lower right corner.
pub fn badge(painter: &Painter, right_bottom: Pos2, size: f32) {
    let spacing = 0.1;
    let h = layout(painter, "H", display(size), SILK, spacing);
    let jt = layout(painter, "JT", display(size), SILK, spacing);
    let o_width = size * 0.62;
    let gap = size * 0.13;

    let text_width = h.size().x + gap + o_width + gap + jt.size().x;
    let padding = vec2(size * 0.42, size * 0.2);
    let plate = Rect::from_min_max(
        Pos2::new(
            right_bottom.x - text_width - padding.x * 2.0,
            right_bottom.y - size - padding.y * 2.0,
        ),
        right_bottom,
    );
    painter.rect_filled(plate.translate(vec2(0.0, 3.0)), 6.0, SHADOW);
    painter.rect_filled(plate, 6.0, PANEL);
    painter.rect_stroke(plate.shrink(2.0), 4.0, Stroke::new(1.5, SILK), StrokeKind::Inside);

    // Bebas Neue's capitals sit a little above the middle of the line
    let mid_y = plate.center().y;
    let text_y = mid_y - h.size().y / 2.0 + size * 0.07;
    let mut x = plate.left() + padding.x;
    let h_width = h.size().x;
    painter.galley(Pos2::new(x, text_y), h, SILK);
    x += h_width + gap;

    let o_center = Pos2::new(x + o_width / 2.0, mid_y);
    painter.circle_stroke(o_center, size * 0.3, Stroke::new(size * 0.105, SILK));
    let slash_angle = -55.0_f32.to_radians();
    painter.line_segment(
        [
            polar(o_center, size * 0.5, slash_angle + std::f32::consts::PI),
            polar(o_center, size * 0.5, slash_angle),
        ],
        Stroke::new(size * 0.085, BRAND_ORANGE),
    );
    x += o_width + gap;

    painter.galley(Pos2::new(x, text_y), jt, SILK);
}

#[cfg(test)]
mod tests {
    use super::*;
    use nih_plug_egui::egui::{CentralPanel, Context, Modifiers, RawInput};

    /// Draws one frame with `events` in it and gives back what `type_into` made of them
    fn typing(text: &mut String, events: Vec<Event>) -> Option<Typed> {
        let mut end = None;
        let input = RawInput { events, ..RawInput::default() };
        let _ = Context::default().run(input, |ctx| {
            CentralPanel::default().show(ctx, |ui| end = type_into(ui, text, 6));
        });
        end
    }

    fn key(key: Key) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }
    }

    #[test]
    fn test_a_name_is_typed_rubbed_out_kept_or_dropped() {
        let mut name = String::new();
        // The letters of a frame land in the name, in their order
        assert_eq!(
            typing(&mut name, vec![Event::Text("Ja".into()), Event::Text("zz".into())]),
            None
        );
        assert_eq!(name, "Jazz");
        // Backspace takes the last letter off, and takes nothing off an empty name
        assert_eq!(typing(&mut name, vec![key(Key::Backspace), key(Key::Backspace)]), None);
        assert_eq!(name, "Ja");
        // A name stops at its length, so it stays on the tape
        typing(&mut name, vec![Event::Text("zz Trio".into())]);
        assert_eq!(name, "Jazz T");
        // Enter keeps it, escape drops it, and neither is a letter of the name
        assert_eq!(typing(&mut name, vec![key(Key::Enter)]), Some(Typed::Done));
        assert_eq!(typing(&mut name, vec![key(Key::Escape)]), Some(Typed::Cancelled));
        assert_eq!(name, "Jazz T");
        // What is not a letter is left alone
        typing(&mut name, vec![Event::Text("\u{7}".into()), key(Key::F1)]);
        assert_eq!(name, "Jazz T");

        let mut empty = String::new();
        assert_eq!(typing(&mut empty, vec![key(Key::Backspace)]), None);
        assert!(empty.is_empty());
    }

    /// Plays `type_on_tape` as a DAW does: one context across frames, the first frame asking
    /// for the keyboard and the next ones carrying the events. Gives back the name, what the
    /// last frame made of it, and whether the first frame told the window to take the focus
    fn typing_on_tape(frames: Vec<Vec<Event>>) -> (String, Option<Typed>, bool) {
        let ctx = Context::default();
        let (mut name, mut end, mut asked) = (String::new(), None, false);
        for (frame, events) in std::iter::once(Vec::new()).chain(frames).enumerate() {
            let input = RawInput { events, ..RawInput::default() };
            let output = ctx.run(input, |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    end = type_on_tape(ui, Pos2::new(100.0, 40.0), 120.0, &mut name, 6);
                });
            });
            // egui-baseview's windows_keyboard_workaround takes the focus on these
            if frame == 0 {
                asked = !output.platform_output.events.is_empty();
            }
        }
        (name, end, asked)
    }

    #[test]
    fn test_a_name_is_typed_on_the_tape_once_it_has_asked_for_the_keyboard() {
        let (name, end, asked) = typing_on_tape(vec![
            vec![Event::Text("Jazz".into())],
            vec![Event::Text(" T".into()), key(Key::Backspace)],
            vec![key(Key::Enter)],
        ]);
        assert_eq!(name, "Jazz ");
        assert_eq!(end, Some(Typed::Done));
        assert!(asked, "the tape must tell the window it is a field, or no letter reaches it");
        let (_, end, _) = typing_on_tape(vec![vec![key(Key::Escape)]]);
        assert_eq!(end, Some(Typed::Cancelled));
    }

    #[test]
    fn test_the_tape_keeps_the_keyboard_from_one_frame_to_the_next() {
        let ctx = Context::default();
        let mut held = Vec::new();
        for _ in 0..3 {
            let _ = ctx.run(RawInput::default(), |ctx| {
                CentralPanel::default().show(ctx, |ui| {
                    let mut name = String::new();
                    type_on_tape(ui, Pos2::new(100.0, 40.0), 120.0, &mut name, 6);
                });
            });
            held.push(ctx.memory(|memory| memory.focused()).is_some());
        }
        assert_eq!(held, [true, true, true]);
    }

    #[test]
    fn test_level_lamps_light_from_the_bottom_five_decibels_apart() {
        assert_eq!(lit_level_lamps(LEVEL_MIN_DB), 0);
        assert_eq!(lit_level_lamps(-46.0), 0);
        assert_eq!(lit_level_lamps(-45.0), 1);
        assert_eq!(lit_level_lamps(-40.1), 1);
        assert_eq!(lit_level_lamps(-40.0), 2);
        assert_eq!(lit_level_lamps(-0.1), LEVEL_LAMPS - 1);
        assert_eq!(lit_level_lamps(0.0), LEVEL_LAMPS);
        assert_eq!(lit_level_lamps(6.0), LEVEL_LAMPS);
        assert_eq!(lit_level_lamps(f32::NAN), 0);
        assert_eq!(lit_level_lamps(f32::NEG_INFINITY), 0);
    }
}
