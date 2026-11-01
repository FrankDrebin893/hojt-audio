//! The panel a standalone program drops over its window for the settings of the machine it
//! runs on (the audio interface and its buffer), which a plugin in a DAW never has: the host
//! owns them there. A plate in the style of `drop_panel`, a row per setting, each a `stepper`
//! on label tape under its caption, and two push buttons at the foot.

use nih_plug_egui::egui::{
    vec2, Area, Id, Key, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use super::theme::*;
use super::widgets::{chrome_button, silk_label, stepper, Step};

const PADDING: f32 = 16.0;
const TITLE_HEIGHT: f32 = 26.0;
const TITLE_SIZE: f32 = 15.0;
const CAPTION_WIDTH: f32 = 130.0;
const TAPE_WIDTH: f32 = 330.0;
// A stepper reaches its buttons this far past its tape on each side
const STEPPER_OVERHANG: f32 = 31.0;
const ROW_HEIGHT: f32 = 40.0;
const NOTE_HEIGHT: f32 = 28.0;
const FOOT_HEIGHT: f32 = 60.0;
const BUTTON_SPREAD: f32 = 110.0;

/// The plate's size, the same for every panel so that it stands in one place
pub fn settings_size(rows: usize) -> Vec2 {
    vec2(
        2.0 * PADDING + CAPTION_WIDTH + TAPE_WIDTH + 2.0 * STEPPER_OVERHANG,
        2.0 * PADDING + TITLE_HEIGHT + rows as f32 * ROW_HEIGHT + NOTE_HEIGHT + FOOT_HEIGHT,
    )
}

/// One setting of the panel: its caption and the entry of its list that shows
#[derive(Clone, Copy, Debug)]
pub struct SettingRow<'a> {
    pub caption: &'a str,
    pub shown: &'a str,
}

/// What a frame of the panel asks for
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsEvent {
    /// A row was stepped to the previous or the next entry of its list
    Step(usize, Step),
    /// The left button: take the settings
    Apply,
    /// The right button, or escape: leave them as they were
    Cancel,
}

/// The words of a panel besides its rows
#[derive(Clone, Copy, Debug)]
pub struct SettingsText<'a> {
    pub title: &'a str,
    /// The line under the rows
    pub note: &'a str,
    /// The captions of the left and the right button
    pub apply: &'a str,
    pub cancel: &'a str,
}

/// The panel, with `text.title` over `rows`, a note line under them and the two buttons,
/// centred on `center`. It is drawn above everything
/// else and takes every click in its area. The caller owns the lists and moves on when an
/// event comes back.
pub fn settings_panel(
    ui: &Ui,
    center: Pos2,
    text: &SettingsText,
    rows: &[SettingRow],
    id_source: &str,
) -> Option<SettingsEvent> {
    let rect = Rect::from_center_size(center, settings_size(rows.len()));
    let mut event = None;
    Area::new(Id::new(("settings-panel", id_source)))
        .order(Order::Foreground)
        .fixed_pos(rect.min)
        .show(ui.ctx(), |ui| {
            ui.allocate_rect(rect, Sense::click_and_drag());
            let painter = ui.painter().clone();
            painter.rect_filled(rect.translate(vec2(0.0, 6.0)).expand(2.0), 8.0, SHADOW);
            painter.rect_filled(rect, 6.0, PANEL);
            painter.rect_stroke(rect, 6.0, Stroke::new(1.0_f32, CHROME_DARK), StrokeKind::Inside);

            let mut y = rect.top() + PADDING;
            silk_label(
                &painter,
                Pos2::new(rect.center().x, y + TITLE_HEIGHT / 2.0),
                text.title,
                TITLE_SIZE,
            );
            y += TITLE_HEIGHT;
            let caption_x = rect.left() + PADDING + CAPTION_WIDTH / 2.0;
            let tape_x =
                rect.left() + PADDING + CAPTION_WIDTH + STEPPER_OVERHANG + TAPE_WIDTH / 2.0;
            for (index, row) in rows.iter().enumerate() {
                let middle = y + ROW_HEIGHT / 2.0;
                silk_label(&painter, Pos2::new(caption_x, middle), row.caption, 13.0);
                let id = format!("{id_source}-{index}");
                if let Some(step) =
                    stepper(ui, Pos2::new(tape_x, middle), TAPE_WIDTH, row.shown, &id)
                {
                    event = Some(SettingsEvent::Step(index, step));
                }
                y += ROW_HEIGHT;
            }
            silk_label(
                &painter,
                Pos2::new(rect.center().x, y + NOTE_HEIGHT / 2.0),
                text.note,
                12.0,
            );
            y += NOTE_HEIGHT;

            let buttons = y + 16.0;
            for (side, caption, id, wanted) in [
                (-1.0, text.apply, "apply", SettingsEvent::Apply),
                (1.0, text.cancel, "cancel", SettingsEvent::Cancel),
            ] {
                let at = Pos2::new(rect.center().x + side * BUTTON_SPREAD, buttons);
                silk_label(&painter, at + vec2(0.0, 25.0), caption, 12.0);
                if chrome_button(ui, at, &format!("{id_source}-{id}")).clicked() {
                    event = Some(wanted);
                }
            }
            if ui.input(|input| input.key_pressed(Key::Escape)) {
                event = Some(SettingsEvent::Cancel);
            }
        });
    event
}
