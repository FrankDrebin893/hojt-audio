//! The picker drawer (#447): the sheet that rises from the bottom of the window, as the effects
//! bank does, with a card for each thing that can be picked for a lane: a miniature of it, its
//! name and a mark for each lane it plays on. The amps' drawer is the first; the cabinets' (#448)
//! is another `Spec` and another list of cards on the same sheet.
//!
//! What the drawer keeps from frame to frame is a `Drawer`, held by the editor's `View`: `None`
//! is closed. It is one more surface beside a plate and the bank, so one is open at a time (the
//! editor closes the others when one opens). The surface itself is `suite_common::ui::sheet`.
//!
//! A click on a card picks it and the drawer stays open, so one can be tried after another. It
//! closes on its Close button, on Escape, on a click outside it, or when the double-click that
//! opens it is made again on the head or cabinet it opened for.

use nih_plug_egui::egui::{
    vec2, CornerRadius, CursorIcon, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use suite_common::ui::{self, theme};

use crate::bank::{close_rect, sheet_button};

/// How long it takes to rise, in seconds
const RISE_S: f64 = 0.2;
/// The space between the sheet's edge and the window's
const INSET: f32 = 20.0;
const PAD: f32 = 22.0;
const CARDS_TOP: f32 = 46.0;
const CARD_GAP: f32 = 10.0;
const FOOT: f32 = 20.0;
const TITLE_SIZE: f32 = 22.0;
const NOTE_SIZE: f32 = 15.0;
const NAME_SIZE: f32 = 18.0;
const MARK_SIZE: f32 = 12.0;
/// A card's room round its miniature, and between the miniature and the print beside it
const FACE_PAD: f32 = 9.0;
const PRINT_GAP: f32 = 14.0;

/// What a kind of drawer shows: its title, the size of the thing it pictures (the miniature is
/// scaled to `face_width`), the card round each and how the cards stand
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spec {
    pub title: &'static str,
    /// The thing at full size: the miniature is drawn into a rectangle this big and scaled
    pub full: Vec2,
    pub face_width: f32,
    pub card: Vec2,
    pub columns: usize,
    /// How many rows the sheet is tall enough for
    pub rows: usize,
}

/// The twelve amps: a head at a third of its size and its name beside it, two rows of six
pub(crate) const AMPS: Spec = Spec {
    title: "Amps",
    full: vec2(420.0, 236.0),
    face_width: 140.0,
    card: vec2(290.0, 96.0),
    columns: 6,
    rows: 2,
};

impl Spec {
    /// How tall the sheet stands once it has risen: the title row, the rows of cards and the
    /// margins. It leaves the first lane's head in view above it
    pub(crate) fn height(&self) -> f32 {
        CARDS_TOP + self.rows as f32 * self.card.y + (self.rows - 1) as f32 * CARD_GAP + FOOT
    }

    /// Where card `index` stands on a sheet filling `sheet`
    pub(crate) fn card_rect(&self, sheet: Rect, index: usize) -> Rect {
        let (column, row) = (index % self.columns, index / self.columns);
        Rect::from_min_size(
            Pos2::new(
                sheet.left() + PAD + column as f32 * (self.card.x + CARD_GAP),
                sheet.top() + CARDS_TOP + row as f32 * (self.card.y + CARD_GAP),
            ),
            self.card,
        )
    }

    /// Where the miniature stands on `card`
    pub(crate) fn face_rect(&self, card: Rect) -> Rect {
        let size = vec2(self.face_width, self.face_width * self.full.y / self.full.x);
        Rect::from_min_size(Pos2::new(card.left() + FACE_PAD, card.center().y - size.y / 2.0), size)
    }

    /// The sheet once it has risen, in a window `width` by `height` points at full size
    #[cfg(test)]
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))] // its tests draw, and run on Linux alone
    pub(crate) fn risen_rect(&self, width: f32, height: f32) -> Rect {
        Rect::from_min_max(
            Pos2::new(INSET, height - self.height()),
            Pos2::new(width - INSET, height),
        )
    }
}

/// What a drawer picks
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum What {
    Amp,
}

impl What {
    pub(crate) fn spec(self) -> &'static Spec {
        match self {
            What::Amp => &AMPS,
        }
    }
}

/// The drawer while it is open
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Drawer {
    /// The lane it picks for: 0 is the first amp's, 1 the second's
    pub lane: usize,
    pub what: What,
    /// When it began to rise, in seconds of the window's clock
    since: f64,
    /// It was opened or moved in this frame, which the click that did it must not close
    pub fresh: bool,
}

impl Drawer {
    pub(crate) fn opened(lane: usize, what: What, now: f64) -> Drawer {
        Drawer { lane, what, since: now, fresh: true }
    }

    /// The same drawer, already risen: as it is a moment after it was opened
    #[cfg(test)]
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(crate) fn risen(lane: usize, what: What) -> Drawer {
        Drawer { lane, what, since: -10.0, fresh: false }
    }

    /// Where the sheet's top edge is, from the window's top, in a window `height` points high,
    /// `now` seconds in: it rises from below the window
    pub(crate) fn top(&self, height: f32, now: f64) -> f32 {
        height - self.what.spec().height() * rise(now - self.since)
    }
}

/// How far up the sheet is, 0 to 1, `elapsed` seconds after it began: it slows as it arrives
fn rise(elapsed: f64) -> f32 {
    let t = (elapsed / RISE_S).clamp(0.0, 1.0) as f32;
    1.0 - (1.0 - t) * (1.0 - t)
}

/// A card: what is pictured, named, and which lanes play it
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Card {
    pub name: String,
    /// One line for each lane it plays on (`ON AMP 1`)
    pub marks: Vec<String>,
    /// It is what plays on the lane the drawer is for: outlined in brass
    pub lit: bool,
}

/// Draws the drawer, if one is open, and does what is done on it. `window` is the layer's top
/// left corner and `visible` how much of the layer the window shows. `cards` are the cards in
/// order, `heads` the places whose double-click opens it (a click there is not a click outside),
/// and `paint_face` draws the miniature of card `index` at full size into the rectangle it is
/// given. Returns the card that was clicked, if one was
#[allow(clippy::too_many_arguments)]
pub(crate) fn show(
    ui: &mut Ui,
    window: Pos2,
    visible: f32,
    slot: &mut Option<Drawer>,
    cards: &[Card],
    note: &str,
    heads: &[Rect],
    paint_face: &dyn Fn(&Ui, usize, Rect),
    now: f64,
) -> Option<usize> {
    let drawer = slot.as_mut()?;
    let spec = drawer.what.spec();
    let elapsed = now - drawer.since;
    if elapsed < RISE_S {
        ui.ctx().request_repaint();
    }
    let height = ui.max_rect().bottom() - window.y;
    let top = window.y + drawer.top(height, now);
    let rect = Rect::from_min_max(
        Pos2::new(window.x + INSET, top),
        Pos2::new(window.x + visible - INSET, top + spec.height()),
    );
    let fresh = std::mem::take(&mut drawer.fresh);
    let (lane, what) = (drawer.lane, drawer.what);
    let pointer = ui.input(|input| input.pointer.interact_pos());
    let pointer = pointer
        .map(|at| ui.ctx().layer_transform_from_global(ui.layer_id()).map_or(at, |from| from * at));

    let mut picked = None;
    let mut closed = false;
    let close = ui::sheet(ui, rect, "drawer", fresh, |ui, rect| {
        let row = rect.top() + 32.0;
        let title = ui::left_label(
            ui.painter(),
            Pos2::new(rect.left() + PAD, row),
            spec.title,
            TITLE_SIZE,
            theme::SILK,
        );
        ui::left_label(
            ui.painter(),
            Pos2::new(title.right() + 34.0, row + 1.0),
            note,
            NOTE_SIZE,
            theme::SILK,
        );
        if sheet_button(ui, close_rect(rect), "drawer-close", "Close") {
            closed = true;
        }
        for (index, card) in cards.iter().enumerate() {
            let at = spec.card_rect(rect, index);
            let response =
                ui.interact(at, Id::new(("drawer-card", what as u8, index)), Sense::click());
            if response.hovered() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            paint_card(ui, spec, at, card, response.hovered());
            ui::miniature(
                ui,
                spec.face_rect(at),
                spec.full,
                ("drawer-face", what as u8, lane, index),
                |ui, face| paint_face(ui, index, face),
            );
            if response.clicked() {
                picked = Some(index);
            }
        }
    });
    // A click on the head the drawer opened from is the double-click that closes it, or the
    // first of it: not a click outside
    let on_head =
        pointer.is_some_and(|at| !rect.contains(at) && heads.iter().any(|head| head.contains(at)));
    if closed || close.escape || (close.outside && !on_head) {
        *slot = None;
        ui.ctx().request_repaint();
        return None;
    }
    if picked.is_some() {
        ui.ctx().request_repaint();
    }
    picked
}

/// A card: a plate with the miniature on it and, beside that, the name and what plays it
fn paint_card(ui: &Ui, spec: &Spec, rect: Rect, card: &Card, hovered: bool) {
    let painter = ui.painter();
    let radius = CornerRadius::same(7);
    painter.rect_filled(rect, radius, theme::blend(theme::PANEL, theme::SILK, 0.07));
    let (width, color) = if card.lit {
        (3.0_f32, ui::BRASS)
    } else if hovered {
        (2.0, ui::BRASS.gamma_multiply(0.6))
    } else {
        (1.5, theme::CHROME_DARK)
    };
    painter.rect_stroke(rect, radius, Stroke::new(width, color), StrokeKind::Inside);
    // Under the miniature, which is drawn in a layer above this one
    let face = spec.face_rect(rect);
    painter.rect_filled(face.translate(vec2(0.0, 6.0)).expand(3.0), 8.0, theme::SHADOW);
    let left = face.right() + PRINT_GAP;
    let lines = 1 + card.marks.len();
    let line = 20.0;
    let mut y = rect.center().y - line * (lines as f32 - 1.0) / 2.0;
    ui::left_label(painter, Pos2::new(left, y), &card.name, NAME_SIZE, theme::SILK);
    for mark in &card.marks {
        y += line;
        ui::left_label(painter, Pos2::new(left, y), mark, MARK_SIZE, ui::BRASS);
    }
}

/// The cards of the amps' drawer for `lane`: one for each of the twelve, the one that plays on
/// the lane outlined and a mark for each lane an amp plays on
pub(super) fn amp_cards(params: &crate::GuitarAmpParams, lane: usize) -> Vec<Card> {
    let playing = [params.amp.value(), params.amp2.value()];
    crate::Amp::ALL
        .iter()
        .map(|&amp| Card {
            name: amp.model().name.to_owned(),
            marks: (0..2)
                .filter(|&each| playing[each] == amp)
                .map(|each| format!("On Amp {}", each + 1))
                .collect(),
            lit: playing[lane] == amp,
        })
        .collect()
}

/// The look of the head the miniature on card `index` of the amps' drawer is drawn in
pub(super) fn amp_face(index: usize) -> &'static suite_common::ui::HeadStyle {
    &crate::looks::look(crate::Amp::ALL[index]).head
}

/// The amps' drawer, if it is open. A click on a card picks the amp for the drawer's lane as a
/// step of the old selector did, through the dial memory
pub(super) fn show_amps(
    ui: &mut Ui,
    window: Pos2,
    visible: f32,
    pedals: &super::Pedals,
    slot: &mut Option<Drawer>,
    now: f64,
) {
    let Some(open) = *slot else {
        return;
    };
    let (params, setter) = (pedals.params, pedals.setter);
    let cards = amp_cards(params, open.lane);
    let note = format!("For Amp {} · click one to play it", open.lane + 1);
    let paint = |ui: &Ui, index: usize, face: Rect| {
        ui::amp_head(ui, face, amp_face(index), true);
    };
    let heads = pedals.heads.get();
    let picked = show(ui, window, visible, slot, &cards, &note, &heads, &paint, now);
    if let Some(amp) = picked.map(|index| crate::Amp::ALL[index]) {
        if open.lane == 0 {
            crate::dial_memory::switch_amp(params, setter, amp);
        } else {
            crate::dial_memory::switch_second_amp(params, setter, amp);
        }
    }
}
