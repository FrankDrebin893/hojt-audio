//! The no-audio lamp: a red signal lamp with NO AUDIO printed under it, for a standalone program
//! whose audio could not be opened and that plays on the dummy backend, silent.
//!
//! It is a `signal_lamp` with a `silk_label` under it, no new surface and never a pop-up. The reason
//! (what the host logged) is the lamp's hover hint; the amp also opens its Audio panel from a click
//! on it. Nothing is drawn when the audio runs, and never in a plugin in a DAW, where the host has
//! no status. The status is set before the window is made and does not change while it is open, so
//! the lamp is a still part of the picture and needs no repaint of its own.
//!
//! Each editor gives it a place of its own where no control, caption or art detail is under it:
//! on the amp's strip right of the settings icon, on a pedal beside its footswitches.

use nih_plug_egui::egui::{vec2, CursorIcon, Id, Pos2, Rect, Response, Sense, Ui};

use super::widgets::{signal_lamp, silk_label};

/// How far under the lamp the caption stands, and its size: a signal lamp's caption
pub const CAPTION_DY: f32 = 18.0;
const CAPTION_SIZE: f32 = 11.0;
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
        match nih_standalone_audio_status()? {
            StandaloneAudioStatus::Dummy { reason } => Some(Self { reason }),
            StandaloneAudioStatus::Running { .. } => None,
        }
    }

    /// The notice the standalone host's status asks for now: none in a build without the host
    #[cfg(not(feature = "standalone"))]
    pub fn current() -> Option<Self> {
        None
    }

    /// What the lamp says when hovered
    pub fn hint(&self) -> String {
        match &self.reason {
            Some(reason) => format!("No audio device could be opened: {reason}"),
            None => "The dummy audio backend was asked for: no audio and no MIDI".to_string(),
        }
    }
}

/// The room the lamp and its caption take, centred on the lamp's `center`
pub fn rect(center: Pos2) -> Rect {
    Rect::from_min_max(center + vec2(-26.0, -12.0), center + vec2(26.0, CAPTION_DY + 8.0))
}

/// Draws the lamp and its caption with the lamp on `center` if there is a notice, and returns the
/// response of the whole of it (a click, a hover) when it was drawn
pub fn audio_lamp(ui: &Ui, center: Pos2, notice: Option<&AudioNotice>) -> Option<Response> {
    let notice = notice?;
    let painter = ui.painter();
    signal_lamp(painter, center, true);
    silk_label(painter, center + vec2(0.0, CAPTION_DY), CAPTION, CAPTION_SIZE);
    let response = ui.interact(rect(center), Id::new("no-audio-lamp"), Sense::click());
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
    fn the_room_holds_the_lamp_and_its_caption() {
        let room = rect(Pos2::new(100.0, 100.0));
        assert!(room.contains(Pos2::new(100.0, 100.0)));
        assert!(room.contains(Pos2::new(100.0, 100.0 + CAPTION_DY)));
    }
}
