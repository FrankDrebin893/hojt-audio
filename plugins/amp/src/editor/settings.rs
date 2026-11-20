use super::*;

// The settings plate: a row to the cabinet files and, in the standalone program, a row to each
// of the audio's choices, with Apply and Cancel under them
pub(super) const SETTINGS_PAD: f32 = 16.0;
pub(super) const SETTINGS_CAPTION_WIDTH: f32 = 130.0;
pub(super) const SETTINGS_TAPE_WIDTH: f32 = 330.0;
pub(super) const SETTINGS_ROW_HEIGHT: f32 = 34.0;
pub(super) const SETTINGS_NOTE_HEIGHT: f32 = 26.0;
/// How often the settings plate asks whether the new audio devices have opened
pub(super) const OPENING_POLL: std::time::Duration = std::time::Duration::from_millis(50);
pub(super) const SETTINGS_FOOT_HEIGHT: f32 = 58.0;
pub(super) const SETTINGS_BUTTON_SPREAD: f32 = 110.0;
// A stepper reaches its buttons this far past its tape on each side
pub(super) const SETTINGS_OVERHANG: f32 = 31.0;
// The longest cabinet name the settings' tape has room for
pub(super) const SETTINGS_CAB_CHARS: usize = 26;
pub(super) const SETTINGS_WIDTH: f32 =
    2.0 * SETTINGS_PAD + SETTINGS_CAPTION_WIDTH + SETTINGS_TAPE_WIDTH + 2.0 * SETTINGS_OVERHANG;

/// The settings plate under its icon, while it is the plate that is open: the cabinet files and,
/// in the standalone program, the machine's audio, which Apply opens while the amp plays (or starts the amp again on)
pub(super) fn settings_plate(
    ui: &mut Ui,
    _setter: &ParamSetter,
    params: &GuitarAmpParams,
    origin: Pos2,
    view: &mut View,
    shared: &Shared,
    anchor: Rect,
) {
    let host = shared.audio_host;
    let Some(Open::Settings(audio)) = view.plate.as_mut() else {
        return;
    };
    // The devices being opened answer from another thread: the panel looks again until they have
    if audio.as_mut().is_some_and(AudioChoice::poll) {
        ui.ctx().request_repaint_after(OPENING_POLL);
    }
    let audio_rows = audio.as_ref().map(AudioChoice::rows);
    let rows = 1 + audio_rows.as_ref().map_or(0, |rows| rows.len());
    let height = settings_height(audio_rows.is_some(), rows);
    let rect = Rect::from_min_size(
        origin + vec2(SETTINGS_X + ui::icons::ICON_REACH - SETTINGS_WIDTH, PLATE_TOP),
        vec2(SETTINGS_WIDTH, height),
    );
    let mut cancelled = false;
    let mut event = None;
    let close = ui::plate(ui, rect, "settings", anchor, |ui, rect| {
        ui::plate::plate_title(ui.painter(), rect, "Settings");
        let painter = ui.painter().clone();
        let mut y = rect.top() + SETTINGS_PAD + ui::plate::PLATE_TITLE_HEIGHT;
        let caption_x = rect.left() + SETTINGS_PAD + SETTINGS_CAPTION_WIDTH / 2.0;
        let tape_x = rect.left()
            + SETTINGS_PAD
            + SETTINGS_CAPTION_WIDTH
            + SETTINGS_OVERHANG
            + SETTINGS_TAPE_WIDTH / 2.0;

        // The cabinet: the amp's own, a built-in one or a file, picked as on the amp's head. Where
        // the files go is shown when the caption is hovered, and opened by a double-click on it
        let middle = y + SETTINGS_ROW_HEIGHT / 2.0;
        silk_label(&painter, Pos2::new(caption_x, middle), "Cabinet", 13.0);
        if let Some(dir) = user_cab::cabinets_dir() {
            let caption = Rect::from_center_size(Pos2::new(caption_x, middle), vec2(90.0, 20.0));
            let response = ui
                .interact(caption, Id::new("settings-cabinet-folder"), Sense::click())
                .on_hover_text(format!("Cabinet files go in {}", dir.display()));
            if response.double_clicked() {
                let _ = std::fs::create_dir_all(&dir);
                user_cab::open_folder(&dir);
            }
        }
        if shared.cab_loader.no_files_showing() {
            // Draw once more when the note is over
            ui.ctx().request_repaint_after(NO_FILES_SHOWN);
        }
        let chosen = params.cabinet.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let shown = shared.cab_loader.display_name(&chosen, SETTINGS_CAB_CHARS);
        if let Some(step) =
            stepper(ui, Pos2::new(tape_x, middle), SETTINGS_TAPE_WIDTH, &shown, "settings-cabinet")
        {
            step_cabinet(params, shared.cab_loader, shared.load_cabinet, step == Step::Next);
        }
        y += SETTINGS_ROW_HEIGHT;

        if let Some(rows) = &audio_rows {
            for (index, (caption, shown)) in rows.iter().enumerate() {
                let middle = y + SETTINGS_ROW_HEIGHT / 2.0;
                silk_label(&painter, Pos2::new(caption_x, middle), caption, 13.0);
                if let Some(step) = stepper(
                    ui,
                    Pos2::new(tape_x, middle),
                    SETTINGS_TAPE_WIDTH,
                    shown,
                    &format!("audio-{index}"),
                ) {
                    event = Some(SettingsEvent::Step(index, step));
                }
                y += SETTINGS_ROW_HEIGHT;
            }
        }
        // The line under the rows, and for the audio the two buttons
        let choice = audio.as_ref();
        let note = match (choice.and_then(|choice| choice.error.as_deref()), choice) {
            (Some(error), _) => error,
            (None, Some(choice)) if choice.is_opening() => "Opening ...",
            (None, Some(choice)) if choice.test_tone => {
                "The tone is on the chosen outputs; mind the volume"
            }
            (None, Some(choice)) if choice.applied => "Playing on the new settings",
            (None, Some(_)) => "Apply opens the new settings",
            (None, None) => "Cabinet files are read from the cabinets folder",
        };
        silk_label(
            &painter,
            Pos2::new(rect.center().x, y + SETTINGS_NOTE_HEIGHT / 2.0),
            note,
            12.0,
        );
        y += SETTINGS_NOTE_HEIGHT;
        if audio.is_some() {
            let buttons = y + 16.0;
            for (side, caption, id, wanted) in [
                (-1.0, "Apply", "apply", SettingsEvent::Apply),
                (1.0, "Cancel", "cancel", SettingsEvent::Cancel),
            ] {
                let at = Pos2::new(rect.center().x + side * SETTINGS_BUTTON_SPREAD, buttons);
                silk_label(&painter, at + vec2(0.0, 25.0), caption, 12.0);
                if chrome_button(ui, at, &format!("settings-{id}")).clicked() {
                    event = Some(wanted);
                }
            }
        }
    });
    match event {
        Some(SettingsEvent::Step(row, step)) => {
            if let (Some(choice), Some(host)) = (audio.as_mut(), host) {
                choice.step(host, row, step == Step::Next);
            }
        }
        Some(SettingsEvent::Apply) => {
            if let (Some(choice), Some(host)) = (audio.as_mut(), host) {
                match choice.apply(host) {
                    // The new program is on its way: this window is done
                    Applied::Restarting => ui.ctx().send_viewport_cmd(ViewportCommand::Close),
                    Applied::Opening => ui.ctx().request_repaint(),
                    Applied::Failed => {}
                }
            }
        }
        Some(SettingsEvent::Cancel) => cancelled = true,
        None => {}
    }
    if close.asked() || cancelled {
        view.close_plate(host);
    }
}

/// What a row of the audio part of the settings plate asked for
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SettingsEvent {
    /// A row was stepped to the previous or the next entry of its list
    Step(usize, Step),
    /// Take the settings: the amp starts again on them
    Apply,
    /// Leave them as they were
    Cancel,
}

/// How high the settings plate is: its title, a row for the cabinet and for each of the audio's
/// choices, the note, and the buttons of the audio
pub(super) fn settings_height(with_audio: bool, rows: usize) -> f32 {
    2.0 * SETTINGS_PAD
        + ui::plate::PLATE_TITLE_HEIGHT
        + rows as f32 * SETTINGS_ROW_HEIGHT
        + SETTINGS_NOTE_HEIGHT
        + if with_audio { SETTINGS_FOOT_HEIGHT } else { 0.0 }
}
