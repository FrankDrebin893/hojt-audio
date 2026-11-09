//! The effects bank (#272): the sheet that rises from the bottom of the window over the signal
//! line when a plus or a cross opens it. It holds a card for every effect, however many of it
//! the chain has. A card is dragged into a gap of the line (`board.rs` opens a place for it and
//! the card lands there when it is let go over the line) or clicked, which puts it in the gap the
//! bank was opened from. A block taken out with its cross opens the bank with a note that its
//! settings are kept, and an Undo.
//!
//! What the bank keeps from frame to frame is a `Sheet`, held by the editor's `View`: `None` is
//! closed, so there is one at most. The surface itself is `suite_common::ui::sheet`.
//!
//! The chain it builds is the model's (`slots::Chain`). What plays today is the effect on the
//! slot of its own pedal: a second block of a kind, or one that stands off its pedal's slot, is
//! drawn, kept and saved, and its knobs work, but the audio does not play it before #268's last
//! piece. The Gate is the line's first piece and always there, so it has no card.

use nih_plug_egui::egui::{
    vec2, Color32, CornerRadius, CursorIcon, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui,
};
use suite_common::ui::{self, theme};

use crate::editor::{kind_look, Pedals};
use crate::order::Zone;
use crate::slots::{Chain, EffectKind, MAX_SLOTS};

/// How tall the sheet stands once it has risen: the title row, two rows of cards and the margins
pub(crate) const SHEET_HEIGHT: f32 = 196.0;
/// How long it takes to rise, in seconds
const RISE_S: f64 = 0.2;
/// The space between the sheet's edge and the window's
const INSET: f32 = 20.0;
const PAD: f32 = 22.0;
const COLUMNS: usize = 6;
const CARD_GAP: f32 = 8.0;
const CARD_HEIGHT: f32 = 64.0;
const CARDS_TOP: f32 = 52.0;
const TITLE_SIZE: f32 = 22.0;
const NOTE_SIZE: f32 = 15.0;
const CLOSE_SIZE: (f32, f32) = (108.0, 30.0);
const UNDO_SIZE: (f32, f32) = (84.0, 30.0);
/// How far above the pointer a card that is dragged is held
const GHOST_LIFT: f32 = 90.0;

/// The cards, in the order they stand on the sheet: two rows of six
pub(crate) const CARDS: [EffectKind; 12] = [
    EffectKind::Pitch,
    EffectKind::Wah,
    EffectKind::Comp,
    EffectKind::Fuzz,
    EffectKind::Drive,
    EffectKind::Eq,
    EffectKind::Phaser,
    EffectKind::Chorus,
    EffectKind::Flanger,
    EffectKind::Tremolo,
    EffectKind::Delay,
    EffectKind::Reverb,
];

/// Where card number `index` of `CARDS` stands on a sheet filling `sheet`: two rows of six
pub(crate) fn card_rect(sheet: Rect, index: usize) -> Rect {
    let width = (sheet.width() - 2.0 * PAD - (COLUMNS - 1) as f32 * CARD_GAP) / COLUMNS as f32;
    let (column, line) = (index % COLUMNS, index / COLUMNS);
    Rect::from_min_size(
        Pos2::new(
            sheet.left() + PAD + column as f32 * (width + CARD_GAP),
            sheet.top() + CARDS_TOP + line as f32 * (CARD_HEIGHT + CARD_GAP),
        ),
        vec2(width, CARD_HEIGHT),
    )
}

/// The sheet's Close button
pub(crate) fn close_rect(sheet: Rect) -> Rect {
    Rect::from_center_size(
        Pos2::new(sheet.right() - PAD - CLOSE_SIZE.0 / 2.0, sheet.top() + 32.0),
        vec2(CLOSE_SIZE.0, CLOSE_SIZE.1),
    )
}

/// The sheet once it has risen, in a window `width` by `height` points at full size
#[cfg(test)]
pub(crate) fn risen_rect(width: f32, height: f32) -> Rect {
    Rect::from_min_max(Pos2::new(INSET, height - SHEET_HEIGHT), Pos2::new(width - INSET, height))
}

/// The Undo button of the note in the sheet's header: beside Close, on its left
pub(crate) fn undo_rect(sheet: Rect) -> Rect {
    let close = close_rect(sheet);
    Rect::from_center_size(
        Pos2::new(close.left() - 16.0 - UNDO_SIZE.0 / 2.0, close.center().y),
        vec2(UNDO_SIZE.0, UNDO_SIZE.1),
    )
}

/// A block that was taken out: what it was and where it stood, for the note and its Undo
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Kept {
    pub kind: EffectKind,
    pub zone: Zone,
    pub place: usize,
}

/// A card being dragged: what it is and where the pointer is, from the window's top left corner
#[derive(Clone, Copy, Debug)]
pub(crate) struct Drag {
    pub kind: EffectKind,
    pub pointer: Pos2,
}

/// The bank while it is open
#[derive(Clone, Debug)]
pub(crate) struct Sheet {
    /// The gap a click on a card adds to: its zone and place in it
    pub from: (Zone, usize),
    /// When it began to rise, in seconds of the window's clock
    pub since: f64,
    /// It was opened or moved in this frame, which the click that did it must not close
    pub fresh: bool,
    /// The block that was taken out to open it, if one was
    pub kept: Option<Kept>,
    pub drag: Option<Drag>,
}

impl Sheet {
    pub(crate) fn opened(from: (Zone, usize), now: f64, kept: Option<Kept>) -> Sheet {
        Sheet { from, since: now, fresh: true, kept, drag: None }
    }

    /// Where the sheet's top edge is, from the window's top, in a window `height` points high,
    /// `now` seconds in: it rises from below the window to `SHEET_HEIGHT` up from its bottom
    pub(crate) fn top(&self, height: f32, now: f64) -> f32 {
        height - SHEET_HEIGHT * rise(now - self.since)
    }
}

/// How far up the sheet is, 0 to 1, `elapsed` seconds after it began: it slows as it arrives
fn rise(elapsed: f64) -> f32 {
    let t = (elapsed / RISE_S).clamp(0.0, 1.0) as f32;
    1.0 - (1.0 - t) * (1.0 - t)
}

/// What a frame of the sheet asks the line to do
enum Action {
    Close,
    /// A card was clicked
    Add(EffectKind),
    /// A card was let go with the pointer at this place, from the window's top left corner
    Drop(EffectKind, Pos2),
    Undo(Kept),
}

/// How many of `kind` the chain has
fn count(chain: &Chain, kind: EffectKind) -> usize {
    chain.blocks().iter().filter(|block| block.kind == kind).count()
}

/// Draws the sheet, if the bank is open, and does what is done on it. `window` is the layer's top
/// left corner, `visible` how much of the layer the window shows, `scroll` how far the line is
/// scrolled (a card is dropped where the line is, not where the window shows it)
pub(crate) fn show(
    ui: &mut Ui,
    window: Pos2,
    visible: f32,
    pedals: &Pedals,
    slot: &mut Option<Sheet>,
    scroll: f32,
    now: f64,
) {
    let Some(sheet) = slot.as_mut() else {
        return;
    };
    let elapsed = now - sheet.since;
    if elapsed < RISE_S {
        ui.ctx().request_repaint();
    }
    let height = ui.max_rect().bottom() - window.y;
    let top = window.y + sheet.top(height, now);
    let rect = Rect::from_min_max(
        Pos2::new(window.x + INSET, top),
        Pos2::new(window.x + visible - INSET, top + SHEET_HEIGHT),
    );
    // Where the sheet stands once risen, which a card is dropped above
    let risen_top = window.y + height - SHEET_HEIGHT;
    let chain = pedals.chain();
    let full = chain.blocks().len() >= MAX_SLOTS;
    let fresh = std::mem::take(&mut sheet.fresh);
    let kept = sheet.kept;

    let mut action = None;
    let close = ui::sheet(ui, rect, "bank", fresh, |ui, rect| {
        // The title, and what the bank says about the chain
        let row = rect.top() + 32.0;
        let title = ui::left_label(
            ui.painter(),
            Pos2::new(rect.left() + PAD, row),
            "Effects bank",
            TITLE_SIZE,
            theme::SILK,
        );
        let note_at = Pos2::new(title.right() + 34.0, row + 1.0);
        if let Some(kept) = kept {
            // What was taken out, and the way back: in the header, clear of the line
            ui::left_label(
                ui.painter(),
                note_at,
                &format!("{} taken out · settings kept", kind_look(kept.kind).title),
                NOTE_SIZE,
                theme::SILK,
            );
            let undo = undo_rect(rect);
            let response = ui.interact(undo, Id::new("bank-undo"), Sense::click());
            let painter = ui.painter();
            painter.rect_filled(
                undo,
                3.0,
                if response.hovered() { theme::CHROME_SHINE } else { theme::CHROME_LIGHT },
            );
            painter.rect_stroke(
                undo,
                3.0,
                Stroke::new(1.0_f32, Color32::from_black_alpha(120)),
                StrokeKind::Inside,
            );
            ui::center_label(painter, undo.center(), "Undo", 14.0, theme::INK);
            if response.clicked() {
                action = Some(Action::Undo(kept));
            }
        } else {
            let note = if full {
                "All 16 slots in use. Take a block out to add another".to_owned()
            } else {
                format!(
                    "Drag a pedal into the chain · {} of {MAX_SLOTS} slots in use",
                    chain.blocks().len()
                )
            };
            ui::left_label(ui.painter(), note_at, &note, NOTE_SIZE, theme::SILK);
        }
        let close = close_rect(rect);
        let response = ui.interact(close, Id::new("bank-close"), Sense::click());
        let painter = ui.painter();
        painter.rect_filled(
            close,
            3.0,
            if response.hovered() { theme::CHROME_SHINE } else { theme::CHROME_LIGHT },
        );
        painter.rect_stroke(
            close,
            3.0,
            Stroke::new(1.0_f32, Color32::from_black_alpha(120)),
            StrokeKind::Inside,
        );
        ui::center_label(painter, close.center(), "Close", 14.0, theme::INK);
        if response.clicked() {
            action = Some(Action::Close);
        }

        // The cards
        for (index, &kind) in CARDS.iter().enumerate() {
            let card = card_rect(rect, index);
            let sense = if full { Sense::hover() } else { Sense::click_and_drag() };
            let response = ui.interact(card, Id::new(("bank-card", kind.id())), sense);
            let carried = sheet.drag.is_some_and(|drag| drag.kind == kind);
            paint_card(ui, card, kind, count(&chain, kind), full, carried, response.hovered());
            if full {
                continue;
            }
            if response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            } else if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }
            if let Some(pointer) = response.interact_pointer_pos() {
                if response.drag_started() || response.dragged() {
                    sheet.drag = Some(Drag { kind, pointer: pointer - window.to_vec2() });
                }
                if response.drag_stopped() {
                    action = Some(Action::Drop(kind, pointer - window.to_vec2()));
                }
            }
            if response.clicked() {
                action = Some(Action::Add(kind));
            }
        }
    });

    // The card that follows the pointer, above everything
    if let Some(drag) = sheet.drag.filter(|_| action.is_none()) {
        let size = vec2(crate::editor::PEDAL_WIDTH - 20.0, 100.0);
        // Held above the pointer, so that the place it opens in the line stays in view
        let at = window + drag.pointer.to_vec2() - vec2(0.0, GHOST_LIFT);
        ui::floating(ui, Rect::from_center_size(at, size), "bank-ghost", |ui, rect| {
            ui.painter().rect_filled(
                rect.translate(vec2(0.0, 8.0)).expand(3.0),
                9.0,
                theme::SHADOW,
            );
            paint_ghost(ui, rect, drag.kind);
        });
    }

    let mut edited = false;
    match action {
        Some(Action::Add(kind)) if !full => {
            let (zone, place) = sheet.from;
            if pedals.put_in(kind, zone, place).is_some() {
                sheet.from = (zone, place + 1);
                sheet.kept = None;
                edited = true;
            }
        }
        Some(Action::Drop(kind, pointer)) => {
            sheet.drag = None;
            // Let go over the sheet it came from, it goes back
            if window.y + pointer.y < risen_top && !full {
                let line = crate::board::Line::of_chain(&chain);
                if let Some(gap) = line.drop_target(kind, pointer.x + scroll) {
                    if pedals.put_in(kind, gap.zone, gap.place).is_some() {
                        sheet.from = (gap.zone, gap.place + 1);
                        sheet.kept = None;
                        edited = true;
                    }
                }
            }
            ui.ctx().request_repaint();
        }
        Some(Action::Undo(kept)) => {
            if pedals.put_in(kept.kind, kept.zone, kept.place).is_some() {
                sheet.kept = None;
                edited = true;
            }
        }
        Some(Action::Close) => {
            *slot = None;
            ui.ctx().request_repaint();
            return;
        }
        Some(Action::Add(_)) | None => {}
    }
    if edited {
        ui.ctx().request_repaint();
    }
    if close.asked() && !edited {
        *slot = None;
        ui.ctx().request_repaint();
    }
}

/// The card as it is carried over the line: the effect's paint and name, small enough to leave the
/// place it will land in in view
fn paint_ghost(ui: &Ui, rect: Rect, kind: EffectKind) {
    let painter = ui.painter();
    let style = kind_look(kind);
    let radius = CornerRadius::same(7);
    painter.rect_filled(rect, radius, card_ground(kind));
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(3.0_f32, theme::blend(style.paint, theme::SILK, 0.8)),
        StrokeKind::Inside,
    );
    ui::center_label(painter, rect.center(), style.title, 24.0, theme::SILK);
}

/// The ground of a card: the effect's paint taken toward the sheet's metal, so the cream print on
/// it reads at least as well as on the pedal itself
pub(crate) fn card_ground(kind: EffectKind) -> Color32 {
    theme::blend(kind_look(kind).paint, theme::PANEL, 0.3)
}

/// A card: the effect in its own paint, its name and how many of it are in the chain. `carried`
/// dims the one that is being dragged, `lit` outlines the one the pointer is on
fn paint_card(
    ui: &Ui,
    rect: Rect,
    kind: EffectKind,
    in_chain: usize,
    dead: bool,
    carried: bool,
    lit: bool,
) {
    let painter = ui.painter();
    let style = kind_look(kind);
    let mut ground = card_ground(kind);
    if dead || carried {
        ground = theme::blend(theme::PANEL, ground, 0.45);
    }
    let radius = CornerRadius::same(7);
    painter.rect_filled(rect, radius, ground);
    let edge = theme::blend(style.paint, theme::SILK, if lit { 0.8 } else { 0.3 });
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(if lit { 3.0_f32 } else { 1.5 }, edge),
        StrokeKind::Inside,
    );

    ui::center_label(
        painter,
        Pos2::new(rect.center().x, rect.top() + 22.0),
        style.title,
        TITLE_SIZE + 2.0,
        theme::SILK,
    );
    let status = match in_chain {
        0 => "Not in chain".to_owned(),
        n => format!("In chain ×{n}"),
    };
    ui::center_label(
        painter,
        Pos2::new(rect.center().x, rect.bottom() - 16.0),
        &status,
        15.0,
        theme::SILK,
    );
}
