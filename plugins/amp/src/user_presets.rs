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
//! pedals, the whole chain and the cabinet choice. One line each, `id = value`, by the frozen
//! parameter ids:
//!
//! ```text
//! amp = torden
//! gain = 0.55
//! gate_on = on
//! order = pitch fuzz comp drive phaser amp chorus flanger tremolo eq delay reverb
//! chain = amps=2 pitch@1 fuzz@3 amp chorus@7 delay@12 delay@13 reverb@13
//! slot13 = 1:0.5,0.3,0.4,0,0
//! cabinet = speaker:4x12-mid
//! cabinet2 = speaker:1x12-open
//! ```
//!
//! The `chain` line is the chain as a project saves it (`Chain::ids`: the amp lanes, each block
//! as `kind@slot` and `amp` between the two zones), and loading a preset replaces the whole
//! chain with it (#271). A block that stands on its own pedal's slot keeps its values in that
//! pedal's parameters, which the lines above hold; any other block (a second delay, a pedal on
//! another slot) has its values in a `slot<n> = <on>:<k1>,..,<k5>` line, in the form a project
//! saves them as. The `order` line is the same chain seen as the twelve pedals, for a build
//! from before there was a chain; a file with no `chain` line is that, and loads as the twelve
//! pedals in that order.
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
use crate::presets::{self, Edit, MATCH_TOLERANCE};
use crate::slots::{Block, Chain, Live, SlotValues, MAX_SLOTS};
use crate::user_cab;
use crate::wah::WahMode;
use crate::{Amp, GuitarAmpParams};
use suite_common::dsp::NoteValue;

/// Set this to a folder to have the presets kept there instead of in Documents
pub const FOLDER_ENV: &str = "HOJT_PRESETS_DIR";
// Below the Documents folder
const FOLDER: [&str; 2] = [suite_common::VENDOR, "Presets"];
pub const EXTENSION: &str = "preset";

// The keys of the lines that are not parameters
const ORDER_KEY: &str = "order";
const CHAIN_KEY: &str = "chain";
// Then the number of the slot, 1 to 16
const SLOT_KEY: &str = "slot";
const CABINET_KEY: &str = "cabinet";
const SECOND_CABINET_KEY: &str = "cabinet2";
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
    /// A whole number, such as the pitch shifter's semitones
    Whole(&'a IntParam),
    Switch(&'a BoolParam),
    Amp(&'a EnumParam<Amp>),
    Mic(&'a EnumParam<MicType>),
    Note(&'a EnumParam<NoteValue>),
    Wah(&'a EnumParam<WahMode>),
}

impl Control<'_> {
    /// The value as its parameter stores it, a switch as 0 or 1 and a choice as its index
    fn plain(self) -> f32 {
        match self {
            Control::Knob(param) => param.unmodulated_plain_value(),
            Control::Whole(param) => param.unmodulated_plain_value() as f32,
            Control::Switch(param) => f32::from(u8::from(param.unmodulated_plain_value())),
            Control::Amp(param) => param.unmodulated_plain_value().to_index() as f32,
            Control::Mic(param) => param.unmodulated_plain_value().to_index() as f32,
            Control::Note(param) => param.unmodulated_plain_value().to_index() as f32,
            Control::Wah(param) => param.unmodulated_plain_value().to_index() as f32,
        }
    }

    fn default_plain(self) -> f32 {
        match self {
            Control::Knob(param) => param.default_plain_value(),
            Control::Whole(param) => param.default_plain_value() as f32,
            Control::Switch(param) => f32::from(u8::from(param.default_plain_value())),
            Control::Amp(param) => param.default_plain_value().to_index() as f32,
            Control::Mic(param) => param.default_plain_value().to_index() as f32,
            Control::Note(param) => param.default_plain_value().to_index() as f32,
            Control::Wah(param) => param.default_plain_value().to_index() as f32,
        }
    }

    fn normalized(self, plain: f32) -> f32 {
        match self {
            Control::Knob(param) => param.preview_normalized(plain),
            Control::Whole(param) => param.preview_normalized(plain.round() as i32),
            Control::Switch(param) => param.preview_normalized(plain >= 0.5),
            Control::Amp(param) => param.preview_normalized(Amp::from_index(plain as usize)),
            Control::Mic(param) => param.preview_normalized(MicType::from_index(plain as usize)),
            Control::Note(param) => param.preview_normalized(NoteValue::from_index(plain as usize)),
            Control::Wah(param) => param.preview_normalized(WahMode::from_index(plain as usize)),
        }
    }

    fn unmodulated_normalized(self) -> f32 {
        match self {
            Control::Knob(param) => param.unmodulated_normalized_value(),
            Control::Whole(param) => param.unmodulated_normalized_value(),
            Control::Switch(param) => param.unmodulated_normalized_value(),
            Control::Amp(param) => param.unmodulated_normalized_value(),
            Control::Mic(param) => param.unmodulated_normalized_value(),
            Control::Note(param) => param.unmodulated_normalized_value(),
            Control::Wah(param) => param.unmodulated_normalized_value(),
        }
    }

    fn edit(self, plain: f32) -> Edit {
        match self {
            Control::Knob(param) => Edit::of(param, plain),
            Control::Whole(param) => Edit::of(param, plain.round() as i32),
            Control::Switch(param) => Edit::of(param, plain >= 0.5),
            Control::Amp(param) => Edit::of(param, Amp::from_index(plain as usize)),
            Control::Mic(param) => Edit::of(param, MicType::from_index(plain as usize)),
            Control::Note(param) => Edit::of(param, NoteValue::from_index(plain as usize)),
            Control::Wah(param) => Edit::of(param, WahMode::from_index(plain as usize)),
        }
    }

    /// The value as a file has it
    fn write(self, plain: f32) -> String {
        match self {
            Control::Knob(_) => format!("{plain}"),
            Control::Whole(_) => format!("{}", plain.round() as i32),
            Control::Switch(_) => if plain >= 0.5 { "on" } else { "off" }.to_owned(),
            Control::Amp(_) => enum_id::<Amp>(plain as usize).to_owned(),
            Control::Mic(_) => enum_id::<MicType>(plain as usize).to_owned(),
            Control::Note(_) => enum_id::<NoteValue>(plain as usize).to_owned(),
            Control::Wah(_) => enum_id::<WahMode>(plain as usize).to_owned(),
        }
    }

    /// The value a file has written, if it is one this parameter can take
    fn read(self, text: &str) -> Option<f32> {
        match self {
            Control::Knob(_) => text.parse::<f32>().ok().filter(|value| value.is_finite()),
            Control::Whole(_) => text.parse::<i32>().ok().map(|value| value as f32),
            Control::Switch(_) => match text {
                "on" => Some(1.0),
                "off" => Some(0.0),
                _ => None,
            },
            Control::Amp(_) => enum_index::<Amp>(text).map(|index| index as f32),
            Control::Mic(_) => enum_index::<MicType>(text).map(|index| index as f32),
            Control::Note(_) => enum_index::<NoteValue>(text).map(|index| index as f32),
            Control::Wah(_) => enum_index::<WahMode>(text).map(|index| index as f32),
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
fn controls(params: &GuitarAmpParams) -> [(&'static str, Control<'_>); 68] {
    use Control::{Knob, Mic, Note, Switch, Wah, Whole};
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
        ("delay_sync", Switch(&params.delay_sync)),
        ("delay_note", Note(&params.delay_note)),
        ("reverb_on", Switch(&params.reverb_on)),
        ("reverb_decay", Knob(&params.reverb_decay)),
        ("reverb_mix", Knob(&params.reverb_mix)),
        // The second amp: a file saved before there were two names none of these, and loads
        // with it off
        ("amp2_on", Switch(&params.amp2_on)),
        ("amp2", Control::Amp(&params.amp2)),
        ("amp2_gain", Knob(&params.amp2_gain)),
        ("amp2_bass", Knob(&params.amp2_bass)),
        ("amp2_mid", Knob(&params.amp2_mid)),
        ("amp2_treble", Knob(&params.amp2_treble)),
        ("amp2_presence", Knob(&params.amp2_presence)),
        ("amp2_master", Knob(&params.amp2_master)),
        ("amp_blend", Knob(&params.amp_blend)),
        // The pitch shifter: a file saved before it names none of these, and loads with it off
        ("pitch_on", Switch(&params.pitch_on)),
        ("pitch_semitones", Whole(&params.pitch_semitones)),
        ("pitch_blend", Knob(&params.pitch_blend)),
        ("pitch_latency", Knob(&params.pitch_latency)),
        // The EQ: likewise
        ("eq_on", Switch(&params.eq_on)),
        ("eq_low", Knob(&params.eq_low)),
        ("eq_mid", Knob(&params.eq_mid)),
        ("eq_freq", Knob(&params.eq_freq)),
        ("eq_high", Knob(&params.eq_high)),
        // The wah: likewise
        ("wah_on", Switch(&params.wah_on)),
        ("wah_pedal", Knob(&params.wah_pedal)),
        ("wah_q", Knob(&params.wah_q)),
        ("wah_mode", Wah(&params.wah_mode)),
        ("wah_sens", Knob(&params.wah_sens)),
    ]
}

/// A sound the player saved
#[derive(Clone, Debug, PartialEq)]
pub struct UserPreset {
    /// In the order of `controls`, as the parameters store them
    values: Vec<f32>,
    /// The whole line (#271): a file from before the chain is the twelve pedals in its `order`
    chain: Chain,
    /// The values of every block of `chain` that is not on its own pedal's slot, by slot
    /// (counted from 0)
    slots: Vec<(u8, SlotValues)>,
    /// What `GuitarAmpParams::cabinet` holds: empty for the amp's own
    cabinet: String,
    /// What `GuitarAmpParams::cabinet_second` holds: empty for the second amp's own
    cabinet_second: String,
}

impl UserPreset {
    /// The sound as it stands now
    pub fn capture(params: &GuitarAmpParams) -> Self {
        let chain = params.chain.get();
        Self {
            values: controls(params).iter().map(|&(_, control)| control.plain()).collect(),
            chain,
            slots: chain
                .blocks()
                .iter()
                .filter(|&&block| !Chain::is_home(block))
                .map(|&block| (block.slot, params.values_of(block)))
                .collect(),
            cabinet: params
                .cabinet
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            cabinet_second: params.cabinet_second.choice(),
        }
    }

    /// What a file says, with the default for whatever it does not
    pub fn parse(params: &GuitarAmpParams, text: &str) -> Self {
        let mut preset = Self {
            values: controls(params).iter().map(|&(_, control)| control.default_plain()).collect(),
            chain: Chain::default(),
            slots: Vec::new(),
            cabinet: String::new(),
            cabinet_second: String::new(),
        };
        let controls = controls(params);
        let mut order = ChainOrder::default();
        let mut chain = None;
        let mut slot_lines: [Option<SlotValues>; MAX_SLOTS] = [None; MAX_SLOTS];
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
                order = ChainOrder::from_ids(value.split_whitespace());
            } else if key == CHAIN_KEY {
                chain = Some(Chain::from_ids(value.split_whitespace()));
            } else if let Some(number) = key.strip_prefix(SLOT_KEY) {
                let slot = number.parse::<usize>().ok().filter(|n| (1..=MAX_SLOTS).contains(n));
                if let (Some(slot), Some(values)) = (slot, SlotValues::from_text(value)) {
                    slot_lines[slot - 1] = Some(values);
                }
            } else if key == CABINET_KEY {
                // A file is only ever named, never reached by a path
                if value.is_empty()
                    || user_cab::is_speaker_choice(value)
                    || user_cab::is_file_name(value)
                {
                    value.clone_into(&mut preset.cabinet);
                }
            } else if key == SECOND_CABINET_KEY {
                // Only a built-in cabinet: the second amp never plays a file
                // that this build has: an id it does not know is the amp's own, as it is
                // when loaded, so the preset does not look edited once it is
                let known = value
                    .strip_prefix("speaker:")
                    .is_some_and(|id| crate::amp::model::speaker_index(id).is_some());
                if value.is_empty() || known {
                    value.clone_into(&mut preset.cabinet_second);
                } else if user_cab::is_speaker_choice(value) {
                    preset.cabinet_second.clear();
                }
            } else if let Some(index) = controls.iter().position(|&(id, _)| id == key) {
                if let Some(plain) = controls[index].1.read(value) {
                    preset.values[index] = plain;
                }
            }
        }
        // A file without a `chain` is from before there was one: the twelve pedals in its order
        preset.chain = chain.unwrap_or_else(|| Chain::from_order(&order));
        // What a block that is not on its own pedal's slot has no line for starts as a slot does
        preset.slots = preset
            .chain
            .blocks()
            .iter()
            .filter(|&&block| !Chain::is_home(block))
            .map(|&block| {
                let values = slot_lines[block.slot as usize].unwrap_or(SlotValues::DEFAULT);
                (block.slot, tidy(block, values))
            })
            .collect();
        preset
    }

    /// The file's text
    pub fn to_text(&self, params: &GuitarAmpParams) -> String {
        let mut text = format!("{HEADER}\n");
        for (&(id, control), &plain) in controls(params).iter().zip(&self.values) {
            text.push_str(&format!("{id} = {}\n", control.write(plain)));
        }
        text.push_str(&format!("{ORDER_KEY} = {}\n", self.chain.order().ids().join(" ")));
        text.push_str(&format!("{CHAIN_KEY} = {}\n", self.chain.ids().join(" ")));
        for (slot, values) in &self.slots {
            text.push_str(&format!("{SLOT_KEY}{} = {}\n", slot + 1, values.to_text()));
        }
        text.push_str(&format!("{CABINET_KEY} = {}\n", self.cabinet));
        text.push_str(&format!("{SECOND_CABINET_KEY} = {}\n", self.cabinet_second));
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
        let amp2 = Amp::from_index(self.value(params, "amp2") as usize);
        let dials2 = SECOND_DIALS.map(|id| second_dial(params, id, self.value(params, id)));
        dial_memory::second_preset_loaded(params, amp2, dials2);
        // The whole chain is the preset's; what it takes out is kept by kind, read before any
        // value is set
        params.chain.replace(&Live { params, setter }, self.chain);
        let mut edits: Vec<Edit> = controls(params)
            .iter()
            .zip(&self.values)
            .map(|(&(_, control), &plain)| control.edit(plain))
            .collect();
        for &(slot, values) in &self.slots {
            if let Some(block) = self.chain.block_on(slot) {
                edits.extend(params.edits_of(block, values));
            }
        }
        presets::set_together(setter, &edits);
        params.cabinet_second.set_choice(&self.cabinet_second);
        let mut cabinet = params.cabinet.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if *cabinet == self.cabinet {
            return false;
        }
        cabinet.clone_from(&self.cabinet);
        true
    }

    // Whether a block that has no pedal's parameters stands as the preset has it: the switch and the
    // knobs its kind uses
    fn slot_stands(&self, params: &GuitarAmpParams, block: Block, values: SlotValues) -> bool {
        let now = params.values_of(block);
        now.on == values.on
            && now
                .knobs
                .iter()
                .zip(values.knobs)
                .take(block.kind.knob_count())
                .all(|(now, wanted)| (now - wanted).abs() <= MATCH_TOLERANCE)
    }

    /// Whether everything the preset holds still stands where it puts it
    pub fn matches(&self, params: &GuitarAmpParams) -> bool {
        params.chain.get() == self.chain
            && self.slots.iter().all(|&(slot, values)| {
                self.chain
                    .block_on(slot)
                    .is_some_and(|block| self.slot_stands(params, block, values))
            })
            && params.cabinet_second.choice() == self.cabinet_second
            && *params.cabinet.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
                == self.cabinet
            && controls(params).iter().zip(&self.values).all(|(&(_, control), &plain)| {
                (control.unmodulated_normalized() - control.normalized(plain)).abs()
                    <= MATCH_TOLERANCE
            })
    }
}

// What a block holds, with the knobs its kind does not use at 0, where the parameters read them
// (`GuitarAmpParams::values_of`): so a sound that is saved and read is the one that was captured
fn tidy(block: Block, mut values: SlotValues) -> SlotValues {
    for knob in values.knobs.iter_mut().skip(block.kind.knob_count()) {
        *knob = 0.0;
    }
    values
}

/// A dial's value as `dial_memory` keeps it: what the parameter stores, kept on its range
fn params_dial(params: &GuitarAmpParams, id: &str, plain: f32) -> f32 {
    let dial = params.amp_dials()[["gain", "bass", "mid", "treble", "presence", "master"]
        .iter()
        .position(|&each| each == id)
        .unwrap_or(0)];
    dial.preview_plain(dial.preview_normalized(plain))
}

// The ids of the second amp's six dials, in the order they stand on the head
const SECOND_DIALS: [&str; 6] =
    ["amp2_gain", "amp2_bass", "amp2_mid", "amp2_treble", "amp2_presence", "amp2_master"];

/// The same for a dial of the second amp
fn second_dial(params: &GuitarAmpParams, id: &str, plain: f32) -> f32 {
    let dial = params.amp2_dials()[SECOND_DIALS.iter().position(|&each| each == id).unwrap_or(0)];
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
    use crate::order::Zone;
    use crate::presets::PRESETS;
    use crate::user_cab::tests::test_dir;
    use std::sync::Mutex;

    // The parameters a preset leaves alone: the player's gain staging, and the switches
    // that are not part of the sound
    const LEFT_ALONE: [&str; 12] = [
        "bypass",
        "in_gain",
        "out_level",
        "tuner",
        "metronome_on",
        "metronome_bpm",
        "metronome_sync",
        "metronome_time_sig",
        "metronome_volume",
        "metronome_sound",
        "metronome_out",
        "looper_level",
    ];

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
            Control::Whole(param) => param.as_ptr(),
            Control::Switch(param) => param.as_ptr(),
            Control::Amp(param) => param.as_ptr(),
            Control::Mic(param) => param.as_ptr(),
            Control::Note(param) => param.as_ptr(),
            Control::Wah(param) => param.as_ptr(),
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
                // The slots are reserved until the audio reads them (#268), and a preset holds
                // the whole chain from #271 on
                None => assert!(
                    LEFT_ALONE.contains(&id.as_str()) || id.starts_with("slot"),
                    "{id} is in no preset"
                ),
            }
        }
        let slots = crate::slots::MAX_SLOTS * (1 + crate::slots::KNOBS);
        assert_eq!(held.len() + LEFT_ALONE.len() + slots, params.param_map().len());
    }

    #[test]
    fn test_a_saved_sound_reads_back_as_it_was_written() {
        let params = GuitarAmpParams::default();
        let saved = UserPreset::parse(&params, SAVED);
        assert_eq!(saved.value(&params, "amp"), Amp::Torden.to_index() as f32);
        assert_eq!(saved.value(&params, "gain"), 0.73);
        assert_eq!(saved.value(&params, "delay_on"), 1.0);
        assert_eq!(saved.value(&params, "cab_mic_type"), MicType::Ribbon.to_index() as f32);
        // Saved before there was a fuzz and a wah: they stand where the default has them
        assert_eq!(saved.chain.order().ids()[..3], ["pitch", "wah", "fuzz"]);
        assert_eq!(saved.cabinet, "speaker:test");

        let text = saved.to_text(&params);
        for line in ["amp = torden", "gain = 0.73", "delay_on = on", "cab_mic_type = ribbon"] {
            assert!(text.lines().any(|each| each == line), "{line} is not in\n{text}");
        }
        // Every parameter is written out, also those at their defaults, and the pedal order, the chain and
        // the two cabinets after them
        assert_eq!(text.lines().filter(|line| line.contains(" = ")).count(), 72);
        assert_eq!(UserPreset::parse(&params, &text), saved);
    }

    #[test]
    fn test_a_file_from_before_the_second_amp_loads_with_it_off() {
        // SAVED names none of the second amp's lines: they get their defaults
        let params = GuitarAmpParams::default();
        let saved = UserPreset::parse(&params, SAVED);
        assert_eq!(saved.value(&params, "amp2_on"), 0.0);
        assert_eq!(saved.value(&params, "amp2"), Amp::Klar.to_index() as f32);
        assert_eq!(saved.value(&params, "amp_blend"), 0.5);
        // Loading it switches the second amp off
        let host = Host::default();
        saved.load(&params, &ParamSetter::new(&host));
        let set = host.set.lock().unwrap();
        assert!(set
            .iter()
            .any(|(param, normalized)| *param == params.amp2_on.as_ptr() && *normalized == 0.0));
    }

    #[test]
    fn test_the_second_amp_is_written_and_read_back() {
        let params = GuitarAmpParams::default();
        let saved = UserPreset::parse(
            &params,
            "amp = torden\namp2_on = on\namp2 = sang\namp2_gain = 0.31\namp2_master = 0.62\n\
             amp_blend = 0.4\ncabinet2 = speaker:4x12-heavy\n",
        );
        assert_eq!(saved.value(&params, "amp2_on"), 1.0);
        assert_eq!(saved.value(&params, "amp2"), Amp::Sang.to_index() as f32);
        let text = saved.to_text(&params);
        for line in [
            "amp2_on = on",
            "amp2 = sang",
            "amp2_gain = 0.31",
            "amp_blend = 0.4",
            "cabinet2 = speaker:4x12-heavy",
        ] {
            assert!(text.lines().any(|each| each == line), "{line} is not in\n{text}");
        }
        assert_eq!(UserPreset::parse(&params, &text), saved);

        // Loading it sets the second amp in the same edit, and its dials are the second
        // amp's memory
        let host = Host::default();
        saved.load(&params, &ParamSetter::new(&host));
        assert!(as_one_edit(&host.told.lock().unwrap()));
        assert_eq!(params.dial_memory_second.enter(Amp::Sang)[0], 0.31);
        assert_eq!(params.dial_memory_second.enter(Amp::Sang)[5], 0.62);
        assert_eq!(params.dial_memory.enter(Amp::Sang)[0], 0.5);
        // and its cabinet: a built-in one, and never a file
        assert_eq!(params.cabinet_second.choice(), "speaker:4x12-heavy");
        let file = UserPreset::parse(&params, "cabinet2 = Alpha.wav\n");
        assert_eq!(file.cabinet_second, "");
        file.load(&params, &ParamSetter::new(&host));
        assert_eq!(params.cabinet_second.get(), None);
        // an id this build does not have is the amp's own, so it matches once loaded
        let unknown = UserPreset::parse(&params, "cabinet2 = speaker:9x99-nothing\n");
        assert_eq!(unknown.cabinet_second, "");
        unknown.load(&params, &ParamSetter::new(&host));
        assert!(unknown.matches(&params));
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
        assert_eq!(params.chain.order(), saved.chain.order());
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
        params.chain.put_order(UserPreset::parse(&params, SAVED).chain.order());
        assert!(!now.matches(&params));
        params.chain.put_order(ChainOrder::default());
        *params.cabinet.lock().unwrap() = "speaker:test".to_owned();
        assert!(!now.matches(&params));
    }

    #[test]
    fn test_a_preset_from_before_sync_opens_with_sync_off_and_a_new_one_keeps_it() {
        let params = GuitarAmpParams::default();
        // Saved before there was Sync: it names neither, and reads the delay off the knob
        let old = UserPreset::parse(&params, SAVED);
        assert_eq!(old.value(&params, "delay_sync"), 0.0);
        assert_eq!(old.value(&params, "delay_note"), NoteValue::DottedEighth.to_index() as f32);
        // Saved now: both are written, and read back
        let text = format!("{SAVED}delay_sync = on\ndelay_note = eighth_triplet\n");
        let synced = UserPreset::parse(&params, &text);
        assert_eq!(synced.value(&params, "delay_sync"), 1.0);
        assert_eq!(synced.value(&params, "delay_note"), NoteValue::EighthTriplet.to_index() as f32);
        let written = synced.to_text(&params);
        for line in ["delay_sync = on", "delay_note = eighth_triplet"] {
            assert!(written.lines().any(|each| each == line), "{line} is not in\n{written}");
        }
        assert_eq!(UserPreset::parse(&params, &written), synced);
        // Either one moved is an edit of the sound as it stands
        let now = UserPreset::capture(&params);
        for line in ["delay_sync = on", "delay_note = sixteenth"] {
            let moved = UserPreset::parse(&params, &format!("{}{line}\n", now.to_text(&params)));
            assert_ne!(moved, now, "{line}");
            assert!(!moved.matches(&params), "{line}");
        }
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
        assert_eq!(read.chain, Chain::default());
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
        assert_eq!(Bank::read(&params, None).files.len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // #271: a chain of the player's own. Two amps, five blocks: a compressor, a gate on a slot of
    // its own, a chorus, the delay on its pedal's slot and a second delay on another
    const RIG: [&str; 7] =
        ["amps=2", "comp@1", "gate@13", "amp", "chorus@3", "delay@7", "delay@14"];

    // The file of a player who built it and moved the knobs of the two blocks that stand on slots
    // of their own
    fn rig_file() -> String {
        format!(
            "{SAVED}order = comp drive amp\nchain = {}\nslot13 = 1:0.2,0.8,0,0,0\n\
             slot14 = 0:0.3,0.4,0.5,0.6,0.7\n",
            RIG.join(" ")
        )
    }

    /// What the host was asked to set, by parameter
    fn asked(host: &Host) -> Vec<(ParamPtr, f32)> {
        host.set.lock().unwrap().clone()
    }

    #[test]
    fn test_a_saved_chain_reads_back_and_is_written_again_as_it_was() {
        let params = GuitarAmpParams::default();
        let preset = UserPreset::parse(&params, &rig_file());
        assert_eq!(preset.chain, Chain::from_ids(RIG));
        assert_eq!(preset.chain.amps(), 2);
        assert_eq!(preset.chain.blocks().len(), 5);
        // The gate and the second delay stand on slots of their own and have a line each; the
        // others are in the lines of their pedals
        let gate = SlotValues { on: true, knobs: [0.2, 0.8, 0.0, 0.0, 0.0] };
        let delay = SlotValues { on: false, knobs: [0.3, 0.4, 0.5, 0.6, 0.7] };
        assert_eq!(preset.slots, [(12, gate), (13, delay)]);

        let text = preset.to_text(&params);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.contains(&"chain = amps=2 comp@1 gate@13 amp chorus@3 delay@7 delay@14"));
        assert!(lines.contains(&"slot13 = 1:0.2,0.8,0,0,0"));
        assert!(lines.contains(&"slot14 = 0:0.3,0.4,0.5,0.6,0.7"));
        assert_eq!(lines.iter().filter(|line| line.starts_with("slot")).count(), 2);
        // The twelve-pedal view is still there, for a build from before the chain
        assert!(lines.iter().any(|line| line.starts_with("order = ")));
        assert_eq!(UserPreset::parse(&params, &text), preset);
    }

    #[test]
    fn test_a_chain_is_loaded_as_the_whole_chain_with_every_knob_of_its_blocks() {
        let params = GuitarAmpParams::default();
        // The player's line is something else when the preset is loaded
        params.chain.put(Chain::from_ids(["amps=1", "amp", "reverb@8"]));
        let preset = UserPreset::parse(&params, &rig_file());
        let host = Host::default();
        preset.load(&params, &ParamSetter::new(&host));
        assert_eq!(params.chain.get(), Chain::from_ids(RIG));
        let told = host.told.lock().unwrap();
        assert!(crate::user_presets::tests::as_one_edit(&told));
        let set = asked(&host);
        let value_of = |ptr: ParamPtr| set.iter().find(|(each, _)| *each == ptr).map(|&(_, v)| v);
        // The gate (on slot 13) is on with its two knobs; the second delay is off with five
        assert_eq!(value_of(params.slot13.on.as_ptr()), Some(1.0));
        assert_eq!(value_of(params.slot13.k1.as_ptr()), Some(0.2));
        assert_eq!(value_of(params.slot13.k2.as_ptr()), Some(0.8));
        assert_eq!(value_of(params.slot13.k3.as_ptr()), None);
        assert_eq!(value_of(params.slot14.on.as_ptr()), Some(0.0));
        assert_eq!(value_of(params.slot14.k5.as_ptr()), Some(0.7));
        // Nothing is set for a slot that no block of the chain stands on
        assert_eq!(value_of(params.slot15.on.as_ptr()), None);
        assert_eq!(value_of(params.slot3.k1.as_ptr()), None);
        // The pedals' own lines are set as they always were
        assert!(value_of(params.delay_time.as_ptr()).is_some());
        drop(told);

        // And loading it again, after another, is the same load: the dials come back as they were
        let again = Host::default();
        params.chain.put(Chain::default());
        preset.load(&params, &ParamSetter::new(&again));
        assert_eq!(params.chain.get(), Chain::from_ids(RIG));
        assert_eq!(asked(&again), set);
    }

    #[test]
    fn test_a_chain_as_it_stands_matches_until_a_block_or_a_slot_moves() {
        let params = GuitarAmpParams::default();
        params.chain.put(Chain::from_ids(RIG));
        let now = UserPreset::capture(&params);
        assert_eq!(now.chain, Chain::from_ids(RIG));
        assert_eq!(now.slots.len(), 2);
        assert!(now.matches(&params));
        // Written and read, it is the same sound
        assert_eq!(UserPreset::parse(&params, &now.to_text(&params)), now);
        // A block moved, one taken out, an amp lane gone: each an edit
        let mut other = Chain::from_ids(RIG);
        other.move_block(2, Zone::Front, 0).unwrap();
        params.chain.put(other);
        assert!(!now.matches(&params));
        params.chain.put(Chain::from_ids([
            "amps=1", "comp@1", "gate@13", "amp", "chorus@3", "delay@7", "delay@14",
        ]));
        assert!(!now.matches(&params));
        // What a preset has in a slot that is not the one the chain has is not a match either
        params.chain.put(Chain::from_ids(RIG));
        let mut moved = now.clone();
        moved.slots[0].1.knobs[0] += 0.2;
        assert!(!moved.matches(&params));
        moved.slots[0].1.on = !moved.slots[0].1.on;
        assert!(!moved.matches(&params));
    }

    #[test]
    fn test_a_file_from_before_the_chain_loads_as_the_twelve_pedals_in_its_order() {
        let params = GuitarAmpParams::default();
        params.chain.put(Chain::from_ids(RIG));
        let old = UserPreset::parse(&params, SAVED);
        assert!(old.slots.is_empty());
        let order = old.chain.order();
        assert_eq!(old.chain, Chain::from_order(&order));
        old.load(&params, &ParamSetter::new(&Host::default()));
        assert_eq!(params.chain.get(), Chain::from_order(&order));
        // Written now, it has the chain line and no slot line, and reads back the same
        let text = old.to_text(&params);
        assert!(text.lines().any(|line| line.starts_with("chain = amps=2 ")));
        assert!(!text.lines().any(|line| line.starts_with("slot")));
        assert_eq!(UserPreset::parse(&params, &text), old);
        // A file with both lines goes by the chain; a damaged chain line is the default chain,
        // and a slot line for a block on its pedal's slot, or past the sixteenth, is no line
        let both = UserPreset::parse(&params, &format!("{SAVED}chain = amps=1 comp@1 amp\n"));
        assert_eq!(both.chain.blocks().len(), 1);
        let damaged = UserPreset::parse(&params, &format!("{SAVED}chain = nonsense\n"));
        assert_eq!(damaged.chain, Chain::default());
        let stray = UserPreset::parse(
            &params,
            "chain = amps=1 comp@1 amp\nslot1 = 1:0,0,0,0,0\nslot17 = 1:0,0,0,0,0\nslot9 = x\n",
        );
        assert!(stray.slots.is_empty());
    }

    #[test]
    fn test_a_chain_is_saved_in_a_file_with_spaces_in_its_name_and_loaded_from_it() {
        let dir = test_dir("user-presets-chain");
        let params = GuitarAmpParams::default();
        params.chain.put(Chain::from_ids(RIG));
        let file = named_file_name("Big Mud Rig", &[]).unwrap();
        assert_eq!(file, "Big Mud Rig.preset");
        save(&params, &dir, &file).unwrap();
        let bank = Bank::read(&params, Some(&dir));
        assert_eq!(bank.files, ["Big Mud Rig.preset"]);
        let read = bank.find(&file).unwrap();
        assert_eq!(*read, UserPreset::capture(&params));
        // Another instance of the plugin takes the line from the file
        let other = GuitarAmpParams::default();
        read.load(&other, &ParamSetter::new(&Host::default()));
        assert_eq!(other.chain.get(), Chain::from_ids(RIG));
        assert!(read.matches(&other));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
