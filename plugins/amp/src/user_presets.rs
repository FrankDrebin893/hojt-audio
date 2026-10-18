//! The player's own presets: the bank they save their sounds in, next to the factory ones.
//!
//! Each preset is one small text file in `Hojt Audio/Presets` in the user's Documents
//! folder. Its name is the file's name, so a preset is renamed, copied or thrown away in the
//! file manager, and a preset can be handed to someone else as a file. There is no dialog:
//! the Preset stepper on the head goes on from the last factory preset into these, in name
//! order, and the Save button beside it writes the sound as it stands.
//!
//! A file holds what a factory preset holds, and also the microphone and the cabinet:
//! every parameter except Bypass, the Tuner switch, Input and Output, then the order of the
//! pedals and the cabinet choice. One line each, `id = value`, by the frozen parameter ids:
//!
//! ```text
//! amp = torden
//! gain = 0.55
//! gate_on = on
//! order = comp drive phaser amp chorus flanger tremolo delay reverb
//! cabinet = speaker:v30
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
use crate::presets::{self, MATCH_TOLERANCE, PRESETS};
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

    fn set(self, setter: &ParamSetter, plain: f32) {
        match self {
            Control::Knob(param) => presets::set(setter, param, plain),
            Control::Switch(param) => presets::set(setter, param, plain >= 0.5),
            Control::Amp(param) => presets::set(setter, param, Amp::from_index(plain as usize)),
            Control::Mic(param) => {
                presets::set(setter, param, MicType::from_index(plain as usize));
            }
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
fn controls(params: &GuitarAmpParams) -> [(&'static str, Control<'_>); 40] {
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
        for (&(_, control), &plain) in controls(params).iter().zip(&self.values) {
            control.set(setter, plain);
        }
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

/// Where in the round of presets the one loaded last is: the factory ones first, then the
/// player's in name order. A player's preset that is no longer in the folder counts as the
/// factory preset loaded before it
pub fn place(factory: usize, user: &str, bank: &Bank) -> usize {
    if user.is_empty() {
        return factory;
    }
    bank.position(user).map_or(factory, |index| PRESETS.len() + index)
}

/// The place before or after `place` in the round of the factory presets and the player's
pub fn neighbour(place: usize, bank: &Bank, forward: bool) -> usize {
    let count = PRESETS.len() + bank.len();
    (place + if forward { 1 } else { count - 1 }) % count
}

/// A file name for a new preset made from `base`, the name of the preset it started as:
/// `base 2`, `base 3` and so on, the first that is not taken (letters in either case)
pub fn new_file_name(base: &str, taken: &[String]) -> String {
    let base: String = base
        .chars()
        .filter(|letter| !matches!(letter, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let base = if base.trim().is_empty() { "Preset".to_owned() } else { base };
    (2..)
        .map(|number| format!("{base} {number}.{EXTENSION}"))
        .find(|name| !taken.iter().any(|each| each.eq_ignore_ascii_case(name)))
        .unwrap_or_default()
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
mod tests {
    use super::*;
    use crate::user_cab::tests::test_dir;
    use std::sync::Mutex;

    // The parameters a preset leaves alone: the player's gain staging, and the switches
    // that are not part of the sound
    const LEFT_ALONE: [&str; 4] = ["bypass", "in_gain", "out_level", "tuner"];

    /// Stands in for the host: writes down what it is asked to set, and sets nothing
    #[derive(Default)]
    struct Host {
        set: Mutex<Vec<(ParamPtr, f32)>>,
    }

    impl GuiContext for Host {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Standalone
        }
        unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, normalized: f32) {
            self.set.lock().unwrap().push((param, normalized));
        }
        unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}
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
        assert_eq!(saved.order.ids()[0], "chorus");
        assert_eq!(saved.cabinet, "speaker:test");

        let text = saved.to_text(&params);
        for line in ["amp = torden", "gain = 0.73", "delay_on = on", "cab_mic_type = ribbon"] {
            assert!(text.lines().any(|each| each == line), "{line} is not in\n{text}");
        }
        // Every parameter is written out, also those at their defaults
        assert_eq!(text.lines().filter(|line| line.contains(" = ")).count(), 42);
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
    fn test_stepping_goes_on_from_the_factory_presets_into_the_players() {
        let dir = test_dir("user-presets-round");
        let params = GuitarAmpParams::default();
        for file in ["Mine.preset", "Other.preset"] {
            save(&params, &dir, file).unwrap();
        }
        let bank = Bank::read(&params, Some(&dir));
        let factory = PRESETS.len();
        assert_eq!(place(3, "", &bank), 3);
        assert_eq!(place(3, "Other.preset", &bank), factory + 1);
        // One that was thrown away counts as the factory preset it was loaded after
        assert_eq!(place(3, "Gone.preset", &bank), 3);
        assert_eq!(neighbour(factory - 1, &bank, true), factory);
        assert_eq!(neighbour(factory + 1, &bank, true), 0);
        assert_eq!(neighbour(0, &bank, false), factory + 1);
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
