//! The player's own presets: the bank they save their sounds in, next to the factory ones.
//!
//! Each preset is one small text file in `Hojt Audio/Presets` in the user's Documents
//! folder. Its name is the file's name, so a preset is renamed, copied or thrown away in the
//! file manager, and a preset can be handed to someone else as a file. There is no dialog:
//! the Bank and Preset displays on the head pick one, in name order, and the Save button
//! beside them writes the sound as it stands into the bank that is showing.
//!
//! A bank is where a set of presets is kept, and a bank id says which: the empty id is the
//! factory presets, `/` is the presets folder itself, where every preset was kept before
//! there were banks, and `/<folder>` is a folder in it that the player made for sounds of
//! one kind of their own. Only the folders directly in the presets folder are banks, and
//! what is deeper is left alone. An id never holds a path: the folder is one plain name.
//!
//! A file holds what a factory preset holds, and also the microphone and the cabinet:
//! every parameter except Bypass, the Tuner switch, Input and Output, then the order of the
//! pedals and the cabinet choice. One line each, `id = value`, by the frozen parameter ids:
//!
//! ```text
//! amp = torden
//! gain = 0.55
//! gate_on = on
//! order = fuzz comp drive phaser amp chorus flanger tremolo delay reverb
//! cabinet = speaker:4x12-mid
//! ```
//!
//! Knobs are written as their parameters store them, switches `on` or `off`, and choices by
//! their ids. Reading forgives: a line it does not understand is skipped, and what a file
//! does not name gets its default, so a preset saved by an older build still loads whole.
//!
//! Everything in here reads or writes files: none of it is for the audio thread.

use nih_plug::prelude::*;
use std::path::{Path, PathBuf};

use crate::cab::MicType;
use crate::dial_memory;
use crate::order::ChainOrder;
use crate::presets::{self, Edit, MATCH_TOLERANCE, PRESETS};
use crate::user_cab;
use crate::{Amp, GuitarAmpParams};

/// Set this to a folder to have the presets kept there instead of in Documents
pub const FOLDER_ENV: &str = "HOJT_PRESETS_DIR";
// Below the Documents folder
const FOLDER: [&str; 2] = [suite_common::VENDOR, "Presets"];
pub const EXTENSION: &str = "preset";

// The keys of the lines that are not parameters
const ORDER_KEY: &str = "order";
const CABINET_KEY: &str = "cabinet";
const HEADER: &str = "# Hojt Guitar Amp preset";

/// Where the presets are: `FOLDER_ENV` if set, else `Hojt Audio/Presets` in Documents.
/// Looks nothing up on the disk and creates nothing
pub fn presets_dir() -> Option<PathBuf> {
    user_cab::folder_in_documents(FOLDER_ENV, &FOLDER)
}

/// The bank of the factory presets: the one that is no folder at all
pub const FACTORY: &str = "";
/// The bank of the presets loose in the presets folder
pub const MINE: &str = "/";
// What the displays call the two banks that are not a folder of the player's
const FACTORY_NAME: &str = "Factory";
const MINE_NAME: &str = "Mine";

/// Whether `bank` is an id a bank may have: the factory presets, the presets folder itself,
/// or one plain folder name in it. Never `..`, and never a path on any system: a backslash
/// is a separator on Windows and a letter of a name elsewhere, so an id that holds one is
/// refused everywhere, and a project carries the same bank from one system to the other
pub fn is_bank(bank: &str) -> bool {
    match bank.strip_prefix('/') {
        None => bank.is_empty(),
        Some("") => true,
        Some(folder) => user_cab::is_file_name(folder) && !folder.contains('\\'),
    }
}

/// The id of the bank kept in the folder `folder` of the presets folder
pub fn folder_bank(folder: &str) -> String {
    format!("{MINE}{folder}")
}

/// Where the presets of `bank` are kept: nothing for the factory bank, for an id no bank
/// may have, or when there is no presets folder
pub fn bank_dir(bank: &str) -> Option<PathBuf> {
    let folder = bank.strip_prefix('/').filter(|_| is_bank(bank))?;
    let dir = presets_dir()?;
    Some(if folder.is_empty() { dir } else { dir.join(folder) })
}

/// Every bank there is now, in the order the displays go round them: the factory presets,
/// the presets folder itself, then the folders in it in name order. `dir` is the presets
/// folder, which does not have to be there
pub fn banks(dir: Option<&Path>) -> Vec<String> {
    let mut banks = vec![FACTORY.to_owned(), MINE.to_owned()];
    if let Some(dir) = dir {
        banks.extend(user_cab::folders(dir).iter().map(|folder| folder_bank(folder)));
    }
    banks
}

/// What a display calls `bank`, with `max_chars` at most
pub fn bank_name(bank: &str, max_chars: usize) -> String {
    let name = match bank.strip_prefix('/') {
        None => FACTORY_NAME,
        Some("") => MINE_NAME,
        Some(folder) => folder,
    };
    user_cab::display_name_without(name, "", max_chars)
}

/// The bank a step arrives at, round the banks there are now
pub fn neighbour_bank(banks: &[String], bank: &str, forward: bool) -> String {
    if banks.is_empty() {
        return FACTORY.to_owned();
    }
    let at = banks.iter().position(|each| each == bank).unwrap_or(0);
    let count = banks.len();
    banks[(at + if forward { 1 } else { count - 1 }) % count].clone()
}

/// The bank a window shows for a project that holds `bank` with `user` loaded out of it:
/// what it holds, and the presets folder itself for a project saved before there were
/// banks, which has a preset of the player's and no bank. An id no bank may have is read
/// as the factory bank
pub fn bank_shown(bank: &str, user: &str) -> String {
    if bank.is_empty() && !user.is_empty() {
        return MINE.to_owned();
    }
    if is_bank(bank) {
        bank.to_owned()
    } else {
        FACTORY.to_owned()
    }
}

/// The bank a project opens on: `bank_shown` of what it holds
pub fn bank_of(params: &GuitarAmpParams) -> String {
    let bank = params.preset_bank.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let user = params.user_preset.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    bank_shown(&bank, &user)
}

/// Makes the bank `name` in `dir` and gives back its id. The name is filtered like a file
/// name, and one that nothing is left of gets the next free `Bank <n>`, so a bank can be
/// made without typing anything and renamed in the folder afterwards. A folder that is
/// already there is simply that bank
pub fn make_bank(dir: &Path, name: &str) -> std::io::Result<String> {
    let mut folder = plain_name(name);
    if folder.is_empty() {
        folder = new_bank_name(dir);
    }
    if !user_cab::is_file_name(&folder) {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    std::fs::create_dir_all(dir.join(&folder))?;
    Ok(folder_bank(&folder))
}

/// A name for a bank nobody named: `Bank 1`, `Bank 2` and so on, the first that is not
/// taken in `dir` (letters in either case)
pub fn new_bank_name(dir: &Path) -> String {
    let taken = user_cab::folders(dir);
    (1..)
        .map(|number| format!("Bank {number}"))
        .find(|name| !taken.iter().any(|each| each.eq_ignore_ascii_case(name)))
        .unwrap_or_default()
}

/// One parameter a preset holds
#[derive(Clone, Copy)]
enum Control<'a> {
    Knob(&'a FloatParam),
    Switch(&'a BoolParam),
    Amp(&'a EnumParam<Amp>),
    Mic(&'a EnumParam<MicType>),
}

impl Control<'_> {
    /// The value as its parameter stores it, a switch as 0 or 1 and a choice as its index
    fn plain(self) -> f32 {
        match self {
            Control::Knob(param) => param.unmodulated_plain_value(),
            Control::Switch(param) => f32::from(u8::from(param.unmodulated_plain_value())),
            Control::Amp(param) => param.unmodulated_plain_value().to_index() as f32,
            Control::Mic(param) => param.unmodulated_plain_value().to_index() as f32,
        }
    }

    fn default_plain(self) -> f32 {
        match self {
            Control::Knob(param) => param.default_plain_value(),
            Control::Switch(param) => f32::from(u8::from(param.default_plain_value())),
            Control::Amp(param) => param.default_plain_value().to_index() as f32,
            Control::Mic(param) => param.default_plain_value().to_index() as f32,
        }
    }

    fn normalized(self, plain: f32) -> f32 {
        match self {
            Control::Knob(param) => param.preview_normalized(plain),
            Control::Switch(param) => param.preview_normalized(plain >= 0.5),
            Control::Amp(param) => param.preview_normalized(Amp::from_index(plain as usize)),
            Control::Mic(param) => param.preview_normalized(MicType::from_index(plain as usize)),
        }
    }

    fn unmodulated_normalized(self) -> f32 {
        match self {
            Control::Knob(param) => param.unmodulated_normalized_value(),
            Control::Switch(param) => param.unmodulated_normalized_value(),
            Control::Amp(param) => param.unmodulated_normalized_value(),
            Control::Mic(param) => param.unmodulated_normalized_value(),
        }
    }

    fn edit(self, plain: f32) -> Edit {
        match self {
            Control::Knob(param) => Edit::of(param, plain),
            Control::Switch(param) => Edit::of(param, plain >= 0.5),
            Control::Amp(param) => Edit::of(param, Amp::from_index(plain as usize)),
            Control::Mic(param) => Edit::of(param, MicType::from_index(plain as usize)),
        }
    }

    /// The value as a file has it
    fn write(self, plain: f32) -> String {
        match self {
            Control::Knob(_) => format!("{plain}"),
            Control::Switch(_) => if plain >= 0.5 { "on" } else { "off" }.to_owned(),
            Control::Amp(_) => enum_id::<Amp>(plain as usize).to_owned(),
            Control::Mic(_) => enum_id::<MicType>(plain as usize).to_owned(),
        }
    }

    /// The value a file has written, if it is one this parameter can take
    fn read(self, text: &str) -> Option<f32> {
        match self {
            Control::Knob(_) => text.parse::<f32>().ok().filter(|value| value.is_finite()),
            Control::Switch(_) => match text {
                "on" => Some(1.0),
                "off" => Some(0.0),
                _ => None,
            },
            Control::Amp(_) => enum_index::<Amp>(text).map(|index| index as f32),
            Control::Mic(_) => enum_index::<MicType>(text).map(|index| index as f32),
        }
    }
}

fn enum_id<T: Enum>(index: usize) -> &'static str {
    T::ids().map_or("", |ids| ids.get(index).copied().unwrap_or(""))
}

fn enum_index<T: Enum>(id: &str) -> Option<usize> {
    T::ids()?.iter().position(|&each| each == id)
}

/// Every parameter a preset holds, by its id: all of them but `LEFT_ALONE`. The amp comes
/// first, so the host hears it change before its dials do
fn controls(params: &GuitarAmpParams) -> [(&'static str, Control<'_>); 43] {
    use Control::{Knob, Mic, Switch};
    [
        ("amp", Control::Amp(&params.amp)),
        ("gain", Knob(&params.gain)),
        ("bass", Knob(&params.bass)),
        ("mid", Knob(&params.mid)),
        ("treble", Knob(&params.treble)),
        ("presence", Knob(&params.presence)),
        ("master", Knob(&params.master)),
        ("gate_on", Switch(&params.gate_on)),
        ("gate_thresh", Knob(&params.gate_thresh)),
        ("gate_release", Knob(&params.gate_release)),
        ("fuzz_on", Switch(&params.fuzz_on)),
        ("fuzz_fuzz", Knob(&params.fuzz_fuzz)),
        ("fuzz_tone", Knob(&params.fuzz_tone)),
        ("comp_on", Switch(&params.comp_on)),
        ("comp_sustain", Knob(&params.comp_sustain)),
        ("comp_level", Knob(&params.comp_level)),
        ("drive_on", Switch(&params.drive_on)),
        ("drive_gain", Knob(&params.drive_gain)),
        ("drive_tone", Knob(&params.drive_tone)),
        ("drive_level", Knob(&params.drive_level)),
        ("chorus_on", Switch(&params.chorus_on)),
        ("chorus_rate", Knob(&params.chorus_rate)),
        ("chorus_depth", Knob(&params.chorus_depth)),
        ("phaser_on", Switch(&params.phaser_on)),
        ("phaser_rate", Knob(&params.phaser_rate)),
        ("phaser_depth", Knob(&params.phaser_depth)),
        ("flanger_on", Switch(&params.flanger_on)),
        ("flanger_rate", Knob(&params.flanger_rate)),
        ("flanger_depth", Knob(&params.flanger_depth)),
        ("trem_on", Switch(&params.trem_on)),
        ("trem_rate", Knob(&params.trem_rate)),
        ("trem_depth", Knob(&params.trem_depth)),
        ("cab_on", Switch(&params.cab_on)),
        ("cab_mic", Knob(&params.cab_mic)),
        ("cab_res", Knob(&params.cab_res)),
        ("cab_mic_type", Mic(&params.cab_mic_type)),
        ("delay_on", Switch(&params.delay_on)),
        ("delay_time", Knob(&params.delay_time)),
        ("delay_feedback", Knob(&params.delay_feedback)),
        ("delay_mix", Knob(&params.delay_mix)),
        ("reverb_on", Switch(&params.reverb_on)),
        ("reverb_decay", Knob(&params.reverb_decay)),
        ("reverb_mix", Knob(&params.reverb_mix)),
    ]
}

/// A sound the player saved
#[derive(Clone, Debug, PartialEq)]
pub struct UserPreset {
    /// In the order of `controls`, as the parameters store them
    values: Vec<f32>,
    order: ChainOrder,
    /// What `GuitarAmpParams::cabinet` holds: empty for the amp's own
    cabinet: String,
}

impl UserPreset {
    /// The sound as it stands now
    pub fn capture(params: &GuitarAmpParams) -> Self {
        Self {
            values: controls(params).iter().map(|&(_, control)| control.plain()).collect(),
            order: params.order.get(),
            cabinet: params
                .cabinet
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        }
    }

    /// What a file says, with the default for whatever it does not
    pub fn parse(params: &GuitarAmpParams, text: &str) -> Self {
        let mut preset = Self {
            values: controls(params).iter().map(|&(_, control)| control.default_plain()).collect(),
            order: ChainOrder::default(),
            cabinet: String::new(),
        };
        let controls = controls(params);
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            if key == ORDER_KEY {
                preset.order = ChainOrder::from_ids(value.split_whitespace());
            } else if key == CABINET_KEY {
                // A file is only ever named, never reached by a path
                if value.is_empty()
                    || user_cab::is_speaker_choice(value)
                    || user_cab::is_file_name(value)
                {
                    value.clone_into(&mut preset.cabinet);
                }
            } else if let Some(index) = controls.iter().position(|&(id, _)| id == key) {
                if let Some(plain) = controls[index].1.read(value) {
                    preset.values[index] = plain;
                }
            }
        }
        preset
    }

    /// The file's text
    pub fn to_text(&self, params: &GuitarAmpParams) -> String {
        let mut text = format!("{HEADER}\n");
        for (&(id, control), &plain) in controls(params).iter().zip(&self.values) {
            text.push_str(&format!("{id} = {}\n", control.write(plain)));
        }
        text.push_str(&format!("{ORDER_KEY} = {}\n", self.order.ids().join(" ")));
        text.push_str(&format!("{CABINET_KEY} = {}\n", self.cabinet));
        text
    }

    fn value(&self, params: &GuitarAmpParams, id: &str) -> f32 {
        let index = controls(params).iter().position(|&(each, _)| each == id).unwrap_or(0);
        self.values[index]
    }

    /// Set every parameter the preset holds, as a factory preset does (see `Preset::load`),
    /// and the cabinet. Returns whether the cabinet changed, which then has to be read
    pub fn load(&self, params: &GuitarAmpParams, setter: &ParamSetter) -> bool {
        let amp = Amp::from_index(self.value(params, "amp") as usize);
        let dials = ["gain", "bass", "mid", "treble", "presence", "master"]
            .map(|id| params_dial(params, id, self.value(params, id)));
        dial_memory::preset_loaded(params, amp, dials);
        params.order.put(self.order);
        let edits: Vec<Edit> = controls(params)
            .iter()
            .zip(&self.values)
            .map(|(&(_, control), &plain)| control.edit(plain))
            .collect();
        presets::set_together(setter, &edits);
        let mut cabinet = params.cabinet.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if *cabinet == self.cabinet {
            return false;
        }
        cabinet.clone_from(&self.cabinet);
        true
    }

    /// Whether everything the preset holds still stands where it puts it
    pub fn matches(&self, params: &GuitarAmpParams) -> bool {
        params.order.get() == self.order
            && *params.cabinet.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
                == self.cabinet
            && controls(params).iter().zip(&self.values).all(|(&(_, control), &plain)| {
                (control.unmodulated_normalized() - control.normalized(plain)).abs()
                    <= MATCH_TOLERANCE
            })
    }
}

/// A dial's value as `dial_memory` keeps it: what the parameter stores, kept on its range
fn params_dial(params: &GuitarAmpParams, id: &str, plain: f32) -> f32 {
    let dial = params.amp_dials()[["gain", "bass", "mid", "treble", "presence", "master"]
        .iter()
        .position(|&each| each == id)
        .unwrap_or(0)];
    dial.preview_plain(dial.preview_normalized(plain))
}

/// The presets in the folder as they were when it was last looked at, by file name
#[derive(Default)]
pub struct Bank {
    pub files: Vec<String>,
    presets: Vec<UserPreset>,
}

impl Bank {
    /// Reads every preset in `dir`. A file that cannot be read is left out
    pub fn read(params: &GuitarAmpParams, dir: Option<&Path>) -> Self {
        let mut bank = Self::default();
        let Some(dir) = dir else {
            return bank;
        };
        for file in user_cab::list_with_extension(dir, EXTENSION) {
            if let Ok(text) = std::fs::read_to_string(dir.join(&file)) {
                bank.presets.push(UserPreset::parse(params, &text));
                bank.files.push(file);
            }
        }
        bank
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn position(&self, file: &str) -> Option<usize> {
        self.files.iter().position(|each| each == file)
    }

    pub fn get(&self, index: usize) -> Option<(&str, &UserPreset)> {
        Some((self.files.get(index)?.as_str(), self.presets.get(index)?))
    }

    pub fn find(&self, file: &str) -> Option<&UserPreset> {
        self.position(file).and_then(|index| self.presets.get(index))
    }
}

/// The factory preset a step arrives at, round that bank
pub fn neighbour_factory(index: usize, forward: bool) -> usize {
    let count = PRESETS.len();
    (index.min(count - 1) + if forward { 1 } else { count - 1 }) % count
}

/// The preset a step arrives at in a bank of the player's, counting from `file`, the one
/// loaded from it: with none loaded, the first one going forward and the last going back.
/// Nothing when the bank holds no presets
pub fn neighbour_user(bank: &Bank, file: &str, forward: bool) -> Option<usize> {
    let count = bank.len();
    if count == 0 {
        return None;
    }
    Some(match bank.position(file) {
        Some(at) => (at + if forward { 1 } else { count - 1 }) % count,
        None if forward => 0,
        None => count - 1,
    })
}

/// A name as a file or a folder may be called: without the characters a name cannot hold,
/// and without the spaces at its ends
fn plain_name(text: &str) -> String {
    text.chars()
        .filter(|letter| !matches!(letter, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect::<String>()
        .trim()
        .to_owned()
}

/// A file name for a new preset made from `base`, the name of the preset it started as:
/// `base 2`, `base 3` and so on, the first that is not taken (letters in either case)
pub fn new_file_name(base: &str, taken: &[String]) -> String {
    let base = plain_name(base);
    let base = if base.is_empty() { "Preset".to_owned() } else { base };
    (2..)
        .map(|number| format!("{base} {number}.{EXTENSION}"))
        .find(|name| !taken.iter().any(|each| each.eq_ignore_ascii_case(name)))
        .unwrap_or_default()
}

/// A file name for a new preset the player named `name`: the name filtered like a file name
/// and `.preset` after it, with `2`, `3` and so on added while it is taken (letters in either
/// case). Nothing when nothing is left of the name, which is a preset nobody named
pub fn named_file_name(name: &str, taken: &[String]) -> Option<String> {
    let name = plain_name(name);
    if name.is_empty() {
        return None;
    }
    let free = |file: &String| !taken.iter().any(|each| each.eq_ignore_ascii_case(file));
    let plain = format!("{name}.{EXTENSION}");
    if free(&plain) {
        return Some(plain);
    }
    (2..).map(|number| format!("{name} {number}.{EXTENSION}")).find(free)
}

/// The file a save writes to. `named` is the name typed for a new preset: it names the file,
/// and a name nothing is left of is one nobody gave, which is named after `started_as`, the
/// preset the sound began as. With no name asked for, the preset loaded last (`user`) is
/// written over, or a new one is made after `started_as` when none is loaded
pub fn save_file_name(
    named: Option<&str>,
    user: &str,
    started_as: &str,
    taken: &[String],
) -> String {
    match named {
        Some(name) => {
            named_file_name(name, taken).unwrap_or_else(|| new_file_name(started_as, taken))
        }
        None if user.is_empty() => new_file_name(started_as, taken),
        None => user.to_owned(),
    }
}

/// The name Save puts on the tape: that of the preset `loaded` out of the bank showing, cut
/// to `max_chars`, or nothing when none is loaded out of it
pub fn name_to_keep(loaded: Option<&str>, max_chars: usize) -> String {
    loaded.map(|file| file_stem(file).chars().take(max_chars).collect()).unwrap_or_default()
}

/// Whether the `name` kept on the tape is that of the preset `loaded`, which is then written
/// over instead of a new preset being made
pub fn keeps_name(loaded: Option<&str>, name: &str, max_chars: usize) -> bool {
    loaded.is_some_and(|_| name_to_keep(loaded, max_chars).eq_ignore_ascii_case(name.trim()))
}

fn file_stem(file: &str) -> &str {
    file.strip_suffix(&format!(".{EXTENSION}")).unwrap_or(file)
}

/// Writes the sound as it stands to `file` in `dir`, making the folder if it is not there
pub fn save(params: &GuitarAmpParams, dir: &Path, file: &str) -> std::io::Result<()> {
    if !user_cab::is_file_name(file) {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(file), UserPreset::capture(params).to_text(params))
}

/// What the head's display shows for the player's preset `file`, with `max_chars` at most
pub fn display_name(file: &str, max_chars: usize) -> String {
    user_cab::display_name_without(file, EXTENSION, max_chars)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::user_cab::tests::test_dir;
    use std::sync::Mutex;

    // The parameters a preset leaves alone: the player's gain staging, and the switches
    // that are not part of the sound
    const LEFT_ALONE: [&str; 4] = ["bypass", "in_gain", "out_level", "tuner"];

    /// Stands in for the host: writes down what it is asked to set, and sets nothing
    #[derive(Default)]
    pub(crate) struct Host {
        set: Mutex<Vec<(ParamPtr, f32)>>,
        /// Everything it was told, in turn: `b`egin, `s`et or `e`nd, and of which parameter
        pub(crate) told: Mutex<Vec<(char, ParamPtr)>>,
    }

    /// Whether the host was told of the changes as one edit: every gesture begun before the
    /// first value, every value set inside them, every gesture ended after the last
    pub(crate) fn as_one_edit(told: &[(char, ParamPtr)]) -> bool {
        let first_set = told.iter().position(|&(kind, _)| kind == 's').unwrap_or(0);
        let last_set = told.iter().rposition(|&(kind, _)| kind == 's').unwrap_or(0);
        let begun = told.iter().filter(|&&(kind, _)| kind == 'b').count();
        let ended = told.iter().filter(|&&(kind, _)| kind == 'e').count();
        begun > 1
            && begun == ended
            && told[..first_set].iter().all(|&(kind, _)| kind == 'b')
            && told[last_set + 1..].iter().all(|&(kind, _)| kind == 'e')
            && told[first_set..=last_set].iter().all(|&(kind, _)| kind == 's')
    }

    impl GuiContext for Host {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Standalone
        }
        unsafe fn raw_begin_set_parameter(&self, param: ParamPtr) {
            self.told.lock().unwrap().push(('b', param));
        }
        unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, normalized: f32) {
            self.set.lock().unwrap().push((param, normalized));
            self.told.lock().unwrap().push(('s', param));
        }
        unsafe fn raw_end_set_parameter(&self, param: ParamPtr) {
            self.told.lock().unwrap().push(('e', param));
        }
        fn get_state(&self) -> PluginState {
            unimplemented!()
        }
        fn set_state(&self, _state: PluginState) {}
        fn request_resize(&self) -> bool {
            false
        }
    }

    fn ptr(control: Control) -> ParamPtr {
        match control {
            Control::Knob(param) => param.as_ptr(),
            Control::Switch(param) => param.as_ptr(),
            Control::Amp(param) => param.as_ptr(),
            Control::Mic(param) => param.as_ptr(),
        }
    }

    // A sound away from the defaults, as a player might have saved it
    const SAVED: &str = "# Hojt Guitar Amp preset\n\
                         amp = torden\n\
                         gain = 0.73\n\
                         delay_on = on\n\
                         delay_time = 512\n\
                         cab_mic_type = ribbon\n\
                         order = chorus comp drive phaser amp flanger tremolo delay reverb\n\
                         cabinet = speaker:test\n";

    #[test]
    fn test_every_parameter_is_held_or_left_alone() {
        let params = GuitarAmpParams::default();
        let held = controls(&params);
        for (id, param, _) in params.param_map() {
            match held.iter().find(|&&(each, _)| each == id) {
                Some(&(_, control)) => {
                    assert!(ptr(control) == param, "{id} is written for another parameter");
                }
                None => assert!(LEFT_ALONE.contains(&id.as_str()), "{id} is in no preset"),
            }
        }
        assert_eq!(held.len() + LEFT_ALONE.len(), params.param_map().len());
    }

    #[test]
    fn test_a_saved_sound_reads_back_as_it_was_written() {
        let params = GuitarAmpParams::default();
        let saved = UserPreset::parse(&params, SAVED);
        assert_eq!(saved.value(&params, "amp"), Amp::Torden.to_index() as f32);
        assert_eq!(saved.value(&params, "gain"), 0.73);
        assert_eq!(saved.value(&params, "delay_on"), 1.0);
        assert_eq!(saved.value(&params, "cab_mic_type"), MicType::Ribbon.to_index() as f32);
        // Saved before there was a fuzz: it stands where the default has it, first
        assert_eq!(saved.order.ids()[..2], ["fuzz", "chorus"]);
        assert_eq!(saved.cabinet, "speaker:test");

        let text = saved.to_text(&params);
        for line in ["amp = torden", "gain = 0.73", "delay_on = on", "cab_mic_type = ribbon"] {
            assert!(text.lines().any(|each| each == line), "{line} is not in\n{text}");
        }
        // Every parameter is written out, also those at their defaults
        assert_eq!(text.lines().filter(|line| line.contains(" = ")).count(), 45);
        assert_eq!(UserPreset::parse(&params, &text), saved);
    }

    #[test]
    fn test_loading_sets_every_parameter_it_holds_and_the_cabinet() {
        let params = GuitarAmpParams::default();
        let saved = UserPreset::parse(&params, SAVED);
        assert!(!saved.matches(&params));
        let host = Host::default();
        assert!(saved.load(&params, &ParamSetter::new(&host)));

        let set = host.set.lock().unwrap();
        let held = controls(&params);
        assert_eq!(set.len(), held.len());
        for ((param, normalized), (&(id, control), &plain)) in
            set.iter().zip(held.iter().zip(&saved.values))
        {
            assert!(*param == ptr(control), "{id} set out of turn");
            assert_eq!(*normalized, control.normalized(plain), "{id}");
        }
        assert_eq!(params.order.get(), saved.order);
        assert_eq!(*params.cabinet.lock().unwrap(), "speaker:test");
        // All of it as one edit, so a host that groups gestures undoes it in one step
        assert!(as_one_edit(&host.told.lock().unwrap()));
        // The amp it picks remembers its dials
        assert_eq!(params.dial_memory.enter(Amp::Torden)[0], 0.73);
        // Loading it again has no cabinet to read
        assert!(!saved.load(&params, &ParamSetter::new(&Host::default())));
    }

    #[test]
    fn test_input_output_bypass_and_tuner_are_never_touched() {
        let params = GuitarAmpParams::default();
        let host = Host::default();
        let text = "in_gain = 7\nout_level = -9\nbypass = on\ntuner = on\n";
        UserPreset::parse(&params, text).load(&params, &ParamSetter::new(&host));
        let left_alone = [
            params.in_gain.as_ptr(),
            params.out_level.as_ptr(),
            params.bypass.as_ptr(),
            params.tuner_on.as_ptr(),
        ];
        assert!(host.set.lock().unwrap().iter().all(|(param, _)| !left_alone.contains(param)));
        assert!(!UserPreset::capture(&params).to_text(&params).contains("in_gain"));
    }

    #[test]
    fn test_the_sound_as_it_stands_matches_until_something_moves() {
        let params = GuitarAmpParams::default();
        let now = UserPreset::capture(&params);
        assert_eq!(now, UserPreset::parse(&params, ""));
        assert!(now.matches(&params));
        params.order.put(UserPreset::parse(&params, SAVED).order);
        assert!(!now.matches(&params));
        params.order.put(ChainOrder::default());
        *params.cabinet.lock().unwrap() = "speaker:test".to_owned();
        assert!(!now.matches(&params));
    }

    #[test]
    fn test_a_file_is_read_forgivingly() {
        let params = GuitarAmpParams::default();
        let text = "# by hand\n\
                    gain = 0.25\n\
                    amp = nonsense\n\
                    reverb_on = maybe\n\
                    delay_mix = NaN\n\
                    unknown_knob = 3\n\
                    no equals sign\n\
                    cabinet = ../../secret.wav\n\
                    bass=0.6\n";
        let read = UserPreset::parse(&params, text);
        let defaults = UserPreset::parse(&params, "");
        assert_eq!(read.value(&params, "gain"), 0.25);
        assert_eq!(read.value(&params, "bass"), 0.6);
        for id in ["amp", "reverb_on", "delay_mix"] {
            assert_eq!(read.value(&params, id), defaults.value(&params, id), "{id}");
        }
        assert_eq!(read.cabinet, "");
        assert_eq!(read.order, ChainOrder::default());
        // An empty file is every default, which is the first factory preset
        assert!(PRESETS[0].matches(&params));
        assert!(defaults.matches(&params));
    }

    #[test]
    fn test_new_names_count_on_from_two_and_skip_what_is_taken() {
        let taken = vec!["Grus 2.preset".to_owned(), "grus 3.PRESET".to_owned()];
        assert_eq!(new_file_name("Grus", &taken), "Grus 4.preset");
        assert_eq!(new_file_name("Tåge", &[]), "Tåge 2.preset");
        assert_eq!(new_file_name("a/b", &[]), "ab 2.preset");
        assert_eq!(new_file_name("", &[]), "Preset 2.preset");
    }

    #[test]
    fn test_stepping_comes_round_within_the_bank_showing() {
        let dir = test_dir("user-presets-round");
        let params = GuitarAmpParams::default();
        for file in ["Mine.preset", "Other.preset"] {
            save(&params, &dir, file).unwrap();
        }
        let bank = Bank::read(&params, Some(&dir));
        // The factory bank comes round in itself and never leaves it
        assert_eq!(neighbour_factory(3, true), 4);
        assert_eq!(neighbour_factory(PRESETS.len() - 1, true), 0);
        assert_eq!(neighbour_factory(0, false), PRESETS.len() - 1);
        // An index out of a project of another build is brought back in
        assert_eq!(neighbour_factory(PRESETS.len() + 5, true), 0);
        // A bank of the player's comes round in its own files
        assert_eq!(neighbour_user(&bank, "Mine.preset", true), Some(1));
        assert_eq!(neighbour_user(&bank, "Other.preset", true), Some(0));
        assert_eq!(neighbour_user(&bank, "Mine.preset", false), Some(1));
        // With nothing loaded from the bank a step lands at the end it comes from
        assert_eq!(neighbour_user(&bank, "", true), Some(0));
        assert_eq!(neighbour_user(&bank, "Gone.preset", false), Some(1));
        // A bank with nothing in it has nowhere to step
        assert_eq!(neighbour_user(&Bank::default(), "", true), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_the_banks_are_the_factory_presets_the_folder_and_the_folders_in_it() {
        let dir = test_dir("user-presets-banks");
        let params = GuitarAmpParams::default();
        save(&params, &dir, "Loose.preset").unwrap();
        for folder in ["jazz", "Metal"] {
            std::fs::create_dir_all(dir.join(folder)).unwrap();
        }
        // What is deeper than one folder is no bank
        std::fs::create_dir_all(dir.join("Metal").join("Deeper")).unwrap();

        assert_eq!(banks(Some(&dir)), [FACTORY, MINE, "/jazz", "/Metal"]);
        // Without a presets folder there are still the two banks that are no folder
        assert_eq!(banks(None), [FACTORY, MINE]);
        assert_eq!(bank_name(FACTORY, 10), "FACTORY");
        assert_eq!(bank_name(MINE, 10), "MINE");
        assert_eq!(bank_name("/jazz", 10), "JAZZ");
        assert_eq!(bank_name("/Tåge og Regn", 8), "TÅG~REGN");

        let banks = banks(Some(&dir));
        assert_eq!(neighbour_bank(&banks, FACTORY, true), MINE);
        assert_eq!(neighbour_bank(&banks, "/Metal", true), FACTORY);
        assert_eq!(neighbour_bank(&banks, FACTORY, false), "/Metal");
        // A bank that is no longer there counts as the factory one
        assert_eq!(neighbour_bank(&banks, "/Gone", true), MINE);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_a_project_from_before_there_were_banks_opens_on_the_presets_folder() {
        // A preset of the player's with no bank is one of the presets kept loose in the
        // folder, which is what every build before banks saved
        assert_eq!(bank_shown("", "Smooth.preset"), MINE);
        assert_eq!(bank_shown("", ""), FACTORY);
        assert_eq!(bank_shown(MINE, "Smooth.preset"), MINE);
        assert_eq!(bank_shown("/Jazz", "Smooth.preset"), "/Jazz");
        assert_eq!(bank_shown("/Jazz", ""), "/Jazz");
        // One out of a project of another build, or a damaged one, is the factory bank
        assert_eq!(bank_shown("/../secret", "Smooth.preset"), FACTORY);
        assert_eq!(bank_shown("Jazz", ""), FACTORY);

        let params = GuitarAmpParams::default();
        assert_eq!(bank_of(&params), FACTORY);
        *params.user_preset.lock().unwrap() = "Smooth.preset".to_owned();
        assert_eq!(bank_of(&params), MINE);
        *params.preset_bank.lock().unwrap() = "/Jazz".to_owned();
        assert_eq!(bank_of(&params), "/Jazz");
    }

    #[test]
    fn test_a_bank_is_never_a_path_out_of_the_presets_folder() {
        for bank in [FACTORY, MINE, "/Jazz", "/Tåge"] {
            assert!(is_bank(bank), "{bank}");
        }
        for bank in ["/..", "/../secret", "/a/b", "/a\\b", "Jazz", "/.", "//"] {
            assert!(!is_bank(bank), "{bank}");
        }
        // An id no bank may have has no folder, and is read as the factory bank
        assert!(bank_dir("/../secret").is_none());
        assert!(bank_dir(FACTORY).is_none());
        assert_eq!(bank_name("nonsense", 10), "FACTORY");
    }

    #[test]
    fn test_a_named_preset_is_the_name_and_a_number_is_added_when_it_is_taken() {
        let taken = vec!["Clean.preset".to_owned(), "clean 2.preset".to_owned()];
        assert_eq!(named_file_name("Lead", &taken).as_deref(), Some("Lead.preset"));
        assert_eq!(named_file_name(" Clean ", &taken).as_deref(), Some("Clean 3.preset"));
        // Characters a file name cannot hold are left out, and nothing left is no name
        assert_eq!(named_file_name("A/B:C?", &[]).as_deref(), Some("ABC.preset"));
        assert_eq!(named_file_name("  ", &taken), None);
        assert_eq!(named_file_name("*?", &taken), None);
    }

    #[test]
    fn test_a_save_writes_over_the_preset_loaded_or_makes_one_named_or_not() {
        let taken = vec!["Lead.preset".to_owned()];
        assert_eq!(save_file_name(None, "Lead.preset", "Clean", &taken), "Lead.preset");
        assert_eq!(save_file_name(None, "", "Clean", &taken), "Clean 2.preset");
        assert_eq!(save_file_name(Some("Crunch"), "", "Clean", &taken), "Crunch.preset");
        assert_eq!(save_file_name(Some("Lead"), "", "Clean", &taken), "Lead 2.preset");
        // No name typed is today's behaviour
        assert_eq!(save_file_name(Some(""), "", "Clean", &taken), "Clean 2.preset");
    }

    #[test]
    fn test_save_starts_from_the_name_of_the_preset_loaded_and_keeping_it_writes_over_it() {
        assert_eq!(name_to_keep(Some("Crunch.preset"), 24), "Crunch");
        assert_eq!(name_to_keep(Some("A very long name of a preset.preset"), 10), "A very lon");
        assert_eq!(name_to_keep(None, 24), "");
        assert!(keeps_name(Some("Crunch.preset"), "Crunch", 24));
        assert!(keeps_name(Some("Crunch.preset"), "crunch ", 24));
        assert!(!keeps_name(Some("Crunch.preset"), "Crunch 2", 24));
        assert!(!keeps_name(Some("Crunch.preset"), "", 24));
        assert!(!keeps_name(None, "", 24));
    }

    #[test]
    fn test_a_new_bank_is_named_like_a_file_and_one_that_is_there_is_simply_picked() {
        let dir = test_dir("user-presets-new-bank");
        assert_eq!(make_bank(&dir, " Jazz ").unwrap(), "/Jazz");
        assert!(dir.join("Jazz").is_dir());
        // The characters a name cannot hold are left out, so it stays one plain name
        assert_eq!(make_bank(&dir, "Jazz/Trio").unwrap(), "/JazzTrio");
        // The same name again is that bank, and nothing in it is lost
        save(&GuitarAmpParams::default(), &dir.join("Jazz"), "Smooth.preset").unwrap();
        assert_eq!(make_bank(&dir, "Jazz").unwrap(), "/Jazz");
        assert!(dir.join("Jazz").join("Smooth.preset").is_file());
        // A name nothing is left of makes a bank nobody named, to rename in the folder
        for (name, made) in [("", "/Bank 1"), ("   ", "/Bank 2"), ("/", "/Bank 3")] {
            assert_eq!(make_bank(&dir, name).unwrap(), made, "{name}");
        }
        assert_eq!(new_bank_name(&dir), "Bank 4");
        // A name that is no name at all is refused
        for name in ["..", "."] {
            assert!(make_bank(&dir, name).is_err(), "{name}");
        }
        // The factory bank, the folder itself, and the five folders made here
        assert_eq!(banks(Some(&dir)).len(), 2 + 5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_the_bank_reads_the_folder_in_name_order() {
        let dir = test_dir("user-presets-bank");
        let params = GuitarAmpParams::default();
        save(&params, &dir, "b.preset").unwrap();
        std::fs::write(dir.join("A.preset"), SAVED).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a preset").unwrap();
        assert!(save(&params, &dir, "../out.preset").is_err());

        let bank = Bank::read(&params, Some(&dir));
        assert_eq!(bank.files, ["A.preset", "b.preset"]);
        assert_eq!(bank.find("b.preset"), Some(&UserPreset::capture(&params)));
        assert_eq!(bank.find("A.preset"), Some(&UserPreset::parse(&params, SAVED)));
        assert_eq!(bank.get(1).map(|(file, _)| file), Some("b.preset"));
        assert_eq!(display_name("Tåge 2.preset", 13), "TÅGE 2");
        assert_eq!(Bank::read(&params, None).len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
