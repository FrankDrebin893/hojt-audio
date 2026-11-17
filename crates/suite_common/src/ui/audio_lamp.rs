//! The no-audio lamp: a red signal lamp with NO AUDIO printed under it, for a standalone program
//! whose audio could not be opened and that plays on the dummy backend, silent.
//!
//! It is a `signal_lamp` in a cream ring with a caption (`SILK`, size 15, drawn heavier) under it, no new surface and never a pop-up. The reason
//! (what the host logged) is the lamp's hover hint; the amp also opens its Audio panel from a click
//! on it. Nothing is drawn when the audio runs, and never in a plugin in a DAW, where the host has
//! no status. The status is set before the window is made and does not change while it is open, so
//! the lamp is a still part of the picture and needs no repaint of its own.
//!
//! Each editor gives it a place of its own where no control, caption or art detail is under it:
//! on the amp's strip right of the settings icon, on a pedal over the model number in its lower
//! left (the Drum Synth, whose footswitches fill that corner, beside the lamp between the name band
//! and the Mixer frame).

use nih_plug_egui::egui::{vec2, Align2, CursorIcon, Id, Pos2, Rect, Response, Sense, Stroke, Ui};

use super::theme::{display, layout, SILK};
use super::widgets::signal_lamp;

/// How far under the lamp the caption stands, and its size: 15 (a signal lamp's is 13) so that it
/// reads at the real size, 9 px of capitals
pub const CAPTION_DY: f32 = 24.0;
const CAPTION_SIZE: f32 = 15.0;
/// The cream ring round the lamp: it tells this lamp from a footswitch's LED, which is the same red
const RING_RADIUS: f32 = 12.5;
/// The words under the lamp
pub const CAPTION: &str = "No Audio";
/// Why the standalone program has no audio: the audio is the dummy backend
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioNotice {
    /// The error the host logged when it fell back to the dummy backend. `None` when the dummy
    /// backend was asked for (`--backend dummy`)
    pub reason: Option<String>,
}

impl AudioNotice {
    /// The notice the standalone host's status asks for now: `Some` while the audio is the dummy
    /// backend, `None` while it runs, in a DAW, and in a build without the standalone host
    #[cfg(feature = "standalone")]
    pub fn current() -> Option<Self> {
        use nih_plug::prelude::{nih_standalone_audio_status, StandaloneAudioStatus};
        notice_for(nih_standalone_audio_status().map(|status| match status {
            StandaloneAudioStatus::Dummy { reason } => HostAudio::Dummy { reason },
            StandaloneAudioStatus::Running { .. } => HostAudio::Running,
        }))
    }

    /// The notice the standalone host's status asks for now: none in a build without the host
    #[cfg(not(feature = "standalone"))]
    pub fn current() -> Option<Self> {
        notice_for(None)
    }

    /// What the lamp says when hovered
    pub fn hint(&self) -> String {
        match &self.reason {
            Some(reason) => format!("No audio device could be opened: {reason}"),
            None => "The dummy audio backend was asked for: no audio and no MIDI".to_string(),
        }
    }
}

/// What the standalone host says of its audio (`nih_standalone_audio_status`), without the host:
/// the part of the mapping to a notice that tests can reach in a build without the standalone feature
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostAudio {
    /// A real audio backend plays
    Running,
    /// The dummy backend plays: `reason` is the error the host logged, `None` when it was asked for
    Dummy { reason: Option<String> },
}

/// The notice for a host status: `None` for a plugin in a DAW (no status) and while the audio runs
pub fn notice_for(status: Option<HostAudio>) -> Option<AudioNotice> {
    match status? {
        HostAudio::Dummy { reason } => Some(AudioNotice { reason }),
        HostAudio::Running => None,
    }
}

/// The room the lamp and its caption take, centred on the lamp's `center`
pub fn rect(center: Pos2) -> Rect {
    Rect::from_min_max(center + vec2(-34.0, -14.0), center + vec2(34.0, CAPTION_DY + 10.0))
}

/// Where the caption stands
#[derive(Clone, Copy, Debug, PartialEq)]
enum Caption {
    /// Centred under the lamp
    Under,
    /// Under the lamp, its left edge on this x (window coordinates)
    UnderLeft(f32),
    /// To the right of the lamp, on its line
    Beside,
    /// None: the lamp alone, where the caption would be too small to read (a hover says why)
    Bare,
}

/// Draws the lamp and its caption with the lamp on `center` if there is a notice, and returns the
/// response of the whole of it (a click, a hover) when it was drawn
pub fn audio_lamp(ui: &Ui, center: Pos2, notice: Option<&AudioNotice>) -> Option<Response> {
    draw(ui, center, Caption::Under, notice)
}

/// `audio_lamp` with the caption's left edge on `left` (window coordinates) instead of centred
/// under the lamp: for a pedal's lower left corner, where it stands over the model number and
/// shares its left edge (`MODEL_LEFT`)
pub fn audio_lamp_left(
    ui: &Ui,
    center: Pos2,
    left: f32,
    notice: Option<&AudioNotice>,
) -> Option<Response> {
    draw(ui, center, Caption::UnderLeft(left), notice)
}

/// `audio_lamp` with the caption to the right of the lamp, for a place that is wide and not high
pub fn audio_lamp_beside(ui: &Ui, center: Pos2, notice: Option<&AudioNotice>) -> Option<Response> {
    draw(ui, center, Caption::Beside, notice)
}

/// The lamp alone, with no caption (a window scaled down so far that print would not read)
pub fn audio_lamp_bare(ui: &Ui, center: Pos2, notice: Option<&AudioNotice>) -> Option<Response> {
    draw(ui, center, Caption::Bare, notice)
}

/// The caption: cream print on whole pixels, laid twice a half pixel apart for a heavier stroke
/// than the display font has at this size. Returns the room it takes
fn caption_text(ui: &Ui, pos: Pos2, anchor: Align2) -> Rect {
    let painter = ui.painter();
    let ppp = ui.ctx().pixels_per_point();
    let galley = layout(painter, &CAPTION.to_uppercase(), display(CAPTION_SIZE), SILK, 0.14);
    let rect = anchor.anchor_size(pos, galley.size());
    let snap = |v: f32| (v * ppp).round() / ppp;
    let min = Pos2::new(snap(rect.min.x), snap(rect.min.y));
    painter.galley(min, galley.clone(), SILK);
    painter.galley(min + vec2(0.5 / ppp, 0.0), galley, SILK);
    Rect::from_min_size(min, rect.size())
}

fn draw(ui: &Ui, center: Pos2, caption: Caption, notice: Option<&AudioNotice>) -> Option<Response> {
    let notice = notice?;
    let painter = ui.painter();
    signal_lamp(painter, center, true);
    painter.circle_stroke(center, RING_RADIUS, Stroke::new(1.5_f32, SILK));
    let under = center + vec2(0.0, CAPTION_DY);
    let mut room = rect(center);
    match caption {
        Caption::Under => room = room.union(caption_text(ui, under, Align2::CENTER_CENTER)),
        Caption::UnderLeft(left) => {
            let at = Pos2::new(left, under.y);
            room = room.union(caption_text(ui, at, Align2::LEFT_CENTER));
        }
        Caption::Beside => {
            let at = center + vec2(RING_RADIUS + 6.0, 0.0);
            room = room.union(caption_text(ui, at, Align2::LEFT_CENTER));
        }
        Caption::Bare => {}
    }
    let response = ui.interact(room, Id::new("no-audio-lamp"), Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    Some(response.on_hover_text(notice.hint()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hint_gives_the_reason_when_there_is_one() {
        let named = AudioNotice { reason: Some("48000 Hz not offered".into()) };
        assert!(named.hint().contains("48000 Hz not offered"));
        let asked = AudioNotice { reason: None };
        assert!(asked.hint().contains("asked for"));
    }

    #[test]
    fn the_notice_follows_the_host_status() {
        assert_eq!(notice_for(None), None, "a plugin in a DAW has no status");
        assert_eq!(notice_for(Some(HostAudio::Running)), None, "audio runs");
        assert_eq!(
            notice_for(Some(HostAudio::Dummy { reason: Some("no device".into()) })),
            Some(AudioNotice { reason: Some("no device".into()) })
        );
        assert_eq!(
            notice_for(Some(HostAudio::Dummy { reason: None })),
            Some(AudioNotice { reason: None })
        );
    }

    #[test]
    fn the_room_holds_the_lamp_and_its_caption() {
        let room = rect(Pos2::new(100.0, 100.0));
        assert!(room.contains(Pos2::new(100.0, 100.0)));
        assert!(room.contains(Pos2::new(100.0, 100.0 + CAPTION_DY)));
    }
}
