//! The update line: one quiet strip of label tape that says a newer version is out.
//!
//! "0.4.0 IS OUT", a DOWNLOAD that opens the download page in the browser, an OFF that turns the
//! check off for good and a cross that closes the line for that version. It is a line of tape
//! like the value read-outs and the amp's displays, no new surface, never a pop-up and never in
//! the way of a control: each editor gives it a place of its own. Nothing is drawn when the
//! check has nothing to say (nothing newer, off, or closed for that version), so an editor
//! that has no update looks as it did. The check itself is `crate::update`.

use nih_plug_egui::egui::{
    vec2, Align2, Color32, CursorIcon, Id, Painter, Pos2, Rect, Sense, Stroke, Ui,
};

use super::hardware::paint_tape;
use super::theme::{layout, mono, TAPE_TEXT};
use crate::update::{open_url, UpdateCheck};

/// The height of the tape
pub const UPDATE_LINE_HEIGHT: f32 = 20.0;
const TEXT_SIZE: f32 = 10.0;
const PAD_X: f32 = 9.0;
const GAP: f32 = 9.0;
const CROSS: f32 = 3.5;
const DIM: f32 = 0.62;

/// What the line is made of, from the left
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Message,
    Download,
    Off,
    Close,
}

const PARTS: [Part; 4] = [Part::Message, Part::Download, Part::Off, Part::Close];

/// Where each part of the line stands when it is centred on `center`, and the tape under them
/// all, for a message of `text_widths` (the message, DOWNLOAD and OFF; the cross has a width of
/// its own). Pure geometry, so a test can hold the parts apart and the tape around them
pub fn parts(center: Pos2, text_widths: [f32; 3]) -> (Rect, [(Part, Rect); 4]) {
    let widths = [text_widths[0], text_widths[1], text_widths[2], CROSS * 2.0];
    let total = widths.iter().sum::<f32>() + GAP * 3.0;
    let mut x = center.x - total / 2.0;
    let mut out = [(Part::Message, Rect::NOTHING); 4];
    for (i, part) in PARTS.into_iter().enumerate() {
        let top_left = Pos2::new(x, center.y - UPDATE_LINE_HEIGHT / 2.0);
        out[i] = (part, Rect::from_min_size(top_left, vec2(widths[i], UPDATE_LINE_HEIGHT)));
        x += widths[i] + GAP;
    }
    let tape = Rect::from_center_size(center, vec2(total + 2.0 * PAD_X, UPDATE_LINE_HEIGHT));
    (tape, out)
}

fn tone(hot: bool) -> Color32 {
    if hot {
        TAPE_TEXT
    } else {
        TAPE_TEXT.gamma_multiply(DIM)
    }
}

/// Draws the line centred on `center` if `check` has a notice, and acts on a click on it.
/// Returns the tape's rectangle when it was drawn
pub fn update_line(ui: &Ui, center: Pos2, check: &UpdateCheck) -> Option<Rect> {
    let notice = check.notice()?;
    let painter = ui.painter();
    let message = format!("{} IS OUT", notice.latest);
    let galleys = [message.as_str(), "DOWNLOAD", "OFF"]
        .map(|text| layout(painter, text, mono(TEXT_SIZE), TAPE_TEXT, 0.04));
    let (tape, spots) = parts(center, galleys.clone().map(|g| g.size().x));
    paint_tape(painter, tape);
    let mut galleys = galleys.into_iter();
    for (part, rect) in spots {
        let response = (part != Part::Message).then(|| {
            let hit = rect.expand2(vec2(GAP / 2.0, 3.0));
            ui.interact(hit, Id::new(("update-line", part as u8)), Sense::click())
        });
        let hot = response.as_ref().is_some_and(|r| r.hovered());
        if hot {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if part == Part::Close {
            cross(painter, rect.center(), tone(hot));
        } else {
            let galley = galleys.next().expect("one text for each part");
            let color = if part == Part::Message { tone(false) } else { tone(hot) };
            let at = Align2::LEFT_CENTER.anchor_size(rect.left_center(), galley.size()).min;
            painter.galley_with_override_text_color(at, galley.clone(), color);
            // DOWNLOAD is the link: underlined
            if part == Part::Download {
                let y = rect.center().y + galley.size().y / 2.0 - 1.0;
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right() - 1.0, y)],
                    Stroke::new(1.0_f32, color),
                );
            }
        }
        if response.is_some_and(|r| r.clicked()) {
            match part {
                Part::Download => open_url(notice.url),
                Part::Off => check.turn_off(),
                Part::Close => check.dismiss(),
                Part::Message => {}
            }
            ui.ctx().request_repaint();
        }
    }
    Some(tape)
}

fn cross(painter: &Painter, center: Pos2, color: Color32) {
    let stroke = Stroke::new(1.4_f32, color);
    painter.line_segment([center + vec2(-CROSS, -CROSS), center + vec2(CROSS, CROSS)], stroke);
    painter.line_segment([center + vec2(-CROSS, CROSS), center + vec2(CROSS, -CROSS)], stroke);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_are_in_order_apart_and_on_the_tape() {
        let (tape, spots) = parts(Pos2::new(200.0, 40.0), [80.0, 52.0, 20.0]);
        assert_eq!(tape.center(), Pos2::new(200.0, 40.0));
        for pair in spots.windows(2) {
            assert!(pair[0].1.right() + GAP - 0.01 <= pair[1].1.left());
        }
        for (_, rect) in spots {
            assert!(tape.contains_rect(rect));
        }
    }
}
