use super::*;

// The list the preset plate holds: the banks in the first column and the presets of the bank that
// is listed in the second
pub(super) const BANK_COLUMN: usize = 0;
pub(super) const DROP_BANK_WIDTH: f32 = 132.0;
pub(super) const DROP_PRESET_WIDTH: f32 = 186.0;
pub(super) const DROP_BANK_CHARS: usize = 12;
pub(super) const DROP_PRESET_CHARS: usize = 17;
// The row at the end of the banks that makes one, and how long a name typed on it may be
pub(super) const NEW_BANK_ROW: &str = "+ New Bank";
pub(super) const NEW_BANK_CHARS: usize = 14;
// The foot of the preset plate: the name display, where a new preset is named, and Save beside it
pub(super) const FOOT_HEIGHT: f32 = 46.0;
pub(super) const FOOT_CHARS: usize = 18;
pub(super) const FOOT_SAVE_REACH: f32 = 25.0;
pub(super) const FOOT_BUTTON_SPREAD: f32 = 40.0;
// How long a name typed for a new preset may be, and how much of it the display has room for
// (the caret takes one place)
pub(super) const NEW_PRESET_CHARS: usize = 24;
pub(super) const NEW_PRESET_SHOWN: usize = FOOT_CHARS - 1;

/// The preset display and the Save button beside it
pub(super) struct PresetDisplay<'a> {
    pub(super) params: &'a GuitarAmpParams,
    pub(super) setter: &'a ParamSetter<'a>,
    pub(super) cab_loader: &'a CabLoader,
    /// Has the chosen cabinet read in the background
    pub(super) load_cabinet: &'a dyn Fn(),
}

impl PresetDisplay<'_> {
    fn user_preset(&self) -> String {
        self.params.user_preset.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    fn set_user_preset(&self, file: &str) {
        file.clone_into(
            &mut self.params.user_preset.lock().unwrap_or_else(PoisonError::into_inner),
        );
    }

    /// The bank showing, which the preset display picks from
    fn bank(&self) -> String {
        user_presets::bank_of(self.params)
    }

    /// Writes down which bank is showing
    fn set_bank(&self, bank: &str) {
        bank.clone_into(
            &mut self.params.preset_bank.lock().unwrap_or_else(PoisonError::into_inner),
        );
    }

    /// Shows `bank`, with nothing loaded out of it: the sound stays as it stands, the
    /// display falls back to the name of the preset it came from, and Save makes a new
    /// preset in the bank
    fn show_bank(&self, bank: &str, view: &mut View) {
        self.set_bank(bank);
        self.set_user_preset("");
        view.bank = Bank::read(self.params, user_presets::bank_dir(bank).as_deref());
    }

    /// The names of the presets in `bank` without reading them: nothing for the factory
    /// bank, whose presets are built in
    fn bank_files(&self, bank: &str) -> Vec<String> {
        user_presets::bank_dir(bank)
            .map(|dir| user_cab::list_with_extension(&dir, user_presets::EXTENSION))
            .unwrap_or_default()
    }

    /// What the preset display says: the bank and the preset loaded last (marked once something was
    /// moved away from it), or for a moment what Save did
    fn shown(&self, view: &mut View, now: f64) -> String {
        match view.note {
            Some((note, until)) if now < until => note.to_owned(),
            _ => {
                view.note = None;
                let factory = presets::index_or_first(self.params.preset.load(Ordering::Relaxed));
                let user = self.user_preset();
                let bank = user_presets::bank_name(&self.bank(), BANK_SHOWN_CHARS);
                format!("{bank} \u{b7} {}", self.shown_name(factory, &user, &view.bank))
            }
        }
    }

    /// The preset display on the strip, with a marker at its end that says it opens: a click on it
    /// opens the plate of banks and presets under it, or closes it. There are no arrows: the plate
    /// is where a preset is picked
    pub(super) fn display(
        &self,
        ui: &mut Ui,
        origin: Pos2,
        view: &mut View,
        now: f64,
        host: Option<&dyn standalone::Host>,
    ) {
        let at = origin + vec2(PRESET_X, STRIP_Y);
        // The marker takes the place of the last two characters
        let shown: String = self.shown(view, now).chars().take(PRESET_CHARS - 2).collect();
        let window = Rect::from_center_size(at, vec2(PRESET_WIDTH, LED_HEIGHT));
        let response = ui
            .interact(window, Id::new("preset-display"), Sense::click())
            .on_hover_text("Banks and presets");
        led_display(ui.painter(), at, PRESET_CHARS, &shown);
        let open = view.is_open(Plates::Presets);
        if response.hovered() || open {
            ui.painter().rect_filled(window, 6.0, Color32::from_white_alpha(14));
        }
        // The marker: a small triangle, pointing down while it is shut and up while it is open
        let tip = if open { -1.0 } else { 1.0 };
        let mark = at + vec2(PRESET_WIDTH / 2.0 - 20.0, 0.0);
        ui.painter().add(nih_plug_egui::egui::Shape::convex_polygon(
            vec![
                mark + vec2(-5.0, -2.5 * tip),
                mark + vec2(5.0, -2.5 * tip),
                mark + vec2(0.0, 3.5 * tip),
            ],
            theme::TAPE_TEXT,
            nih_plug_egui::egui::Stroke::NONE,
        ));
        if response.clicked() {
            let bank = self.bank();
            view.toggle_plate(Plates::Presets, host, || {
                Open::Presets(Menu {
                    listed: bank.clone(),
                    files: self.bank_files(&bank),
                    naming: None,
                })
            });
        }
    }

    /// Keeps the name that was typed for a new preset and saves the sound under it
    fn finish_save(
        &self,
        dir: &Option<std::path::PathBuf>,
        factory: usize,
        user: &str,
        view: &mut View,
        now: f64,
    ) {
        let name = view.saving.take().unwrap_or_default();
        // The name of the preset loaded out of this bank, kept as it is, writes over it
        let loaded = dir.is_some().then_some(user).filter(|user| !user.is_empty());
        let named = (!user_presets::keeps_name(loaded, &name, NEW_PRESET_CHARS)).then_some(name);
        self.finish_save_as(dir, factory, user, view, now, named.as_deref());
    }

    /// Saves the sound into the bank showing and says how it went. `named` is the name typed
    /// for a new preset; none is the preset loaded being written over
    fn finish_save_as(
        &self,
        dir: &Option<std::path::PathBuf>,
        factory: usize,
        user: &str,
        view: &mut View,
        now: f64,
        named: Option<&str>,
    ) {
        // Nothing is written into the factory bank: a sound saved there goes into the
        // presets folder, which is the bank showing from then on
        let into = dir.clone().or_else(user_presets::presets_dir);
        let saved = into.as_deref().is_some_and(|into| self.save(into, factory, user, named, view));
        if saved && dir.is_none() {
            self.set_bank(user_presets::MINE);
        }
        view.note = Some((if saved { "SAVED" } else { "NOT SAVED" }, now + SAVE_NOTE_S));
    }

    /// Starts naming a new preset on the plate's foot, which starts as the name of the preset
    /// loaded out of this bank, so enter keeps it and writes over it; nothing is written until the
    /// name is kept. With a name being typed already, it is kept as it stands: the mouse alone
    /// is enough
    fn save_pressed(
        &self,
        dir: &Option<std::path::PathBuf>,
        factory: usize,
        user: &str,
        view: &mut View,
        now: f64,
    ) {
        if view.saving.is_some() {
            self.finish_save(dir, factory, user, view, now);
        } else {
            let loaded = dir.is_some().then_some(user).filter(|user| !user.is_empty());
            view.saving = Some(user_presets::name_to_keep(loaded, NEW_PRESET_CHARS));
            view.note = None;
        }
    }

    /// The plate under the preset display: the banks in the first column, the presets of the bank
    /// that is listed in the second, and under them the foot, where a preset is named and saved.
    /// It is drawn after everything else in the window, in a layer above it, so nothing behind it
    /// answers a click while it is open
    pub(super) fn plate(&self, ui: &mut Ui, origin: Pos2, view: &mut View, now: f64) {
        let mut menu = match view.plate.take() {
            Some(Open::Presets(menu)) => menu,
            other => {
                view.plate = other;
                return;
            }
        };
        let params = self.params;
        let presets_dir = user_presets::presets_dir();
        let banks = user_presets::banks(presets_dir.as_deref());

        // What is typed for a new bank is taken before the list is drawn, so the letter is
        // on the tape in the same frame. Escape ends the naming, not the plate
        let typed = menu.naming.as_mut().and_then(|name| type_into(ui, name, NEW_BANK_CHARS));
        match typed {
            Some(Typed::Done) => self.make_bank(&mut menu, presets_dir.as_deref(), view, now, ui),
            Some(Typed::Cancelled) => menu.naming = None,
            None => {}
        }

        let showing = self.bank();
        let user = self.user_preset();
        let factory = presets::index_or_first(params.preset.load(Ordering::Relaxed));
        let dir = user_presets::bank_dir(&showing);
        let listed_is_factory = user_presets::bank_dir(&menu.listed).is_none();
        // Which preset is lit: only the bank that is listed can hold the one loaded
        let loaded = match (menu.listed == showing, listed_is_factory) {
            (false, _) => Loaded::Nothing,
            (true, true) if user.is_empty() => Loaded::Factory(factory),
            (true, true) => Loaded::Nothing,
            (true, false) => Loaded::File(&user),
        };
        let bank_rows = bank_rows(&banks, &menu.listed, menu.naming.as_deref());
        let preset_rows =
            preset_rows((!listed_is_factory).then_some(menu.files.as_slice()), loaded);

        let columns = [
            Column { title: "Bank", key: "banks", width: DROP_BANK_WIDTH, rows: &bank_rows },
            Column {
                title: "Preset",
                key: &menu.listed,
                width: DROP_PRESET_WIDTH,
                rows: &preset_rows,
            },
        ];
        let anchor = Rect::from_center_size(
            origin + vec2(PRESET_X, STRIP_Y),
            vec2(PRESET_WIDTH, LED_HEIGHT),
        );
        let top = origin + vec2(PRESET_X, PLATE_TOP);
        // What is typed on the foot is taken in it, and whether it ended a name is kept
        let save_typed = std::cell::Cell::new(None);
        let picked =
            drop_panel_with(ui, top, &columns, "presets", anchor, FOOT_HEIGHT, |ui, foot| {
                let width = led_width(FOOT_CHARS);
                let name = Pos2::new(foot.left() + width / 2.0, foot.top() + 20.0);
                // A name being typed is taken before the display is drawn, so the letter is on it in
                // the same frame. Escape drops the name, and the sound is not saved
                if let Some(typing) = view.saving.as_mut() {
                    save_typed.set(type_on_tape(ui, name, width, typing, NEW_PRESET_CHARS));
                }
                match save_typed.get() {
                    Some(Typed::Done) => self.finish_save(&dir, factory, &user, view, now),
                    Some(Typed::Cancelled) => view.saving = None,
                    None => {}
                }
                let shown = match (&view.saving, view.note) {
                    (Some(typing), _) => {
                        let skip = typing.chars().count().saturating_sub(NEW_PRESET_SHOWN);
                        format!("{}_", typing.chars().skip(skip).collect::<String>())
                    }
                    (None, Some((note, until))) if now < until => note.to_owned(),
                    (None, _) => {
                        view.note = None;
                        self.shown_name(factory, &user, &view.bank)
                    }
                };
                let window = Rect::from_center_size(name, vec2(width, LED_HEIGHT));
                let click = ui.interact(window, Id::new("foot-name"), Sense::click());
                led_display(ui.painter(), name, FOOT_CHARS, &shown);
                if click.hovered() {
                    ui.painter().rect_filled(window, 6.0, Color32::from_white_alpha(14));
                }
                let save = Pos2::new(foot.right() - FOOT_SAVE_REACH + 3.0, foot.top() + 14.0);
                silk_label(ui.painter(), save + vec2(0.0, 22.0), "Save", 11.0);
                let pressed = chrome_button(ui, save, "save-preset").clicked();
                if click.clicked() || pressed {
                    self.save_pressed(&dir, factory, &user, view, now);
                }
                // The folder the presets of the bank are kept in: hovered it says where, and a click
                // opens it, to rename, move, copy or throw them away. The factory bank has no folder,
                // so it names the presets folder itself
                if let Some(folder) = dir.clone().or_else(user_presets::presets_dir) {
                    let at = save - vec2(FOOT_BUTTON_SPREAD, 0.0);
                    silk_label(ui.painter(), at + vec2(0.0, 22.0), "Folder", 11.0);
                    let button = chrome_button(ui, at, "preset-folder")
                        .on_hover_text(format!("Your presets are kept in {}", folder.display()));
                    if button.clicked() {
                        let _ = std::fs::create_dir_all(&folder);
                        user_cab::open_folder(&folder);
                    }
                }
            });
        // Neither the escape that ended a name nor the one that ended a bank's closes the plate
        let named = typed.is_some() || save_typed.get().is_some();
        let mut open = !(picked.closed && !named);
        // A note is only up for a while: draw once more when it is over
        if view.note.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(SAVE_NOTE_S));
        }

        match picked.row {
            // The row at the end of the banks takes a name for a new one. Clicked again it
            // keeps the name as it stands, so a bank can be made without the keyboard
            Some((BANK_COLUMN, row)) if row == banks.len() => {
                if menu.naming.is_some() {
                    self.make_bank(&mut menu, presets_dir.as_deref(), view, now, ui);
                } else {
                    menu.naming = Some(String::new());
                }
            }
            Some((BANK_COLUMN, row)) => {
                menu.naming = None;
                if let Some(bank) = banks.get(row) {
                    menu.files = self.bank_files(bank);
                    menu.listed = bank.to_owned();
                }
            }
            Some((_, row)) => {
                // The bank a preset is picked from is the bank showing from then on
                self.set_bank(&menu.listed);
                view.bank = Bank::read(params, user_presets::bank_dir(&menu.listed).as_deref());
                if listed_is_factory {
                    self.load_factory(row);
                } else {
                    self.load_user(row, view);
                }
                view.note = None;
                open = false;
            }
            None => {}
        }
        if open {
            view.plate = Some(Open::Presets(menu));
        } else {
            view.saving = None;
        }
    }

    /// Makes the bank of the name that was typed, and lists it. A name nothing was typed
    /// for is `Bank 1`, `Bank 2` and so on, which is renamed in the folder
    fn make_bank(
        &self,
        menu: &mut Menu,
        presets_dir: Option<&Path>,
        view: &mut View,
        now: f64,
        ui: &Ui,
    ) {
        let name = menu.naming.take().unwrap_or_default();
        match presets_dir.map(|dir| user_presets::make_bank(dir, &name)) {
            Some(Ok(made)) => {
                self.show_bank(&made, view);
                menu.files = self.bank_files(&made);
                menu.listed = made;
            }
            _ => {
                // The display says so where the name was typed, so it is drawn again
                view.note = Some(("NO BANK", now + SAVE_NOTE_S));
                ui.ctx().request_repaint();
            }
        }
    }

    /// The name of the preset loaded last, marked once something was moved away from it. A
    /// player's preset that is no longer in the folder has the cabinets' missing mark
    fn shown_name(&self, factory: usize, user: &str, bank: &Bank) -> String {
        if user.is_empty() {
            return PRESETS[factory].display_name(self.params);
        }
        let name = user_presets::display_name(user, USER_NAME_CHARS);
        match bank.find(user) {
            Some(preset) if preset.matches(self.params) => name,
            Some(_) => format!("{name}{}", presets::EDITED_MARK),
            None => format!("{}{name}", user_cab::MISSING_MARK),
        }
    }

    /// Loads the factory preset `index`
    fn load_factory(&self, index: usize) {
        let index = index.min(PRESETS.len() - 1);
        PRESETS[index].load(self.params, self.setter);
        self.params.preset.store(index as u32, Ordering::Relaxed);
        self.set_user_preset("");
    }

    /// Loads the preset at `at` of the bank the window has read, and the cabinet with it
    fn load_user(&self, at: usize, view: &View) {
        let Some((file, preset)) = view.bank.get(at) else {
            return;
        };
        if preset.load(self.params, self.setter) {
            self.cab_loader.chosen();
            (self.load_cabinet)();
        }
        self.set_user_preset(file);
    }

    /// Writes the sound as it stands: over the player's preset loaded last, or, as a
    /// new one, under the name typed, or after the preset it started as with none. Returns whether it was written
    fn save(
        &self,
        dir: &Path,
        factory: usize,
        user: &str,
        named: Option<&str>,
        view: &mut View,
    ) -> bool {
        view.bank = Bank::read(self.params, Some(dir));
        let file =
            user_presets::save_file_name(named, user, PRESETS[factory].name, &view.bank.files);
        if user_presets::save(self.params, dir, &file).is_err() {
            return false;
        }
        self.set_user_preset(&file);
        view.bank = Bank::read(self.params, Some(dir));
        true
    }
}

/// Which preset of the bank a column lists is loaded, and so lit
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Loaded<'a> {
    /// None of them: the sound came from another bank
    Nothing,
    Factory(usize),
    File(&'a str),
}

/// The rows of the bank column: every bank with the one that is listed lit, and the row
/// that makes a bank at the end, which holds the name while it is being typed
pub(super) fn bank_rows(banks: &[String], listed: &str, naming: Option<&str>) -> Vec<Row> {
    let mut rows: Vec<Row> = banks
        .iter()
        .map(|bank| Row {
            text: user_presets::bank_name(bank, DROP_BANK_CHARS),
            lit: bank == listed,
            ..Row::default()
        })
        .collect();
    rows.push(match naming {
        // The name as it is typed, with the caret after it. That row takes the keyboard
        Some(name) => Row { text: format!("{name}_"), lit: true, dim: false, typing: true },
        None => Row { text: NEW_BANK_ROW.to_owned(), ..Row::default() },
    });
    rows
}

/// The rows of the preset column: the factory presets when `files` is nothing, else the
/// files of the bank that is listed, with the one that is loaded lit. A bank with nothing
/// in it has one row that says so and is not picked
pub(super) fn preset_rows(files: Option<&[String]>, loaded: Loaded) -> Vec<Row> {
    let mut rows: Vec<Row> = match files {
        None => PRESETS
            .iter()
            .enumerate()
            .map(|(at, preset)| Row {
                text: preset.name.to_owned(),
                lit: loaded == Loaded::Factory(at),
                ..Row::default()
            })
            .collect(),
        Some(files) => files
            .iter()
            .map(|file| Row {
                text: user_presets::display_name(file, DROP_PRESET_CHARS),
                lit: loaded == Loaded::File(file),
                ..Row::default()
            })
            .collect(),
    };
    if rows.is_empty() {
        rows.push(Row { text: "No presets".to_owned(), dim: true, ..Row::default() });
    }
    rows
}
